//! The session explorer and the session timeline.

use std::collections::{HashMap, HashSet};

use veyra_analytics::activity::{DEFAULT_IDLE_THRESHOLD_MS, session_active_ms};
use veyra_analytics::composition::AccuracyComposition;
use veyra_analytics::port::EventTimesQuery;
use veyra_analytics::sessions::{self as reads, SessionQuery, SessionSummary};
use veyra_domain::time::UtcMillis;

use super::{accuracy_class, internal, measures};
use crate::DataServices;
use crate::dto::{
    SessionDetail, SessionList, SessionRow, SessionSort, TimelineEvent, TokenBreakdown,
    ToolCallSummary,
};
use crate::error::ApiError;
use crate::scope::Resolved;

/// The largest page the explorer may request.
const MAX_PAGE_SIZE: u32 = 500;

fn row(summary: &SessionSummary, active_ms: i64) -> SessionRow {
    let sums = &summary.sums;
    SessionRow {
        id: summary.id,
        key: summary.key.clone(),
        app: summary.app.clone(),
        project: summary.project.clone(),
        models: summary.models.clone(),
        is_subagent: summary.is_subagent,
        parent_id: summary.parent_id,
        started_at: summary.first_at.0,
        ended_at: summary.last_at.0,
        active_ms,
        events: sums.events,
        measures: measures(sums),
        unpriced_requests: sums.events_unpriced,
        cost_assumptions: sums.events_partial > 0,
        accuracy: AccuracyComposition::of(sums).weakest().map(accuracy_class),
    }
}

const fn sort(sort: SessionSort) -> reads::SessionSort {
    match sort {
        SessionSort::LastActive => reads::SessionSort::LastActive,
        SessionSort::Tokens => reads::SessionSort::Tokens,
        SessionSort::Cost => reads::SessionSort::Cost,
        SessionSort::Events => reads::SessionSort::Events,
    }
}

/// One page of sessions active in the scope; numbers cover the scope's period.
pub(crate) fn list(
    data: &DataServices,
    scope: &Resolved,
    order: SessionSort,
    page: u32,
    page_size: u32,
) -> Result<SessionList, ApiError> {
    if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
        return Err(ApiError::Validation {
            message: format!("page size must be 1 to {MAX_PAGE_SIZE}"),
        });
    }
    let offset = page
        .checked_mul(page_size)
        .ok_or_else(|| ApiError::Validation {
            message: "page is out of range".to_owned(),
        })?;
    let result = data
        .analytics
        .sessions(&SessionQuery {
            from: scope.period.from(),
            until: scope.period.until(),
            filters: scope.filters.clone(),
            sort: sort(order),
            limit: page_size,
            offset,
        })
        .map_err(|e| internal(&e))?;

    // Active time of the listed sessions, from their event times in the period.
    let wanted: HashSet<i64> = result.sessions.iter().map(|s| s.id).collect();
    let mut times: HashMap<i64, Vec<UtcMillis>> = HashMap::new();
    for (session, at) in data
        .analytics
        .event_times(&EventTimesQuery {
            from: scope.period.from(),
            until: scope.period.until(),
            filters: scope.filters.clone(),
        })
        .map_err(|e| internal(&e))?
    {
        if wanted.contains(&session) {
            times.entry(session).or_default().push(at);
        }
    }
    let rows = result
        .sessions
        .iter()
        .map(|s| {
            let active = times
                .get(&s.id)
                .map_or(0, |t| session_active_ms(t, DEFAULT_IDLE_THRESHOLD_MS));
            row(s, active)
        })
        .collect();
    Ok(SessionList {
        total: result.total,
        rows,
    })
}

fn timeline_event(event: &reads::SessionEvent) -> TimelineEvent {
    let t = &event.tokens;
    TimelineEvent {
        at: event.at.0,
        model: event.model.clone().or_else(|| event.raw_model.clone()),
        tokens: TokenBreakdown {
            input: t.input_uncached,
            cache_read: t.cache_read,
            cache_write: t.cache_write_total(),
            output: t.output_total,
            reasoning: t.reasoning,
            total: t.total().unwrap_or(0),
        },
        cost_nano_usd: event.cost.map(|c| c.0),
        accuracy: accuracy_class(event.token_accuracy),
        tools: event
            .tools
            .iter()
            .map(|tool| ToolCallSummary {
                name: tool.name.clone(),
                mcp_server: tool.mcp_server.clone(),
                is_error: tool.is_error,
            })
            .collect(),
    }
}

fn active_of(events: &[reads::SessionEvent]) -> i64 {
    let times: Vec<UtcMillis> = events.iter().map(|e| e.at).collect();
    session_active_ms(&times, DEFAULT_IDLE_THRESHOLD_MS)
}

/// A whole session with its timeline and subagents.
pub(crate) fn detail(data: &DataServices, id: i64) -> Result<SessionDetail, ApiError> {
    let found = data
        .analytics
        .session(id)
        .map_err(|e| internal(&e))?
        .ok_or_else(|| ApiError::NotFound {
            message: format!("session {id} does not exist"),
        })?;
    let mut subagents = Vec::with_capacity(found.subagents.len());
    for child in &found.subagents {
        let active = data
            .analytics
            .session(child.id)
            .map_err(|e| internal(&e))?
            .map_or(0, |d| active_of(&d.events));
        subagents.push(row(child, active));
    }
    Ok(SessionDetail {
        session: row(&found.summary, active_of(&found.events)),
        events: found.events.iter().map(timeline_event).collect(),
        subagents,
    })
}
