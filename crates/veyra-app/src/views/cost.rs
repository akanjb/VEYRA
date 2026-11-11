//! The Cost page: API-equivalent cost and its trend, month outlook and budgets,
//! breakdowns, composition, effective rates and the period-change waterfall, beside the
//! subscription spend and charges the user recorded. The three cost kinds are separate
//! fields and are never added together (ADR-004). Every figure comes from the analytics
//! crate (ADR-008).

use jiff::ToSpan;
use jiff::civil::Date;
use veyra_analytics::billing::{
    CHARGES_DEFINITION, DailyBilling, SUBSCRIPTION_DEFINITION, daily_billing,
};
use veyra_analytics::change::{Contributions, WATERFALL_DEFINITION, contributions};
use veyra_analytics::dimension::{Dimension, DimensionValue, Filters};
use veyra_analytics::engine::GroupResult;
use veyra_analytics::forecast::{
    BUDGET_DEFINITION, BudgetState, FORECAST_DEFINITION, RUN_RATE_DAYS, RUN_RATE_DEFINITION,
    budget_state, cumulative, forecast, projection, run_rate,
};
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::{Period, QuerySpec, TimeGrain};
use veyra_analytics::sums::BaseSums;
use veyra_domain::billing::BillingEntryDraft;
use veyra_domain::money::NanoUsd;
use veyra_domain::project::ProjectRecord;
use veyra_domain::time::UtcMillis;

use super::kpis::{kpi, metric_kpi, token_accuracy};
use super::rankings::{Labeller, RankBy, RowLabel, dimension_ranking, groups_of};
use super::{evaluated, internal, local_time, measures};
use crate::DataServices;
use crate::dto::{
    AccuracyClass, BreakdownDimension, BudgetLevel, BudgetStatus, Cost, CostBreakdown, CostDay,
    CostDefinitions, CostKpis, CostMix, Labelled, MonthOutlook, OutlookDay, RateRow, SessionSort,
    Waterfall, WaterfallStep,
};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Rows of each breakdown before "Other".
const TOP_ROWS: usize = 10;
/// Models shown in the composition, after the all-models row.
const COMPOSITION_MODELS: usize = 8;
/// Steps of a waterfall before "Other".
const WATERFALL_STEPS: usize = 8;
/// Most expensive sessions listed.
const SESSIONS_SHOWN: u32 = 10;

/// The breakdowns, in the order the page shows them.
const BREAKDOWNS: [(BreakdownDimension, Dimension); 4] = [
    (BreakdownDimension::Model, Dimension::Model),
    (BreakdownDimension::Provider, Dimension::Provider),
    (BreakdownDimension::App, Dimension::App),
    (BreakdownDimension::Project, Dimension::Project),
];

/// The waterfalls, in the order the page shows them.
const WATERFALLS: [(BreakdownDimension, Dimension); 2] = [
    (BreakdownDimension::Model, Dimension::Model),
    (BreakdownDimension::Project, Dimension::Project),
];

const fn money(amount: NanoUsd) -> MetricValue {
    MetricValue::Money(amount)
}

/// The number of local days from `first` to `last`, inclusive.
fn day_count(first: Date, last: Date) -> Result<i32, ApiError> {
    Ok(first.until(last).map_err(|e| internal(&e))?.get_days() + 1)
}

fn days_after(date: Date, days: i32) -> Result<Date, ApiError> {
    date.checked_add(days.days()).map_err(|e| internal(&e))
}

/// API-equivalent cost per local day over `days` days from `first`.
fn daily_cost(
    data: &DataServices,
    scope: &Resolved,
    filters: &Filters,
    first: Date,
    days: i32,
) -> Result<Vec<NanoUsd>, ApiError> {
    let period = Period::local_days(first, days, &scope.time_zone).map_err(|e| internal(&e))?;
    let result = evaluated(
        data,
        &QuerySpec {
            period,
            comparison: None,
            filters: filters.clone(),
            group_by: vec![],
            grain: TimeGrain::Day,
            time_zone: scope.time_zone.clone(),
        },
    )?;
    Ok(result.series.iter().map(|p| p.sums.cost.total).collect())
}

/// Billing per day over the scope's period and, when compared, the previous one.
struct Billing {
    current: DailyBilling,
    previous: Option<DailyBilling>,
}

impl Billing {
    fn of(entries: &[BillingEntryDraft], scope: &Resolved, days: i32) -> Result<Self, ApiError> {
        let first = days_after(scope.end, 1 - days)?;
        let length = usize::try_from(days).unwrap_or(0);
        let previous = scope
            .previous
            .map(|_| days_after(first, -days).map(|start| daily_billing(entries, start, length)))
            .transpose()?;
        Ok(Self {
            current: daily_billing(entries, first, length),
            previous,
        })
    }
}

fn kpis(
    current: &BaseSums,
    previous: Option<&BaseSums>,
    accuracy: Option<AccuracyClass>,
    billing: &Billing,
) -> CostKpis {
    let estimated = Some(AccuracyClass::Estimated);
    let before = billing.previous.as_ref();
    CostKpis {
        api_equivalent_nano_usd: metric_kpi(
            MetricId::ApiEquivalentCost,
            current,
            previous,
            estimated,
        ),
        pricing_coverage: metric_kpi(MetricId::PricingCoverage, current, previous, accuracy),
        actual_charge_nano_usd: kpi(
            Some(money(billing.current.charge_total().amount())),
            before.map(|b| money(b.charge_total().amount())),
            None,
            CHARGES_DEFINITION,
        ),
        subscription_spend_nano_usd: kpi(
            Some(money(billing.current.subscription_total().amount())),
            before.map(|b| money(b.subscription_total().amount())),
            None,
            SUBSCRIPTION_DEFINITION,
        ),
        cost_per_request_nano_usd: metric_kpi(
            MetricId::CostPerRequest,
            current,
            previous,
            estimated,
        ),
        cost_per_million_output_nano_usd: metric_kpi(
            MetricId::CostPerMillionOutputTokens,
            current,
            previous,
            estimated,
        ),
    }
}

/// Each day's cost and running total beside the previous period's running total.
fn daily(
    data: &DataServices,
    scope: &Resolved,
    days: i32,
    current: &[NanoUsd],
) -> Result<Vec<CostDay>, ApiError> {
    let first = days_after(scope.end, 1 - days)?;
    let before = daily_cost(data, scope, &scope.filters, days_after(first, -days)?, days)?;
    let running = cumulative(current);
    let previous_running = cumulative(&before);
    (0..days)
        .zip(current.iter().zip(running.iter().zip(&previous_running)))
        .map(|(offset, (cost, (total, previous)))| {
            Ok(CostDay {
                date: days_after(first, offset)?.to_string(),
                api_equivalent_nano_usd: cost.0,
                cumulative_nano_usd: total.0,
                previous_cumulative_nano_usd: previous.0,
            })
        })
        .collect()
}

/// The month being looked at and how much of it is past.
struct MonthFrame {
    first: Date,
    days: i32,
    /// Days of the month counted as month to date (1 to `days`).
    elapsed: i32,
    /// The first day not yet complete: today, or the day after the month.
    complete_until: Date,
}

impl MonthFrame {
    /// The month containing the scope's last day, as of `today`.
    fn of(scope: &Resolved, today: Date) -> Result<Self, ApiError> {
        let first = scope.end.first_of_month();
        let last = scope.end.last_of_month();
        let days = day_count(first, last)?;
        let elapsed = if today < first {
            0
        } else if today > last {
            days
        } else {
            day_count(first, today)?
        };
        Ok(Self {
            first,
            days,
            elapsed,
            complete_until: today.min(days_after(last, 1)?),
        })
    }

    fn days_left(&self) -> u32 {
        u32::try_from(self.days - self.elapsed).unwrap_or(0)
    }
}

/// Month-to-date cost, daily cost of the month and the run rate, for some filters.
struct MonthCost {
    days: Vec<NanoUsd>,
    month_to_date: NanoUsd,
    run_rate: NanoUsd,
}

impl MonthCost {
    fn of(
        data: &DataServices,
        scope: &Resolved,
        frame: &MonthFrame,
        filters: &Filters,
    ) -> Result<Self, ApiError> {
        let days = daily_cost(data, scope, filters, frame.first, frame.days)?;
        let elapsed = usize::try_from(frame.elapsed).unwrap_or(0);
        let month_to_date = days[..elapsed.min(days.len())].iter().copied().sum();
        let window = i32::try_from(RUN_RATE_DAYS).unwrap_or(i32::MAX);
        let recent = daily_cost(
            data,
            scope,
            filters,
            days_after(frame.complete_until, -window)?,
            window,
        )?;
        Ok(Self {
            days,
            month_to_date,
            run_rate: run_rate(&recent),
        })
    }

    fn forecast(&self, frame: &MonthFrame) -> NanoUsd {
        forecast(self.month_to_date, self.run_rate, frame.days_left())
    }
}

fn outlook(frame: &MonthFrame, cost: &MonthCost) -> Result<MonthOutlook, ApiError> {
    let elapsed = usize::try_from(frame.elapsed).unwrap_or(0);
    let running = cumulative(&cost.days);
    let projected = projection(cost.month_to_date, cost.run_rate, frame.days_left());
    let days = (0..frame.days)
        .zip(0_usize..)
        .map(|(offset, index)| {
            // The day `asOf` carries both values, so the projection joins the actual line.
            let actual = running.get(index).filter(|_| index < elapsed).map(|v| v.0);
            let projection = match index.checked_sub(elapsed) {
                Some(after) => projected.get(after).map(|p| p.0),
                None if index + 1 == elapsed => Some(cost.month_to_date.0),
                None => None,
            };
            Ok(OutlookDay {
                date: days_after(frame.first, offset)?.to_string(),
                cumulative_nano_usd: actual,
                projected_nano_usd: projection,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let as_of = days_after(frame.first, (frame.elapsed - 1).max(0))?;
    Ok(MonthOutlook {
        month: frame.first.strftime("%Y-%m").to_string(),
        as_of: as_of.to_string(),
        days,
        month_to_date_nano_usd: cost.month_to_date.0,
        run_rate_nano_usd: cost.run_rate.0,
        forecast_nano_usd: cost.forecast(frame).0,
        days_left: frame.days_left(),
    })
}

const fn level(state: BudgetState) -> BudgetLevel {
    match state {
        BudgetState::Within => BudgetLevel::Within,
        BudgetState::Warning => BudgetLevel::Warning,
        BudgetState::Over => BudgetLevel::Over,
    }
}

/// The project a budget follows: the one its project is merged into, if any.
fn budget_project(project_id: i64, projects: &[ProjectRecord]) -> Option<&ProjectRecord> {
    let record = projects.iter().find(|p| p.id == project_id)?;
    let effective = record.merged_into.unwrap_or(record.id);
    projects.iter().find(|p| p.id == effective)
}

/// Every budget this month. Page filters do not apply to budgets.
fn budgets(
    data: &DataServices,
    scope: &Resolved,
    frame: &MonthFrame,
) -> Result<Vec<BudgetStatus>, ApiError> {
    let stored = data.billing.budgets().map_err(|e| internal(&e))?;
    let projects = data.projects.list_projects().map_err(|e| internal(&e))?;
    stored
        .into_iter()
        .map(|stored| {
            let budget = stored.budget;
            let project = budget
                .project_id
                .and_then(|id| budget_project(id, &projects));
            let filters = project.map_or_else(Filters::new, |p| {
                Filters::new().only(Dimension::Project, [Some(p.id.to_string())])
            });
            let cost = MonthCost::of(data, scope, frame, &filters)?;
            let limit = NanoUsd(budget.limit_nano_usd);
            let forecast = cost.forecast(frame);
            Ok(BudgetStatus {
                id: stored.id,
                label: budget.label,
                project: project.map(|p| Labelled {
                    key: p.id.to_string(),
                    label: p.display_name.clone().unwrap_or_else(|| p.path.clone()),
                }),
                limit_nano_usd: limit.0,
                warn_percent: budget.warn_percent,
                month_to_date_nano_usd: cost.month_to_date.0,
                forecast_nano_usd: forecast.0,
                level: level(budget_state(cost.month_to_date, limit, budget.warn_percent)),
                forecast_level: level(budget_state(forecast, limit, budget.warn_percent)),
            })
        })
        .collect()
}

/// Groups of one dimension, labelled, in the current and the previous period.
struct Grouped {
    dimension: BreakdownDimension,
    labeller: Labeller,
    current: Vec<GroupResult>,
}

impl Grouped {
    fn of(
        data: &DataServices,
        scope: &Resolved,
        (dimension, grouped): (BreakdownDimension, Dimension),
    ) -> Result<Self, ApiError> {
        let labeller = Labeller::new(data, grouped)?;
        let current = groups_of(data, scope, &labeller)?;
        Ok(Self {
            dimension,
            labeller,
            current,
        })
    }

    fn breakdown(&self, data: &DataServices, scope_cost: u64) -> CostBreakdown {
        CostBreakdown {
            dimension: self.dimension,
            rows: dimension_ranking(
                data,
                self.current.clone(),
                &self.labeller,
                TOP_ROWS,
                RankBy::Cost,
                scope_cost,
            ),
        }
    }

    /// The change in cost from the previous period, by group.
    fn waterfall(&self, data: &DataServices, scope: &Resolved) -> Result<Waterfall, ApiError> {
        let mut spec = scope.spec(self.labeller.group_by(), TimeGrain::Total, false);
        spec.period = scope.period.previous();
        let previous = evaluated(data, &spec)?.groups;
        let value = |g: GroupResult| (g.key, g.totals.cost.total.0);
        let change: Contributions<Vec<DimensionValue>> = contributions(
            self.current.iter().cloned().map(value),
            previous.into_iter().map(value),
            WATERFALL_STEPS,
        );
        let mut steps: Vec<WaterfallStep> = change
            .steps
            .iter()
            .map(|step| {
                let RowLabel {
                    key,
                    label,
                    provider,
                } = self.labeller.label_key(data, &step.key);
                WaterfallStep {
                    key,
                    label,
                    provider,
                    change_nano_usd: step.change(),
                    group_count: 1,
                    is_other: false,
                }
            })
            .collect();
        if let Some((count, other)) = change.other {
            steps.push(WaterfallStep {
                key: None,
                label: "Other".to_owned(),
                provider: None,
                change_nano_usd: other.change(),
                group_count: u64::try_from(count).unwrap_or(u64::MAX),
                is_other: true,
            });
        }
        Ok(Waterfall {
            dimension: self.dimension,
            previous_nano_usd: change.previous_total,
            current_nano_usd: change.current_total,
            steps,
        })
    }
}

fn mix(label: RowLabel, sums: &BaseSums) -> CostMix {
    let cost = &sums.cost;
    CostMix {
        key: label.key,
        label: label.label,
        provider: label.provider,
        input_nano_usd: cost.input.0,
        cache_read_nano_usd: cost.cache_read.0,
        cache_write_nano_usd: cost.cache_write.0,
        output_nano_usd: cost.output.0,
    }
}

/// Cost by token kind for all models, then the most expensive models.
fn composition(data: &DataServices, models: &Grouped, totals: &BaseSums) -> Vec<CostMix> {
    let all = RowLabel {
        key: None,
        label: "All models".to_owned(),
        provider: None,
    };
    let mut ranked = models.current.clone();
    ranked.sort_by_key(|g| std::cmp::Reverse(g.totals.cost.total));
    std::iter::once(mix(all, totals))
        .chain(
            ranked
                .iter()
                .filter(|g| g.totals.cost.total.0 > 0)
                .take(COMPOSITION_MODELS)
                .map(|g| mix(models.labeller.label(data, g), &g.totals)),
        )
        .collect()
}

/// Each priced model's effective cost per million output tokens, highest first.
fn rates(data: &DataServices, models: &Grouped) -> Vec<RateRow> {
    let mut rows: Vec<RateRow> = models
        .current
        .iter()
        .filter_map(|group| {
            let rate = match MetricId::CostPerMillionOutputTokens.compute(&group.totals)? {
                MetricValue::Money(m) if m.0 > 0 => m.0,
                _ => return None,
            };
            let label = models.labeller.label(data, group);
            Some(RateRow {
                key: label.key,
                label: label.label,
                provider: label.provider,
                cost_per_million_output_nano_usd: rate,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.cost_per_million_output_nano_usd
            .cmp(&a.cost_per_million_output_nano_usd)
            .then_with(|| a.label.cmp(&b.label))
    });
    rows
}

fn definitions() -> CostDefinitions {
    CostDefinitions {
        run_rate: RUN_RATE_DEFINITION.to_owned(),
        forecast: FORECAST_DEFINITION.to_owned(),
        budget: BUDGET_DEFINITION.to_owned(),
        waterfall: WATERFALL_DEFINITION.to_owned(),
        subscriptions: SUBSCRIPTION_DEFINITION.to_owned(),
        charges: CHARGES_DEFINITION.to_owned(),
    }
}

/// Builds the Cost page for a scope, with `now` deciding which days are complete.
pub(crate) fn cost(
    data: &DataServices,
    scope: &Resolved,
    now: UtcMillis,
) -> Result<Cost, ApiError> {
    let result = evaluated(data, &scope.spec(vec![], TimeGrain::Day, true))?;
    let totals = &result.totals;
    let days = i32::try_from(result.series.len()).unwrap_or(0);
    let current: Vec<NanoUsd> = result.series.iter().map(|p| p.sums.cost.total).collect();
    let entries: Vec<BillingEntryDraft> = data
        .billing
        .billing_entries()
        .map_err(|e| internal(&e))?
        .into_iter()
        .map(|e| e.entry)
        .collect();
    let billing = Billing::of(&entries, scope, days)?;
    let today = local_time(now, &scope.time_zone)?.date();
    let frame = MonthFrame::of(scope, today)?;
    let month = MonthCost::of(data, scope, &frame, &scope.filters)?;
    let grouped = BREAKDOWNS
        .into_iter()
        .map(|pair| Grouped::of(data, scope, pair))
        .collect::<Result<Vec<_>, _>>()?;
    let scope_cost = RankBy::Cost.amount(&measures(totals));
    let models = &grouped[0];
    let waterfalls = WATERFALLS
        .iter()
        .filter_map(|(shown, _)| grouped.iter().find(|g| g.dimension == *shown))
        .map(|g| g.waterfall(data, scope))
        .collect::<Result<Vec<_>, _>>()?;
    let sessions = super::sessions::list(data, scope, SessionSort::Cost, 0, SESSIONS_SHOWN)?;
    Ok(Cost {
        kpis: kpis(
            totals,
            result.previous_totals.as_ref(),
            token_accuracy(totals),
            &billing,
        ),
        accuracy: token_accuracy(totals),
        other_currency_entries: u32::try_from(billing.current.other_currency_entries)
            .unwrap_or(u32::MAX),
        daily: daily(data, scope, days, &current)?,
        month: outlook(&frame, &month)?,
        budgets: budgets(data, scope, &frame)?,
        breakdowns: grouped
            .iter()
            .map(|g| g.breakdown(data, scope_cost))
            .collect(),
        sessions: sessions.rows,
        composition: composition(data, models, totals),
        rates: rates(data, models),
        waterfalls,
        definitions: definitions(),
    })
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::{MonthFrame, budget_project, day_count};
    use crate::scope::resolve;
    use veyra_domain::project::ProjectRecord;

    fn scope(end: &str) -> crate::scope::Resolved {
        resolve(&crate::dto::Scope {
            time_zone: "UTC".to_owned(),
            start: "2026-11-01".to_owned(),
            end: end.to_owned(),
            comparison: crate::dto::Comparison::None,
            filters: crate::dto::ScopeFilters::default(),
        })
        .unwrap()
    }

    #[test]
    fn the_month_frame_counts_days_up_to_today() {
        let frame = MonthFrame::of(&scope("2026-11-20"), date(2026, 11, 12)).unwrap();
        assert_eq!((frame.days, frame.elapsed, frame.days_left()), (30, 12, 18));
        assert_eq!(frame.complete_until, date(2026, 11, 12));
    }

    #[test]
    fn a_past_month_is_complete() {
        let frame = MonthFrame::of(&scope("2026-11-20"), date(2027, 1, 5)).unwrap();
        assert_eq!((frame.elapsed, frame.days_left()), (30, 0));
        assert_eq!(frame.complete_until, date(2026, 12, 1));
    }

    #[test]
    fn a_future_month_has_nothing_to_date() {
        let frame = MonthFrame::of(&scope("2026-11-20"), date(2026, 10, 30)).unwrap();
        assert_eq!((frame.elapsed, frame.days_left()), (0, 30));
    }

    #[test]
    fn a_budget_follows_its_project_into_a_merge() {
        let record = |id, merged_into| ProjectRecord {
            id,
            path: format!("/p/{id}"),
            display_name: None,
            merged_into,
        };
        let projects = [record(1, None), record(2, Some(1))];
        assert_eq!(budget_project(2, &projects).map(|p| p.id), Some(1));
        assert_eq!(budget_project(1, &projects).map(|p| p.id), Some(1));
        assert!(budget_project(9, &projects).is_none());
        assert_eq!(day_count(date(2026, 2, 1), date(2026, 2, 28)).unwrap(), 28);
    }
}
