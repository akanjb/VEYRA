//! Test support shared by integration tests (dev-dependency only): the synthetic
//! connector fixtures and temporary directories that remove themselves.
//!
//! Helpers panic on I/O failure: in a test, a failed setup must abort the test.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// The busy timeout the concurrent-writer tests give the owning application: a fiftieth
/// of the 5 seconds OpenCode uses. While a commit is in progress, any SQLite reader (the
/// application's own or Veyra's) may hold the WAL write lock for an instant to re-read
/// the WAL index, so a writer with no timeout at all can be refused by any reader.
pub const OWNER_BUSY_TIMEOUT: Duration = Duration::from_millis(100);

/// The synthetic Claude Code `projects` directory (format v2.1).
#[must_use]
pub fn claude_code_projects() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude-code/v2.1/projects")
}

/// The synthetic Codex home directory (rollout format of Codex 0.167).
#[must_use]
pub fn codex_home() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/codex/v0.167/codex-home")
}

/// The synthetic Gemini CLI `~/.gemini/tmp` directory (format of Gemini CLI 0.67).
#[must_use]
pub fn gemini_tmp() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/gemini-cli/v0.67/gemini-tmp")
}

/// The synthetic GitHub Copilot CLI home directory (session events of Copilot CLI 1.0).
#[must_use]
pub fn copilot_home() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/copilot-cli/v1.0/copilot-home")
}

/// Creates `<data_dir>/opencode.db` from the synthetic OpenCode fixture (tables of
/// OpenCode 1.2 and 2.2), in WAL mode as OpenCode keeps it. Returns the database path.
///
/// # Panics
/// When the fixture cannot be read or the database cannot be created.
#[must_use]
pub fn opencode_database(data_dir: &Path) -> PathBuf {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/opencode/v2.2/opencode.sql");
    let sql = std::fs::read_to_string(&script)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
    std::fs::create_dir_all(data_dir)
        .unwrap_or_else(|e| panic!("cannot create {}: {e}", data_dir.display()));
    let path = data_dir.join("opencode.db");
    let db = rusqlite::Connection::open(&path)
        .unwrap_or_else(|e| panic!("cannot create {}: {e}", path.display()));
    db.execute_batch(&format!(
        "PRAGMA journal_mode = WAL;
{sql}"
    ))
    .unwrap_or_else(|e| panic!("cannot load the OpenCode fixture: {e}"));
    path
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
        self.with_home_copy(&claude_code_projects(), ".claude/projects")
    }

    /// A home directory inside this one holding a copy of a fixture tree at
    /// `<home>/<at>` (for example `.codex`). Returns the home directory.
    #[must_use]
    pub fn with_home_copy(&self, fixture: &Path, at: &str) -> PathBuf {
        let home = self.0.join("home");
        copy_tree(fixture, &home.join(at));
        home
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
