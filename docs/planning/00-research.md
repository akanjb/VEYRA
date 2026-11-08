# Phase 0 — Research and Investigation

## 0.1 Workspace and environment findings

| Item | Finding | Implication |
|---|---|---|
| Workspace | `veyra/` was empty and is not a git repository | Greenfield. `git init` and the repository skeleton belong to milestone M0 |
| OS | Windows 11 (build 26300) | WebView2 is preinstalled. Windows is the first development target |
| Node / npm | Node 22.14.0, npm 10.9.2 | Fine for Vite 8. pnpm via Corepack (M0) |
| git / GitHub CLI | git 2.48.1; `gh` 2.102.0, authenticated | CI and Releases on GitHub cost nothing for public repositories |
| Rust | **Not installed** (`rustc`/`cargo` missing) | M0 installs `rustup` (stable toolchain) and **Visual Studio Build Tools (MSVC C++ workload)**, which Rust needs on Windows |
| Python | Not installed | Not required by the planned stack |

## 0.2 Reference repository comparison

All four were cloned (shallow) and read. No code was executed. Repository metadata was taken from the GitHub API on 2026-11-08.

### 0.2.1 Comparison matrix

| Dimension | **AgentTelemetry** (uttamdeb) | **OpenUsage** (janekbaraniewski) | **ai-usage** (kierandrewett) | **ai-coding-usage-tracker** (SuvenSeo) |
|---|---|---|---|---|
| **1. Features** | Overview, Cost, Models, Tools & agents, Projects, Sessions, Local servers, Storage. Device comparison. Session drawer with timeline. Storage cleanup commands. PWA. VS Code extension | Quota and rate-limit monitor for 37 providers. TUI dashboard. Background daemon. Daily/weekly/monthly/session/5-hour-block reports. Statusline/tmux. Prometheus hub | Multi-source token and cost dashboard. Session table. Daily, model and project charts. CLI `today`/`stats` | Codex/Claude/Cursor reports (daily, weekly, project, model, session). Source audit. Budgets and alerts. Tkinter GUI. HTML/CSV/JSON export. WakaTime sync |
| **2. Architecture / stack** | Python stdlib HTTP server. Vanilla JS with vendored Chart.js 4.4.6 and custom SVG calendar/heatmap. macOS SwiftUI menu-bar app. Windows Win32 tray. Bundles CPython. `parser.py` is about 7k lines | Go 1.26 with CGO SQLite. Bubble Tea TUI. cobra CLI. About 108k lines. Providers are compiled into a hard-coded registry | Python stdlib only. Flat modules. Dashboard HTML/JS is embedded in a Python string | Python stdlib. One file of 6,305 lines containing CLI, parsers, GUI and server |
| **3. Collection method** | Reads JSONL from a stored byte offset. Re-reads SQLite and `.zst` files in full on change. Re-parses every ~15 s | Per-provider polling with backoff and Retry-After handling. A daemon, a hook endpoint and a spool. **Reads other apps' OAuth tokens and cookies and calls undocumented endpoints** | Change detection by mtime plus line count. Delete-and-reinsert for most sources | Fingerprint by mtime plus size. Reparses the whole file. Opens SQLite `immutable=1` read-only |
| **4. Supported apps** | Claude Code, Claude Desktop agent mode, Codex (including `.zst`), Copilot (VS Code and CLI), Cursor, Antigravity, OpenCode, Hermes, OpenClaw | 37 providers. Claude Code, Codex, Cursor and Copilot are deep; many others are thin readers or header probes | Claude Code, Codex, Oh My Pi, OpenCode, OpenRouter API | Codex, Claude Code, Cursor. Admin billing APIs for OpenAI, Anthropic and Cursor |
| **5. Exact vs estimated** | Missing data is labelled "Not recorded". Cost is always labelled API-equivalent. Has an "exact tokens only" filter | Quotas are provider-reported. Costs are API-equivalent. **No per-metric provenance flag** | Separates actual cost from estimated cost. Folds reasoning tokens into output; **likely double-counts for Codex** | Source audit labels each source exact, estimated or blocked. **Cursor tokens are characters ÷ 4 plus an invented cache-replay model** |
| **6. Storage** | **A single JSON ledger** (schema version 64). Daily backups. Keeps history after the source logs are deleted | SQLite: raw events, then normalised events with a unique `dedup_key`, a trigger-maintained change log, reconciliation windows, balance observations and daily rollups | SQLite under `~/.claude/`. **No unique key on turns.** Ad-hoc schema changes | No database. A JSON cache keyed by the hash of the command arguments |
| **7. Security / privacy** | No telemetry. Binds to 127.0.0.1 with **no authentication**. **LAN sharing is unencrypted** and protected by a 59-bit pairing code. Session titles come from prompt text. Has a SECURITY.md | No telemetry SDK. **API keys and cookies stored in a plain JSON file** (0600 permissions). Optional hub authentication. Writes hooks into third-party tool configuration | No prompt storage. Reads the API key from `.env`. **Loads Chart.js from a CDN without an integrity hash** | Read-only SQLite access. Redaction and project-hashing flags. **The on-disk cache is unredacted** |
| **8. UI/UX** | **Best of the four.** Filters read as a sentence and are stored in the URL. A large headline number with change versus the previous period. KPI tiles with sparklines. A data table behind every chart. Monochrome chrome with colour reserved for data. Colour-blind-safe palette checked in CI | TUI with 17 themes. Compact quota tiles. An issue asks for a "just percentages" view | A single page of Chart.js charts | Tkinter tabs. Tables are capped at 400 rows. Static HTML report |
| **9. Performance / scale** | The whole JSON ledger is loaded and saved. Filtering happens in the browser. The README warns of a 30–60 s first parse | Metrics are stored as untyped string maps. Pricing depends on the network and uses fuzzy matching | Likely inflated totals (no deduplication). Cost is computed only in the browser at today's prices | Full reparse on every change. Rereads Cursor's `state.vscdb`, which can reach many GB |
| **10. Worth independently implementing** | Sentence-style filters. Headline number plus change. Table behind every chart. Keeping history after source logs are deleted. Storage view. Cache "savings" view | Event pipeline with a unique dedup key (raw → normalised → rollup). Reconciliation table. Balance history. Polling etiquette (backoff, Retry-After). Context-length price tiers | Separating reported cost from estimated cost. Notes on the OpenCode, Codex and Pi formats | Exact/estimated/blocked source audit. Opening SQLite read-only. Redaction flags. Converting Codex cumulative counters to deltas |
| **Licence / health** | MIT. About 72 test files. CI on 3 operating systems. Very active (v2.2.3-beta.1 on 2026-11-08). 17 stars. Solo maintainer. **Open-core:** some features moved to a closed "Pro" tier | MIT. 315 test files. CodeQL, govulncheck and Scorecard in CI. 241 stars. Effectively solo | MIT (upstream `phuryn/claude-usage`). **No tests and no CI.** No commits since 2026-07-16 | MIT. 31 tests. CI on Ubuntu only. No commits since 2026-06-21. 5 stars |

### 0.2.2 Correctness defects found in references (we design against these)

| # | Defect | Seen in | Veyra countermeasure |
|---|---|---|---|
| C1 | Claude Code streams and resumes sessions, so the same assistant message appears more than once; without deduplication it is counted more than once | ai-usage | Deduplication key `(provider, request_id ‖ message_id)`. Upsert keeps the most complete record |
| C2 | Codex reasoning tokens added on top of output tokens, although reasoning is part of output | ai-usage | Canonical schema: `output_total` *includes* reasoning; `reasoning` is a reported subset that is never added again |
| C3 | Historical events priced at today's rates | ai-usage, tracker | Price rules carry effective dates; each cost is stored with the catalogue version used |
| C4 | Unknown model shown as $0 or silently priced as Sonnet | ai-usage, tracker | Cost status `unpriced`; the UI shows "no price" and coverage percentage, never $0 |
| C5 | A session's cost priced at its single most common model | ai-usage, tracker | Cost is computed per event (per request) |
| C6 | Two pricing tables that disagree (CLI vs dashboard) | ai-usage | One pricing engine in Rust is the single source of truth |
| C7 | Provider confused with the tool that called it | ai-usage, AgentTelemetry | Separate fields for provider, model, application, interface and gateway |
| C8 | Line-count or whole-file checkpoints break on truncation and rotation | ai-usage, tracker | Byte offset plus a fingerprint of the file's first bytes; truncation is detected and the file re-ingested idempotently |
| C9 | Cursor "estimates" presented beside exact data | tracker | Accuracy class on every metric; estimates are excluded from exact totals by default |
| C10 | Credentials scraped and private endpoints called | OpenUsage | Forbidden by design (decision D1) |
| C11 | Local server with no authentication; LAN sharing without encryption | AgentTelemetry, OpenUsage hub | Local HTTP is off by default and requires a token plus a Host-header check; org sync uses TLS with paired device keys |

### 0.2.3 Licence position

All four are MIT. Veyra will be a **clean-room implementation**: ideas only, with no code, CSS, palette values, icons or names copied. Third-party tool logos (for example the brand icons in AgentTelemetry) carry their own licences. Veyra will use its own neutral glyphs and coloured monograms, and will add official brand marks only after a per-brand licence review.

## 0.3 Source-coverage matrix (data sources per tool)

Legend:
- **Token exactness:** E = Exact, PR = Provider-reported, Est = Estimated, P = Partial, U = Unsupported.
- **Doc:** whether the vendor documents the data format.
- **Tier:** T1 = MVP, T2 = next, T3 = later, X = not pursued (shown as Unsupported in the UI).

| Tool | Interface | Location (Windows first) | Format | Doc | Tokens | Credential needed | Tier |
|---|---|---|---|---|---|---|---|
| Claude Code CLI | CLI | `%USERPROFILE%\.claude\projects\<slug>\<session>.jsonl`, subagents in `<session>\subagents\` (`CLAUDE_CONFIG_DIR` overrides) | JSONL | Location and 30-day retention documented; line schema not | E (with dedup) | No | **T1** |
| Claude Desktop "Code" tab | Desktop | Writes to the same `.claude\projects` store; metadata under app data `claude-code-sessions\` | JSONL | Yes (3P data-storage docs) | E | No | **T1** (same connector) |
| Claude Code in VS Code | IDE | Likely the same store (the CLI runs as a subprocess). **UNVERIFIED** | JSONL | — | E if same store | No | T1 (verify in M2) |
| Claude Code OpenTelemetry | CLI/IDE | User enables `CLAUDE_CODE_ENABLE_TELEMETRY=1` and sends OTLP to Veyra's local receiver | OTLP | **Yes** | E (`api_request`: tokens, cost, model, request_id, 5m/1h cache split) | No | T2 (M7) |
| Claude Code statusline | CLI | User-installed statusline command forwards its stdin JSON to Veyra | JSON | **Yes** | Quota `rate_limits.five_hour/seven_day` (Pro/Max), `cost.total_cost_usd` | No | T2 (M6) |
| Claude Desktop Chat/Cowork | Desktop | `%APPDATA%\Claude\local-agent-mode-sessions\…\audit.jsonl` (Store install uses a `Packages\Claude_*` path) | JSON/JSONL | Documented only for the 3P build; **standard build UNVERIFIED** | E likely ("model and token usage") | No | T2 |
| Codex CLI | CLI | `%USERPROFILE%\.codex\sessions\YYYY\MM\DD\rollout-*.jsonl` (+ `archived_sessions`, `.jsonl.zst`; `CODEX_HOME`) | JSONL (cumulative `total_token_usage`; `rate_limits`) | `CODEX_HOME` documented; schema not | E (input, cached, output). Reasoning semantics **UNVERIFIED** | No | **T1** |
| Codex OpenTelemetry | CLI | `[otel]` in `config.toml` (flushes on exit) | OTLP | **Yes** | E | No | T2 (M7) |
| Codex app (desktop) | Desktop | Whether it shares `~/.codex/sessions` is **UNVERIFIED** | — | — | ? | No | T2 (verify) |
| ChatGPT desktop | Desktop | Electron chat cache only | — | No | U | — | X |
| Gemini CLI | CLI | `%USERPROFILE%\.gemini\tmp\<hash>\chats\session-*.jsonl` (30-day retention default) | JSONL (`tokens{input,cached,output,thoughts,tool,total}`) | Path and retention documented; schema not | E (`thoughts`/`total` semantics **UNVERIFIED**) | No | **T1** |
| Gemini CLI OpenTelemetry | CLI | `telemetry.enabled`, local target | OTLP / file | **Yes** | E | No | T2 (M7) |
| OpenCode | CLI/TUI | `opencode.db` (Linux `~/.local/share/opencode/`; Windows path **UNVERIFIED**) | SQLite (WAL) | Path documented; schema not | E per message | No | **T1** |
| GitHub Copilot CLI | CLI | `%USERPROFILE%\.copilot\session-state\<id>\events.jsonl` (`COPILOT_HOME`) | JSONL; tokens only at `session.shutdown` | Partly | P–E (aggregation **UNVERIFIED**) | No | T1 after a real-file spike (M4) |
| Copilot in VS Code | IDE | Opt-in OTel `github.copilot.chat.otel.*` (file or OTLP exporter) | OTel GenAI spans | **Yes** | E where emitted | No | T2 (M7) |
| Cursor | IDE | `%APPDATA%\Cursor\User\globalStorage\state.vscdb` (**UNVERIFIED** on Windows); can reach many GB | SQLite blobs | No | **Est** (counts mostly zeroed) | No | T3 (labelled estimate) |
| Cursor Admin API | API | `POST /teams/filtered-usage-events` | JSON | **Yes** | E (`chargedCents`, cache) | Enterprise team key (user-supplied) | T3 |
| Cline / Roo Code | IDE | VS Code `globalStorage\<ext-id>\tasks\<id>\ui_messages.json` | JSON | No | E when provider reports usage | No | T2 |
| Aider | CLI | `.aider.chat.history.md`, opt-in `--analytics-log` | MD / JSONL | Flags documented | P (sent/received only) | No | T2 |
| Continue | IDE | `~/.continue/dev_data/<ver>/*.jsonl` | JSONL | Location only | P | No | T3 |
| OpenClaw | CLI | `~/.openclaw/agents/<id>/agent/openclaw-agent.sqlite` | SQLite | Location documented | P (**UNVERIFIED**) | No | T3 |
| Windsurf / Devin Desktop | IDE | Cloud analytics (Enterprise) | — | Yes | U locally | Enterprise | X |
| Antigravity | IDE | Transcripts without token usage | — | No | U | — | X |
| Zed | IDE | Thread DB; token persistence **UNVERIFIED** | — | No | U | — | X |
| Gemini Code Assist | IDE | GCP Cloud Monitoring (org) | — | Yes | Org only | GCP role | X |
| Ollama / LM Studio / OpenAI-compatible endpoints | Local runtime / API | No usage history on disk; the response `usage` fields are exact | — | Yes | E via **opt-in local capture proxy** | No | T2 |
| OpenRouter | API | `GET /api/v1/activity` (30-day window) | JSON | **Yes** | PR (tokens and USD) | Management key (user-supplied) | T2 |
| OpenAI org usage/costs | API | `/v1/organization/usage/*`, `/costs` | JSON | Yes | PR | Admin key | T2 |
| Anthropic org usage/cost | API | Usage and Cost Admin API | JSON | **Yes** | PR | Admin key (org only) | T2 |
| xAI | API | Management API billing usage | JSON | **Yes** | PR | Management key | T3 |
| Groq / DeepSeek / Mistral / Together | API | No verified per-token history API (DeepSeek: balance only) | — | — | U | — | X (use the local proxy) |

**Validation backlog.** Each item below is checked on real files before its connector ships:
- Claude Code in VS Code: whether it uses the same store.
- Codex: reasoning-token semantics, and whether the Codex app shares the session store.
- Gemini: what `total`, `tool` and `thoughts` contain.
- Copilot CLI: how shutdown records aggregate.
- Windows paths for Cursor and OpenCode.
- Claude Desktop (standard build): the audit-log schema.

## 0.4 Technology research summary

Verified on 2026-11-08 from the npm registry, the crates.io API and the GitHub API. The full selection rationale is in [02-architecture.md §2](02-architecture.md#2-technology-stack).

**Desktop shell.**
- Tauri **2.12.5** (Apache-2.0 OR MIT). Recent security advisories are all fixed at or above 2.11.6.
- Tauri 3 is alpha and is not used.
- Rejected alternatives: Electron 45 (heavier), Wails v3 (Go backend), Electrobun (young), Neutralino (weak security model).

**Data.**
- rusqlite **0.41.1** (bundled SQLite) with rusqlite_migration **2.6.0**.
- DuckDB **2.0** is held back as an optional later analysis engine.
- Rejected: diesel (four SQLite-backend advisories in 2026), sqlx (async-first and macro-heavy) and SeaORM (unnecessary abstraction).

**Background processing.**
- tokio 1.54.1 and notify 8.2.1 (the 9.0 release candidate is not used).
- keyring 4.2.0 for the OS credential store.
- Logging uses tracing.

**Type contracts.**
- ts-rs **12.0.1** for stable Rust-to-TypeScript type generation.
- specta / tauri-specta have **no stable 2.x**. Revisit when they do.

**Frontend.**
- React **19.3.1** with React Compiler 1.0, built with Vite **8.3.6**.
- TanStack Query 5, TanStack Router 1, TanStack Table 9 and TanStack Virtual 3.
- Tailwind 4.3, with Radix + shadcn components (copied into the repository).
- **Charts:** Apache ECharts **6.1.0**.

**TypeScript version.**
- npm `latest` is **7.0.2**, the native port, released *today*. typescript-eslint does not support it yet.
- We use **TypeScript 6.0.3** with type-aware ESLint, and move to 7 later.

**Supply chain.**
- In May 2026, 84 malicious `@tanstack/*` versions were published (GHSA-g7cv-rxg3-hmpx). Current versions are clean.
- Mitigation:
  - exact version pins and a committed lockfile;
  - pnpm `minimumReleaseAge`, so brand-new releases are not installed immediately;
  - dependency install scripts disabled by default;
  - GitHub Actions pinned by commit SHA;
  - cargo-deny and cargo-audit in CI.

**Code signing.**
- Windows unsigned: SmartScreen warns, and Smart App Control may block.
- macOS unsigned: Gatekeeper requires a right-click "Open".
- Paid Apple and Azure options are not required.
- SignPath Foundation offers free signing for OSI-licensed projects, subject to approval.

## 0.5 Pricing research

- **No vendor publishes a machine-readable price feed.** Checked: Anthropic, OpenAI, Google. Anthropic's Models API returns capabilities and lifecycle data but no prices.
- **Pricing structures to model:**
  - Batch discounts (50%).
  - Cache-write tiers (Anthropic 5m = 1.25× base input, 1h = 2×).
  - **Model-specific cache-read multipliers.** Anthropic: 0.1× by default, 0.05× on Opus/Sonnet 5.5, 0.025× on Fable 5.1. A single global multiplier is wrong.
  - Prompt-length tiers (Claude Haiku 5.5 above 100k; OpenAI above 272k; Gemini Pro above 200k).
  - Regional multiplier (Anthropic `inference_geo: us` = ×1.1) and fast/priority tiers.
  - Announced future price changes (Google flagship and Flash on 2026-12-01 and 2027-01-01). This makes effective-dated rules mandatory.
- **Tokenizer change:** Claude 4.7+ tokenizers produce about 30% more tokens for the same text. Cross-model "efficiency" comparisons must state this.
- **Seed snapshot.** A per-model rate snapshot (USD per 1M tokens) was captured for Anthropic, OpenAI and Google flagship and small models. It goes into the M1 catalogue only after **row-by-row re-verification against the official pages**, because the pages were read through a summarising fetcher.
