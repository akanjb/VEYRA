//! Incremental reading decisions for append-only files (FR-ING-04).
//!
//! A checkpoint stores the offset after the last complete line and a fingerprint of the
//! file's first bytes. The next read continues from the offset, unless the file shrank
//! (truncated), its first bytes changed (replaced), or the parser changed; then it is
//! re-read from the start. Re-reading is safe because ingestion is idempotent.

use std::hash::Hasher;

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
    use super::{ReadPlan, Restart, complete_lines, fingerprint, plan};
    use crate::fs::FoundFile;
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
