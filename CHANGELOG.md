# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Claude Code ingestion (M2).**
  - Connector contract (`SourceConnector`) and the Claude Code connector: main and subagent logs, order-independent deduplication, tool-call metadata and outcomes.
  - Ingestion engine: consent-gated discovery, root-scoped read-only file access with a credential deny-list, transactional checkpoints, truncation and replacement handling, a debounced watcher, periodic and manual scans, and overlapped parsing and committing.
  - Migration 0002: sources, file checkpoints, ingest runs and issues, tool calls, and observation precedence.
  - Commands `list_sources`, `set_source_enabled` and `request_rescan`, and a typed `data-changed` event, generated into the TypeScript contracts.
  - Synthetic fixtures with canary markers; end-to-end, share-mode and concurrent-writer tests; an ingest throughput budget in the `Performance` workflow.
  - Connector documentation and ADR-023.

### Changed
- **Store.**
  - A larger writer page cache.
  - Statements are built once.
  - Session links are applied once per batch.
  - Dropped the unused `usage_events_occurred_at` index.
- **Supply chain.** `cargo-deny` accepts the permissive ISC licence (used by the Linux file-watcher backend).

### Added
- **Data core (M1).**
  - Domain model: events, canonical tokens, accuracy classes, integer money and non-addable cost kinds.
  - Pricing engine with effective dates, prompt tiers, service tiers and recorded assumptions.
  - Registry of providers and models, and price catalogue 2026-11-08.1 (Anthropic, OpenAI, Google), verified on the official pages.
  - Metric registry and time-zone-aware analytics engine with comparisons and top-N.
  - SQLite store with idempotent writes, three rollup levels, re-pricing and backups.
  - Synthetic performance tool and a `Performance` CI workflow.

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
