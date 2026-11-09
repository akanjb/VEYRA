//! Reading decisions (FR-ING-04).
//!
//! For append-only files, a checkpoint stores the offset after the last complete line and a fingerprint of the
//! file's first bytes. The next read continues from the offset, unless the file shrank
//! (truncated), its first bytes changed (replaced), or the parser changed; then it is
//! re-read from the start. Files read in full, and databases, are re-read whenever their
//! size or modification time changed. Re-reading is safe because ingestion is
//! idempotent.

use std::hash::Hasher;

use veyra_domain::time::UtcMillis;
use veyra_store::StoredCheckpoint;

use crate::fs::FoundFile;

/// Bytes fingerprinted at the start of each file.
pub const HEAD_LEN: u64 = 4_096;

/// What to do with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadPlan {
    /// Unchanged since the last read.
    Skip,
    /// New bytes start at this offset.
    From(u64),
}

/// Why a file is re-read from the start (for diagnostics and tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    New,
    ParserChanged,
    Truncated,
    Replaced,
}

/// Decides how to read a file. `head_hash` hashes the file's first
/// `checkpoint.head_len` bytes now (only consulted when a checkpoint exists).
pub fn plan(
    file: &FoundFile,
    checkpoint: Option<&StoredCheckpoint>,
    parser_version: u32,
    head_hash: impl FnOnce(u32) -> Option<i64>,
) -> (ReadPlan, Option<Restart>) {
    let Some(checkpoint) = checkpoint else {
        return (ReadPlan::From(0), Some(Restart::New));
    };
    if checkpoint.parser_version != parser_version {
        return (ReadPlan::From(0), Some(Restart::ParserChanged));
    }
    if file.len < checkpoint.offset {
        return (ReadPlan::From(0), Some(Restart::Truncated));
    }
    // Same size and modification time as when last read: nothing new (a trailing
    // partial line is still partial). Avoids even the head read for idle files.
    if file.len == checkpoint.size && file.modified == checkpoint.modified_at {
        return (ReadPlan::Skip, None);
    }
    if head_hash(checkpoint.head_len) != Some(checkpoint.head_hash) {
        return (ReadPlan::From(0), Some(Restart::Replaced));
    }
    if file.len == checkpoint.offset {
        return (ReadPlan::Skip, None);
    }
    (ReadPlan::From(checkpoint.offset), None)
}

/// Whether content read in full is unchanged since its checkpoint: same parser, and the
/// same `(size, modified)` version.
#[must_use]
pub fn unchanged(
    (size, modified): (u64, Option<UtcMillis>),
    checkpoint: Option<&StoredCheckpoint>,
    parser_version: u32,
) -> bool {
    checkpoint.is_some_and(|c| {
        c.parser_version == parser_version && c.size == size && c.modified_at == modified
    })
}

/// FNV-1a 64 of `bytes`, as a SQLite-storable integer.
#[must_use]
pub fn fingerprint(bytes: &[u8]) -> i64 {
    struct Fnv(u64);
    impl Hasher for Fnv {
        fn finish(&self) -> u64 {
            self.0
        }
        fn write(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    let mut hasher = Fnv(0xcbf2_9ce4_8422_2325);
    hasher.write(bytes);
    i64::from_ne_bytes(hasher.finish().to_ne_bytes())
}

/// Splits a buffer at its last newline: `(complete lines, bytes consumed)`.
#[must_use]
pub fn complete_lines(buffer: &[u8]) -> (&[u8], usize) {
    buffer
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or((&buffer[..0], 0), |end| (&buffer[..=end], end + 1))
}

#[cfg(test)]
mod tests {
    use super::{ReadPlan, Restart, complete_lines, fingerprint, plan, unchanged};
    use crate::fs::FoundFile;
    use veyra_domain::time::UtcMillis;
    use veyra_store::StoredCheckpoint;

    fn file(len: u64) -> FoundFile {
        FoundFile {
            relative: "p/s.jsonl".to_owned(),
            len,
            modified: None,
        }
    }

    fn checkpoint(offset: u64) -> StoredCheckpoint {
        StoredCheckpoint {
            size: offset,
            modified_at: None,
            offset,
            head_hash: 7,
            head_len: 10,
            parser_version: 1,
        }
    }

    #[test]
    fn continues_skips_or_restarts() {
        let same_head = |_| Some(7);
        assert_eq!(
            plan(&file(50), None, 1, same_head),
            (ReadPlan::From(0), Some(Restart::New))
        );
        assert_eq!(
            plan(&file(50), Some(&checkpoint(50)), 1, same_head),
            (ReadPlan::Skip, None)
        );
        assert_eq!(
            plan(&file(80), Some(&checkpoint(50)), 1, same_head),
            (ReadPlan::From(50), None)
        );
        assert_eq!(
            plan(&file(20), Some(&checkpoint(50)), 1, same_head),
            (ReadPlan::From(0), Some(Restart::Truncated))
        );
        assert_eq!(
            plan(&file(80), Some(&checkpoint(50)), 1, |_| Some(8)),
            (ReadPlan::From(0), Some(Restart::Replaced))
        );
        assert_eq!(
            plan(&file(50), Some(&checkpoint(50)), 2, same_head),
            (ReadPlan::From(0), Some(Restart::ParserChanged))
        );
    }

    #[test]
    fn whole_files_are_reread_only_when_their_version_changes() {
        let stored = StoredCheckpoint {
            modified_at: Some(UtcMillis(5)),
            ..checkpoint(40)
        };
        assert!(unchanged((40, Some(UtcMillis(5))), Some(&stored), 1));
        assert!(
            !unchanged((40, Some(UtcMillis(6))), Some(&stored), 1),
            "touched"
        );
        assert!(
            !unchanged((41, Some(UtcMillis(5))), Some(&stored), 1),
            "resized"
        );
        assert!(
            !unchanged((40, Some(UtcMillis(5))), Some(&stored), 2),
            "new parser"
        );
        assert!(!unchanged((40, Some(UtcMillis(5))), None, 1), "never read");
    }

    #[test]
    fn idle_files_are_skipped_without_reading() {
        let partial = StoredCheckpoint {
            size: 60,
            ..checkpoint(50)
        };
        let unread = |_| -> Option<i64> { panic!("the head must not be read") };
        assert_eq!(
            plan(&file(60), Some(&partial), 1, unread),
            (ReadPlan::Skip, None)
        );
        // The partial line was completed: continue from the last complete line.
        assert_eq!(
            plan(&file(75), Some(&partial), 1, |_| Some(7)),
            (ReadPlan::From(50), None)
        );
    }

    #[test]
    fn keeps_partial_trailing_lines_for_later() {
        assert_eq!(complete_lines(b"a\nb\nparti"), (&b"a\nb\n"[..], 4));
        assert_eq!(complete_lines(b"no newline yet"), (&b""[..], 0));
        assert_eq!(complete_lines(b"done\n"), (&b"done\n"[..], 5));
    }

    #[test]
    fn fingerprints_are_stable() {
        assert_eq!(fingerprint(b"abc"), fingerprint(b"abc"));
        assert_ne!(fingerprint(b"abc"), fingerprint(b"abd"));
    }
}
