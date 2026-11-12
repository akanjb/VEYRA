# 0030. Page rollups and token-only reads

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

ADR-029 made each page run its reads in parallel. The `Performance` workflow then timed every page at ten million synthetic events, against NFR-06's 400 ms budget:

- every page took 0.86–1.9 s (p95);
- the rollup queries behind trends and rankings stayed at 50–70 ms;
- the slow reads went through every raw event in the period (about 820,000 for 30 days). These were session activity (sessions, active and wall time), the session list and session sizes, and the pricing outcomes.
- queries grouped by several of provider, model and application read `rollup_15m`. That level is keyed by every dimension, so it holds 212,000 rows for those 30 days.

ADR-029 named this the signal to add rollups.

## Decision

Migration 0007 adds four levels. They are rebuilt with the existing ones for every bucket a write, a re-pricing or a project merge touches. Right after the migration, the store fills them for existing data with the same rebuild code.

1. **`rollup_15m_sessions`: one row per session, bucket, project, application and interface.** Each row holds the first and last request in the bucket, the active intervals between consecutive requests inside it, the distinct models and the usual sums.
   - The active intervals come from an SQL aggregate (`veyra_active_intervals`) that calls the analytics engine's own `span_of`, so there is one definition of active time.
   - The activity engine now works on these spans. Spans are built either from this level or from raw request times, with identical results: every period boundary is a bucket boundary, and the gap between two of a session's spans is the gap between the last request of one and the first of the next. A property test checks this.
   - The level serves session activity, per-project activity, the session list (ranking and sums), session sizes and the active time of listed sessions.
   - When a session's bucket has several rows (it changed project, application or interface within 15 minutes), that one bucket's span is rebuilt from its events, so the union of intervals stays exact.

2. **`rollup_15m_services`: sums per bucket, provider, model, application and interface.** The reader uses it for any query that groups or filters only by those dimensions and that neither the totals nor the single-dimension level answers. Over 30 days it holds about 84,000 rows rather than 212,000.

3. **`rollup_15m_pricing`: events, tokens and cost per bucket, provider, model id, reported model name, price rule, outcome and set of assumptions.** It serves the cost assumptions panel.

4. **`rollup_15m_size_totals`: requests per bucket and size bin across every dimension.** It serves request sizes without filters, from about 58,000 rows for 30 days rather than 670,000 in `rollup_15m_sizes`.

5. **Token-only reads.** `BaseSumsReader::read_tokens` returns total tokens per group. Its default derives them from the full sums through the `TotalTokens` metric; the store answers from the same planned level but sums one column instead of 28. A test checks that both give the same values at every level. Cards that show tokens only use it: the Models & Providers matrices and each project's top model.

6. **Raw events remain the exact fallback.** A read uses a page rollup only when it answers exactly: the bounds fall on buckets, and the filters are on that rollup's keys. The keys are project, application and interface for sessions, and provider and model for pricing. Any other read uses the events, as before.

## Alternatives considered

- **Daily rollups.** Local days in time zones with half-hour or quarter-hour offsets do not align with UTC days or hours. Only 15-minute buckets align with every period (ADR-022).
- **Keeping provider and model in the session level's key.** Rows would multiply with every model switch, and the interval union would then need merging across rows of the same bucket in most sessions rather than rarely.
- **Caching results by data version.** First loads and every filter change would stay slow.

## Consequences

- At ten million events, a 30-day period reads about 127,000 session rows instead of 820,000 events, 23,000 pricing rows and 84,000 service rows. The 1M and 10M measurements are in the README.
- The store grows by the four levels; the migration of a large existing database rebuilds every rollup once.
- A test compares every read served by the page rollups with the same read from the events on random data. The data includes sessions switching project and application within a bucket, late writes and a project merge. Another test checks that the migration's backfill equals the incremental rebuild.
- Active time is tied to the default idle threshold. Making the threshold configurable would require rebuilding this level.

## Revisit when

A page misses its budget in the `Performance` workflow, or provider and model filters on session pages become common enough to need their own level.
