# Contributing to Veyra

Thank you for your interest. This guide covers the contribution terms, setup, engineering standards and the review checklist.

## Contribution terms

Veyra is source-available, not open source ([LICENSE](LICENSE)). Before you contribute, note:

- **Acceptance is at the maintainer's sole discretion.** Pull requests may be changed, declined or closed without review. Contributing never grants write access or any role in the project.
- **Rights.** By submitting a contribution you assign its rights to the author, as section 5 of the licence sets out, and confirm it is your own work.
- **No ownership.** A contribution, accepted or not, does not make you an author, owner or co-owner of Veyra.
- **Talk first.** Open an issue to discuss a change before you write code for it.

## Setup

Follow the development section of the [README](README.md), then run `pnpm hooks:install` to enable the pre-commit checks.

## Architecture in one paragraph

- **Layers and dependencies.** Veyra is a modular monolith with hexagonal boundaries (ADR-002). Dependencies point inwards.
- **Rust core.**
  - The domain holds pure logic and the port traits.
  - The application layer (`veyra-app`) exposes one method per use case.
  - Adapters (connectors, storage, OS) implement the ports.
  - `src-tauri` is the only place that wires them together.
- **Dashboard.** The dashboard renders typed results and holds **no** business logic: no metric formulas, pricing or parsing.
- **Further reading.** [docs/planning/02-architecture.md](docs/planning/02-architecture.md).

## Rules that are enforced

| Rule | Enforced by |
|---|---|
| Module boundaries: `shared/` never imports `modules/` or `app/`; modules never import each other; only `shared/api` imports Tauri | `veyra/module-boundaries` ESLint rule |
| Strict TypeScript; no unsafe `any` flows | `tsc` strict + typescript-eslint `strictTypeChecked` |
| Rust lints, no `unsafe` | `cargo clippy -D warnings`, `unsafe_code = "forbid"` |
| Rust ↔ TS contracts never drift | `pnpm contracts:check` (CI and pre-commit) |
| Only the `main` window may call commands; each command must be granted | Tauri capabilities + `src-tauri/tests/ipc_access.rs` |
| Formatting | Prettier, rustfmt |

## Adding a command

1. Add the request and response DTOs in `crates/veyra-app/src/dto/`.
2. Add one line to `for_each_command!` in `crates/veyra-app/src/commands.rs`.
3. Implement the `Api` method with the same name.
4. Grant `allow-<command-name>` in `src-tauri/capabilities/main.json`.
5. Run `pnpm contracts:generate` and commit the generated files.

Events the core emits to the dashboard are declared the same way, in `for_each_event!`.

## Adding a connector

A connector is a pure parser; the ingest pipeline does all reading, checkpointing, pricing and storage (ADR-023). Use `crates/veyra-connectors/src/claude_code` as the model.

1. Run a structure-only spike on real logs: field names, types and counts. Never print or store content.
2. Create a module in `crates/veyra-connectors/src/` implementing `SourceConnector`:
   - `default_roots`: documented locations and location variables only, never credentials (ADR-014);
   - `is_source_file`: a strict allow-list of paths;
   - `parse`: complete lines in, raw records out, with a precedence that makes duplicates order-independent.
3. Register it in `veyra_connectors::all()`.
4. Add synthetic fixtures under `tests/fixtures/<connector>/<format-version>/`, with `VEYRA-CANARY-…` markers wherever real logs hold content, and tests asserting every mapped field.
5. Check it on your own logs with the counts-only diagnostic:

   ```bash
   cargo run --release -p veyra-ingest --example scan_root -- <projects-dir> <new-db-file>
   ```

6. Document it in `docs/connectors/<connector>.md`: paths, fields used, mapping rules, precedence and diagnostics.

## Updating prices

Prices live in `registry/pricing/*.toml` and are the single source of truth for every cost Veyra shows (ADR-007).

1. Read the provider's **official** pricing page. Never use third-party aggregators.
2. For a price change, end the old rule with `until = "<change date>"` and add a new rule with `from = "<change date>"`, so historical events keep their historical price.
3. Update `verified_at` on every rule you re-checked, and record judgement calls (display rounding, unpriceable items) as comments next to the rule.
4. Bump `version` in `registry/pricing/catalog.toml`. Stored events are re-priced against the new version.
5. Add or adjust a spot-check test in `crates/veyra-registry/src/tests.rs` that reproduces a published figure exactly.

## Code standards

- **Single responsibility.** Each file, function and component has one clear purpose. Prefer cohesion over many tiny files.
- **Pure core, effects at the edges.** Calculations are pure functions. I/O stays in adapters.
- **One source of truth.** Shared definitions (providers, models, prices, metric formulas, design tokens) live in exactly one place.
- **Explicit errors.** Rust uses typed errors (`thiserror`). The UI receives the closed `ApiError` union.
- **Comments explain why, not what.**
- **Tests for every calculation.** A correct-looking chart is not evidence of correct numbers.
- Size warnings (300 lines per file, 60 per function) start a conversation. They are not mechanical limits.

## Dependency admission checklist

Before adding **any** dependency, record these in the PR description:

1. Upstream repository and maintainers.
2. Licence: must be on the allow-list in `deny.toml`, or permissive for npm.
3. Maintenance activity: recent releases, open-issue health.
4. Security history and current advisories.
5. Transitive footprint.
6. Why it is necessary; what we would do without it.
7. An exact pinned version, published at least 7 days ago (ADR-017).

A new technology or a major dependency also needs an ADR (`docs/adr/template.md`).

## Test data

**Use synthetic fixtures only.** Never commit real transcripts, usage logs, credentials or anything derived from a real user's files.

## Commits and pull requests

- Keep each PR focused on one change. Describe *why*, list how it was verified, and link the relevant ADR or plan section.
- `pnpm check` must pass.
- Contributions are accepted only under the contribution terms above.
