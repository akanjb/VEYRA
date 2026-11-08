# 0017. Supply-chain policy

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

In May 2026, 84 malicious versions of 42 `@tanstack/*` packages were published and stole developer credentials (GHSA-g7cv-rxg3-hmpx). Veyra reads sensitive local files, so a compromised dependency would be severe.

## Decision

1. **Exact pins.** npm dependencies are pinned to exact versions, and Cargo workspace dependencies use `=` requirements. `pnpm-lock.yaml` and `Cargo.lock` are committed.
2. **Release-age gate.** No version younger than **7 days** is installed (pnpm `minimumReleaseAge: 10080`). The same rule applies to crates, the toolchain and GitHub Actions, chosen by hand.
3. **No install scripts.** Dependency lifecycle scripts do not run (pnpm default). Any exception must be allow-listed with a reason.
4. **Pinned Actions.** GitHub Actions are pinned to full commit SHAs. Workflows use `permissions: contents: read` by default and `persist-credentials: false`, and never use `pull_request_target`.
5. **CI checks.** CI runs cargo-deny (advisories, licences, sources), `pnpm audit`, dependency review and CodeQL.
6. **Admission review.** Every new dependency goes through the admission checklist in `CONTRIBUTING.md`.

**Recorded exceptions:**
- `GHSA-g3hv-75wg-m249` (minimatch DoS, moderate). Present only in dev-time lint tooling, where the glob patterns come from our own configuration, and not shipped. Ignored in `pnpm-workspace.yaml` until ESLint and typescript-eslint depend on minimatch ≥ 11.0.1.
- `RUSTSEC-2024-0370` (`proc-macro-error` unmaintained; not a vulnerability). A build-time macro dependency of Tauri's Linux GTK3 bindings (`glib-macros` 0.18). Ignored in `deny.toml` until Tauri moves off gtk-rs 0.18.

## Alternatives considered

- **Caret ranges with Renovate auto-merge:** faster updates, but malicious releases would land quickly.

## Consequences

- Security fixes arrive at least 7 days late, unless a maintainer overrides the gate deliberately for a critical fix.

## Revisit when

The ecosystem offers verified provenance (e.g. npm provenance plus Sigstore) that we can enforce at install time.
