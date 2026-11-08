# 0021. CI scope while the repository is private

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

The repository is private during initial testing and will become public later. On private repositories:

- **macOS** GitHub Actions minutes count 10× against the free monthly allowance, and Windows minutes count 2×.
- **CodeQL code scanning and dependency review** require GitHub Advanced Security, a paid feature. The first CodeQL run failed at the upload step.

Veyra must have no recurring cost (NFR-01).

## Decision

While the repository is private:

- **CI** runs the web job and the Rust matrix on **Ubuntu 22.04 and Windows** for pushes to `main` and for pull requests.
- **macOS** runs only on manual dispatch: *Actions → CI → Run workflow*.
- **Security** keeps `cargo-deny` and `pnpm audit`, on every push and weekly. CodeQL and dependency review are removed.

## Alternatives considered

- **Make the repository public now:** restores every check for free, but the owner wants to test privately first.
- **Keep every job on a private repository:** CodeQL fails, and macOS minutes would exhaust the free allowance after roughly a dozen pushes.

## Consequences

- **Mac regressions show up later.** A macOS-specific break is caught only by a manual run. Run it before every release and after shell or webview changes.
- **Less automated scanning.** There is no CodeQL scanning of the TypeScript code and no automatic review of new dependencies in pull requests. The dependency admission checklist (`CONTRIBUTING.md`) covers the latter manually.

## Revisit when

The repository becomes public. Then restore macOS in the default matrix and re-add the CodeQL and dependency-review jobs (see the M0 commit for the original definitions).
