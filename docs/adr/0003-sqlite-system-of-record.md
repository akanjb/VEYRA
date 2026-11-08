# 0003. SQLite (rusqlite) as the system of record

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Veyra needs an embedded, zero-operations store for millions of usage events with fast time-range aggregation. It must be crash-safe and readable while ingestion runs.

## Decision

- **Engine.** SQLite through **rusqlite** with the bundled SQLite, in WAL mode.
- **Connections.** One writer thread owns the write connection and commits each batch, with its checkpoints, in one transaction. A small pool of reader connections serves queries.
- **Migrations.** Managed with `rusqlite_migration` (`user_version`).
- **SQL style.** Hand-written SQL behind repository interfaces.

## Alternatives considered

- **sqlx:** async-first; its compile-time query checks need a live database.
- **diesel:** four SQLite-backend advisories in 2026.
- **SeaORM:** an abstraction the mostly analytical schema doesn't need.
- **DuckDB 2.0:** faster scans, but a large binary and long compile times. Kept as an optional analysis engine later (attach the SQLite file read-only).

## Consequences

- Aggregation speed relies on rollup tables and covering indexes (ADR-009).
- The database must live on a local disk (WAL), in the per-user app data directory.

## Revisit when

Analytics queries miss the performance budgets at 10M events despite rollups.
