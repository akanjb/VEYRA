# 0031. Reads sized to what each card shows

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

After the page rollups of ADR-030, the `Performance` workflow still had four pages over NFR-06's p95 budget: Overview, Cost and Models & Providers at one million events (150 ms), and Projects at ten million (400 ms). Every single read was within budget. The pages missed because of the total work they did: GitHub's runners for private repositories have two cores, so a page's parallel reads (ADR-029) share them.

Timing each read showed where the work went:

- **Decoding every sum.** A rollup row carries 28 sums. Reading all of them cost four to six times as much as reading total tokens alone, and each column's position was looked up by name for every row.
- **One row per 15-minute bucket.** Daily and hourly series (the trend by application, each provider's share, each model's timeline) read every bucket of every group, about 13,000 rows for 30 days by model, then summed them into days in Rust.
- **The activity calendar.** It read every sum of every bucket for a year (35,000 rows) to draw tokens per day. The hour-of-week grid did the same for the period.
- **Previous-period cost.** The Cost page's waterfalls read every sum of each model and project in the previous period to use one: API-equivalent cost.
- **Sorting for a few groups.** Each project's top model sums tokens per project and model. At ten million events, 30 days hold 212,000 rows of the full rollup but only 320 such groups. Reading the rows took 47 ms. SQL's `GROUP BY` sorts every row first, which added about 225 ms.
- **Page copies.** Each reader connection copied pages into its own 2 MB cache.

## Decision

1. **Series are summed per local period in SQL.** `BaseSumsReader::read_base_sums_by_period` takes the local period starts the engine already computes. The store sends them as one JSON parameter and joins each period to its own key-range seek of the rollup, summing in SQL. A 30-day daily series by model returns about 300 rows instead of 13,000. The trait's default reads by bucket and relabels each row with its period, so other readers keep working. `rollup_15m_totals` is the exception: it already holds one row per bucket, and reading those rows as stored is cheaper than summing them again.

2. **Cards that show one sum read one sum.**
   - `engine::token_series` gives total tokens per local period. The activity calendar and the hour-of-week grid use it.
   - `BaseSumsReader::read_costs` gives API-equivalent cost per group, as `read_tokens` gives tokens. The Cost waterfalls use it for the previous period.

   Both trait defaults derive the value from the full sums, and a test compares the store's answers with those defaults.

   The store answers these single-sum reads (`read_tokens` and `read_costs`) by reading the one column of each stored row and adding the rows up per group in Rust, without `GROUP BY`. Every stored row holds at least one event, so every group found is non-empty. Reads of all 28 sums still group in SQL, where decoding every column in Rust would cost more than the sort.

3. **Column positions are found at compile time.** `rollup_sum_index` is a `const fn`, so decoding a row no longer searches the column names.

4. **Readers map the database file.** Reader connections set `mmap_size` to 1 GiB, so they read pages from the operating system's file cache, which all of them share, instead of copying them into small private caches. Each reader also keeps up to 128 prepared statements rather than 16, because readers take turns serving every page and the pages together make more distinct reads than 16. The writer is unchanged.

## Alternatives considered

- **Daily rollup tables.** Local days in time zones with half-hour or quarter-hour offsets do not align with UTC days (ADR-022, ADR-030). Summing 15-minute buckets per period in SQL keeps one source of truth.
- **A `CASE` expression or bound parameters per period boundary.** A `CASE` is evaluated row by row through every branch, which is slow for a year of days. Two parameters per period would exceed SQLite's parameter limit for hourly series over long custom ranges. One JSON parameter has no such limit and keeps the statement text constant, so it is prepared once.
- **Larger page caches per reader.** These gave the same speed as memory mapping, but four readers would each hold their own copy of the same pages.
- **Merging more reads per page.** This would save less than the changes above and make each view harder to follow.

## Consequences

- The four pages are within budget, with margin. The measurements are in the README's Performance section.
- Series reads cost about as much as a total over the same range, whatever the grain.
- The port has two more methods with defaults. A store that implements neither gets the same results more slowly.
- Memory mapping reserves address space. On a system where mapping fails, SQLite falls back to ordinary reads.
- On Windows a file cannot be shortened while another connection maps it. Nothing shortens the database today: there is no `VACUUM`, `auto_vacuum` is off, and backups use `VACUUM INTO` to a separate file. Any future feature that shrinks the file, such as compacting storage after deleting old data, must first close the readers or turn their mapping off.

## Revisit when

A feature needs to shrink the database file (see Consequences), a page misses its budget again, or custom ranges of several years become common enough that reading every bucket of the range, even per period, is the slow part.
