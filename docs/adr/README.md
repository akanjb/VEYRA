# Architecture Decision Records

Each record documents one significant decision: its context, the decision itself, the alternatives considered and the consequences. Records are immutable once accepted. A later decision **supersedes** an earlier one rather than editing it.

**Process.** Copy [`template.md`](template.md), take the next number, open a PR. A new technology, a major dependency or a new architectural pattern requires an ADR.

| ADR | Title | Status |
|---|---|---|
| [001](0001-tauri-desktop-shell.md) | Tauri 2 as the desktop shell | Accepted |
| [002](0002-modular-monolith-hexagonal.md) | Modular monolith with hexagonal boundaries | Accepted |
| [003](0003-sqlite-system-of-record.md) | SQLite (rusqlite) as the system of record | Accepted, amended by 022 |
| [004](0004-integer-money-and-cost-kinds.md) | Integer money and three non-addable cost kinds | Accepted |
| [005](0005-canonical-token-semantics.md) | Canonical token semantics | Accepted, amended by 022 |
| [006](0006-per-metric-accuracy.md) | Per-metric accuracy classes; NULL for unavailable | Accepted |
| [007](0007-effective-dated-pricing-catalogue.md) | Effective-dated, version-controlled pricing catalogue | Accepted |
| [008](0008-derived-metrics-in-rust.md) | Additive sums in SQL, derived metrics in one Rust engine | Accepted |
| [009](0009-fifteen-minute-utc-rollups.md) | 15-minute UTC rollup buckets | Accepted, amended by 022 |
| [010](0010-generated-contracts.md) | Generated Rust→TypeScript contracts from one command catalogue | Accepted |
| [011](0011-echarts.md) | Apache ECharts as the single chart library | Accepted |
| [012](0012-typescript-6.md) | TypeScript 6.0 with type-aware ESLint | Accepted |
| [013](0013-url-as-filter-state.md) | TanStack Router; the URL is the single source of filter state | Accepted |
| [014](0014-no-credential-access.md) | No credential access or private endpoints | Accepted |
| [015](0015-read-only-sqlite-snapshots.md) | Read-only snapshots for live third-party SQLite sources | Accepted |
| [016](0016-encryption-at-rest-deferred.md) | Database encryption at rest deferred | Accepted |
| [017](0017-supply-chain-policy.md) | Supply-chain policy | Accepted |
| [018](0018-unsigned-v1-distribution.md) | Unsigned v1 distribution with checksums | Accepted |
| [019](0019-licence.md) | Licence: Apache-2.0 OR MIT | Superseded by 035 |
| [020](0020-ipc-security-baseline.md) | IPC and webview security baseline | Accepted, amended by 024 |
| [021](0021-ci-scope-while-private.md) | CI scope while the repository is private | Accepted |
| [022](0022-data-core-implementation.md) | Data-core implementation refinements | Accepted |
| [023](0023-incremental-ingestion.md) | Incremental, consent-gated ingestion | Accepted, extended by 025 |
| [024](0024-dashboard-frontend.md) | Dashboard frontend: stack, design system and deviations from the plan | Accepted |
| [025](0025-reading-modes-and-capabilities.md) | Reading modes, side files and capability matrices for connectors | Accepted |
| [026](0026-request-size-bins.md) | Request-size distributions from additive size bins | Accepted |
| [027](0027-project-identity-and-merges.md) | Project identity, renaming and merging | Accepted |
| [028](0028-billing-entries-and-budgets.md) | Billing entries, budgets and the month forecast | Accepted |
| [029](0029-page-reads-in-parallel.md) | Dashboard pages read in parallel, with a budget per page | Accepted |
| [030](0030-page-rollups.md) | Page rollups and token-only reads | Accepted |
| [031](0031-reads-sized-to-each-card.md) | Reads sized to what each card shows | Accepted |
| [032](0032-report-exports-and-audit-log.md) | Report exports, stored settings and the audit log | Accepted |
| [033](0033-storage-backups-and-restore.md) | Storage page, daily backups and restore at start | Accepted |
| [034](0034-tools-and-agents-page.md) | Tools & Agents page and the tool rollup | Accepted |
| [035](0035-source-available-licence.md) | Source-available licence | Accepted |
| [036](0036-what-if-repricing.md) | Pricing a period on another model | Accepted |
| [037](0037-deleting-stored-usage.md) | Deleting stored usage | Accepted |
| [038](0038-local-opentelemetry-receiver.md) | Local OpenTelemetry receiver | Accepted |
| [039](0039-local-capture-proxy.md) | Local capture proxy for API usage | Accepted |
