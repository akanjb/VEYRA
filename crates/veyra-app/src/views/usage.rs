//! The Usage page: how many tokens of which kinds, through what, and how large requests
//! and sessions are. Every number comes from the analytics engine (ADR-008).

use veyra_analytics::concentration::{HERFINDAHL_DEFINITION, cumulative_shares, herfindahl};
use veyra_analytics::dimension::Dimension;
use veyra_analytics::engine::QueryResult;
use veyra_analytics::metrics::{MetricId, MetricValue};
use veyra_analytics::query::TimeGrain;
use veyra_analytics::sizes::{
    REQUEST_PERCENTILES_DEFINITION, SESSION_SIZES_DEFINITION, SizeHistogram, SizeSummary,
    bin_lower, bin_upper, histograms_by_period, sessions_by_period,
};
use veyra_analytics::sums::BaseSums;
use veyra_domain::time::UtcMillis;

use super::kpis::{metric_kpi, token_accuracy};
use super::rankings::{Labeller, RankBy, dimension_ranking, groups_of};
use super::{evaluated, internal, local_date, measures};
use crate::DataServices;
use crate::dto::{
    AccuracyClass, Breakdown, BreakdownDimension, SizeBinCount, SizePoint, TokenMixPoint, Usage,
    UsageKpis,
};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Rows of each breakdown before "Other".
const TOP_ROWS: usize = 10;

/// The breakdowns, in the order the page shows them.
const BREAKDOWNS: [(BreakdownDimension, Dimension); 6] = [
    (BreakdownDimension::Model, Dimension::Model),
    (BreakdownDimension::Provider, Dimension::Provider),
    (BreakdownDimension::App, Dimension::App),
    (BreakdownDimension::Interface, Dimension::Interface),
    (BreakdownDimension::Project, Dimension::Project),
    (BreakdownDimension::Device, Dimension::Device),
];

fn kpis(
    current: &BaseSums,
    previous: Option<&BaseSums>,
    accuracy: Option<AccuracyClass>,
) -> UsageKpis {
    let tile = |metric| metric_kpi(metric, current, previous, accuracy);
    UsageKpis {
        tokens: tile(MetricId::TotalTokens),
        input_tokens: tile(MetricId::InputTokens),
        output_tokens: tile(MetricId::OutputTokens),
        reasoning_tokens: tile(MetricId::ReasoningTokens),
        requests: tile(MetricId::Requests),
        // A ratio of two measures: its accuracy is that of its parts, shown on them.
        tokens_per_request: metric_kpi(MetricId::TokensPerRequest, current, previous, None),
    }
}

fn mix_point(date: String, sums: &BaseSums) -> TokenMixPoint {
    let t = &sums.tokens;
    TokenMixPoint {
        date,
        input_uncached: t.input_uncached.unwrap_or(0),
        cache_read: t.cache_read.unwrap_or(0),
        cache_write: t.cache_write_total().unwrap_or(0),
        output: t.output_total.unwrap_or(0),
        reasoning: t.reasoning,
    }
}

fn breakdown(
    data: &DataServices,
    scope: &Resolved,
    dimension: BreakdownDimension,
    grouped: Dimension,
    scope_tokens: u64,
) -> Result<Breakdown, ApiError> {
    let labeller = Labeller::new(data, grouped)?;
    let groups = groups_of(data, scope, &labeller)?;
    let tokens: Vec<u64> = groups.iter().map(|g| measures(&g.totals).tokens).collect();
    let rows = dimension_ranking(
        data,
        groups,
        &labeller,
        TOP_ROWS,
        RankBy::Tokens,
        scope_tokens,
    );
    let ranked: Vec<u64> = rows.iter().map(|r| r.measures.tokens).collect();
    Ok(Breakdown {
        dimension,
        cumulative_share: cumulative_shares(&ranked),
        concentration: herfindahl(&tokens),
        rows,
    })
}

fn size_point(date: &str, summary: SizeSummary) -> SizePoint {
    SizePoint {
        date: date.to_owned(),
        count: summary.count,
        mean: summary.mean,
        p50: summary.p50,
        p90: summary.p90,
    }
}

/// Request sizes per day (percentiles from size bins, mean from exact sums) and the
/// distribution over the whole period.
fn requests(
    data: &DataServices,
    scope: &Resolved,
    daily: &QueryResult,
    starts: &[UtcMillis],
    dates: &[String],
) -> Result<(Vec<SizePoint>, Vec<SizeBinCount>), ApiError> {
    let rows = data
        .analytics
        .request_sizes(&scope.range(scope.period))
        .map_err(|e| internal(&e))?;
    let per_day = histograms_by_period(&rows, starts);
    let mut overall = SizeHistogram::default();
    for histogram in &per_day {
        overall.merge(histogram);
    }
    let points = per_day
        .iter()
        .zip(&daily.series)
        .zip(dates)
        .map(|((histogram, point), date)| {
            let mean = MetricId::TokensPerRequest
                .compute(&point.sums)
                .map(MetricValue::as_f64);
            size_point(date, SizeSummary::of_requests(histogram, mean))
        })
        .collect();
    let bins = overall
        .bins()
        .map(|(bin, requests)| SizeBinCount {
            lower: bin_lower(bin),
            upper: bin_upper(bin),
            requests,
        })
        .collect();
    Ok((points, bins))
}

/// Session sizes per local day of each session's first request (exact statistics).
fn sessions(
    data: &DataServices,
    scope: &Resolved,
    starts: &[UtcMillis],
    dates: &[String],
) -> Result<Vec<SizePoint>, ApiError> {
    let sizes = data
        .analytics
        .session_sizes(&scope.range(scope.period))
        .map_err(|e| internal(&e))?;
    Ok(sessions_by_period(&sizes, starts)
        .iter()
        .zip(dates)
        .map(|(values, date)| size_point(date, SizeSummary::of_values(values)))
        .collect())
}

/// Builds the Usage page for a scope.
pub(crate) fn usage(data: &DataServices, scope: &Resolved) -> Result<Usage, ApiError> {
    let daily = evaluated(data, &scope.spec(vec![], TimeGrain::Day, true))?;
    let current = &daily.totals;
    let accuracy = token_accuracy(current);
    let starts: Vec<UtcMillis> = daily.series.iter().map(|p| p.start).collect();
    let dates = starts
        .iter()
        .map(|start| local_date(*start, &scope.time_zone))
        .collect::<Result<Vec<_>, _>>()?;
    let scope_tokens = measures(current).tokens;
    let breakdowns = BREAKDOWNS
        .iter()
        .map(|(shown, grouped)| breakdown(data, scope, *shown, *grouped, scope_tokens))
        .collect::<Result<Vec<_>, _>>()?;
    let (per_request, request_sizes) = requests(data, scope, &daily, &starts, &dates)?;
    Ok(Usage {
        kpis: kpis(current, daily.previous_totals.as_ref(), accuracy),
        accuracy,
        composition: daily
            .series
            .iter()
            .zip(&dates)
            .map(|(point, date)| mix_point(date.clone(), &point.sums))
            .collect(),
        reasoning_reported: current.tokens.reasoning.is_some(),
        breakdowns,
        per_request,
        per_session: sessions(data, scope, &starts, &dates)?,
        request_sizes,
        request_definition: REQUEST_PERCENTILES_DEFINITION.to_owned(),
        session_definition: SESSION_SIZES_DEFINITION.to_owned(),
        concentration_definition: HERFINDAHL_DEFINITION.to_owned(),
    })
}
