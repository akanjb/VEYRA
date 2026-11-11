//! Contract tests: the analytics commands return exactly what the
//! analytics engine computes for the same scope, on data ingested from the Claude Code
//! fixtures through the real pipeline.

#![allow(
    clippy::unwrap_used,
    reason = "integration tests: a failure aborts the test"
)]

mod support;

use veyra_analytics::metrics::MetricId;
use veyra_app::dto::{
    ActivityRequest, CacheRequest, Highlight, Labelled, MergeProjectRequest, ModelsRequest,
    OverviewRequest, ProjectRequest, ProjectsRequest, RenameProjectRequest, SessionRequest,
    SessionSort, SessionsRequest, UsageRequest,
};

use support::{api, engine_value, ingested, scope};

#[test]
fn overview_numbers_equal_the_metric_engine() {
    let data = ingested();
    let overview = api(&data.store)
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    let k = &overview.kpis;
    for (kpi, metric) in [
        (&k.tokens, MetricId::TotalTokens),
        (&k.cost_nano_usd, MetricId::ApiEquivalentCost),
        (&k.requests, MetricId::Requests),
        (&k.cache_hit_ratio, MetricId::CacheHitRatio),
    ] {
        assert_eq!(kpi.value, engine_value(&data.store, metric), "{metric:?}");
        assert_eq!(
            kpi.definition,
            metric.definition().description,
            "one source of definitions"
        );
        assert!(kpi.value.is_some(), "{metric:?} is reported");
    }
    // Nothing was recorded the day before: not reported, never shown as zero.
    assert_eq!(k.tokens.previous, None);
    assert_eq!(k.tokens.change, None);
    assert_eq!(
        k.sessions.value,
        Some(2.0),
        "a main session and its subagent"
    );
    assert!(k.active_ms.value.unwrap() > 0.0);

    // The daily trend, the model ranking and the calendar add up to the headline.
    let tokens = k.tokens.value.unwrap();
    let daily: u64 = overview
        .daily
        .iter()
        .flat_map(|d| d.values.iter().map(|v| v.tokens))
        .sum();
    let ranked: u64 = overview.models.iter().map(|m| m.measures.tokens).sum();
    let calendar: u64 = overview.calendar.iter().map(|d| d.tokens).sum();
    let hours: u64 = overview.hours.iter().map(|h| h.tokens).sum();
    #[allow(clippy::cast_precision_loss, reason = "comparing with an f64 KPI")]
    for (part, name) in [
        (daily, "daily"),
        (ranked, "models"),
        (calendar, "calendar"),
        (hours, "hours"),
    ] {
        assert_eq!(part as f64, tokens, "{name}");
    }
    let named = |key: &str, label: &str| Labelled {
        key: key.to_owned(),
        label: label.to_owned(),
    };
    assert_eq!(overview.apps, [named("claude-code", "Claude Code")]);
    assert_eq!(overview.providers, [named("anthropic", "Anthropic")]);
    let labels: Vec<&str> = overview.models.iter().map(|m| m.label.as_str()).collect();
    assert_eq!(labels.len(), 2);
    assert!(
        overview
            .models
            .iter()
            .all(|m| m.provider.as_deref() == Some("anthropic"))
    );
    assert_eq!(overview.composition.exact, 1.0);
    assert_eq!(overview.projects.len(), 1);
    assert!(overview.projects[0].label.contains("work/demo"));
}

#[test]
fn sessions_and_their_timelines_match_the_overview() {
    let data = ingested();
    let api = api(&data.store);
    let list = api
        .list_sessions(SessionsRequest {
            scope: scope(),
            sort: SessionSort::Tokens,
            page: 0,
            page_size: 50,
        })
        .unwrap();
    assert_eq!(list.total, 2);
    let main = list.rows.iter().find(|r| !r.is_subagent).unwrap();
    let sub = list.rows.iter().find(|r| r.is_subagent).unwrap();
    assert_eq!(sub.parent_id, Some(main.id));
    assert_eq!(main.events, 3);
    // req_C reported an inconsistent cache-write split: priced with an assumption.
    assert!(main.cost_assumptions);
    assert!(!sub.cost_assumptions);
    assert_eq!(main.unpriced_requests + sub.unpriced_requests, 0);

    let overview = api
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    let listed: u64 = list.rows.iter().map(|r| r.measures.tokens).sum();
    #[allow(clippy::cast_precision_loss, reason = "comparing with an f64 KPI")]
    let listed = listed as f64;
    assert_eq!(Some(listed), overview.kpis.tokens.value);

    let detail = api.get_session(SessionRequest { id: main.id }).unwrap();
    assert_eq!(detail.session.measures, main.measures);
    assert_eq!(detail.events.len(), 3);
    assert_eq!(detail.subagents.len(), 1);
    let tools: Vec<&str> = detail
        .events
        .iter()
        .flat_map(|e| e.tools.iter().map(|t| t.name.as_str()))
        .collect();
    assert!(tools.contains(&"Bash"), "{tools:?}");
    let events_total: u64 = detail.events.iter().map(|e| e.tokens.total).sum();
    assert_eq!(events_total, main.measures.tokens);

    // Every page names a model the same way.
    let overview_names: Vec<&str> = overview.models.iter().map(|m| m.label.as_str()).collect();
    let session_names = main.models.iter().map(|m| m.label.as_str());
    let timeline_names = detail
        .events
        .iter()
        .flat_map(|e| &e.model)
        .map(|m| m.label.as_str());
    for name in session_names.chain(timeline_names) {
        assert!(
            overview_names.contains(&name),
            "{name} vs {overview_names:?}"
        );
    }
}

#[test]
fn usage_numbers_equal_the_metric_engine_and_add_up() {
    let data = ingested();
    let usage = api(&data.store)
        .get_usage(UsageRequest { scope: scope() })
        .unwrap();
    let k = &usage.kpis;
    for (kpi, metric) in [
        (&k.tokens, MetricId::TotalTokens),
        (&k.input_tokens, MetricId::InputTokens),
        (&k.output_tokens, MetricId::OutputTokens),
        (&k.requests, MetricId::Requests),
        (&k.tokens_per_request, MetricId::TokensPerRequest),
    ] {
        assert_eq!(kpi.value, engine_value(&data.store, metric), "{metric:?}");
        assert_eq!(kpi.definition, metric.definition().description);
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "whole tokens"
    )]
    let tokens = k.tokens.value.unwrap() as u64;

    // The four token kinds add up to the total, day by day.
    let mixed: u64 = usage
        .composition
        .iter()
        .map(|d| d.input_uncached + d.cache_read + d.cache_write + d.output)
        .sum();
    assert_eq!(mixed, tokens);

    // Every breakdown covers every token once, and its Pareto line ends at 100%.
    assert_eq!(usage.breakdowns.len(), 6);
    for breakdown in &usage.breakdowns {
        let total: u64 = breakdown.rows.iter().map(|r| r.measures.tokens).sum();
        assert_eq!(total, tokens, "{:?}", breakdown.dimension);
        assert_eq!(breakdown.cumulative_share.len(), breakdown.rows.len());
        assert_eq!(breakdown.cumulative_share.last().copied(), Some(1.0));
        let index = breakdown.concentration.unwrap();
        assert!(index > 0.0 && index <= 1.0, "{:?}", breakdown.dimension);
    }

    // Each request is in one size bin; each session counts once.
    let binned: u64 = usage.request_sizes.iter().map(|b| b.requests).sum();
    assert_eq!(binned, 4, "the fixtures hold four requests");
    let per_day: u64 = usage.per_request.iter().map(|p| p.count).sum();
    assert_eq!(per_day, 4);
    let sessions: u64 = usage.per_session.iter().map(|p| p.count).sum();
    assert_eq!(sessions, 2, "a main session and its subagent");
    let day = &usage.per_request[0];
    assert!(day.p50.unwrap() <= day.p90.unwrap());
    assert_eq!(
        usage.request_definition,
        veyra_analytics::sizes::REQUEST_PERCENTILES_DEFINITION
    );
}

#[test]
fn cache_numbers_equal_the_metric_engine_and_add_up() {
    let data = ingested();
    let cache = api(&data.store)
        .get_cache(CacheRequest { scope: scope() })
        .unwrap();
    let k = &cache.kpis;
    for (kpi, metric) in [
        (&k.net_benefit_nano_usd, MetricId::NetCacheBenefit),
        (&k.gross_saving_nano_usd, MetricId::GrossCacheSaving),
        (&k.write_overhead_nano_usd, MetricId::CacheWriteOverhead),
        (&k.hit_ratio, MetricId::CacheHitRatio),
        (&k.reuse_ratio, MetricId::CacheReuseRatio),
    ] {
        assert_eq!(kpi.value, engine_value(&data.store, metric), "{metric:?}");
        assert_eq!(kpi.definition, metric.definition().description);
    }
    assert_eq!(
        cache.pricing_coverage,
        engine_value(&data.store, MetricId::PricingCoverage)
    );

    #[allow(
        clippy::cast_precision_loss,
        reason = "comparing with f64 engine values"
    )]
    let as_f64 = |n: u64| Some(n as f64);
    let reads: u64 = cache.daily.iter().map(|d| d.reads).sum();
    let writes: u64 = cache.daily.iter().map(|d| d.writes).sum();
    assert_eq!(
        as_f64(reads),
        engine_value(&data.store, MetricId::CacheReadTokens)
    );
    assert_eq!(
        as_f64(writes),
        engine_value(&data.store, MetricId::CacheWriteTokens)
    );

    // The hit-ratio rankings cover every input token once.
    for rows in [&cache.by_model, &cache.by_project] {
        let input: u64 = rows.iter().map(|r| r.input_tokens).sum();
        assert_eq!(
            as_f64(input),
            engine_value(&data.store, MetricId::InputTokens)
        );
    }
    let written: u64 = cache
        .writes_by_model
        .iter()
        .map(|r| r.short + r.long + r.unspecified)
        .sum();
    assert_eq!(
        as_f64(written),
        engine_value(&data.store, MetricId::CacheWriteTokens)
    );

    // Listed sessions lost money on caching, largest loss first.
    assert!(cache.inefficient.iter().all(|s| s.net_benefit_nano_usd < 0));
    assert!(
        cache
            .inefficient
            .windows(2)
            .all(|w| w[0].net_benefit_nano_usd <= w[1].net_benefit_nano_usd)
    );
    assert_eq!(
        cache.inefficient_definition,
        veyra_analytics::metrics::CACHE_LOSS_DEFINITION
    );
}

#[test]
fn models_numbers_add_up_to_the_scope() {
    let data = ingested();
    let models = api(&data.store)
        .get_models(ModelsRequest { scope: scope() })
        .unwrap();
    #[allow(
        clippy::cast_precision_loss,
        reason = "comparing with f64 engine values"
    )]
    let as_f64 = |n: u64| Some(n as f64);
    let tokens = engine_value(&data.store, MetricId::TotalTokens);

    // The leaderboard covers every token once, largest model first.
    let ranked: Vec<u64> = models
        .leaderboard
        .iter()
        .map(|m| m.measures.tokens)
        .collect();
    assert_eq!(as_f64(ranked.iter().sum()), tokens);
    assert!(ranked.windows(2).all(|w| w[0] >= w[1]));
    assert!(
        models
            .leaderboard
            .iter()
            .all(|m| m.first_day == "2026-11-08")
    );
    assert!(
        models.leaderboard.iter().all(
            |m| m.provider_label == "Anthropic" && m.cost_per_million_output_nano_usd.is_some()
        )
    );

    // Each day's provider shares add up to one (or are all zero without tokens).
    for day in &models.provider_share {
        let total: f64 = day.shares.iter().sum();
        assert!(total == 0.0 || (total - 1.0).abs() < 1e-9, "{}", day.date);
    }

    // Both matrices cover every token; the timeline covers the top models' tokens.
    for matrix in [&models.provider_by_app, &models.model_by_app] {
        let cells: u64 = matrix.values.iter().flatten().sum();
        assert_eq!(as_f64(cells), tokens);
        assert!(
            matrix
                .values
                .iter()
                .all(|row| row.len() == matrix.columns.len())
        );
    }
    let lanes: u64 = models.timeline.iter().flat_map(|l| l.tokens.iter()).sum();
    assert_eq!(as_f64(lanes), tokens);
    assert_eq!(
        models.rate_definition,
        MetricId::CostPerMillionOutputTokens
            .definition()
            .description
    );
}

#[test]
fn activity_days_add_up_to_the_period_and_match_the_overview() {
    let data = ingested();
    let api = api(&data.store);
    let activity = api
        .get_activity(ActivityRequest { scope: scope() })
        .unwrap();
    let overview = api
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    let k = &activity.kpis;
    assert_eq!(k.sessions.value, overview.kpis.sessions.value);
    assert_eq!(k.active_ms.value, overview.kpis.active_ms.value);
    assert_eq!(k.active_days.value, Some(1.0));
    assert!(
        k.wall_ms.value >= k.active_ms.value,
        "wall time includes idle gaps"
    );

    // Daily times add up to the period's; sessions are counted on each day they touch.
    #[allow(clippy::cast_precision_loss, reason = "comparing with f64 KPIs")]
    let total = |pick: fn(&veyra_app::dto::ActivityDay) -> i64| {
        Some(activity.daily.iter().map(pick).sum::<i64>() as f64)
    };
    assert_eq!(total(|d| d.active_ms), k.active_ms.value);
    assert_eq!(total(|d| d.wall_ms), k.wall_ms.value);

    // Every session falls in exactly one length range, by either measure.
    let by_active: u64 = activity.session_lengths.iter().map(|b| b.by_active).sum();
    let by_wall: u64 = activity.session_lengths.iter().map(|b| b.by_wall).sum();
    assert_eq!((by_active, by_wall), (2, 2));
    assert_eq!(activity.calendar, overview.calendar);
    assert_eq!(activity.hours, overview.hours);

    // The longest sessions come first.
    assert_ne!(activity.longest.len(), 0);
    assert!(
        activity
            .longest
            .windows(2)
            .all(|w| w[0].wall_ms >= w[1].wall_ms)
    );
    assert!(activity.longest.iter().all(|s| s.active_ms <= s.wall_ms));
}

#[test]
fn projects_match_the_overview_and_can_be_renamed() {
    let data = ingested();
    let api = api(&data.store);
    let list = api
        .list_projects(ProjectsRequest { scope: scope() })
        .unwrap();
    let overview = api
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    assert_eq!(list.rows.len(), 1);
    let row = &list.rows[0];
    assert_eq!(row.measures, overview.projects[0].measures);
    assert_eq!(row.daily_tokens.iter().sum::<u64>(), row.measures.tokens);
    assert_eq!(row.daily_tokens.len(), list.dates.len());
    #[allow(clippy::cast_precision_loss, reason = "comparing with an f64 KPI")]
    let sessions = Some(row.sessions as f64);
    assert_eq!(sessions, overview.kpis.sessions.value);
    assert!(row.top_model.is_some() && row.last_active_at.is_some());

    let detail = api
        .get_project(ProjectRequest {
            scope: scope(),
            id: row.project.id,
        })
        .unwrap();
    assert_eq!(detail.kpis.tokens.value, overview.kpis.tokens.value);
    assert_eq!(detail.models, overview.models);
    assert_eq!(detail.sessions.len(), 2);

    let renamed = api
        .rename_project(RenameProjectRequest {
            id: row.project.id,
            name: Some("Demo".to_owned()),
        })
        .unwrap();
    assert_eq!(renamed.label, "Demo");
    let relabelled = api
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    assert_eq!(
        relabelled.projects[0].label, "Demo",
        "rankings use the name"
    );
    assert!(matches!(
        api.rename_project(RenameProjectRequest {
            id: row.project.id,
            name: Some("x".repeat(81)),
        }),
        Err(veyra_app::ApiError::Validation { .. })
    ));
    assert!(matches!(
        api.merge_project(MergeProjectRequest {
            from: row.project.id,
            into: row.project.id,
        }),
        Err(veyra_app::ApiError::Validation { .. })
    ));
    assert!(matches!(
        api.get_project(ProjectRequest {
            scope: scope(),
            id: 9_999,
        }),
        Err(veyra_app::ApiError::NotFound { .. })
    ));
}

#[test]
fn highlights_restate_the_overview() {
    let data = ingested();
    let overview = api(&data.store)
        .get_overview(OverviewRequest { scope: scope() })
        .unwrap();
    assert!(overview.highlights.len() <= 5);
    for highlight in &overview.highlights {
        match highlight {
            Highlight::BusiestDay { date, tokens } => {
                let day = overview.daily.iter().find(|d| &d.date == date).unwrap();
                let total: u64 = day.values.iter().map(|m| m.tokens).sum();
                assert_eq!(total, *tokens);
                assert!(
                    overview
                        .daily
                        .iter()
                        .all(|d| { d.values.iter().map(|m| m.tokens).sum::<u64>() <= *tokens })
                );
            }
            Highlight::TopModel { label, share, .. } => {
                let first = overview
                    .models
                    .iter()
                    .find(|r| !r.is_other && r.key.is_some());
                assert_eq!(first.map(|r| (&r.label, r.share)), Some((label, *share)));
            }
            Highlight::TopProject { key, share, .. } => {
                let first = overview
                    .projects
                    .iter()
                    .find(|r| !r.is_other && r.key.is_some());
                assert_eq!(
                    first.map(|r| (r.key.as_ref(), r.share)),
                    Some((Some(key), *share))
                );
            }
            Highlight::CacheSaved { net_nano_usd } => assert!(*net_nano_usd > 0),
            Highlight::CostChange { change } => {
                assert_eq!(overview.kpis.cost_nano_usd.change, Some(*change));
            }
        }
    }
    assert!(
        matches!(
            overview.highlights.first(),
            Some(Highlight::BusiestDay { .. })
        ),
        "the fixtures have usage, so the busiest day comes first"
    );
}
