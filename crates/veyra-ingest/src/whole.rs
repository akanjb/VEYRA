//! Whole-file reading, for sources re-read in full whenever they change (rewritten
//! documents, and formats whose records depend on earlier lines). Files are capped in
//! size, after decompression too, so a corrupt or hostile archive cannot exhaust memory.

use std::io::Read;

use ruzstd::decoding::StreamingDecoder;
use veyra_domain::connector::Compression;

use crate::IngestError;
use crate::fs::ScopedFs;

/// The largest file read in one piece, after decompression.
pub const MAX_WHOLE_BYTES: u64 = 256 * 1024 * 1024;

/// The whole content of a file below the root, decompressed.
///
/// # Errors
/// Returns [`IngestError::Oversize`] above [`MAX_WHOLE_BYTES`], and [`IngestError`] for
/// denied paths, I/O failures or corrupt compressed data.
pub fn read_whole(
    fs: &ScopedFs,
    relative: &str,
    compression: Compression,
) -> Result<Vec<u8>, IngestError> {
    let raw = capped(fs.read_at(relative, 0, MAX_WHOLE_BYTES + 1)?, relative)?;
    match compression {
        Compression::None => Ok(raw),
        Compression::Zstd => capped(
            decompress(&raw).map_err(IngestError::io(relative))?,
            relative,
        ),
    }
}

/// `bytes`, unless they exceed [`MAX_WHOLE_BYTES`].
fn capped(bytes: Vec<u8>, relative: &str) -> Result<Vec<u8>, IngestError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_WHOLE_BYTES {
        return Err(IngestError::Oversize(relative.into()));
    }
    Ok(bytes)
}

/// Decodes every zstd frame of `compressed`, stopping one byte past the cap.
fn decompress(compressed: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut input = compressed;
    let mut output = Vec::new();
    while !input.is_empty() {
        let room = (MAX_WHOLE_BYTES + 1).saturating_sub(output.len() as u64);
        if room == 0 {
            break;
        }
        let decoder = StreamingDecoder::new(&mut input)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        decoder.take(room).read_to_end(&mut output)?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use ruzstd::encoding::{CompressionLevel, compress_to_vec};
    use veyra_domain::connector::Compression;
    use veyra_testkit::TempDir;

    use super::read_whole;
    use crate::IngestError;
    use crate::fs::ScopedFs;

    #[test]
    fn reads_plain_and_zstd_files_including_several_frames() {
        let dir = TempDir::new("whole");
        let text = b"{\"a\":1}\n{\"b\":2}\n".repeat(50);
        let mut frames = compress_to_vec(&text[..], CompressionLevel::Fastest);
        frames.extend(compress_to_vec(
            &b"{\"c\":3}\n"[..],
            CompressionLevel::Fastest,
        ));
        std::fs::write(dir.path().join("a.jsonl"), &text).unwrap();
        std::fs::write(dir.path().join("b.jsonl.zst"), &frames).unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();

        assert_eq!(read_whole(&fs, "a.jsonl", Compression::None).unwrap(), text);
        let mut expected = text.clone();
        expected.extend_from_slice(b"{\"c\":3}\n");
        assert_eq!(
            read_whole(&fs, "b.jsonl.zst", Compression::Zstd).unwrap(),
            expected
        );
    }

    #[test]
    fn rejects_corrupt_archives() {
        let dir = TempDir::new("whole-bad");
        std::fs::write(dir.path().join("bad.zst"), b"not zstd at all").unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();
        let error = read_whole(&fs, "bad.zst", Compression::Zstd).unwrap_err();
        assert_eq!(error.file_issue(), Some("unreadable-file"));
        assert!(matches!(error, IngestError::Io { .. }));
    }
}
