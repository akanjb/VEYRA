# 0002. Modular monolith with hexagonal boundaries

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

The product must grow to many connectors and analytics modules and stay maintainable by a small team. It needs no distributed infrastructure.

## Decision

- **One process.** A single desktop process. The Rust core is a Cargo workspace of focused crates, and dependencies point inwards.
  - `veyra-domain` is pure and owns the port traits.
  - Adapters implement those ports: connectors, store, infrastructure.
  - `src-tauri` is the only composition root.
- **Dashboard.** The dashboard is the presentation layer. It talks only to the application API.
- **Crates arrive when needed (YAGNI).** M0 has `veyra-app`, `veyra-desktop` (`src-tauri`) and the `veyra-contracts-gen` tool. Domain, store, ingest, connectors and analytics crates are added in the milestone that first needs them.

## Alternatives considered

- **Microservices / sidecar processes:** operational complexity with no benefit for a single-user local app.
- **One crate with modules:** weaker enforcement of boundaries, slower incremental builds.

## Consequences

- Boundaries are enforced by crate dependencies (Rust) and the `veyra/module-boundaries` lint rule (dashboard).
- The org Hub can reuse domain, store and analytics crates unchanged.

## Revisit when

A module needs independent deployment or scaling (e.g. Hub load).
