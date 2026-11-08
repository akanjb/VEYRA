# 0022. Data-core implementation refinements

- **Status:** Accepted (amends ADR-003, ADR-005 and ADR-009)
- **Date:** 2026-11-08

## Context

Building M1 (domain, registry, store, analytics) and measuring it at one million synthetic events showed where the plan needed refinement:
- An external migration crate was incompatible with the pinned SQLite driver.
- Cache-write names were provider-specific.
- A single dimensional rollup could not meet the overview query budget for long ranges.

## Decision

1. **Migrations.** Use our own forward-only runner on `PRAGMA user_version`: about 40 lines, tested, and each step runs in one transaction. `rusqlite_migration` 2.6.0 requires rusqlite 0.40, but we pin 0.41 (amends ADR-003).
2. **Cache-write fields.** The canonical fields are `cache_write_short` and `cache_write_long`: the provider's short and long cache TTLs (Anthropic 5 minutes and 1 hour; OpenAI 30 minutes and 12 hours). They replace `cache_write_5m` / `cache_write_1h` (amends ADR-005).
3. **Three rollup levels, all keyed by 15-minute UTC buckets and clustered by time** (`WITHOUT ROWID` tables). This amends ADR-009.

   | Level | Rows | Serves |
   |---|---|---|
   | `rollup_15m_totals` | one per bucket | Ungrouped, unfiltered queries: headline numbers, activity calendar, long-range trends |
   | `rollup_15m_by_dimension` | one per bucket and value of each single dimension | Queries grouped and/or filtered by one dimension: most dashboard cards |
   | `rollup_15m` | one per bucket and dimension combination | Everything else |

   The reader picks the smallest level that answers a query exactly. A test asserts that all levels agree on random data, including unknown values and integer-id dimensions.
4. **Set-based rebuilds.** The buckets a batch touches go into a temporary table, and each level is rebuilt for all of them with one statement. Under identical conditions this measured 2–4× faster than rebuilding bucket by bucket (about 34.6K versus 9–18K events/s at 100K events).
5. **Local-period mapping by binary search.** The engine maps buckets to local hours, days, weeks and months with a binary search over the period boundaries instead of a time-zone conversion per row. This is exact because every boundary falls on a 15-minute bucket.
6. **Content fingerprints.** Each event stores an FNV-1a fingerprint of its content and cost, so re-ingesting an unchanged event does nothing. A stable hash is used because the standard hasher may change between Rust releases.

## Measurements

These come from `veyra-perf`: synthetic, realistic sessions, ingested in time order, with the 1M database (about 517 MB including all rollup levels).

| Run | 30-day daily by app (with comparison) | 30-day top models | 12-month trend |
|---|---|---|---|
| Single rollup level | p95 137–163 ms | p95 110–135 ms | p95 396 ms (fail) |
| Three levels (machine with free memory) | p95 76 ms | p95 51 ms | p95 81 ms |

- **Memory pressure.** Runs taken while the development machine had under 1 GB of free RAM were disk-bound and are not representative.
- **CI check.** The `Performance` workflow runs the same budget check on a clean runner, manually or weekly.
- **Scattered input.** The worst case (events in random time order) ingests about 13K events/s. Real ingestion follows file and session order.

## Consequences

- **Storage.** The rollup levels add storage, about 18% on the 1M dataset.
- **Two more levels to keep consistent.** This is safe because all three are rebuilt together in the same transaction.
- **Planning-document terms.** Documents written before this ADR that mention `cache_write_5m` / `cache_write_1h` refer to the short/long fields.

## Revisit when

Query budgets fail on the clean CI runner, or multi-dimension queries become common enough to warrant further levels.
