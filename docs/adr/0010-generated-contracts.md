# 0010. Generated Rust→TypeScript contracts from one command catalogue

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

The dashboard and the Rust core must agree on command names, request and response types, and errors. Hand-maintained TypeScript copies drift.

## Decision

- **The catalogue.** `veyra_app::for_each_command!` is the single declaration of every command (`name(Request) -> Response;`). It expands into three things:
  - the Tauri command handlers (`src-tauri/src/commands.rs`);
  - the Tauri permission allow-list (`build.rs` via `veyra_app::COMMAND_NAMES`);
  - the TypeScript command map (`tools/contracts-gen`).
- **Types.** DTOs derive `ts_rs::TS` (ts-rs 12.0.1). `pnpm contracts:generate` writes `packages/contracts/src/generated/`.
- **Drift check.** `pnpm contracts:check` fails when the committed files are stale, and runs in CI and in the pre-commit hook.
- **Request shape.** Every command takes exactly one `request` argument.

## Alternatives considered

- **specta / tauri-specta:** would also generate the command bindings, but has no stable 2.x release (specta 2.0.0-rc.28, tauri-specta stable 1.0.2 from 2023).
- **Zod schemas validating IPC responses:** would duplicate the Rust types. The in-process boundary is trusted (external data is validated at ingestion instead).

## Consequences

- Adding a command means one catalogue entry, one `Api` method and a regenerate. The compiler and the drift check catch the rest.
- ts-rs maps 64-bit integers to `bigint` by default; M1 decides how token counts are represented.

## Revisit when

tauri-specta reaches a stable 2.x release.
