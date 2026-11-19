# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Local capture proxy for API usage** (ADR-039). Apps and scripts that call the OpenAI, Anthropic, Gemini or OpenRouter APIs can send their requests through Veyra, which forwards them unchanged and records the usage each response reports.
  - A switch on the Sources page, off by default. When on, Veyra listens on `127.0.0.1:4320` only and lists the base URL to give each provider's clients. Commands `get_capture_proxy` and `set_capture_proxy`; the choice is kept across restarts.
  - Streams are relayed as they arrive. Usage is read from each response on the way; keys, prompts and replies are never stored.
  - Requests from web pages or for other host names are refused, so no page can use the proxy as a relay.
  - Anthropic requests use the request id Claude Code's logs use, so a Claude Code request seen both ways is counted once. OpenRouter's reported cost is kept as provider-reported.
- The OpenTelemetry receiver card shows how many exports arrived and why the latest one was refused, if it was.
- **Local OpenTelemetry receiver** (ADR-038). Claude Code can send each request's usage to Veyra as it happens, through its official OpenTelemetry log export.
  - A switch on the Sources page, off by default. When on, Veyra listens on `127.0.0.1:4318` only and shows the settings to give Claude Code, what it has received and any port problem. Commands `get_telemetry_receiver` and `set_telemetry_receiver`; the choice is kept across restarts.
  - Accepts OTLP/HTTP JSON log exports. Requests from web pages or for other host names, compressed or protobuf bodies, and oversized requests are refused.
  - Each request has the same key as its log line, and a completed log line always wins, so turning on both counts every request once. Records are handled in memory; only usage fields are read, and nothing else is stored.
  - Connectors declare their OpenTelemetry service and mapping, so other tools can be added without changing the receiver.
- **Delete stored usage** (ADR-037). A card on the Storage page and commands `preview_delete` and `delete_data`: choose an application and, optionally, the first and last day; see how many events match and their dates; confirm. Veyra takes a backup first, deletes the events with their costs and tool calls, rebuilds the summaries of the time they covered in the same transaction, and records the deletion in the audit log. The tools' own files are never touched.
- **What if another model did the work** (ADR-036). A card on the Cost page and command `get_what_if`: pick any model with a current published price to see what the period's tokens would cost on it, per model used and in total, beside their API-equivalent cost. Models with unpriced requests are left out of both sides, and anything the pricing had to assume is listed.
- **Tools & Agents page** (ADR-034).
  - Command `get_tools`: tool calls, calls per request, failure rate, MCP share and the share made by subagents, each compared with the previous period; calls per day by kind; the most used tools; MCP servers with the tools used and their failure rate; and the tools that fail most.
  - Tool names from every application are grouped into eight kinds (read and search, edit, shell, web, subagent, planning, MCP, other) by one definition in the domain.
  - Migration 0009 adds a tool-call rollup kept exact by the existing bucket rebuild, including calls and outcomes that arrive after their request was stored. Every rollup level is now filled once, after the last migration that adds one.
- **Storage page and backups** (ADR-033).
  - Command `get_storage`: database size, space per table with its indexes, events and their first and last time, events per month, and the backups.
  - Daily backups with `VACUUM INTO`, keeping the newest seven, and a "Back up now" button (`create_backup`).
  - Restore with a preview: `preview_restore` shows what a backup holds beside what is stored now, and `restore_backup` stages it, records it in the audit log and restarts Veyra. The backup replaces the database at start, before anything opens it, and the replaced database is kept beside it.
  - Backup names from the dashboard must match the backup pattern exactly, so no request can reach another file.
- **Report exports** (ADR-032).
  - An Export button in the header of every page with a date range opens a dialog. Choose a report (daily, weekly, monthly, per project, per model, usage by application, or cost summary) and a format (CSV, CSV for Excel, JSON, XLSX). Preview the rows exactly as they will be written, then save through the operating system's save dialog.
  - Commands `preview_export` and `save_export`. Reports add up to the totals the pages show for the same filters, and include a cost-kind and a token-accuracy column. Recorded subscriptions and charges stay in rows of their own cost kind.
  - Redaction: project names can be replaced with a secret-salted hash that stays stable on this device, and folder paths can be left out. Hashed reports never include paths.
  - CSV follows RFC 4180, and text a spreadsheet would treat as a formula is kept as text.
  - Migration 0008 adds stored settings and an audit log. Each saved export is recorded without the file's name or location.
  - Dependencies: the official Tauri dialog plugin, of which only the Rust API is used (the window cannot call its commands), and `rust_xlsxwriter` for XLSX, with its `zip` dependency pinned to a release at least seven days old.
- **How cost is estimated.** Command `get_cost_assumptions` and a panel on the Cost page. They list:
  - the outcome of pricing each request;
  - every assumption the pricing engine made, with how many requests and how much cost it affects;
  - every price applied, with its rates, prompt-length tiers, multipliers, effective dates, source and the date it was last checked; prices last checked more than 120 days ago are marked stale;
  - models with no published price.
  The panel loads only when opened.
- **Overview highlights.** Up to five facts chosen by fixed rules from the figures the Overview shows: the busiest day, the top model and project, what caching saved and how API-equivalent cost changed. The core picks them; the dashboard words them.
- **Cost page and billing settings.**
  - Command `get_cost`: API-equivalent cost, subscription spend and actual charges as separate tiles with comparisons, never added together; pricing coverage, cost per request and per million output tokens; daily cost with a running total; this period's running cost against the previous one; the month to date, run rate and forecast with budget lines; budgets with month-to-date and forecast states; cost by model, provider, application and project; the most expensive sessions; cost by token kind; effective cost per million output tokens by model; and the change from the previous period by model or project as a waterfall.
  - Billing entries (subscriptions and charges, once, monthly or yearly, with a currency) and monthly budgets, edited on a new Settings page through `get_billing`, `save_billing_entry`, `delete_billing_entry`, `save_budget` and `delete_budget`. Recurring entries are spread evenly over the days each payment covers; only US dollar entries are totalled (ADR-028).
  - A test over every generated contract and every dashboard source file fails if any field or line adds different cost kinds together.
  - Chart primitives: lines with dashed series and threshold lines, a waterfall, 100% stacked bars and a dot plot, each with a table twin; a trend overlay can use its own axis.
  - Forms show the core's own message when it refuses an input.
- **Projects pages.**
  - Command `list_projects`: every project with tokens, cost, sessions, active time, top model, last activity and daily tokens; search by name, path or merged folder.
  - Command `get_project`: one project's headline figures, daily trend, model mix, providers, cache benefit and most expensive sessions.
  - Commands `rename_project`, `merge_project` and `unmerge_project`. A name is a label; merging moves usage to another project, including usage recorded later, and unmerging restores it exactly. Rollups are rebuilt in the same transaction (ADR-027).
- **Activity page.**
  - Command `get_activity`: active time, wall time, sessions and active days with comparisons; active and wall time per day; sessions per length range by either measure; the longest sessions; the activity calendar and hours of the week.
  - Wall time is defined once next to active time: each session from its first request to its last, idle gaps included, overlapping sessions counted once. Per-day figures split time at day boundaries, so the days add up to the period.
  - Sessions can be ordered by duration.
  - The calendar and hours-of-week cards are shared by the Overview and Activity pages.
- **Models & Providers page.**
  - Command `get_models`: each provider's daily share of tokens; a leaderboard of every model with tokens, requests, cost, cost per million output tokens, cache hit ratio, price coverage and first and last day used; tokens per provider and per model in each application; daily tokens of the most used models.
  - Comparison of two to four models, one bar chart per measure (no radar charts).
  - Chart primitives: swimlanes and a shaded matrix table.
- **Cache page.**
  - Command `get_cache`: net cache benefit, gross saving, write overhead, hit ratio and reuse with comparisons, and the share of tokens the money figures cover; cache reads and writes per day; hit ratio by model and by project; cache writes per lifetime next to reads; the sessions whose cache writes cost more than their reads saved.
  - Sessions can be ordered by cache writes.
  - Chart primitives: a mirrored trend and grouped bars, each with a table twin; rankings can label bars with their value and explain each bar in its tooltip.
- **Usage page.**
  - Command `get_usage`: headline tokens by kind with comparisons, daily token composition with reasoning shown where reported, breakdowns by model, provider, application, interface, project and device with a Pareto line and a concentration index, tokens per request and per session (mean, median and 90th percentile per day), and the distribution of request sizes.
  - Request counts per size bin kept as a rollup, so request-size percentiles stay within the query budgets (ADR-026). Session sizes are exact.
  - Chart primitives: an overlay line on the stacked trend, a percentile band, a size histogram and a Pareto chart, each with a table twin.
- **Connectors for Codex, Gemini CLI, OpenCode and GitHub Copilot CLI (M4).**
  - Codex (CLI, IDE extension, desktop app): plain and zstd-compressed rollouts, exact per-response records or running-total increases with reset and fork handling.
  - Gemini CLI: current JSONL recordings and older JSON documents, subagent sessions, the project folder from `.project_root`.
  - OpenCode: the live SQLite database read through a deleted-after-use snapshot, both table generations, gateways recorded separately.
  - GitHub Copilot CLI: per-session, per-model totals following the tool's summary rule; documented as session-level.
  - Pipeline reading modes: whole files with a size cap, zstd decoding, SQLite snapshots, and small side files.
  - A capability matrix per source on Sources & Health, and the supported tools not found on the device.
  - Synthetic fixtures with canary markers, unit tests per connector, and end-to-end tests including a concurrent database writer and cross-source attribution.
  - Connector documentation pages and ADR-025.
- **Dashboard shell and first pages (M3).**
  - Commands `get_overview`, `list_sessions` and `get_session`, served by the analytics engine. Activity metrics: sessions, and active time as the union of request gaps capped at 5 minutes.
  - Design tokens for light and dark themes, a validated colour-blind-safe data palette, and bundled Inter and JetBrains Mono.
  - App shell:
    - grouped sidebar and header status;
    - a query bar with period stepping and comparison;
    - a filters sheet with named chips;
    - a command palette and keyboard shortcuts;
    - an error boundary;
    - live refresh on `data-changed`.
  - The URL holds all filter state, so Back and Forward restore any view.
  - Chart primitives on Apache ECharts, each with an accessible table twin: stacked trend, ranking bars, treemap, calendar and hour heatmaps, and session timeline.
  - Overview: KPI tiles with definitions, deltas and accuracy badges; daily usage by application; model mix; projects; activity calendar; hours of the week.
  - Sessions: a paged, sortable explorer, and a session detail with timeline, tool calls and subagents.
  - Sources & Health, and the first-run privacy promise and consent flow.
  - A synthetic fixture client for development and UI tests (never in production builds).
  - Playwright and axe UI tests (journeys, keyboard use, WCAG 2.2 A/AA in both themes), and a `UI` CI job.
  - A production-CSP UI test: the fixture build served with the policy of `tauri.conf.json` must run every page, overlay and tooltip without a violation. Chart tooltips are now built as DOM nodes, fonts are never inlined as `data:` URLs, and runtime-injected styles adopt Tauri's per-load style nonce.
  - ADR-024.
- **Claude Code ingestion (M2).**
  - Connector contract (`SourceConnector`) and the Claude Code connector: main and subagent logs, order-independent deduplication, tool-call metadata and outcomes.
  - Ingestion engine: consent-gated discovery, root-scoped read-only file access with a credential deny-list, transactional checkpoints, truncation and replacement handling, a debounced watcher, periodic and manual scans, and overlapped parsing and committing.
  - Migration 0002: sources, file checkpoints, ingest runs and issues, tool calls, and observation precedence.
  - Commands `list_sources`, `set_source_enabled` and `request_rescan`, and a typed `data-changed` event, generated into the TypeScript contracts.
  - Synthetic fixtures with canary markers; end-to-end, share-mode and concurrent-writer tests; an ingest throughput budget in the `Performance` workflow.
  - Connector documentation and ADR-023.

### Changed
- **Licence** (ADR-035). Veyra is now published under the Veyra Source-Available License 1.0 instead of Apache-2.0 OR MIT. Anyone may read the code and build and run unmodified Veyra; selling, redistribution, modified copies, clones and rebranding need written permission. Contributions assign their rights to the author.
- **Every page within its speed budget** (ADR-031). Reads now fetch only what a card shows:
  - daily and hourly series are summed per local day or hour in SQL, so a 30-day series returns one row per day rather than one per 15 minutes;
  - the activity calendar and hour-of-week grid read total tokens only;
  - the Cost waterfalls read only the previous period's cost;
  - reads of a single sum (tokens or cost) add up stored rows directly instead of having SQL sort them, so each project's top model is found several times faster at ten million events;
  - decoding a row no longer looks up each column by name;
  - reader connections read the database through memory mapping instead of private page caches, and keep the statements of every page prepared.

  Results are unchanged; tests compare each new read with the full sums.
- **Pages stay fast at ten million events** (ADR-030). Four new rollup levels:
  - **Session spans and sums per bucket:** activity, the session list, session sizes and per-project activity no longer read every raw event of the period.
  - **Sums per provider, model, application and interface:** the Models & Providers page uses these.
  - **Pricing outcomes per bucket:** the cost assumptions panel uses these.
  - **Request counts per size bin across every dimension:** unfiltered request sizes use these.

  Cards that show tokens only read them without the other sums.

  Activity is now computed from per-bucket spans with the same results as before. Reads that the new levels cannot answer exactly still use the events. Upgrading fills the new levels once.
- **Faster dashboard pages** (ADR-029):
  - each page runs its independent reads at the same time, on four reader connections instead of two;
  - readers sort in memory;
  - the session list ranks sessions before adding up only the shown page.
  - `veyra-perf` now times the command behind every page, with a p95 budget of 150 ms up to one million events and 400 ms above. `--database` re-times a kept database.
- **Model names through gateways.** The registry resolves `vendor/model` and dotted spellings (`claude-sonnet-4.5`) to canonical models; exact names still match first.
- **Display names and definitions come from the core.** KPI definitions come from the metric registry. Provider, model and application names come from the registry and connector descriptors, through one `DisplayNames` value.
- **One clock and one test kit.** `UtcMillis::now()` replaces duplicated clocks. A new `veyra-testkit` crate holds the shared test fixtures and temporary directories.
- **IPC.** The dashboard window may listen to (and stop listening to) application events; emitting events and every other core plugin stay denied (amends ADR-020).
- **Store.**
  - A larger writer page cache.
  - Per-event statements are prepared once per batch.
  - Session links are applied once per batch.
  - Dropped the unused `usage_events_occurred_at` index.
  - Migration 0003: dropped the unused cost catalogue-version index, and keyed tool calls by their tool-use id (WITHOUT ROWID).
- **Performance budget.** The CI ingest budget is ≥ 35,000 events/s, best of three runs, about 15% below the slowest best-of-three result measured (41,400), matching runner variance. GitHub-hosted runners measure 39,000–49,000 events/s per attempt (best of three 41,400–48,900); developer machines measure 50,000–60,000.
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
