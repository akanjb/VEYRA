//! Test support shared by integration tests (dev-dependency only): the synthetic
//! connector fixtures and temporary directories that remove themselves.
//!
//! Helpers panic on I/O failure: in a test, a failed setup must abort the test.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The synthetic Claude Code `projects` directory (format v2.1).
#[must_use]
pub fn claude_code_projects() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude-code/v2.1/projects")
}

/// Copies a directory tree.
///
/// # Panics
/// When a file cannot be read or written.
pub fn copy_tree(from: &Path, to: &Path) {
    let fail = |what: &str, path: &Path, error: std::io::Error| -> ! {
        panic!("cannot {what} {}: {error}", path.display())
    };
    std::fs::create_dir_all(to).unwrap_or_else(|e| fail("create", to, e));
    for entry in std::fs::read_dir(from).unwrap_or_else(|e| fail("read", from, e)) {
        let entry = entry.unwrap_or_else(|e| fail("read", from, e));
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap_or_else(|e| fail("copy to", &target, e));
        }
    }
}

/// A fresh, uniquely named directory under the system temp folder, removed on drop.
#[derive(Debug)]
pub struct TempDir(PathBuf);

impl TempDir {
    /// # Panics
    /// When the directory cannot be created.
    #[must_use]
    pub fn new(prefix: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        if let Err(error) = std::fs::create_dir_all(&path) {
            panic!("cannot create {}: {error}", path.display());
        }
        Self(path)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// A home directory inside this one holding a copy of the Claude Code fixtures at
    /// `<home>/.claude/projects`. Returns the home directory.
    #[must_use]
    pub fn with_claude_code_home(&self) -> PathBuf {
        let home = self.0.join("home");
        copy_tree(&claude_code_projects(), &home.join(".claude/projects"));
        home
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
