//! Contract tests of the Cost page and of billing entries and budgets, on the ingested
//! Claude Code fixtures: figures equal the metric engine, parts add up to their totals,
//! and recorded billing never changes the API-equivalent estimate.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

mod support;

use veyra_analytics::metrics::MetricId;
use veyra_app::ApiError;
use veyra_app::dto::{
    BillingEntryInput, BillingKind, BillingRecurrence, BillingRequest, BudgetInput, BudgetLevel,
    Cost, CostAssumptionsRequest, CostRequest, DeleteBillingEntryRequest, DeleteBudgetRequest,
    SaveBillingEntryRequest, SaveBudgetRequest, WhatIfRequest,
};

use support::{api, engine_value, ingested, scope};

const USD: i64 = 1_000_000_000;

#[expect(
    clippy::cast_precision_loss,
    reason = "test comparison of small amounts"
)]
fn as_f64(nano: i64) -> f64 {
    nano as f64
}

fn cost(api: &veyra_app::Api) -> Cost {
    api.get_cost(CostRequest { scope: scope() }).unwrap()
}

fn entry(kind: BillingKind, recurrence: BillingRecurrence, amount: i64) -> BillingEntryInput {
    BillingEntryInput {
        kind,
        label: "Plan".to_owned(),
        provider: Some("anthropic".to_owned()),
        amount_nano: amount,
        currency: "USD".to_owned(),
        recurrence,
        starts_on: "2026-11-01".to_owned(),
        ends_on: None,
    }
}

#[test]
fn cost_figures_equal_the_metric_engine() {
    let data = ingested();
    let page = cost(&api(&data));
    let k = &page.kpis;
    for (kpi, metric) in [
        (&k.api_equivalent_nano_usd, MetricId::ApiEquivalentCost),
        (&k.pricing_coverage, MetricId::PricingCoverage),
        (&k.cost_per_request_nano_usd, MetricId::CostPerRequest),
        (
            &k.cost_per_million_output_nano_usd,
            MetricId::CostPerMillionOutputTokens,
        ),
    ] {
        assert_eq!(kpi.value, engine_value(&data.store, metric), "{metric:?}");
    }
    assert!(
        k.api_equivalent_nano_usd.value.unwrap() > 0.0,
        "fixtures are priced"
    );
}

#[test]
fn days_breakdowns_composition_and_waterfalls_add_up() {
    let data = ingested();
    let page = cost(&api(&data));
    let total = page.kpis.api_equivalent_nano_usd.value.unwrap();
    let daily: i64 = page.daily.iter().map(|d| d.api_equivalent_nano_usd).sum();
    assert_eq!(as_f64(daily), total);
    assert_eq!(
        as_f64(page.daily.last().unwrap().cumulative_nano_usd),
        total
    );

    assert_eq!(page.breakdowns.len(), 4);
    for breakdown in &page.breakdowns {
        let sum: i64 = breakdown
            .rows
            .iter()
            .map(|r| r.measures.cost_nano_usd)
            .sum();
        assert_eq!(as_f64(sum), total, "{:?}", breakdown.dimension);
        let shares: f64 = breakdown.rows.iter().map(|r| r.share).sum();
        assert!((shares - 1.0).abs() < 1e-9, "shares of cost add to one");
    }

    let all = &page.composition[0];
    assert_eq!(all.key, None);
    let parts = all.input_nano_usd
        + all.cache_read_nano_usd
        + all.cache_write_nano_usd
        + all.output_nano_usd;
    assert_eq!(as_f64(parts), total, "token kinds add up to the cost");

    assert_eq!(page.waterfalls.len(), 2);
    for waterfall in &page.waterfalls {
        let steps: i64 = waterfall.steps.iter().map(|s| s.change_nano_usd).sum();
        assert_eq!(
            waterfall.previous_nano_usd + steps,
            waterfall.current_nano_usd
        );
        assert_eq!(as_f64(waterfall.current_nano_usd), total);
    }

    assert!(page.rates.windows(2).all(|w| {
        w[0].cost_per_million_output_nano_usd >= w[1].cost_per_million_output_nano_usd
    }));
    assert!(
        page.sessions
            .windows(2)
            .all(|w| { w[0].measures.cost_nano_usd >= w[1].measures.cost_nano_usd })
    );
}

#[test]
fn the_month_outlook_is_consistent() {
    let data = ingested();
    let month = cost(&api(&data)).month;
    assert_eq!(month.month, "2026-11");
    assert_eq!(month.days.len(), 30);
    assert_eq!(
        month.forecast_nano_usd,
        month.month_to_date_nano_usd + month.run_rate_nano_usd * i64::from(month.days_left)
    );
    let elapsed = 30 - usize::try_from(month.days_left).unwrap();
    if elapsed > 0 {
        assert_eq!(
            month.days[elapsed - 1].cumulative_nano_usd,
            Some(month.month_to_date_nano_usd)
        );
    }
    if let Some(last) = month.days.last().and_then(|d| d.projected_nano_usd) {
        assert_eq!(last, month.forecast_nano_usd);
    }
}

#[test]
fn recorded_billing_stays_apart_from_the_estimate() {
    let data = ingested();
    let api = api(&data);
    let before = cost(&api);
    let monthly = SaveBillingEntryRequest {
        id: None,
        entry: entry(
            BillingKind::Subscription,
            BillingRecurrence::Monthly,
            30 * USD,
        ),
    };
    api.save_billing_entry(monthly).unwrap();
    let charge = BillingEntryInput {
        starts_on: "2026-11-08".to_owned(),
        ..entry(BillingKind::Charge, BillingRecurrence::Once, 5 * USD)
    };
    api.save_billing_entry(SaveBillingEntryRequest {
        id: None,
        entry: charge,
    })
    .unwrap();
    let euros = BillingEntryInput {
        currency: "eur".to_owned(),
        ..entry(BillingKind::Charge, BillingRecurrence::Once, USD)
    };
    api.save_billing_entry(SaveBillingEntryRequest {
        id: None,
        entry: euros,
    })
    .unwrap();

    let after = cost(&api);
    let k = &after.kpis;
    // A thirtieth of November's plan falls on the scope's single day.
    assert_eq!(k.subscription_spend_nano_usd.value, Some(as_f64(USD)));
    assert_eq!(k.subscription_spend_nano_usd.previous, Some(as_f64(USD)));
    assert_eq!(k.actual_charge_nano_usd.value, Some(as_f64(5 * USD)));
    assert_eq!(k.actual_charge_nano_usd.previous, Some(0.0));
    assert_eq!(after.other_currency_entries, 1);
    assert_eq!(
        k.api_equivalent_nano_usd, before.kpis.api_equivalent_nano_usd,
        "billing never changes the estimate"
    );
    let stored = api.get_billing(BillingRequest {}).unwrap();
    assert_eq!(stored.entries.len(), 3);
    let currencies: Vec<&str> = stored
        .entries
        .iter()
        .map(|e| e.entry.currency.as_str())
        .collect();
    // By start date: the plan and the euro charge on the 1st, then the charge on the 8th.
    assert_eq!(currencies, ["USD", "EUR", "USD"]);
}

#[test]
fn budgets_report_their_month_and_level() {
    let data = ingested();
    let api = api(&data);
    let page = cost(&api);
    let tiny = api
        .save_budget(SaveBudgetRequest {
            id: None,
            budget: BudgetInput {
                label: "Tiny".to_owned(),
                limit_nano_usd: 1,
                warn_percent: 80,
                project_id: None,
            },
        })
        .unwrap();
    let project = api
        .list_projects(veyra_app::dto::ProjectsRequest { scope: scope() })
        .unwrap()
        .rows[0]
        .project
        .id;
    api.save_budget(SaveBudgetRequest {
        id: None,
        budget: BudgetInput {
            label: "Project".to_owned(),
            limit_nano_usd: 1_000 * USD,
            warn_percent: 80,
            project_id: Some(project),
        },
    })
    .unwrap();

    let budgets = cost(&api).budgets;
    assert_eq!(budgets.len(), 2);
    let overall = budgets.iter().find(|b| b.id == tiny.id).unwrap();
    assert_eq!(
        overall.month_to_date_nano_usd,
        page.month.month_to_date_nano_usd
    );
    assert_eq!(overall.forecast_nano_usd, page.month.forecast_nano_usd);
    if overall.month_to_date_nano_usd > 0 {
        assert_eq!(overall.level, BudgetLevel::Over);
    }
    let scoped = budgets.iter().find(|b| b.id != tiny.id).unwrap();
    assert_eq!(scoped.project.as_ref().unwrap().key, project.to_string());
    assert_eq!(scoped.level, BudgetLevel::Within);

    api.delete_budget(DeleteBudgetRequest { id: tiny.id })
        .unwrap();
    assert_eq!(api.get_billing(BillingRequest {}).unwrap().budgets.len(), 1);
}

#[test]
fn invalid_and_unknown_records_are_refused() {
    let data = ingested();
    let api = api(&data);
    let invalid = |entry| {
        api.save_billing_entry(SaveBillingEntryRequest { id: None, entry })
            .unwrap_err()
    };
    let bad_date = BillingEntryInput {
        starts_on: "2026-02-30".to_owned(),
        ..entry(BillingKind::Charge, BillingRecurrence::Once, USD)
    };
    assert!(matches!(invalid(bad_date), ApiError::Validation { .. }));
    let zero = entry(BillingKind::Charge, BillingRecurrence::Once, 0);
    assert!(matches!(invalid(zero), ApiError::Validation { .. }));
    let unknown = api
        .delete_billing_entry(DeleteBillingEntryRequest { id: 42 })
        .unwrap_err();
    assert!(matches!(unknown, ApiError::NotFound { .. }));
    let replaced = api
        .save_billing_entry(SaveBillingEntryRequest {
            id: Some(42),
            entry: entry(BillingKind::Charge, BillingRecurrence::Once, USD),
        })
        .unwrap_err();
    assert!(matches!(replaced, ApiError::NotFound { .. }));
    let no_project = api
        .save_budget(SaveBudgetRequest {
            id: None,
            budget: BudgetInput {
                label: "Missing".to_owned(),
                limit_nano_usd: USD,
                warn_percent: 80,
                project_id: Some(999),
            },
        })
        .unwrap_err();
    assert!(matches!(no_project, ApiError::NotFound { .. }));
}

#[test]
fn cost_assumptions_add_up_to_the_estimate() {
    let data = ingested();
    let api = api(&data);
    let panel = api
        .get_cost_assumptions(CostAssumptionsRequest { scope: scope() })
        .unwrap();
    let estimate = engine_value(&data.store, MetricId::ApiEquivalentCost).unwrap();
    // Every event has exactly one outcome, so the outcomes add up to the estimate.
    let by_status: i64 = panel
        .statuses
        .iter()
        .map(|s| s.tally.api_equivalent_nano_usd)
        .sum();
    assert!((as_f64(by_status) - estimate).abs() < 1.0);
    // Every priced event has exactly one rule, so the rules add up to the same.
    let by_rule: i64 = panel
        .rules
        .iter()
        .map(|r| r.tally.api_equivalent_nano_usd)
        .sum();
    assert_eq!(by_rule, by_status);
    assert_ne!(panel.rules.len(), 0, "the fixtures use priced models");
    for rule in &panel.rules {
        assert!(rule.source_url.starts_with("https://"), "{}", rule.id);
        assert_eq!(rule.verified_at.len(), "YYYY-MM-DD".len());
        assert!(rule.base.output > 0);
    }
    assert!(
        panel
            .unpriced
            .iter()
            .all(|m| m.tally.api_equivalent_nano_usd == 0)
    );
    assert_eq!(panel.stale_after_days, 120);
    assert_ne!(panel.catalog_version, "");
}

#[test]
fn the_what_if_card_reprices_the_same_tokens_on_another_model() {
    let data = ingested();
    let api = api(&data);
    let request = |model: Option<String>| WhatIfRequest {
        scope: scope(),
        model,
    };
    let choices = api.get_what_if(request(None)).unwrap();
    assert!(choices.comparison.is_none());
    assert_ne!(choices.choices.len(), 0, "the catalogue prices models");

    let estimate = engine_value(&data.store, MetricId::ApiEquivalentCost).unwrap();
    for choice in &choices.choices {
        let page = api
            .get_what_if(request(Some(choice.model.key.clone())))
            .unwrap();
        let comparison = page.comparison.unwrap();
        assert_eq!(comparison.target, *choice);
        // The rows add up to the totals on both sides.
        let rows = &comparison.rows;
        let current: i64 = rows.iter().map(|r| r.current_nano_usd).sum();
        let repriced: i64 = rows.iter().map(|r| r.repriced_nano_usd).sum();
        assert_eq!(current, comparison.current_nano_usd);
        assert_eq!(repriced, comparison.repriced_nano_usd);
        // Only models whose every request was priced are compared, so the recorded side
        // never exceeds the scope's estimate.
        assert!(as_f64(comparison.current_nano_usd) <= estimate + 1.0);
        assert!(comparison.tokens_compared > 0);
    }

    let unknown = api
        .get_what_if(request(Some("no-such-model".to_owned())))
        .unwrap_err();
    assert!(matches!(unknown, ApiError::Validation { .. }));
}
