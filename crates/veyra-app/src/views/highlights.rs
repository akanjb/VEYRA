//! Highlights on the Overview: up to five deterministic facts about the period, each
//! read from figures the page already shows (03-ux.md §4). No free text is generated;
//! the dashboard words each kind.

use crate::dto::{DailyPoint, Highlight, Kpi, RankRow};

/// At most this many highlights are shown.
const MAX_HIGHLIGHTS: usize = 5;

/// The day with the most tokens; the earliest on a tie. `None` without tokens.
fn busiest_day(daily: &[DailyPoint]) -> Option<Highlight> {
    let tokens = |p: &DailyPoint| p.values.iter().map(|m| m.tokens).sum::<u64>();
    let (best, most) = daily.iter().map(|p| (p, tokens(p))).fold(
        None,
        |best: Option<(&DailyPoint, u64)>, (p, n)| match best {
            Some((_, m)) if m >= n => best,
            _ => Some((p, n)),
        },
    )?;
    (most > 0).then(|| Highlight::BusiestDay {
        date: best.date.clone(),
        tokens: most,
    })
}

/// The first named (not "Other", not unknown) row of a ranking with a share.
fn leader(rows: &[RankRow]) -> Option<&RankRow> {
    rows.iter()
        .find(|r| !r.is_other && r.key.is_some() && r.share > 0.0)
}

fn top_model(models: &[RankRow]) -> Option<Highlight> {
    leader(models).map(|r| Highlight::TopModel {
        label: r.label.clone(),
        provider: r.provider.clone(),
        share: r.share,
    })
}

fn top_project(projects: &[RankRow]) -> Option<Highlight> {
    leader(projects).map(|r| Highlight::TopProject {
        key: r.key.clone().unwrap_or_default(),
        label: r.label.clone(),
        share: r.share,
    })
}

/// Cache saved money when the net benefit is known and positive.
fn cache_saved(net_cache_benefit_nano_usd: Option<i64>) -> Option<Highlight> {
    net_cache_benefit_nano_usd
        .filter(|n| *n > 0)
        .map(|net| Highlight::CacheSaved { net_nano_usd: net })
}

/// How API-equivalent cost changed against the comparison period, when defined.
fn cost_change(cost: &Kpi) -> Option<Highlight> {
    cost.change
        .filter(|c| c.is_finite())
        .map(|change| Highlight::CostChange { change })
}

/// The Overview's highlights, in a fixed order, at most [`MAX_HIGHLIGHTS`].
pub(crate) fn highlights(
    daily: &[DailyPoint],
    models: &[RankRow],
    projects: &[RankRow],
    cost: &Kpi,
    net_cache_benefit_nano_usd: Option<i64>,
) -> Vec<Highlight> {
    [
        busiest_day(daily),
        top_model(models),
        top_project(projects),
        cache_saved(net_cache_benefit_nano_usd),
        cost_change(cost),
    ]
    .into_iter()
    .flatten()
    .take(MAX_HIGHLIGHTS)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::highlights;
    use crate::dto::{DailyPoint, Highlight, Kpi, Measures, RankRow};

    fn day(date: &str, tokens: u64) -> DailyPoint {
        DailyPoint {
            date: date.to_owned(),
            values: vec![Measures {
                tokens,
                cost_nano_usd: 0,
                requests: 0,
            }],
        }
    }

    fn row(key: Option<&str>, share: f64, is_other: bool) -> RankRow {
        RankRow {
            key: key.map(str::to_owned),
            label: key.unwrap_or("Other").to_owned(),
            provider: None,
            measures: Measures::default(),
            share,
            group_count: 1,
            is_other,
        }
    }

    fn kpi(change: Option<f64>) -> Kpi {
        Kpi {
            value: None,
            previous: None,
            change,
            accuracy: None,
            definition: String::new(),
        }
    }

    #[test]
    fn every_fact_appears_in_a_fixed_order() {
        let found = highlights(
            &[
                day("2026-11-01", 5),
                day("2026-11-02", 9),
                day("2026-11-03", 9),
            ],
            &[row(None, 0.5, false), row(Some("opus"), 0.4, false)],
            &[row(Some("7"), 0.6, false)],
            &kpi(Some(-0.25)),
            Some(1_000),
        );
        assert_eq!(
            found,
            vec![
                Highlight::BusiestDay {
                    date: "2026-11-02".to_owned(),
                    tokens: 9
                },
                Highlight::TopModel {
                    label: "opus".to_owned(),
                    provider: None,
                    share: 0.4
                },
                Highlight::TopProject {
                    key: "7".to_owned(),
                    label: "7".to_owned(),
                    share: 0.6
                },
                Highlight::CacheSaved {
                    net_nano_usd: 1_000
                },
                Highlight::CostChange { change: -0.25 },
            ]
        );
    }

    #[test]
    fn missing_facts_are_left_out() {
        let found = highlights(
            &[day("2026-11-01", 0)],
            &[row(None, 1.0, true)],
            &[],
            &kpi(None),
            Some(-5),
        );
        assert_eq!(found, vec![]);
    }
}
