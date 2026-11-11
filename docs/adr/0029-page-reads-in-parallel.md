# 0029. Dashboard pages read in parallel, with a budget per page

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

NFR-06 sets a p95 of 150 ms at one million events and 400 ms at ten million. Until now `veyra-perf` timed only three analytics queries. Each dashboard page, however, is one command that makes many reads: rollup queries for trends and rankings, plus reads over raw events for sessions, active time, request sizes and pricing outcomes. Timing every page through the application facade on one million synthetic events showed:

- every page took 110–250 ms on its own, because its reads ran one after another;
- one page of the session list summed every session in the period (about 2,000) to show 50;
- readers wrote temporary sort files to disk, which antivirus software scanned before use, slowing every grouping read several times over.

## Decision

1. **A page runs its independent reads at the same time.** Each view starts the parts that do not depend on each other on scoped threads (`std::thread::scope`) and collects them with `views::joined`, which passes on a panic. A page then costs about as much as its slowest part. Parts that need another part's result, such as the shares that need the scope's totals, still wait for it.

2. **Four reader connections, and a free one is used first.** The store keeps four read-only connections (it was two). A read takes the first free connection, starting from the next in turn, and waits only when all are busy. SQLite's WAL mode lets them read at the same time while the single writer commits.

3. **Readers sort in memory.** Like the writer, readers use `temp_store = MEMORY`. The largest sort a page makes is over one period's events, which fits comfortably.

4. **A session page is ranked before it is summed.** The session list first ranks sessions by the sort key alone and keeps the page's ids. Only those sessions are then summed, using the session index. The totals are unchanged, because both steps use the same event filter.

5. **Every page has a budget.** `veyra-perf` times the command behind every page through `veyra_app::Api`, as the desktop shell calls it, over the last 30 days with comparison. The p95 budget is 150 ms up to one million events and 400 ms above. `--database` re-times a database kept by `--keep`.

## Alternatives considered

- **New rollups for sessions, active time and pricing outcomes.** These would make raw-event reads independent of volume, but each adds a table, a rebuild path and a migration. They stay the next step if the ten-million-event budget is missed.
- **A thread-pool crate.** It would save a few microseconds per page over spawning scoped threads, at the cost of a new dependency.
- **Caching page results.** Results would have to be invalidated on every write and on every scope change. It also hides slow queries rather than fixing them.

## Consequences

- Pages are bounded by their slowest read rather than by the sum of their reads.
- Two extra reader connections cost a few megabytes of page cache.
- A view's independent parts must be written as functions of `&DataServices` and `&Resolved`, which they already were.

## Revisit when

The `Performance` workflow misses a page budget at ten million events. That is the signal to add rollups for the raw-event reads.
