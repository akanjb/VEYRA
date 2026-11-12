//! Sessions, active time and wall time (PRD §7).
//!
//! - **Sessions:** distinct sessions with at least one event in the period.
//! - **Active time:** the union over sessions of the intervals
//!   `[tᵢ, tᵢ + min(tᵢ₊₁ − tᵢ, T)]` between consecutive events of a session, where `T` is the
//!   idle threshold. Gaps longer than `T` count as idle beyond `T`; parallel sessions are
//!   not double-counted. It is a *derived* measure and is labelled as such.
//! - **Wall time:** the union over sessions of `[first event, last event]`, so idle gaps
//!   count in full and parallel sessions are again counted once.

use veyra_domain::time::UtcMillis;

/// What the session count means (shown with every session figure).
pub const SESSIONS_DEFINITION: &str =
    "Distinct sessions with at least one request in the period; subagents count separately.";

/// What active time means (shown with every active-time figure).
pub const ACTIVE_TIME_DEFINITION: &str = "Derived from exact request times: the gaps between consecutive requests of each session, each capped at 5 minutes of idle time, with overlapping sessions counted once.";

/// What wall time means (shown with every wall-time figure).
pub const WALL_TIME_DEFINITION: &str = "From each session's first request to its last, idle gaps included, with overlapping sessions counted once.";

/// Default idle threshold `T`: five minutes.
pub const DEFAULT_IDLE_THRESHOLD_MS: i64 = 5 * 60_000;

/// Lower bounds of the session-length bins: under 1 minute, 1–5 minutes, 5–15, 15–30,
/// 30–60, 1–2 hours, 2–4 hours, and 4 hours or more.
pub const DURATION_BOUNDS_MS: [i64; 8] = [
    0,
    60_000,
    5 * 60_000,
    15 * 60_000,
    30 * 60_000,
    3_600_000,
    2 * 3_600_000,
    4 * 3_600_000,
];

/// Activity of a set of events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Activity {
    pub sessions: u64,
    pub active_ms: i64,
    pub wall_ms: i64,
}

/// One session's lengths over the events considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLength {
    pub session: i64,
    pub active_ms: i64,
    pub wall_ms: i64,
}

/// A session's requests within one 15-minute bucket: the first and last request times
/// and the active intervals between consecutive requests inside the bucket, merged.
///
/// Activity is computed from spans, so it can be read from a per-bucket rollup or built
/// from raw request times with the same result: every period boundary falls on a bucket
/// boundary, and the gap between two spans of a session is the gap between the last
/// request of one and the first of the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub session: i64,
    pub first: i64,
    pub last: i64,
    /// Disjoint, ordered intervals `[tᵢ, tᵢ + min(tᵢ₊₁ − tᵢ, T)]` inside the bucket.
    pub active: Vec<(i64, i64)>,
}

/// The interval each gap between consecutive times counts as active, capped at the idle
/// threshold. Zero-length gaps are left out.
fn gap_interval(from: i64, to: i64, idle_threshold_ms: i64) -> Option<(i64, i64)> {
    let end = from + (to - from).clamp(0, idle_threshold_ms);
    (end > from).then_some((from, end))
}

/// The span of one session's request times within a bucket (ascending); `None` when
/// there are none.
#[must_use]
pub fn span_of(session: i64, times: &[i64], idle_threshold_ms: i64) -> Option<Span> {
    let (first, last) = (*times.first()?, *times.last()?);
    let gaps = times
        .windows(2)
        .filter_map(|pair| gap_interval(pair[0], pair[1], idle_threshold_ms))
        .collect();
    Some(Span {
        session,
        first,
        last,
        active: merged(gaps),
    })
}

/// One span per session and 15-minute bucket, from events given as `(session id, time)`
/// ordered by session and then time.
#[must_use]
pub fn spans_of(events: &[(i64, UtcMillis)], idle_threshold_ms: i64) -> Vec<Span> {
    let mut spans = Vec::new();
    for group in events.chunk_by(|a, b| a.0 == b.0 && a.1.bucket_start() == b.1.bucket_start()) {
        let times: Vec<i64> = group.iter().map(|(_, at)| at.0).collect();
        spans.extend(span_of(group[0].0, &times, idle_threshold_ms));
    }
    spans
}

/// Spans grouped by session, from spans ordered by session and then time.
fn by_session(spans: &[Span]) -> Vec<&[Span]> {
    spans.chunk_by(|a, b| a.session == b.session).collect()
}

/// Every active interval: inside each span, and across the gap to the session's next span.
fn active_intervals(sessions: &[&[Span]], idle_threshold_ms: i64) -> Vec<(i64, i64)> {
    sessions
        .iter()
        .flat_map(|spans| {
            let inside = spans.iter().flat_map(|span| span.active.iter().copied());
            let across = spans.windows(2).filter_map(move |pair| {
                gap_interval(pair[0].last, pair[1].first, idle_threshold_ms)
            });
            inside.chain(across)
        })
        .collect()
}

/// Each session from its first request to its last.
fn wall_intervals(sessions: &[&[Span]]) -> Vec<(i64, i64)> {
    sessions
        .iter()
        .filter_map(|spans| {
            let (first, last) = (spans.first()?.first, spans.last()?.last);
            (last > first).then_some((first, last))
        })
        .collect()
}

/// Activity of spans ordered by session and then time.
#[must_use]
pub fn activity(spans: &[Span], idle_threshold_ms: i64) -> Activity {
    let sessions = by_session(spans);
    Activity {
        sessions: u64::try_from(sessions.len()).unwrap_or(u64::MAX),
        active_ms: total_length(&merged(active_intervals(&sessions, idle_threshold_ms))),
        wall_ms: total_length(&merged(wall_intervals(&sessions))),
    }
}

/// Activity of each local period `[starts[i], starts[i + 1])`, the last ending at `until`.
/// Every boundary must fall on a bucket boundary. Intervals spanning a boundary are split
/// at it; a session counts in every period where it has a request.
#[must_use]
pub fn activity_by_period(
    spans: &[Span],
    idle_threshold_ms: i64,
    starts: &[UtcMillis],
    until: UtcMillis,
) -> Vec<Activity> {
    let sessions = by_session(spans);
    let bounds: Vec<i64> = starts.iter().map(|s| s.0).chain([until.0]).collect();
    let active = split_lengths(
        &merged(active_intervals(&sessions, idle_threshold_ms)),
        &bounds,
    );
    let wall = split_lengths(&merged(wall_intervals(&sessions)), &bounds);
    let mut counts = vec![0_u64; starts.len()];
    for spans in &sessions {
        let mut last_period = None;
        for span in *spans {
            let period = bounds.partition_point(|b| *b <= span.first).checked_sub(1);
            if period != last_period
                && let Some(index) = period.filter(|i| *i < starts.len())
            {
                counts[index] += 1;
            }
            last_period = period;
        }
    }
    (0..starts.len())
        .map(|i| Activity {
            sessions: counts[i],
            active_ms: active[i],
            wall_ms: wall[i],
        })
        .collect()
}

/// Each session's active and wall time, from spans ordered by session and then time.
#[must_use]
pub fn session_lengths(spans: &[Span], idle_threshold_ms: i64) -> Vec<SessionLength> {
    by_session(spans)
        .into_iter()
        .map(|spans| SessionLength {
            session: spans[0].session,
            active_ms: total_length(&active_intervals(&[spans], idle_threshold_ms)),
            wall_ms: spans[spans.len() - 1].last - spans[0].first,
        })
        .collect()
}

/// Active time of one session's event times (ascending).
#[must_use]
pub fn session_active_ms(times: &[UtcMillis], idle_threshold_ms: i64) -> i64 {
    times
        .windows(2)
        .map(|pair| (pair[1].0 - pair[0].0).clamp(0, idle_threshold_ms))
        .sum()
}

/// The session-length bin of a duration (an index into [`DURATION_BOUNDS_MS`]).
#[must_use]
pub fn duration_bin(ms: i64) -> usize {
    DURATION_BOUNDS_MS
        .partition_point(|lower| *lower <= ms)
        .saturating_sub(1)
}

/// Overlapping intervals merged into disjoint ones, in order.
fn merged(mut intervals: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    intervals.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (start, end) in intervals {
        match out.last_mut() {
            Some((_, last_end)) if start <= *last_end => *last_end = (*last_end).max(end),
            _ => out.push((start, end)),
        }
    }
    out
}

fn total_length(disjoint: &[(i64, i64)]) -> i64 {
    disjoint.iter().map(|(start, end)| end - start).sum()
}

/// The length of disjoint, ordered intervals falling in each `[bounds[i], bounds[i + 1])`.
fn split_lengths(disjoint: &[(i64, i64)], bounds: &[i64]) -> Vec<i64> {
    let periods = bounds.len().saturating_sub(1);
    let mut lengths = vec![0_i64; periods];
    for &(start, end) in disjoint {
        let first = bounds.partition_point(|b| *b <= start).saturating_sub(1);
        for (period, length) in lengths.iter_mut().enumerate().skip(first) {
            let (from, until) = (bounds[period], bounds[period + 1]);
            if from >= end {
                break;
            }
            *length += (end.min(until) - start.max(from)).max(0);
        }
    }
    lengths
}

#[cfg(test)]
mod tests;
