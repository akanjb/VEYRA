# 0036. Pricing a period on another model

- **Status:** Accepted
- **Date:** 2026-11-19

## Context

People choosing between models want to know what their own usage would have cost on a different one. The Cost page shows what each model cost and an effective rate per million output tokens, but comparing rates by eye ignores how much of the usage is input, cache reads and cache writes, which differ by model.

Veyra already holds the token counts by kind for every model and the published price of every model (ADR-007). It needs no new data to answer the question.

## Decision

1. **One command, `get_what_if`.** It takes the page scope and an optional model. It always returns the models with a price in effect now. When a model is given, it also returns the period priced on it. The card loads with the Cost page but costs only one grouped read of the period, which the page already makes for its breakdowns.
2. **Same tokens on both sides.** For each model used in the period, its summed tokens of each kind are priced at the other model's current base rates, beside the API-equivalent cost already recorded for them. A model with any request that has no published price is left out of both sides, and the tokens left out are shown, so the two figures always cover the same tokens.
3. **One formula.** `price_tokens` in the domain prices summed token counts with the same component arithmetic as `price_event`, at the standard service tier. Summed counts carry no per-request prompt size, so prompt-length tiers cannot apply; that, and any token kind the other model has no rate for, is recorded as an assumption and listed on the card.
4. **Change computed in Rust.** The relative change per model and in total uses the metric engine's `delta` (ADR-008); the dashboard only words it.

## Consequences

- The comparison is an estimate of price, not of behaviour: a different model may need a different number of tokens for the same work. The card's definition says so.
- Batch, priority and regional multipliers recorded on the original requests are not carried over to the other model, so a heavy batch user sees the other model at standard prices.
- No migration and no new stored data.
