# 0006. Per-metric accuracy classes; NULL for unavailable

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Sources range from exact per-request logs to rough estimates (e.g. Cursor). Presenting them uniformly would mislead users.

## Decision

- **Accuracy classes.** Each metric group on each event carries an accuracy class:
  - Exact;
  - Provider-reported;
  - Estimated;
  - Partial;
  - Unsupported.
- **NULL, not zero.** Unavailable values are `NULL`, never `0`.
- **Propagation.** An aggregate takes the weakest class among its contributions. The UI also shows the accuracy composition, and an "Exact only" filter excludes Estimated and Partial events.

## Alternatives considered

- **A per-source flag only:** too coarse. A single source can be exact for tokens but estimated for duration.

## Consequences

- Schema columns are nullable. Every query and DTO carries accuracy information.
- The UI needs accuracy badges and coverage meters everywhere.

## Revisit when

Never in principle. This is a product promise.
