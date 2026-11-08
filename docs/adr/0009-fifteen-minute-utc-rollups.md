# 0009. 15-minute UTC rollup buckets

- **Status:** Accepted; amended by [ADR-022](0022-data-core-implementation.md)
- **Date:** 2026-11-08

## Context

Dashboard queries must be fast at 10M events. Hourly UTC buckets misalign with half-hour time zones such as Asia/Kolkata (+05:30) and Asia/Kathmandu (+05:45).

## Decision

- **Rollup table.** Maintain `rollup_15m`, which holds additive sums per 15-minute UTC bucket for each combination of the core dimensions. It is updated in the same transaction as the events.
- **Why 15 minutes.** Every real-world UTC offset is a multiple of 15 minutes, so local days, hours and weekdays are exact.

## Alternatives considered

- **Hourly buckets:** wrong for half-hour and 45-minute zones.
- **Raw-event aggregation only:** too slow at scale.
- **Per-time-zone rollups:** they multiply storage.

## Consequences

- Up to 96 buckets per day per dimension combination, still far smaller than raw events for heavy users.
- Distinct counts (e.g. sessions) need separate handling.

## Revisit when

Rollup size approaches raw-event size for typical users.
