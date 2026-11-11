# 0026. Request-size distributions from additive size bins

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

The Usage page shows how large requests are: the median (P50) and 90th percentile (P90) of tokens per request for each day, and the distribution of request sizes over the period (03-ux.md §4).

Percentiles are not additive. Unlike tokens or costs, they cannot be summed from the 15-minute rollups (ADR-008, ADR-009). Computing them exactly needs every request's size, which means reading every event in the period. That is acceptable for a session's timeline, but not for a page that must stay within the query budgets (NFR-06: p95 ≤ 150 ms at 1M events and ≤ 400 ms at 10M).

## Decision

1. **Request counts per size bin are stored as a rollup.** `rollup_15m_sizes` counts the request-level events of each 15-minute bucket per dimension combination and size bin. It is keyed like `rollup_15m`, so every filter applies to it, and it is rebuilt with the other rollup levels whenever a bucket changes. Counts are additive, so any period, filter or grouping merges exactly.

2. **A bin is a quarter of a doubling.** Bin 0 holds requests with no tokens. Bin `1 + ⌊4 · log₂ n⌋` holds a request of `n` tokens, so each bin's upper bound is about 1.19 times its lower bound. The bin is computed exactly with integer arithmetic in one Rust function, `size_bin`. The store registers it as an SQLite function on its writer connection, so the migration's backfill and the rebuild use that same definition.

3. **Percentiles are read from the bins.** Within a bin, sizes are taken as spread evenly on a log scale. A percentile is therefore within about ±9% of the exact value, and is labelled as an estimate. The mean per request still comes from exact sums (tokens divided by requests).

4. **Only request-level events are binned.** Sources that report totals for a whole session or time bucket (Copilot CLI) have no request sizes. They are counted in tokens and requests, but not in the distribution, and the definitions say so.

5. **Session sizes are exact.** Tokens per session come from one grouped read of the period's events (one row per session). The mean, median and P90 per day are computed from those totals. Each session is counted on the day of its first request in the period.

## Alternatives considered

- **Read every request's size and sort.** Exact, but the work grows with the number of events and misses the 10M budget. Kept only for session sizes, where the result has one row per session.
- **Percentile sketches (t-digest, DDSketch) stored per rollup row.** Similar accuracy, but sketches are opaque blobs that SQLite cannot sum, and merging them needs custom code in every read path. Fixed bins merge with `SUM`.
- **SQLite's `log2` in SQL.** Depends on how SQLite was compiled, and floating-point rounding at bin boundaries would put some sizes into the neighbouring bin. A Rust function gives one exact definition.
- **Bins of a whole doubling.** Fewer rows, but percentiles would be within about ±41%, which is too coarse to compare days.

## Consequences

- The Usage page reads request sizes from a rollup, at the same cost as the other cards.
- The rollup rebuild has one more statement per batch, and the database has one more table, roughly as many rows as `rollup_15m` multiplied by the number of size bins per bucket, usually a few.
- Percentiles carry the "estimated" accuracy badge and their definition states the ±9% bound.
- Writing to the database requires the `size_bin` function to be registered. Every writer connection is opened through the store, which registers it.

## Revisit when

Users need exact percentiles, or a chart needs finer bins. Either is a new rollup migration with a different bin width.
