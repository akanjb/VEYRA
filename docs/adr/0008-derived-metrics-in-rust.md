# 0008. Additive sums in SQL, derived metrics in one Rust engine

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Ratios averaged across groups are wrong (the average of ratios is not the ratio of sums). Formulas duplicated across SQL, Rust and UI drift apart.

## Decision

- **SQL sums only.** SQL returns only additive base measures (sums and counts) grouped by the requested dimensions.
- **One engine derives the rest.** The Rust metric engine computes every derived metric (ratios, averages, per-unit costs, savings, shares, deltas) from those sums, using one `MetricDefinition` registry.
- **Tooltips from the same source.** Definitions are exported to the UI, so tooltips come from the same place.

## Alternatives considered

- **Derived metrics in SQL views:** formulas duplicated per query and hard to test.
- **Computing in the UI:** violates the rule that the dashboard holds no business logic.

## Consequences

- Every metric has one formula, one test and one description.

## Revisit when

A metric genuinely cannot be derived from additive sums (e.g. distinct counts across buckets). Such metrics get dedicated, documented queries.
