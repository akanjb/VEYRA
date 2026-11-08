# Veyra — Planning Package (Phases 0–5)

**Status:** Approved 2026-11-08, including the recommendations for D1–D5 (recorded as ADR-014, ADR-019, ADR-018). Implementation in progress: see the [roadmap](05-roadmap.md).
**Prepared:** 2026-11-08
**Workspace:** `veyra/` (currently empty apart from this planning package; not yet a git repository)

Veyra is a local-first, zero-cost desktop application that consolidates AI usage across model providers, coding agents, IDEs, desktop apps and CLIs into one accurate, privacy-preserving analytics product.

## Documents

| # | Document | Contents |
|---|---|---|
| 0 | [00-research.md](00-research.md) | Workspace findings, reference-repository comparison (10 dimensions), source-coverage matrix, stack and pricing research |
| 1 | [01-prd.md](01-prd.md) | Vision, users, use cases, functional and non-functional requirements, metric definitions, acceptance criteria |
| 2 | [02-architecture.md](02-architecture.md) | Technology stack with versions, layered architecture, directory structure, contracts, schema, pricing engine, analytics engine, ingestion, organization mode, threat model, performance, tooling, decision records |
| 3 | [03-ux.md](03-ux.md) | Sitemap, navigation, page anatomy, per-module layouts and chart choices, design system, journeys, states, accessibility |
| 4 | [04-feasibility-risks.md](04-feasibility-risks.md) | Feasibility by feature, risks, hidden costs, licensing, prioritisation into five tiers |
| 5 | [05-roadmap.md](05-roadmap.md) | Dependency-ordered milestones with tasks, acceptance criteria, tests and release points |

## Executive summary

1. **Build a Tauri 2 + Rust + SQLite core with a React/TypeScript dashboard.** Research confirms this is the lightest mature option that meets the security, memory and cross-platform goals. The Rust core does all collection, pricing and analytics. The UI only renders typed results.
2. **Accuracy is a first-class data type, not a footnote.** Every metric on every event carries one of Exact / Provider-reported / Estimated / Partial / Unsupported. Unavailable values are `NULL`, never `0`. Each aggregate shows how much of it is exact.
3. **Four sources have exact, local, credential-free token data today:** Claude Code (which also covers the Claude Desktop "Code" tab), Codex CLI, Gemini CLI and OpenCode. These form the MVP. Copilot CLI joins after validation on real files. Official opt-in OpenTelemetry export from Claude Code, Codex, Gemini CLI and Copilot is the schema-stable route, and it arrives as a later milestone.
4. **Some popular tools cannot be tracked accurately, and Veyra will say so.**
   - **Cursor:** local data is estimated at best. Exact figures need its Enterprise Admin API.
   - **No usable token data:** Windsurf, Antigravity, Zed and the ChatGPT desktop app expose none locally.
5. **Retention is the main operational risk.** Claude Code and Gemini CLI delete transcripts after 30 days by default. Veyra must ingest continuously into its own durable store.
6. **Cost has three separate kinds that are never summed:** actual charges, API-equivalent estimates and subscription spend. The type system enforces the separation. Every estimate is priced from a version-controlled catalogue that records effective dates, source URLs and verification dates, so historical events use historical prices.
7. **None of the reference projects is a sound foundation to fork.** All four are MIT licensed but are solo-maintained, and three of them are effectively monoliths. We take ideas only (clean-room) and fix their documented correctness gaps:
   - no deduplication of streamed records;
   - reasoning tokens double-counted;
   - historical events priced at today's rates;
   - a JSON file used as the database;
   - credentials scraped from other apps.
8. **Zero recurring cost is achievable.** The only items that could cost money are optional OS code signing (Apple notarization, Windows signing). The plan ships unsigned builds with SHA-256 checksums and documented install steps, and applies to SignPath Foundation's free open-source signing if Veyra is published under an OSI licence.

## Decisions (approved 2026-11-08: all recommendations accepted)

| # | Decision | Recommendation | Why it matters |
|---|---|---|---|
| D1 | **Subscription quota collection.** Some references read other apps' OAuth tokens and browser cookies and call undocumented endpoints to show live quota. | **Do not do this.** Use only official or first-party channels: the Claude Code statusline feed, rate-limit fields Codex writes to its own logs, and OpenTelemetry. Mark everything else "Unsupported". | Your rules forbid credential extraction. The undocumented endpoints also break without notice. |
| D2 | **Veyra's licence.** | Dual **Apache-2.0 OR MIT** (the Rust-ecosystem norm, OSI-approved). | Makes Veyra eligible for free SignPath Foundation code signing. Clear for contributors. |
| D3 | **Platform order.** | Develop on **Windows first** (your machine), with a CI build and tests on macOS and Linux from the first milestone. Formal macOS and Linux release support follows at milestone M8. | Your machine is the main real-world validation environment. |
| D4 | **Session titles.** Claude Code and others derive titles from prompt text. | **Off by default.** Opt-in setting: off / truncated / full. | Titles are prompt content. |
| D5 | **Code signing for v1.** | **Unsigned + SHA-256 + documented install steps.** Apply to SignPath once D2 is settled. | Avoids paid certificates. |

## Research method and confidence

- **Reference repositories:** cloned shallowly into a scratch directory and read (source, docs, tests, CI, issues, commit metadata through the GitHub API). Nothing was installed or executed. Their behaviour is described from code reading.
- **Data sources, versions and prices:** gathered from official documentation, the npm registry, the crates.io API and the GitHub API on 2026-11-08. Web pages were read through a summarising fetcher, so every price and version is re-verified at the milestone that first depends on it.
- **Unverified items** are labelled **UNVERIFIED** throughout. They are scheduled as validation spikes against real files on this machine ([05-roadmap.md](05-roadmap.md), M2 and M4). They are never assumed.
