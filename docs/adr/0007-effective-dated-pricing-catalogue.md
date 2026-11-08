# 0007. Effective-dated, version-controlled pricing catalogue

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Prices change over time (announced changes on 2026-12-01 and 2027-01-01, for example), vary by tier and modifier, and no vendor publishes a machine-readable price feed. Pricing old events at today's rates is wrong.

## Decision

- **The catalogue.** Prices live in `registry/pricing/*.toml`, reviewed by PR. Each rule carries:
  - provider and model pattern;
  - effective-from and effective-to dates;
  - rates per component, plus tiers and modifiers;
  - currency;
  - `source_url` and `verified_at`.
- **Pricing an event.** Each event is priced with the rule in effect at its timestamp.
- **Stored with provenance.** Costs are stored per event with the catalogue version used. A catalogue change triggers re-pricing of the affected events.
- **Updates.** An opt-in update downloads a signed catalogue from GitHub Releases.

## Alternatives considered

- **LiteLLM or models.dev price feeds:** unofficial, fuzzy-matched, and with no history.
- **Hard-coded tables in code:** they drift and have no provenance.

## Consequences

- Maintainers verify rows against official pages: a checklist workflow, at no cost.
- Unknown models are shown as "no price", never $0. Stale rates are flagged.

## Revisit when

A vendor publishes an official machine-readable price feed.
