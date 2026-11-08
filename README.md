<div align="center">

# Veyra

### One honest, private view of all your AI usage

Veyra is a local-first desktop application that consolidates AI usage from model providers, coding agents, IDEs, desktop apps and CLIs into one accurate, privacy-preserving analytics product. Everything runs on your own machine.

![Status](https://img.shields.io/badge/status-early%20development%20(M0)-orange)
![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)
![Stack](https://img.shields.io/badge/stack-Tauri%202%20%C2%B7%20Rust%20%C2%B7%20React%2019%20%C2%B7%20SQLite-6f42c1)
![Author](https://img.shields.io/badge/author-Nikunj%20Bansal-0a7ea4)

[Overview](#overview) ·
[Features](#features) ·
[How it works](#how-it-works) ·
[Architecture](#architecture) ·
[Getting started](#getting-started) ·
[Development](#development) ·
[Roadmap](#roadmap) ·
[Documentation](#documentation) ·
[Author](#author)

</div>

---

> [!IMPORTANT]
> **Project status: Milestone M0 (Foundations) complete.** The secure desktop shell, the generated Rust-to-TypeScript contract pipeline, the quality tooling and CI are in place. Usage collection and the analytics dashboard arrive in milestones M1–M8. This README marks every capability as **Available** or **Planned**.

## Table of contents

- [Overview](#overview)
- [Features](#features)
- [Data accuracy model](#data-accuracy-model)
- [Supported sources](#supported-sources)
- [How it works](#how-it-works)
- [Architecture](#architecture)
  - [System context](#system-context)
  - [Layered architecture](#layered-architecture)
  - [Backend (Rust core)](#backend-rust-core)
  - [Frontend (dashboard)](#frontend-dashboard)
  - [Command catalogue and generated contracts](#command-catalogue-and-generated-contracts)
  - [Data model](#data-model)
- [Privacy and security](#privacy-and-security)
- [Technology stack](#technology-stack)
- [Repository structure](#repository-structure)
- [Getting started](#getting-started)
- [Development](#development)
- [Testing strategy](#testing-strategy)
- [Continuous integration](#continuous-integration)
- [Roadmap](#roadmap)
- [Documentation](#documentation)
- [Author](#author)
- [Contributing](#contributing)
- [Security policy](#security-policy)
- [License](#license)

## Overview

A developer today might use Claude Code in a terminal and inside a desktop app, Codex, Gemini CLI, Copilot and Cursor, all on one project. Each tool reports (at best) its own numbers, in its own units, for a limited window, and some delete their history after 30 days.

Veyra answers the questions no single tool can:

- **Cost per project:** what did this project cost in AI usage this month, across every tool?
- **Model value:** which model gives the best value for the work I actually do?
- **Caching:** is prompt caching working, and what does it save?
- **Trust:** which of these numbers can I trust?

### Principles

| Principle | What it means in practice |
|---|---|
| **Truth over coverage** | Unavailable data is shown as unavailable, never as zero. Estimates are never presented as measurements. |
| **Private by construction** | No telemetry, no accounts. Veyra never reads prompts, responses, source code or other applications' credentials. |
| **Zero cost** | No servers, subscriptions, paid APIs or managed databases, on your side or ours. |
| **Calm, expert-grade UI** | The most important answer comes first. Every detail is one click away. Every chart has a table. |
| **Extensible** | A new data source is a new connector module. Nothing else changes. |

## Features

| Capability | Status |
|---|---|
| Secure desktop shell (Windows, macOS, Linux) with a capability-restricted IPC and a strict Content Security Policy | **Available** |
| Typed command layer generated from a single Rust catalogue (handlers, permissions, TypeScript types) | **Available** |
| Local SQLite store with idempotent, deduplicated, incremental ingestion | Planned (M1–M2) |
| Version-controlled pricing engine with historical, effective-dated prices | Planned (M1) |
| Connectors: Claude Code (CLI, desktop Code tab, IDE), Codex CLI, Gemini CLI, OpenCode, Copilot CLI | Planned (M2–M4) |
| Dashboard: Overview, Usage, Cost, Cache, Models & Providers, Projects, Sessions, Activity | Planned (M3–M5) |
| Tools & Agents, Quotas & Limits, deterministic Optimization recommendations | Planned (M7) |
| Opt-in local OpenTelemetry receiver | Planned (M7) |
| CSV / JSON / XLSX exports with redaction; storage and retention controls | Planned (M6) |
| Installers, opt-in updater, checksums and SBOMs | Planned (M8) |
| Optional self-hosted organisation hub | Planned (M10) |

### Three kinds of cost, never mixed

Veyra keeps **actual charges**, **API-equivalent estimates** and **subscription spend** separate, and never adds them together. A flat-rate subscriber does not pay the per-token API price, and Veyra will never label an estimate as a bill.

## Data accuracy model

Every metric on every event carries an accuracy class. Aggregates show their composition (for example, *96% exact · 4% estimated*).

| Class | Meaning | Example |
|---|---|---|
| **Exact** | Recorded directly by the source | Token counts in Claude Code session logs |
| **Provider-reported** | Supplied by the provider's own usage interface | An organisation usage API |
| **Estimated** | Calculated with a documented method | API-equivalent cost; Cursor token estimates |
| **Partial** | Some events or fields are missing | Sessions that ended before final totals were written |
| **Unsupported** | No reliable, permitted source exists | Tools that store no usage data locally |

```mermaid
flowchart LR
  E[Exact] --> AGG{Aggregate}
  P[Provider-reported] --> AGG
  S[Estimated] --> AGG
  A[Partial] --> AGG
  AGG --> W[Weakest class<br/>shown as badge]
  AGG --> C[Composition<br/>e.g. 96% exact · 4% estimated]
  AGG --> X["Exact-only filter<br/>excludes Estimated and Partial"]
```

## Supported sources

The tiers below come from the research in [`docs/planning/00-research.md`](docs/planning/00-research.md). A tool appears as **Unsupported** when it exposes no trustworthy usage data that Veyra may read without credentials.

| Source | Interface | Data | Accuracy | Tier |
|---|---|---|---|---|
| Claude Code (CLI, desktop Code tab, IDE extension) | CLI / Desktop / IDE | Local JSONL session logs | Exact | MVP |
| Codex CLI | CLI | Local JSONL rollouts | Exact | MVP |
| Gemini CLI | CLI | Local JSONL sessions | Exact | MVP |
| OpenCode | CLI | Local SQLite | Exact | MVP |
| GitHub Copilot CLI | CLI | Local session events | Partial → Exact | MVP after validation |
| Claude Code / Codex / Gemini CLI / Copilot (VS Code) | Official OpenTelemetry (opt-in) | OTLP to a local receiver | Exact | Next |
| Claude Desktop Chat/Cowork, Aider, Cline, Roo Code | Desktop / CLI / IDE | Local files | Exact–Partial | Next |
| Ollama, LM Studio, OpenAI-compatible endpoints | Local runtime / API | Opt-in local capture proxy | Exact | Later |
| OpenRouter, OpenAI, Anthropic, xAI admin APIs | API | User-supplied keys, OS keyring | Provider-reported | Next |
| Cursor | IDE | Local database (estimates); Enterprise Admin API | Estimated / Provider-reported | Later |
| Windsurf, Antigravity, Zed, ChatGPT desktop | — | No usable local usage data | Unsupported | — |

## How it works

```mermaid
flowchart LR
  subgraph Your machine
    direction LR
    T1[AI tools write<br/>their own usage logs] -->|read-only| C[Connectors]
    C --> N[Normaliser<br/>canonical tokens · attribution]
    N --> P[Pricing engine<br/>effective-dated rates]
    P --> DB[(Local SQLite<br/>events · costs · rollups)]
    DB --> A[Analytics engine<br/>metrics · comparisons · rules]
    A -->|typed IPC| UI[Veyra dashboard]
  end
  UI -. optional, opt-in .-> HUB[(Organisation hub<br/>self-hosted)]
```

1. **Discover.** Veyra detects supported tools at their documented locations and asks for consent before reading anything.
2. **Collect.** Connectors read new data incrementally from byte-offset checkpoints. They never lock, modify or delete source files.
3. **Normalise.** One normaliser converts vendor-specific token semantics into canonical fields and separates provider, model, application, interface and gateway.
4. **Price.** Each event is priced using the rate that was in effect at that event's timestamp. Unknown models show "no price", never $0.
5. **Analyse.** SQL returns only additive sums. Every ratio, average and saving is derived in one Rust metric engine.
6. **Present.** The dashboard renders typed results. It contains no business logic.

## Architecture

Veyra is a **modular monolith with hexagonal boundaries**: one desktop process, with focused crates, inward-pointing dependencies and a single composition root. See [ADR-002](docs/adr/0002-modular-monolith-hexagonal.md).

### System context

```mermaid
flowchart TB
  user([Developer])
  subgraph veyra [Veyra desktop application]
    ui[Dashboard<br/>React · TypeScript]
    core[Rust core<br/>Tauri 2]
    db[(SQLite)]
  end
  tools[[AI tools' local usage files<br/>Claude Code · Codex · Gemini CLI · OpenCode …]]
  otel[[Opt-in OpenTelemetry<br/>from AI tools]]
  apis[[Opt-in official usage APIs<br/>user-supplied keys]]
  hub[(Self-hosted organisation hub<br/>optional)]

  user --> ui
  ui <-->|typed IPC| core
  core <--> db
  core -->|read-only| tools
  otel -->|localhost only| core
  core -->|HTTPS, opt-in| apis
  core -. aggregated, policy-filtered .-> hub
```

### Layered architecture

```mermaid
flowchart TB
  subgraph L1 [1 · Presentation]
    UI[React dashboard · apps/desktop]
    CMD[Tauri command adapters · src-tauri]
  end
  subgraph L2 [2 · Application]
    APP[veyra-app · use cases · DTOs · command catalogue]
  end
  subgraph L3 [3 · Domain]
    DOM[veyra-domain · events · accuracy · money · pricing & metric engines · ports]
  end
  subgraph L7 [7 · Background processing]
    ING[veyra-ingest · scheduler · watcher · checkpoints]
  end
  subgraph L6 [6 · Provider integration]
    CON[veyra-connectors · one module per source]
  end
  subgraph L5 [5 · Data access]
    STO[veyra-store · SQLite · migrations · rollups]
  end
  subgraph L4 [4 · Infrastructure]
    INF[OS paths · filesystem · keyring · clock · logging]
  end

  UI --> CMD --> APP
  APP --> ING
  APP --> DOM
  ING --> DOM
  CON -. implements ports .-> DOM
  STO -. implements ports .-> DOM
  INF -. implements ports .-> DOM
```

| Layer | Owns | Never does |
|---|---|---|
| Presentation | Routing, filter state, rendering, formatting | Calculate metrics or costs, parse sources, run SQL |
| Application | Use cases, DTO mapping, cross-port transactions | Contain SQL or file parsing |
| Domain | Business rules, pure engines, port traits | Perform I/O |
| Provider integration | Parsing source formats, declaring capabilities | Price, store or render |
| Data access | SQLite, migrations, repositories, rollups | Make business decisions |
| Background processing | Scheduling, watching, retries, cancellation | Know any specific source format |

### Backend (Rust core)

**Crate dependency graph.** Solid boxes exist today; dashed boxes arrive in later milestones.

```mermaid
flowchart LR
  desktop[veyra-desktop<br/>src-tauri · composition root]
  app[veyra-app<br/>use cases · DTOs · catalogue]
  gen[veyra-contracts-gen<br/>tools/contracts-gen]
  dom[veyra-domain]:::planned
  reg[veyra-registry]:::planned
  con[veyra-connectors]:::planned
  ing[veyra-ingest]:::planned
  sto[veyra-store]:::planned
  ana[veyra-analytics]:::planned

  desktop --> app
  gen --> app
  app -.-> dom
  app -.-> ing
  app -.-> ana
  desktop -.-> sto
  desktop -.-> con
  ing -.-> dom
  ana -.-> dom
  con -.-> dom
  sto -.-> dom
  reg -.-> dom

  classDef planned stroke-dasharray: 5 5,opacity:0.7
```

**IPC request flow (implemented).** Every command takes one typed `request` and returns a typed response or a typed `ApiError`.

```mermaid
sequenceDiagram
  autonumber
  participant C as React component
  participant H as Hook (useAppInfo)
  participant AC as ApiClient (shared/api)
  participant T as Tauri IPC
  participant ACL as Capability check
  participant CH as Command handler (generated)
  participant API as veyra_app::Api

  C->>H: render
  H->>AC: call("get_app_info", {})
  AC->>T: invoke("get_app_info", { request: {} })
  T->>ACL: is "allow-get-app-info" granted to window "main"?
  ACL-->>T: granted
  T->>CH: deserialise AppInfoRequest
  CH->>API: api.get_app_info(request)
  API-->>CH: Ok(AppInfo) or Err(ApiError)
  CH-->>T: serialise
  T-->>AC: AppInfo, or rejection
  AC-->>H: typed AppInfo, or ApiCallError / TransportError
  H-->>C: state: ready | error
```

**Ingestion pipeline (planned, M2).** It is crash-safe: events, costs, rollups and checkpoints commit in one transaction.

```mermaid
sequenceDiagram
  participant W as File watcher (hint)
  participant S as Scheduler
  participant C as Connector
  participant N as Normaliser
  participant P as Pricing engine
  participant DB as SQLite writer
  participant UI as Dashboard
  W->>S: path changed (debounced)
  S->>S: coalesce · bounded concurrency · low priority
  S->>C: plan(location, checkpoints)
  C->>C: stream from byte offset (read-only, shared access)
  C-->>N: parsed records + per-line errors
  N->>N: canonical tokens · provider/model/app/interface · dedup key
  N->>P: price at event timestamp
  P-->>DB: events + costs + rollup deltas + checkpoint (one transaction)
  DB-->>UI: data.changed {range, dimensions}
  UI->>UI: invalidate affected queries only
```

### Frontend (dashboard)

```mermaid
flowchart TB
  main[main.tsx<br/>bootstrap] --> app[app/<br/>shell · router · providers]
  app --> mods
  subgraph mods [modules/ · one folder per product page]
    ov[overview]
    us[usage]
    co[cost]
    more[sessions · projects · …]
  end
  subgraph shared [shared/ · module-agnostic building blocks]
    api[api<br/>ApiClient · transports]
    fil[filters<br/>URL state]
    ch[charts]
    tb[data-table]
    ui[ui<br/>design system]
    fmt[format]
  end
  mods --> shared
  app --> shared
  api -->|only place allowed| tauri[["@tauri-apps/api"]]
  api --> contracts[["@veyra/contracts<br/>generated types"]]
```

Lint-enforced rules (`veyra/module-boundaries`):

- `shared/` never imports `modules/` or `app/`.
- A module never imports another module.
- Only `shared/api` may import Tauri packages. The rest of the UI is transport-agnostic, so the same dashboard can later run over localhost HTTP or against an organisation hub.

**Data flow (target design).**

```mermaid
flowchart LR
  URL[(URL search params<br/>single source of filter state)] --> R[Route]
  R --> Q[TanStack Query hook<br/>key = command + filters]
  Q --> AC[ApiClient]
  AC --> CORE[Rust core]
  CORE --> DTO[Typed DTO]
  DTO --> VM[View model<br/>pure mapping, unit-tested]
  VM --> V[Presentational components<br/>charts · tables · KPI tiles]
  CORE -.->|data.changed event| Q
```

### Command catalogue and generated contracts

One declaration drives three outputs, so the Rust core, the IPC permissions and the TypeScript types cannot drift apart. See [ADR-010](docs/adr/0010-generated-contracts.md).

```mermaid
flowchart LR
  CAT["for_each_command!<br/>crates/veyra-app/src/commands.rs<br/><code>get_app_info(AppInfoRequest) -> AppInfo;</code>"]
  CAT --> H[Tauri command handlers<br/>src-tauri/src/commands.rs]
  CAT --> PERM[IPC permission allow-list<br/>src-tauri/build.rs]
  CAT --> GEN[contracts generator<br/>tools/contracts-gen]
  DTO[DTOs with ts-rs derives<br/>crates/veyra-app/src/dto] --> GEN
  GEN --> TS[packages/contracts/src/generated<br/>CommandMap · DTO types · ApiError]
  TS --> CLIENT[Typed ApiClient<br/>apps/desktop/src/shared/api]
  CHECK[pnpm contracts:check<br/>CI + pre-commit] -. fails on drift .-> TS
```

### Data model

This is the planned core schema (M1). Unavailable values are stored as `NULL`, never `0`. Money is stored as integer nano-USD.

```mermaid
erDiagram
  SOURCES ||--o{ SOURCE_FILES : "checkpoints"
  SOURCES ||--o{ SESSIONS : "produces"
  PROJECTS ||--o{ SESSIONS : "groups"
  SESSIONS ||--o{ USAGE_EVENTS : "contains"
  SESSIONS ||--o{ SESSIONS : "parent of (subagents)"
  USAGE_EVENTS ||--|| EVENT_COSTS : "priced as"
  PRICE_RULES ||--o{ EVENT_COSTS : "applied"
  USAGE_EVENTS ||--o{ TOOL_CALLS : "records"

  USAGE_EVENTS {
    text id PK
    text dedup_key UK
    int occurred_at
    text provider_id
    text model_id
    text app_id
    text interface
    int input_uncached
    int cache_read
    int cache_write_5m
    int cache_write_1h
    int output_total
    int reasoning
    text accuracy_flags
  }
  EVENT_COSTS {
    text event_id FK
    text catalog_version
    int total_nusd
    text status
  }
  PRICE_RULES {
    text model_pattern
    date effective_from
    date effective_to
    text source_url
    date verified_at
  }
```

A 15-minute UTC rollup table backs fast dashboard queries. Every real-world UTC offset is a multiple of 15 minutes, so local days and hours stay exact in zones like Asia/Kolkata (+05:30). See [ADR-009](docs/adr/0009-fifteen-minute-utc-rollups.md).

## Privacy and security

| Veyra reads | Veyra never reads or stores |
|---|---|
| Timestamps, model identifiers, token counts | Prompts, responses or conversation text |
| Session and request identifiers | Source code or file contents |
| Working directory and git branch name (for project attribution) | Tool-call arguments or outputs |
| Tool *names* and categories | Other apps' credentials, keychains, cookie stores |
| Cost figures reported by the tool itself | Environment variables other than documented location overrides |

**Security model (implemented in M0).**

```mermaid
flowchart LR
  page[Dashboard page] -->|invoke| ipc{Tauri IPC}
  ipc -->|window = main<br/>and allow-get-app-info granted| ok[Command runs]
  ipc -->|any other window| deny1[Rejected: not allowed]
  ipc -->|command not in catalogue| deny2[Rejected: not found]
  csp[["Strict CSP<br/>script-src self · style-src self<br/>connect-src ipc only"]] --- page
```

- **IPC permissions.** IPC is limited to the commands declared in the catalogue, and only the `main` window may call them. This is verified by the integration tests in [`src-tauri/tests/ipc_access.rs`](src-tauri/tests/ipc_access.rs).
- **Content Security Policy.** The production CSP allows no `unsafe-inline` and no `unsafe-eval`. This was verified on a built app through WebView2 DevTools.
- **Plugins.** No shell plugin, no SQL plugin and no global Tauri object. The JavaScript prototype is frozen.
- **No telemetry.** Network access happens only through features you explicitly enable.
- **Supply chain.** Exact version pins, a 7-day minimum release age, dependency install scripts disabled, GitHub Actions pinned to commit SHAs, and cargo-deny plus `pnpm audit` in CI. CodeQL and dependency review return when the repository becomes public ([ADR-021](docs/adr/0021-ci-scope-while-private.md)). See [ADR-017](docs/adr/0017-supply-chain-policy.md).

## Technology stack

| Layer | Technology | Why |
|---|---|---|
| Desktop shell | **Tauri 2.12** | System webview: small installers, low memory, capability-based IPC security |
| Core | **Rust 1.99** (edition 2024) | Memory safety, low footprint, excellent SQLite and filesystem libraries |
| Storage | **SQLite** via rusqlite (planned) | Embedded and zero-ops, with WAL for safe concurrent reads |
| UI | **React 19.3** + **TypeScript 6.0** (strict) | Mature ecosystem; strict typing end to end |
| Build | **Vite 8.3** | Fast, Tauri-native |
| Contracts | **ts-rs 12** + command catalogue macro | Rust is the single source of truth for every IPC type |
| Charts | **Apache ECharts 6** (planned) | One library for calendar, heatmap, treemap, sankey and large datasets |
| Data fetching / routing | **TanStack Query** and **TanStack Router** (planned) | Cached server state; the URL as the single source of filter state |
| Quality | ESLint 10 + typescript-eslint (strict, type-aware), Prettier, rustfmt, clippy, Vitest | Enforced in CI and pre-commit |
| Supply chain | pnpm 12 (release-age gate), cargo-deny, pnpm audit | Defence against compromised dependencies |

Every choice and its rejected alternatives are recorded in the [architecture decision records](docs/adr/README.md).

## Repository structure

```text
veyra/
├── apps/
│   └── desktop/                 # React + TypeScript dashboard (Vite)
│       └── src/
│           ├── app/             # bootstrap, shell, providers
│           ├── modules/         # one folder per product page (from M3)
│           ├── shared/          # module-agnostic building blocks
│           │   └── api/         # ApiClient, Tauri transport, typed errors
│           └── styles/
├── packages/
│   └── contracts/               # GENERATED TypeScript contracts (never edit)
├── crates/
│   └── veyra-app/               # application layer: Api, DTOs, command catalogue
├── src-tauri/                   # Tauri shell: composition root, commands, capabilities
│   ├── capabilities/            # IPC permission grants
│   └── tests/                   # IPC access-control integration tests
├── tools/
│   └── contracts-gen/           # generates packages/contracts from Rust
├── tooling/
│   └── eslint/                  # custom module-boundaries lint rule + tests
├── docs/
│   ├── planning/                # research, PRD, architecture, UX, risks, roadmap
│   └── adr/                     # architecture decision records 0001–0021
├── .github/workflows/           # CI and security pipelines
└── .githooks/                   # pre-commit checks
```

## Getting started

### Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| Node.js | ≥ 22.12 | |
| pnpm | 12.17.0 | `npm i -g pnpm@12.17.0`, or Corepack |
| Rust | pinned in `rust-toolchain.toml` | Install [rustup](https://rustup.rs); the toolchain installs automatically |
| Windows | Visual Studio Build Tools (C++ workload), WebView2 | WebView2 ships with Windows 11 |
| macOS | Xcode Command Line Tools | `xcode-select --install` |
| Linux | WebKitGTK 4.1 and build essentials | See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) |

### Install and run

```bash
pnpm install
```

```bash
pnpm hooks:install
```

```bash
pnpm dev
```

`pnpm dev` starts the Vite dev server on `127.0.0.1:1420` and opens the desktop window with hot reload.

### Build

```bash
pnpm build
```

## Development

| Command | Purpose |
|---|---|
| `pnpm dev` | Run the desktop app with hot reload |
| `pnpm build` | Build the production app and installers |
| `pnpm check` | Run everything CI runs (below) |
| `pnpm lint` / `pnpm lint:rust` | ESLint (type-aware) / clippy with warnings as errors |
| `pnpm typecheck` | Strict TypeScript across all packages |
| `pnpm test` / `pnpm test:rust` | Vitest / Rust unit and integration tests |
| `pnpm format` / `pnpm format:check` | Prettier and rustfmt |
| `pnpm contracts:generate` | Regenerate TypeScript contracts after changing Rust DTOs or commands |
| `pnpm contracts:check` | Fail if the generated contracts are out of date |

### Adding a command

```mermaid
flowchart LR
  A[1 · Add DTOs<br/>crates/veyra-app/src/dto] --> B[2 · Add one catalogue line<br/>for_each_command!]
  B --> C[3 · Implement Api method<br/>same name]
  C --> D[4 · Grant allow-name<br/>capabilities/main.json]
  D --> E[5 · pnpm contracts:generate]
  E --> F["6 · Call it from the UI<br/>client.call(name, request)"]
```

The compiler, the IPC access tests and the contract drift check catch any missed step.

## Testing strategy

| Level | What it covers | Tooling |
|---|---|---|
| Unit | Domain rules, use cases, error contracts, API client behaviour, lint rules | `cargo test`, Vitest |
| Integration | IPC access control against a mock Tauri runtime, with the real configuration | `cargo test` (`src-tauri/tests`) |
| Contract | Generated TypeScript matches Rust DTOs and the command catalogue | `pnpm contracts:check` |
| Parser / golden (M2+) | Each connector and format version: fixtures → expected normalised events | `cargo test` |
| Performance (M1+) | Query and ingest budgets on 1M / 10M synthetic events | Nightly CI |
| End-to-end (M3+) | UI flows with IPC fixtures; smoke tests on real bundles for all three OSes | Playwright, WebdriverIO |

Test data is always synthetic. Real transcripts or credentials are never committed.

## Continuous integration

```mermaid
flowchart LR
  PR[Push to main / pull request] --> WEB[Web job<br/>Prettier · ESLint · typecheck · Vitest]
  PR --> RUST[Rust jobs · Ubuntu 22.04 · Windows<br/>rustfmt · clippy · tests · contract drift · desktop build]
  MAN[Manual run<br/>Actions → CI → Run workflow] --> RUST
  MAN --> MAC[Rust job · macOS]
  PR --> SEC[Security<br/>cargo-deny · pnpm audit]
  SCHED[Weekly schedule] --> SEC
```

- All actions are pinned to full commit SHAs, with read-only permissions by default.
- While the repository is private, macOS builds run only on manual dispatch, because macOS minutes are billed at 10×. CodeQL and dependency review are off, because they require GitHub Advanced Security on private repositories.
- Both come back when the repository is made public. See [ADR-021](docs/adr/0021-ci-scope-while-private.md).

## Roadmap

```mermaid
flowchart LR
  M0[M0 Foundations<br/>✅ complete]:::done --> M1[M1 Data core<br/>domain · store · pricing · metrics]
  M1 --> M2[M2 Claude Code<br/>end-to-end ingestion]
  M2 --> M3[M3 Dashboard shell<br/>design system]
  M2 --> M4[M4 Codex · Gemini CLI<br/>OpenCode · Copilot CLI]
  M3 --> M5[M5 Analytics modules]
  M4 --> M5
  M5 --> M6[M6 Exports · Storage<br/>Settings]
  M5 --> M7[M7 Tools & Agents · Quotas<br/>Optimization · OTLP]
  M6 --> M8[M8 Release v1.0]
  M7 --> M8
  M8 --> M9[M9 More connectors<br/>localhost access]
  M9 --> M10[M10 Organisation hub]
  classDef done fill:#d1f7d6,stroke:#2f9e44
```

| Release | Contents |
|---|---|
| 0.1 (internal alpha) | M0–M3: Claude Code, Overview, Sessions, Sources |
| 0.5 (public beta) | All MVP connectors and analytics modules, exports |
| 1.0 | Tools & Agents, Quotas, Optimization, OpenTelemetry receiver, installers, updater |
| 2.0 | Self-hosted organisation hub |

The detailed, dependency-ordered plan with acceptance criteria is in [`docs/planning/05-roadmap.md`](docs/planning/05-roadmap.md).

## Documentation

| Document | Contents |
|---|---|
| [Planning package](docs/planning/README.md) | Overview, approved decisions |
| [Research](docs/planning/00-research.md) | Reference-project comparison, source-coverage matrix, stack and pricing research |
| [Product requirements](docs/planning/01-prd.md) | Users, use cases, requirements, metric definitions, acceptance criteria |
| [Architecture](docs/planning/02-architecture.md) | Stack, layers, contracts, schema, pricing and analytics engines, threat model |
| [UX plan](docs/planning/03-ux.md) | Sitemap, page anatomy, chart choices, design system, accessibility |
| [Feasibility and risks](docs/planning/04-feasibility-risks.md) | Feasibility, risk register, hidden costs, prioritisation |
| [Roadmap](docs/planning/05-roadmap.md) | Milestones, tasks, acceptance criteria |
| [Decision records](docs/adr/README.md) | ADR 0001–0021 |

## Author

**Nikunj Bansal**: creator and maintainer of Veyra.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for the engineering standards, the module rules, the dependency admission checklist and the pull-request process, and follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Security policy

Please report vulnerabilities privately. See [SECURITY.md](SECURITY.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Veyra is dual-licensed as above, without additional terms or conditions.
