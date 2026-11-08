# Phase 1 — Product Requirements Document

## 1. Vision

**One honest, private, beautiful view of all your AI usage.**

Veyra runs on your own machine. It reads the usage records that your AI tools already write, and consolidates them across providers, models, applications and interfaces. It tells you exactly how much of each number is measured, reported or estimated. It never uploads anything unless you set up sharing with your own organisation.

### Problem

A developer today might use Claude Code in a terminal and inside Claude Desktop, Codex, Gemini CLI, Copilot and Cursor, all in one project. Each tool shows (at best) its own numbers, in its own units, for a limited window. Some tools delete their history after 30 days. No tool answers:

- "What did this project cost me in AI this month, across every tool?"
- "Which model gives me the best value for the work I actually do?"
- "Is my caching working?"
- "Which of these numbers can I trust?"

### Product principles

1. **Truth over coverage.** An integration that is honestly labelled is worth more than many integrations whose numbers are guessed.
2. **Private by construction.** Veyra reads usage metadata, not conversations. Nothing leaves the device by default.
3. **Zero cost, forever.** No servers, accounts or subscriptions on Veyra's side.
4. **Calm, expert-grade UI.** It shows the most important answer first, and every detail is one click away.
5. **Extensible.** A new source is a new connector module. Nothing else changes.

## 2. Target users

| Persona | Context | Primary jobs |
|---|---|---|
| **Solo multi-tool developer** (primary) | Uses 2–6 AI tools daily; pays for one or more subscriptions plus some API usage | See total usage and cost; compare tools and models; per-project attribution; keep history beyond tool retention |
| **Cost-conscious freelancer / consultant** | Bills clients per project | Project-level reports and exports; separate subscription spend from API-equivalent value |
| **Engineering lead (small team)** | 5–30 engineers on mixed tools | Optional self-hosted aggregation; model and cost trends; no surveillance |
| **Organisation admin / FinOps** | Owns AI budgets | Budgets, run rates, forecasts; actual charges from official billing APIs; audit trail |
| **Privacy-sensitive engineer** | Works on confidential code | Certainty that no prompts, code or credentials are read or stored |

## 3. Core use cases

| ID | Use case |
|---|---|
| UC-01 | Install Veyra, see which supported tools are detected, consent to the exact folders read, and get a populated dashboard after the initial import |
| UC-02 | See this month's total tokens, API-equivalent cost and change against last month, filtered to one project |
| UC-03 | Find out why yesterday's usage spiked: click the day → see the sessions → open the costliest session → see its model changes and tool activity |
| UC-04 | Compare models by effective cost per output token and cache efficiency |
| UC-05 | Record a subscription (e.g. a monthly plan) and see it beside the API-equivalent value of usage, never summed with it |
| UC-06 | See Claude subscription quota usage (5-hour and 7-day windows) with the time it was last observed |
| UC-07 | Get deterministic recommendations with evidence (e.g. "1-hour cache writes that were never reused cost X") |
| UC-08 | Export a monthly per-project cost report as CSV, JSON or XLSX, with project names hashed |
| UC-09 | Keep history after Claude Code or Gemini CLI deletes transcripts older than 30 days |
| UC-10 | See connector health: last success, parse errors, data freshness, metric coverage |
| UC-11 | (Org) Pair a device with the company's self-hosted Veyra Hub, see exactly what is shared, and pause or revoke it |
| UC-12 | Open the same dashboard in a local browser (opt-in) |

## 4. Scope

**In scope (v1.0):**
- local collection from T1 sources;
- unified event store;
- pricing engine;
- analytics modules Overview, Usage, Cost, Cache, Models & Providers, Tools & Agents, Projects, Sessions, Activity, Quotas (official channels), Optimization, Storage, Sources & Diagnostics, Settings;
- exports;
- Windows/macOS/Linux builds.

**Later:**
- T2/T3 connectors;
- OTLP receiver;
- local capture proxy;
- billing connectors;
- localhost browser access;
- organisation mode.

**Non-goals:**
- Not a chat viewer or transcript search tool.
- Not a network interceptor. The capture proxy is opt-in, configured by the user and visible.
- Not a hosted SaaS.
- No AI-generated recommendations (no paid LLM calls).
- No employee surveillance features: no keystroke, screen or prompt capture, no hidden monitoring.
- No modification of other tools' data or configuration. Veyra never renames sessions or writes hooks or settings into other tools without an explicit, reversible user action.

## 5. Functional requirements

IDs are stable and referenced by tests and the roadmap.

### 5.1 Sources and ingestion (FR-ING)

| ID | Requirement |
|---|---|
| FR-ING-01 | Auto-detect supported sources by checking documented default locations and documented environment overrides (e.g. `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`). Veyra reads only location variables, never credential variables |
| FR-ING-02 | Show a consent screen before the first read, listing each detected source, its folder, the fields extracted and the fields ignored. Sources can be enabled or disabled individually |
| FR-ING-03 | Read source files read-only. Never write to, lock, rename or delete them. On Windows, open with full sharing flags so the owning tool can continue to write, rotate or delete |
| FR-ING-04 | Incremental ingestion: per-file byte-offset checkpoints plus a fingerprint of the file's first bytes; detect truncation, rotation and replacement |
| FR-ING-05 | Idempotent writes: re-ingesting any file any number of times yields identical results |
| FR-ING-06 | Deduplicate within and across sources using connector-defined natural keys (request id / message id) |
| FR-ING-07 | Filesystem watching with debounce, plus periodic reconciliation and reconciliation on startup and after resume from sleep |
| FR-ING-08 | Malformed records are skipped, counted and surfaced in diagnostics, and never abort ingestion of the rest of the file |
| FR-ING-09 | Each event records its connector id, parser version and source tool version (when present) |
| FR-ING-10 | SQLite sources are read through a read-only snapshot (the SQLite online backup API from a read-only connection) so the owning app's live database is never locked |
| FR-ING-11 | Historical backfill runs in the background at bounded throughput, and the dashboard stays usable during backfill |
| FR-ING-12 | Veyra's data is retained after the source tool deletes its own logs |

### 5.2 Normalisation and attribution (FR-NORM)

| ID | Requirement |
|---|---|
| FR-NORM-01 | Every event has separate fields for **provider** (model vendor), **model**, **application**, **interface** (CLI, Desktop, IDE, Browser, API, Local runtime) and **gateway** (e.g. Cursor, Copilot, OpenRouter, Bedrock) |
| FR-NORM-02 | Model identifiers are canonicalised through the model registry (aliases, date suffixes, gateway prefixes). Unknown models are kept verbatim and flagged |
| FR-NORM-03 | Token fields are converted into Veyra's canonical meaning (see §7.1), correcting per-vendor differences (e.g. OpenAI `input` includes cached tokens; Anthropic's does not) |
| FR-NORM-04 | Each metric group carries an accuracy class: Exact / Provider-reported / Estimated / Partial / Unsupported. Missing values are `NULL` |
| FR-NORM-05 | Project = normalised working directory, grouped to the repository root when the user allows reading `.git` *presence* (not contents). Users can rename, merge and alias projects |
| FR-NORM-06 | Subagent/sidechain events are linked to their parent session when the source records the link |
| FR-NORM-07 | Tool calls are normalised into canonical categories (file read, file edit, search, shell, web, MCP, subagent/task, other). MCP calls are split into server and tool. Arguments and outputs are never stored |

### 5.3 Pricing and cost (FR-PRICE)

| ID | Requirement |
|---|---|
| FR-PRICE-01 | A version-controlled pricing catalogue with: provider, model pattern, effective-from/to, input, output, cache-read, cache-write (5m/1h), reasoning (if separate), context tiers, batch/priority modifiers, unit, currency, source URL, last-verified date |
| FR-PRICE-02 | Each event is priced with the rule effective at **the event's timestamp** |
| FR-PRICE-03 | Costs are computed in one engine, using exact integer arithmetic, and stored per event with the catalogue version used. Changing the catalogue recomputes the affected events |
| FR-PRICE-04 | Three cost kinds are kept separate: **Actual charge** (verified billing), **API-equivalent estimate**, **Subscription spend** (manual or reported). They are never summed into one figure |
| FR-PRICE-05 | Events with unknown or unpriced models show "no price" and count towards a visible pricing-coverage percentage |
| FR-PRICE-06 | A price not verified within a configurable interval (default 120 days) is flagged "stale rate" in the UI |
| FR-PRICE-07 | User price overrides and custom models in a local file; overridden values are marked as user-supplied |
| FR-PRICE-08 | Manual billing entries (subscriptions, invoices) with recurrence and currency |
| FR-PRICE-09 | Budgets with thresholds, run rate and forecast (forecasts are labelled as projections) |
| FR-PRICE-10 | An assumptions panel for every estimated cost: which rule, which rates, which tier, which modifiers |

### 5.4 Analytics modules (FR-AN)

Each module is detailed in [03-ux.md](03-ux.md). Requirements common to all modules:

| ID | Requirement |
|---|---|
| FR-AN-01 | Global filters (period, comparison period, provider, model, application, interface, gateway, project, device, accuracy) apply consistently to every view |
| FR-AN-02 | Derived metrics (ratios, averages, per-unit costs) are computed from summed base measures in one metric engine, never by averaging ratios |
| FR-AN-03 | Every metric shown has a definition tooltip that comes from the metric registry (single source) |
| FR-AN-04 | Every aggregate displays its accuracy composition (e.g. "96% exact · 4% estimated") |
| FR-AN-05 | Period comparisons use an equal-length previous period (or a chosen period) and show absolute and relative change |
| FR-AN-06 | Every chart has an equivalent sortable table |
| FR-AN-07 | Drill-down from any aggregate to its sessions, and from a session to its event timeline |
| FR-AN-08 | Time is shown in the user's chosen IANA time zone, including half-hour zones such as Asia/Kolkata |
| FR-AN-09 | Active time is reported separately from wall-clock duration, with its definition shown |
| FR-AN-10 | Optimization recommendations are deterministic rules. Each shows evidence, method, confidence and measurable impact. Overlapping savings are not added together |
| FR-AN-11 | Quota views show the last observation time and never present a cached observation as a live balance |

### 5.5 Exports and reports (FR-EXP)

| ID | Requirement |
|---|---|
| FR-EXP-01 | CSV (RFC 4180, UTF-8, optional BOM for Excel), JSON, XLSX |
| FR-EXP-02 | Exports respect current filters and date ranges, and include an accuracy column and a cost-kind column |
| FR-EXP-03 | Redaction options: hash project names, drop paths, drop session titles, drop device names |
| FR-EXP-04 | Report presets: usage summary, cost summary, per-project, per-model, daily/weekly/monthly |

### 5.6 Storage and data lifecycle (FR-STO)

| ID | Requirement |
|---|---|
| FR-STO-01 | Show database size, per-table size, event counts and growth, backup size, and the size of the source directories (size only, no reading) |
| FR-STO-02 | Retention policies: keep all; or roll up detail older than N days into aggregates while keeping totals exact |
| FR-STO-03 | Daily rotating backups (`VACUUM INTO`), restore with a preview |
| FR-STO-04 | Delete Veyra data (all, by source, by date range), with confirmation. **Never** delete source-tool data |

### 5.7 Organisation mode (FR-ORG, optional, later)

| ID | Requirement |
|---|---|
| FR-ORG-01 | A self-hosted Veyra Hub that the organisation runs on its own infrastructure (single binary or container) |
| FR-ORG-02 | Explicit device pairing with an invite code; device key pair; revocable device tokens |
| FR-ORG-03 | Only aggregated, policy-allowed metrics are shared; never prompts, responses, code, paths or titles unless policy *and* user both allow |
| FR-ORG-04 | The employee sees exactly what is shared, can pause sharing, and the device keeps working fully without the hub |
| FR-ORG-05 | Role-based access (owner, admin, analyst, member-self), audit log, isolation between organisations, TLS |

## 6. Non-functional requirements

| ID | Category | Requirement |
|---|---|---|
| NFR-01 | Cost | No paid infrastructure, APIs or libraries. Distribution through GitHub Releases |
| NFR-02 | Privacy | No telemetry. No network access except: opt-in update check, opt-in pricing-catalogue update, user-configured billing connectors, user-configured org hub |
| NFR-03 | Privacy | Prompts, responses, tool arguments, file contents and credentials are never stored. Session titles are off by default |
| NFR-04 | Security | Tauri capabilities restricted to the main window; no shell plugin; no SQL plugin; strict CSP; all output escaped |
| NFR-05 | Performance | Idle CPU ≈ 0% (event-driven); background ingest at below-normal priority; never blocks source tools |
| NFR-06 | Performance | Cold start to interactive overview ≤ 1.5 s on a mid-range laptop with 1M events. Overview queries p95 ≤ 150 ms at 1M events and ≤ 400 ms at 10M |
| NFR-07 | Performance | Core process memory ≤ 120 MB steady state, excluding the webview (measured in CI perf job) |
| NFR-08 | Reliability | Crash-safe ingestion (transactions plus checkpoints committed together); automatic recovery from interrupted runs |
| NFR-09 | Portability | Windows 10/11 x64 and arm64, macOS 13+ (Apple Silicon and Intel), Ubuntu 22.04+ / Fedora (WebKitGTK 4.1) |
| NFR-10 | Accessibility | WCAG 2.2 AA: keyboard navigable, visible focus, contrast checked, table alternative to every chart, reduced-motion respected |
| NFR-11 | Maintainability | Strict typing end to end; generated contracts; module boundaries enforced by lint; ≥ 85% line coverage for domain, pricing and analytics crates |
| NFR-12 | Transparency | Every connector documents its source, fields used, accuracy and known gaps in `docs/connectors/` |

## 7. Analytics definitions (excerpt of the metric registry)

### 7.1 Canonical token fields (per event)

| Field | Meaning | Notes |
|---|---|---|
| `input_uncached` | Prompt tokens billed at the normal input rate | OpenAI/Codex: `input − cached_input`; Anthropic: `input_tokens` |
| `cache_read` | Prompt tokens served from cache | |
| `cache_write_5m`, `cache_write_1h` | Tokens written to cache, by TTL | When only an undifferentiated count exists → `cache_write_unspecified` |
| `output_total` | All generated tokens **including** reasoning | |
| `reasoning` | Reasoning tokens, a reported **subset** of `output_total` | Informational; never added to totals again |
| `tool_tokens` | Tool-related tokens where a source reports them separately (e.g. Gemini `tool`) | Semantics per connector; documented |

### 7.2 Core metrics

All derived metrics are computed from sums (Σ) over the filtered event set.

| Metric | Definition |
|---|---|
| Input tokens | Σ(input_uncached + cache_read + cache_write_*) |
| Total tokens | Input tokens + Σ output_total |
| Cache hit ratio | Σ cache_read ÷ Input tokens |
| Cache reuse ratio | Σ cache_read ÷ Σ cache_write_* (reads per written token) |
| Requests | Σ request_count (one API call = one request where the source records calls) |
| Sessions | Count of distinct sessions with ≥ 1 event in the period |
| API-equivalent cost | Σ priced event cost (priced events only), shown with its coverage |
| Pricing coverage | Tokens on priced events ÷ total tokens |
| Effective $/1M output | API-equivalent cost ÷ Σ output_total × 10⁶ |
| Avg cost per request / session / active hour | Cost ÷ requests / sessions / active hours |
| Gross cache saving | Σ cache_read × (input_rate − cache_read_rate) |
| Cache write overhead | Σ cache_write_5m × (w5m_rate − input_rate) + Σ cache_write_1h × (w1h_rate − input_rate) |
| Net cache benefit | Gross cache saving − Cache write overhead (only over events where all four rates are known) |
| Active time | Union over sessions of intervals [tᵢ, tᵢ + min(tᵢ₊₁ − tᵢ, T)], with idle threshold T (default 5 min, configurable). Labelled *Derived* |
| Wall-clock duration | Last event − first event, per session |
| Daily run rate | Cost over the last 7 complete days ÷ 7 |
| Month forecast | Month-to-date + run rate × remaining days (labelled *Projection*) |
| Concentration | Share of the top-N items (and the Herfindahl index, in the advanced panel) |
| Accuracy composition | Share of the metric's base sum contributed by each accuracy class |

**Accuracy propagation.** An aggregate's class is the weakest class among its contributions, and the UI also shows the composition. "Exact only" filtering excludes Estimated and Partial events.

## 8. Privacy requirements (summary)

1. **What Veyra reads.** Only fields needed for usage: identifiers, timestamps, model, token counts, cost fields reported by the tool, cwd, git branch name, tool *names*, and subagent links.
2. **Read in a bounded way, never stored.** Message bodies may have to be parsed to reach fields in the same JSON line. They are dropped in memory and never persisted or logged.
3. **What Veyra never touches.** No environment variables other than documented location overrides. No keychains, credential files (`.credentials.json`, `auth.json`, `oauth_creds.json`) or browser cookie stores.
4. **Logs.** Veyra's own logs are redacted: no paths beyond the source root and no tokens or keys. Diagnostic bundles are user-reviewed before sharing.
5. **Leaving the machine.** Data leaves the device only through user-initiated exports or user-configured org sync.

## 9. Cost constraints

- **Infrastructure.** Zero developer-side infrastructure. Optional updates and pricing-catalogue updates are static files on GitHub Releases.
- **Dependencies.** Only OSI-licensed, free dependencies (verified in CI by cargo-deny licence policy and a license-checker for npm).
- **Optional costs.** Optional paid OS signing is a documented choice, not a requirement.

## 10. Deployment requirements

- **Installers.** Windows (NSIS `.exe`, MSI), macOS (`.dmg`, `.app.tar.gz` for updates), Linux (AppImage, `.deb`, `.rpm`).
- **Release artefacts.** SHA-256 checksums, a CycloneDX SBOM and build provenance attestation per release.
- **Updates.** Optional auto-update through the Tauri updater with a signed `latest.json` on GitHub Releases. Off until the user allows update checks.
- **Background running.** Optional start at login and a tray/menu-bar presence; closing the window keeps the collector running only if the user chose that.
- **Org mode.** Veyra Hub is distributed as a single binary and a container image (GitHub Container Registry, free for public packages); the organisation hosts it.

## 11. Product acceptance criteria (v1.0)

| # | Criterion |
|---|---|
| AC-1 | On a clean machine with Claude Code, Codex, Gemini CLI and OpenCode history, the overview shows totals that match an independent recount of the synthetic and real-file fixtures **exactly** (token sums) |
| AC-2 | Re-running full ingestion produces zero changed rows (idempotency) |
| AC-3 | Deleting a source's old transcripts does not change Veyra's historical totals |
| AC-4 | An event dated before a price change is costed at the old rate; the assumptions panel names the rule |
| AC-5 | No UI surface sums actual, estimated and subscription costs |
| AC-6 | Unpriced models display "no price" and reduce the visible pricing coverage |
| AC-7 | With collection running during an active Claude Code session, Claude Code shows no errors, file-lock failures or measurable slowdown (verified on Windows) |
| AC-8 | Network monitor during a default-settings session shows **zero** outbound connections |
| AC-9 | All pages are operable by keyboard alone; automated accessibility checks show no AA violations |
| AC-10 | Performance budgets NFR-06/07 are met in the CI performance job with the 1M- and 10M-event synthetic datasets |
