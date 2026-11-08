# 0004. Integer money and three non-addable cost kinds

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Floating-point money drifts. Users also confuse API-equivalent estimates with what they actually pay, especially on flat-rate subscriptions.

## Decision

- **Integer units.** Money is stored as integer **nano-USD** (i64). Rates are integer **micro-USD per 1M tokens**. Arithmetic uses i128 with half-even rounding per component.
- **Distinct types.** `ApiEquivalentCost`, `ActualCharge` and `SubscriptionSpend` are distinct Rust types with no `Add` implementation between them. Analytics DTOs expose them in separate fields.

## Alternatives considered

- **f64 dollars:** rounding drift and non-reproducible totals.
- **A single `cost` field plus a flag:** too easy to sum by mistake.

## Consequences

- Summing different cost kinds is a compile error (tested with a compile-fail test in M1).
- Non-USD currencies need per-currency handling (separate columns or a currency-tagged type).

## Revisit when

A provider bills in a currency or unit that needs a different representation.
