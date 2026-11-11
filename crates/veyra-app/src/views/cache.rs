//! The Cache page: is caching working, and what does it save? Every number comes from
//! the analytics engine (ADR-008).

use std::thread;

use veyra_analytics::dimension::Dimension;
use veyra_analytics::engine::GroupResult;
use veyra_analytics::metrics::{
    CACHE_LOSS_DEFINITION, MetricId, MetricValue, net_cache_benefit_nano_usd,
};
use veyra_analytics::query::TimeGrain;
use veyra_analytics::sessions::{SessionQuery, SessionSort, SessionSummary};
use veyra_analytics::sums::BaseSums;
use veyra_analytics::top_n::top_n;

use super::kpis::{metric_kpi, token_accuracy};
use super::rankings::{Labeller, RowLabel, groups_of};
use super::{evaluated, internal, joined, local_date};
use crate::DataServices;
use crate::dto::{AccuracyClass, Cache, CacheKpis, CachePoint, CacheSession, RatioRow, TtlRow};
use crate::error::ApiError;
use crate::scope::Resolved;

/// Groups shown in each hit-ratio ranking before "Other".
const TOP_RATIO_ROWS: usize = 10;
/// Models shown in the cache-write comparison.
const TOP_WRITING_MODELS: usize = 8;
/// Sessions with the most cache writes examined for losses.
const CANDIDATE_SESSIONS: u32 = 200;
/// Loss-making sessions listed.
const LOSSES_SHOWN: usize = 10;

fn kpis(
    current: &BaseSums,
    previous: Option<&BaseSums>,
    accuracy: Option<AccuracyClass>,
) -> CacheKpis {
    // Money is an estimate at API prices; ratios carry the accuracy of their tokens.
    let money = |metric| metric_kpi(metric, current, previous, Some(AccuracyClass::Estimated));
    let ratio = |metric| metric_kpi(metric, current, previous, accuracy);
    CacheKpis {
        net_benefit_nano_usd: money(MetricId::NetCacheBenefit),
        gross_saving_nano_usd: money(MetricId::GrossCacheSaving),
        write_overhead_nano_usd: money(MetricId::CacheWriteOverhead),
        hit_ratio: ratio(MetricId::CacheHitRatio),
        reuse_ratio: ratio(MetricId::CacheReuseRatio),
    }
}

fn tokens(metric: MetricId, sums: &BaseSums) -> u64 {
    match metric.compute(sums) {
        Some(MetricValue::Count(n)) => n,
        _ => 0,
    }
}

fn ratio_row(label: RowLabel, sums: &BaseSums, is_other: bool) -> RatioRow {
    RatioRow {
        key: label.key,
        label: label.label,
        provider: label.provider,
        hit_ratio: MetricId::CacheHitRatio
            .compute(sums)
            .map(MetricValue::as_f64),
        input_tokens: tokens(MetricId::InputTokens, sums),
        cache_read: tokens(MetricId::CacheReadTokens, sums),
        is_other,
    }
}

/// The groups with the most input tokens, each with its hit ratio, plus "Other".
fn hit_ratios(
    data: &DataServices,
    scope: &Resolved,
    dimension: Dimension,
) -> Result<Vec<RatioRow>, ApiError> {
    let labeller = Labeller::new(data, dimension)?;
    let ranked = top_n(
        groups_of(data, scope, &labeller)?,
        MetricId::InputTokens,
        TOP_RATIO_ROWS,
    );
    let mut rows: Vec<RatioRow> = ranked
        .top
        .iter()
        .map(|group| ratio_row(labeller.label(data, group), &group.totals, false))
        .collect();
    if let Some(other) = ranked.other {
        let label = RowLabel {
            key: None,
            label: format!("Other ({} groups)", other.group_count),
            provider: None,
        };
        rows.push(ratio_row(label, &other.totals, true));
    }
    Ok(rows)
}

fn ttl_row(label: RowLabel, group: &GroupResult) -> TtlRow {
    let t = &group.totals.tokens;
    TtlRow {
        key: label.key,
        label: label.label,
        provider: label.provider,
        short: t.cache_write_short.unwrap_or(0),
        long: t.cache_write_long.unwrap_or(0),
        unspecified: t.cache_write_unspecified.unwrap_or(0),
        reads: t.cache_read.unwrap_or(0),
    }
}

/// The models that write the most to the cache, with their writes per TTL.
fn writes_by_model(data: &DataServices, scope: &Resolved) -> Result<Vec<TtlRow>, ApiError> {
    let labeller = Labeller::new(data, Dimension::Model)?;
    let ranked = top_n(
        groups_of(data, scope, &labeller)?,
        MetricId::CacheWriteTokens,
        TOP_WRITING_MODELS,
    );
    Ok(ranked
        .top
        .iter()
        .filter(|group| tokens(MetricId::CacheWriteTokens, &group.totals) > 0)
        .map(|group| ttl_row(labeller.label(data, group), group))
        .collect())
}

fn cache_session(data: &DataServices, summary: &SessionSummary, net: i64) -> CacheSession {
    let sums = &summary.sums;
    CacheSession {
        id: summary.id,
        app: data.names.label_app(summary.app.clone()).label,
        project: summary.project.clone(),
        models: data.names.label_models(&summary.models),
        ended_at: summary.last_at.0,
        cache_write: tokens(MetricId::CacheWriteTokens, sums),
        cache_read: tokens(MetricId::CacheReadTokens, sums),
        reuse_ratio: MetricId::CacheReuseRatio
            .compute(sums)
            .map(MetricValue::as_f64),
        net_benefit_nano_usd: net,
    }
}

/// Of the sessions writing the most to the cache, those whose writes cost more than their
/// reads saved, largest loss first.
fn losses(data: &DataServices, scope: &Resolved) -> Result<Vec<CacheSession>, ApiError> {
    let page = data
        .analytics
        .sessions(&SessionQuery {
            from: scope.period.from(),
            until: scope.period.until(),
            filters: scope.filters.clone(),
            sort: SessionSort::CacheWrites,
            limit: CANDIDATE_SESSIONS,
            offset: 0,
        })
        .map_err(|e| internal(&e))?;
    let mut losing: Vec<(i64, &SessionSummary)> = page
        .sessions
        .iter()
        .filter_map(|s| net_cache_benefit_nano_usd(&s.sums).map(|net| (net, s)))
        .filter(|(net, _)| *net < 0)
        .collect();
    losing.sort_by_key(|(net, s)| (*net, s.id));
    Ok(losing
        .into_iter()
        .take(LOSSES_SHOWN)
        .map(|(net, s)| cache_session(data, s, net))
        .collect())
}

/// Builds the Cache page for a scope.
pub(crate) fn cache(data: &DataServices, scope: &Resolved) -> Result<Cache, ApiError> {
    let daily = evaluated(data, &scope.spec(vec![], TimeGrain::Day, true))?;
    let current = &daily.totals;
    let points = daily
        .series
        .iter()
        .map(|point| {
            Ok(CachePoint {
                date: local_date(point.start, &scope.time_zone)?,
                reads: tokens(MetricId::CacheReadTokens, &point.sums),
                writes: tokens(MetricId::CacheWriteTokens, &point.sums),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    thread::scope(|s| {
        let by_model = s.spawn(|| hit_ratios(data, scope, Dimension::Model));
        let by_project = s.spawn(|| hit_ratios(data, scope, Dimension::Project));
        let writes = s.spawn(|| writes_by_model(data, scope));
        let inefficient = losses(data, scope)?;
        Ok(Cache {
            kpis: kpis(
                current,
                daily.previous_totals.as_ref(),
                token_accuracy(current),
            ),
            accuracy: token_accuracy(current),
            pricing_coverage: MetricId::PricingCoverage
                .compute(current)
                .map(MetricValue::as_f64),
            coverage_definition: MetricId::PricingCoverage
                .definition()
                .description
                .to_owned(),
            daily: points,
            by_model: joined(by_model)?,
            by_project: joined(by_project)?,
            writes_by_model: joined(writes)?,
            inefficient,
            inefficient_definition: CACHE_LOSS_DEFINITION.to_owned(),
        })
    })
}
