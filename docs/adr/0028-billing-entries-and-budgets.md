# 0028. Billing entries, budgets and the month forecast

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

The Cost page shows three cost kinds that are never added together (ADR-004, AC-5): the API-equivalent estimate, actual charges and subscription spend. Until billing connectors exist (M9), the only source of actual charges and subscription spend is the user. FR-PRICE-08 asks for manual billing entries with recurrence and currency, and FR-PRICE-09 for budgets with thresholds, a run rate and a forecast labelled as a projection.

A period on the dashboard can be any range of local days, while subscriptions are paid per month or per year. A monthly plan therefore has to be shown for a week, a quarter or a single day in a way that adds up.

## Decision

1. **Entries are records the user owns.** A billing entry has a kind, a label, an optional provider, an amount in integer billionths of its currency, an ISO 4217 currency, a recurrence (once, monthly or yearly), a start date and an optional inclusive end date. A subscription is subscription spend and a charge is an actual charge. Entries live in `billing_entries`. Every rule is checked in the domain (`BillingEntryDraft::validated`) and again by `CHECK` constraints.

2. **Recurring entries accrue by day.** A one-off entry counts in full on its date. A recurring entry splits each payment evenly over the days that payment covers. A cycle starts on the start date's day of the month (or the month's last day when the month is shorter), and the remainder of the integer division goes to the cycle's first days. One cycle of days therefore adds up to exactly one payment, and any split of a range adds up to the whole. The end date stops accrual on that day.

3. **Only US dollars are totalled.** Entries in other currencies are stored and listed, and the page says how many were left out. No exchange rates are fetched or assumed.

4. **Kinds stay separate in every layer.** The analytics crate returns subscription spend and actual charges as their own domain types (`SubscriptionSpend`, `ActualCharge`), which cannot be added to `ApiEquivalentCost`. The Cost contract has one named field per kind. A test over every generated contract and every dashboard source file fails if a new field or line mixes them. The value framing ("subscriptions $X; the same usage would cost $Y at API rates") is a comparison, never a total.

5. **Budgets limit API-equivalent cost per calendar month.** A budget has a label, a limit in nano-USD, a warning share (default 80%) and optionally a project. It covers all usage or that project, whatever the page's filters. A budget whose project was later merged follows the project it was merged into (ADR-027). Its state is `within`, `warning` (month to date at or above the warning share) or `over` (at or above the limit), for month to date and for the forecast.

6. **The run rate and the forecast follow the PRD.** The run rate is the cost of the last 7 complete local days divided by 7, with days without usage counted as zero. Today is not complete. The forecast is month to date plus the run rate for each day left in the month. The month is the one containing the period's last day. For a past month, month to date is the whole month and nothing is projected. Both are labelled as projections of an estimate.

7. **The period-change waterfall uses the previous period of the same length.** Each group's change is its cost now minus its cost then, including groups present in only one of the periods. The largest changes by size are shown and the rest are merged into "Other", so the steps always lead from the previous total to the current one.

## Alternatives considered

- **Count a subscription in full on each renewal date.** This matches cash flow, but a week without a renewal date would show no subscription at all, and comparisons with daily API-equivalent cost would swing.
- **Convert currencies.** This needs a rate source (a network call, against NFR-02) or user-maintained rates. Deferred until users ask for it.
- **Budgets over any cost kind or with the page's filters.** An actual-charge budget needs billing connectors, and a budget that changes with the filters is not a budget. Kept to API-equivalent cost over all usage or one project.
- **A rolling 30-day budget window.** Calendar months match how plans and invoices are billed.

## Consequences

- The Cost page shows subscriptions and charges as soon as the user records them in Settings, without any network access.
- The month outlook and every budget each read one month of daily sums and one week for the run rate. Both reads use the rollups.
- Deleting an entry or a budget is permanent, and it changes only what the user recorded, never ingested usage.

## Revisit when

Billing connectors (M9) report actual charges, users need other currencies totalled, or budgets are needed for other periods or other cost kinds.
