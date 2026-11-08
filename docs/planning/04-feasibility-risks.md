# Phase 4 — Feasibility, Risks and Prioritisation

## 1. Feasibility by capability

| Capability | Feasibility | Basis / condition |
|---|---|---|
| Exact tokens, model, time, project and session for Claude Code (CLI, Desktop Code tab) | **Feasible now** | Local JSONL; deduplication required |
| Same for Codex CLI, Gemini CLI, OpenCode | **Feasible now** | Local JSONL/SQLite; unverified semantics flagged for spikes |
| Copilot CLI | **Feasible after validation** | Tokens only at `session.shutdown`; crashed sessions lost; aggregation rule unverified |
| Claude Desktop Chat/Cowork | **Partial / to verify** | Audit log documented for the 3P build only |
| Cursor, local | **Estimated only** | Counts mostly zero; the UI must label it as an estimate and exclude it from exact totals |
| Cursor exact | **Requires official API** | Enterprise Admin API with a user-supplied key |
| Windsurf, Antigravity, Zed, ChatGPT desktop | **Not reliably implementable** | No local token data; shown as Unsupported with reasons |
| Ollama / LM Studio / OpenAI-compatible / DeepSeek / Groq / Mistral / Together | **Requires opt-in local hook** | Local capture proxy that the user points their client at; exact `usage` from responses |
| OpenTelemetry for Claude Code / Codex / Gemini CLI / Copilot VS Code | **Requires opt-in local hook** | User enables telemetry; Veyra runs an OTLP receiver on localhost |
| Claude subscription quota | **Requires opt-in local hook** | Statusline command forwards documented JSON; Pro/Max only |
| Codex rate-limit windows | **Feasible (undocumented)** | `rate_limits` inside the user's own rollout files; no credentials; labelled best-effort |
| Cursor, Copilot, Gemini subscription quotas | **Not without private endpoints/credentials** | Excluded by D1; shown as Unsupported |
| Org billing (OpenAI, Anthropic, OpenRouter, xAI) | **Requires official APIs** | Admin/management keys supplied by the user, stored in the OS keyring |
| Actual subscription charges | **Manual entry** | No official per-user billing API for consumer subscriptions |
| Historical pricing | **Feasible** | Effective-dated catalogue; accuracy depends on the curated history |
| Cache savings | **Feasible where all four rates and fields exist** | Otherwise "not computable" for those events |
| Active time | **Derived** | Heuristic with a stated definition and adjustable threshold |
| First-pass success / rework signals | **Partial** | Only where tool-result errors are recorded (Claude Code `is_error`); otherwise unavailable |
| Organisation sync | **Feasible (self-hosted)** | Separate binary; no developer-hosted infrastructure |
| Localhost browser access | **Feasible** | Same dashboard via HTTP transport; token-protected |
| Signed installers at zero cost | **Conditional** | Free only through SignPath Foundation (OSI licence, approval); otherwise unsigned with warnings |

## 2. Risk register

Ratings: L = likelihood, I = impact (H/M/L).

| # | Risk | L | I | Mitigation | Owner milestone |
|---|---|---|---|---|---|
| R1 | Undocumented source formats change with tool updates | H | H | Tolerant parsers; per-version fixtures; parser versioning; health alerts on parse-error spikes; OTel path as the stable alternative; fast-release process | M2+, ongoing |
| R2 | Source tools delete history (30-day defaults) before ingestion | H | H | Continuous watching; ingest at startup; warning when Veyra was not running for > 20 days; Veyra keeps its own history | M2 |
| R3 | Double counting across sources (JSONL + OTel, resumed sessions) | M | H | Shared deduplication keys; primary-source policy per app; reconciliation tests | M2, M7 |
| R4 | Wrong token semantics (reasoning, cached-in-input, Gemini `tool`/`thoughts`) | M | H | Canonical schema; per-connector semantics tags; validation spikes on real files; golden tests | M2, M4 |
| R5 | Price data stale or mis-entered | M | H | `verified_at` per row; stale flags; checklist workflow; tests on effective-date logic; user overrides | M1, ongoing |
| R6 | Users read API-equivalent cost as a bill | M | M | Separate cost kinds in types and UI; persistent labelling ("≈ API-equivalent"); assumptions panel | M5 |
| R7 | Collector interferes with the user's tools (file locks, CPU) | L | H | Share-mode reads; snapshot for SQLite; below-normal priority; bounded throughput; AC-7 test on Windows | M2 |
| R8 | WebKitGTK/WKWebView render differences (CSS, canvas performance) | M | M | Early 3-OS CI build smoke; visual checks on Linux; conservative CSS feature use; ECharts canvas | M0, M3 |
| R9 | Unsigned binaries deter users (SmartScreen, Gatekeeper) | H | M | Clear install docs; checksums; SignPath application; reproducible build notes | M8 |
| R10 | Supply-chain compromise of an npm or crate dependency | M | H | Pins, release-age gate, disabled scripts, cargo-deny, SBOM, minimal dependency set | M0, ongoing |
| R11 | Privacy leak through logs, exports or titles | L | H | Allow-list extraction; canary-string tests; redaction layer; titles off by default | M2, M6 |
| R12 | Performance degrades with years of history | M | M | Rollups; indexes; retention; perf CI with 10M events | M1, M3, ongoing |
| R13 | TypeScript 7 / Tauri 3 transitions | M | L | ADRs with explicit revisit triggers; no premature adoption | ongoing |
| R14 | Contract drift between Rust and TS | L | M | Generated contracts + CI drift check | M0 |
| R15 | Scope creep (37-provider breadth like OpenUsage) | H | M | Tiered connector roadmap; a connector ships only with fixtures, documentation and a capability matrix | ongoing |
| R16 | Small-team maintenance burden | M | M | Modular connectors; contributor guide; connector template; golden-file test harness | M0, M8 |
| R17 | Tool vendors' terms of service on reading local data | L | M | Read only the user's own local files; no credentials or private endpoints; document the position | ongoing |
| R18 | The updater signing key is lost or leaked | L | H | Offline backup; documented rotation; key held only in GitHub encrypted secrets and the offline store | M8 |

## 3. Hidden and potential costs

| Item | Cost | Required? | Zero-cost path |
|---|---|---|---|
| Apple Developer Program (notarisation) | Paid annual fee (amount to verify) | No | Unsigned/ad-hoc builds with documented Gatekeeper steps; source build |
| Windows code signing (OV, Azure Artifact Signing) | Paid; Azure needs a paid subscription and eligible region | No | SignPath Foundation (OSI licence, approval) or unsigned |
| GitHub Actions | Free for public repositories (standard runners) | — | Keep the repository public; private repos get a limited monthly credit |
| GitHub Releases / GHCR | Free for public | — | — |
| Domain / website | Optional | No | GitHub Pages on `github.io` |
| macOS CI minutes | Free on public repos; expensive on private | — | Public repo |
| Org Hub hosting | Paid by the organisation on its own infrastructure | Org only | — |

**Developer-side recurring cost: $0**, provided the repository is public.

## 4. Licensing

- **Veyra licence (D2).** Recommended dual Apache-2.0 OR MIT.
- **Dependency licences.** All selected dependencies are permissive: MIT, Apache-2.0, CC0, BSD/ISC and OFL for fonts. Copyleft is blocked by a cargo-deny licence policy and an npm licence check.
- **Reference repositories.** All MIT. Clean-room use only. No assets copied.
- **Brand logos.** Third-party marks are trademarked. v1 uses neutral monograms, and official logos are added only per each brand's guidelines.

## 5. Prioritisation

### Tier 1 — Core MVP (v1.0)
- **Platform:** Tauri shell, security baseline, contracts pipeline, CI on 3 operating systems.
- **Data core:** domain model, SQLite store, migrations, rollups, pricing engine with a verified catalogue (Anthropic, OpenAI, Google), metric engine.
- **Connectors:**
  - Claude Code (covers the CLI, the Desktop Code tab, and VS Code once verified);
  - Codex CLI;
  - Gemini CLI;
  - OpenCode;
  - Copilot CLI (if the validation spike passes).
- **Modules:** Overview, Usage, Cost (three kinds, manual subscriptions, budgets), Cache, Models & Providers, Projects, Sessions, Activity, Sources & Health, Storage (basic), Settings.
- **Product:** exports (CSV, JSON, XLSX), installers, checksums, SBOM, documentation.

### Tier 2 — High-priority extensions (v1.x)
- **Tools & Agents.**
- **Quotas:** the Claude statusline forwarder and Codex rate limits.
- **Optimization** rules v1.
- **OTLP receiver:** Claude Code, Codex, Gemini CLI, Copilot VS Code.
- **More connectors:** Claude Desktop Chat/Cowork, Aider, Cline/Roo.
- **Opt-in updater.**

### Tier 3 — Advanced analytics (v1.x–v2)
- Advanced forecasting with seasonality.
- Saved views and custom dashboards.
- Model comparison workspace.
- Retention rollup pruning UI.
- Optional DuckDB analysis mode.
- Localhost browser access.

### Tier 4 — Optional organisation capabilities (v2)
- Veyra Hub: pairing, policies, RBAC, audit, the Hub dashboard and a team view.
- Billing connectors: OpenAI, Anthropic, OpenRouter and xAI admin APIs (also useful for individuals with keys).

### Tier 5 — Future integrations
- Local capture proxy (Ollama, LM Studio, OpenAI-compatible).
- Cursor (local estimate and Enterprise API).
- Continue, OpenClaw.
- New tools as they publish usage data or OTel. A tool moves from Tier 5 to Tier 2 when it ships a documented local usage record or OTel.
