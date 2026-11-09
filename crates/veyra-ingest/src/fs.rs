//! Read-only, root-scoped file access for usage sources (ADR-014, FR-ING-03).
//!
//! - Only paths below the source root are reachable: relative paths must contain plain
//!   components only, and symbolic links are never followed.
//! - Credential and cookie stores are refused by name even inside a root.
//! - Files are opened with full sharing on Windows so the owning tool can keep writing,
//!   renaming or deleting them while Veyra reads.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use veyra_domain::time::UtcMillis;

use crate::IngestError;

/// File names Veyra never opens, whatever the source (lower-case comparison).
const DENIED_NAMES: [&str; 8] = [
    ".credentials.json",
    "credentials.json",
    "auth.json",
    "oauth_creds.json",
    "cookies",
    "login data",
    "keychain",
    ".env",
];

/// A file found under a source root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    /// Path relative to the root, with `/` separators.
    pub relative: String,
    pub len: u64,
    pub modified: Option<UtcMillis>,
}

/// Read-only access confined to one source root.
#[derive(Debug, Clone)]
pub struct ScopedFs {
    root: PathBuf,
}

/// Extensions of key material, refused whatever the file is called.
const DENIED_EXTENSIONS: [&str; 4] = ["pem", "key", "p12", "pfx"];

fn is_denied(name: &str) -> bool {
    let denied_extension = Path::new(name).extension().is_some_and(|ext| {
        DENIED_EXTENSIONS
            .iter()
            .any(|denied| ext.eq_ignore_ascii_case(denied))
    });
    denied_extension || DENIED_NAMES.contains(&name.to_ascii_lowercase().as_str())
}

fn modified(metadata: &std::fs::Metadata) -> Option<UtcMillis> {
    let since = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    i64::try_from(since.as_millis()).ok().map(UtcMillis)
}

impl ScopedFs {
    /// Confines access to `root`, which must be an existing directory.
    ///
    /// # Errors
    /// Returns [`IngestError::MissingRoot`] when the root is not a directory.
    pub fn new(root: &Path) -> Result<Self, IngestError> {
        let metadata =
            std::fs::symlink_metadata(root).map_err(|_| IngestError::MissingRoot(root.into()))?;
        if !metadata.is_dir() {
            return Err(IngestError::MissingRoot(root.into()));
        }
        Ok(Self { root: root.into() })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a relative path, refusing anything that could leave the root or that
    /// names a credential store.
    fn resolve(&self, relative: &str) -> Result<PathBuf, IngestError> {
        let path = Path::new(relative);
        let plain = path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
        let denied = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_none_or(is_denied);
        if !plain || denied || relative.is_empty() {
            return Err(IngestError::Denied(path.into()));
        }
        Ok(self.root.join(path))
    }

    /// Lists files accepted by `accept`, at most `max_depth` directories deep, without
    /// following symbolic links. Unreadable directories are skipped.
    #[must_use]
    pub fn find_files(&self, max_depth: usize, accept: &dyn Fn(&Path) -> bool) -> Vec<FoundFile> {
        let mut found = Vec::new();
        let mut pending = vec![(self.root.clone(), PathBuf::new(), 0)];
        while let Some((dir, relative_dir, depth)) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
                    continue;
                };
                let name = entry.file_name();
                let relative = relative_dir.join(&name);
                if metadata.is_dir() && depth < max_depth {
                    pending.push((entry.path(), relative, depth + 1));
                } else if metadata.is_file()
                    && name.to_str().is_some_and(|n| !is_denied(n))
                    && accept(&relative)
                    && let Some(relative) = relative.to_str()
                {
                    found.push(FoundFile {
                        relative: relative.replace('\\', "/"),
                        len: metadata.len(),
                        modified: modified(&metadata),
                    });
                }
            }
        }
        found.sort_by(|a, b| a.relative.cmp(&b.relative));
        found
    }

    /// The absolute path of an existing file below the root that is neither denied nor a
    /// symbolic link: for readers that must open the file themselves (SQLite).
    ///
    /// # Errors
    /// Returns [`IngestError`] for denied paths, symbolic links or I/O failures.
    pub fn verified_path(&self, relative: &str) -> Result<PathBuf, IngestError> {
        let path = self.resolve(relative)?;
        if std::fs::symlink_metadata(&path)
            .map_err(IngestError::io(&path))?
            .file_type()
            .is_symlink()
        {
            return Err(IngestError::Denied(path));
        }
        Ok(path)
    }

    /// Size and modification time of a file below the root; `None` when it does not
    /// exist.
    ///
    /// # Errors
    /// Returns [`IngestError`] for denied paths or I/O failures other than absence.
    pub fn stat(&self, relative: &str) -> Result<Option<(u64, Option<UtcMillis>)>, IngestError> {
        let path = self.resolve(relative)?;
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => Ok(Some((metadata.len(), modified(&metadata)))),
            Ok(_) => Err(IngestError::Denied(path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(IngestError::io(path)(error)),
        }
    }

    fn open(&self, relative: &str) -> Result<File, IngestError> {
        let path = self.verified_path(relative)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
            options.share_mode(0x1 | 0x2 | 0x4);
        }
        options.open(&path).map_err(IngestError::io(path))
    }

    /// Reads up to `max` bytes starting at `offset`.
    ///
    /// # Errors
    /// Returns [`IngestError`] for denied paths or I/O failures.
    pub fn read_at(&self, relative: &str, offset: u64, max: u64) -> Result<Vec<u8>, IngestError> {
        let mut file = self.open(relative)?;
        file.seek(SeekFrom::Start(offset))
            .map_err(IngestError::io(relative))?;
        let mut buffer = Vec::new();
        file.take(max)
            .read_to_end(&mut buffer)
            .map_err(IngestError::io(relative))?;
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::ScopedFs;
    use crate::IngestError;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("veyra-fs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        dir
    }

    #[test]
    fn refuses_paths_outside_the_root_and_credential_files() {
        let root = temp_root("deny");
        std::fs::write(root.join("auth.json"), "{}").unwrap();
        std::fs::write(root.join("sub").join("log.jsonl"), "x\n").unwrap();
        let fs = ScopedFs::new(&root).unwrap();
        for bad in [
            "../outside.jsonl",
            "/etc/passwd",
            "auth.json",
            "sub/.credentials.json",
            "",
        ] {
            assert!(
                matches!(fs.read_at(bad, 0, 10), Err(IngestError::Denied(_))),
                "{bad}"
            );
        }
        assert_eq!(fs.read_at("sub/log.jsonl", 0, 10).unwrap(), b"x\n");
        let listed = fs.find_files(4, &|_| true);
        let names: Vec<_> = listed.iter().map(|f| f.relative.as_str()).collect();
        assert_eq!(
            names,
            ["sub/log.jsonl"],
            "credential files are never listed"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reads_ranges() {
        let root = temp_root("range");
        std::fs::write(root.join("a.jsonl"), "0123456789").unwrap();
        let fs = ScopedFs::new(&root).unwrap();
        assert_eq!(fs.read_at("a.jsonl", 3, 4).unwrap(), b"3456");
        assert_eq!(fs.read_at("a.jsonl", 8, 100).unwrap(), b"89");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_owning_tool_can_append_rotate_and_delete_while_a_file_is_open() {
        use std::io::Write as _;
        let root = temp_root("share");
        let path = root.join("live.jsonl");
        std::fs::write(&path, "a\n").unwrap();
        let fs = ScopedFs::new(&root).unwrap();
        let held = fs.open("live.jsonl").unwrap();
        let mut writer = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writer.write_all(b"b\n").unwrap();
        drop(writer);
        std::fs::rename(&path, root.join("live.1.jsonl")).unwrap();
        std::fs::write(&path, "c\n").unwrap();
        std::fs::remove_file(root.join("live.1.jsonl")).unwrap();
        drop(held);
        assert_eq!(fs.read_at("live.jsonl", 0, 10).unwrap(), b"c\n");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_missing_roots() {
        assert!(matches!(
            ScopedFs::new(std::path::Path::new("/definitely/not/here")),
            Err(IngestError::MissingRoot(_))
        ));
    }
}
