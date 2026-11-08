# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- **CI scope while the repository is private (ADR-021).**
  - macOS builds run only on manual dispatch.
  - CodeQL and dependency review are disabled until the repository is public.
- **README.** Credits the author.

### Added
- **Foundations (M0).**
  - Monorepo: pnpm workspaces and a Cargo workspace.
  - Tauri 2 shell with a capability-restricted IPC and a strict CSP.
  - A command catalogue that generates the Tauri handlers, the permission allow-list and the TypeScript contracts.
  - Dashboard placeholder showing application info.
  - Lint, format, type and test tooling, including the module-boundary rule.
  - CI and security workflows.
  - Architecture decision records 0001–0020.
