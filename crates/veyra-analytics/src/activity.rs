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

/// Per-session intervals of events given as `(session id, time)`, ordered by session and
/// then time.
struct Intervals {
    sessions: Vec<(i64, Vec<i64>)>,
}

impl Intervals {
    fn of(events: &[(i64, UtcMillis)]) -> Self {
        let mut sessions: Vec<(i64, Vec<i64>)> = Vec::new();
        for &(session, UtcMillis(at)) in events {
            match sessions.last_mut() {
                Some((last, times)) if *last == session => times.push(at),
                _ => sessions.push((session, vec![at])),
            }
        }
        Self { sessions }
    }

    /// Each gap between consecutive events, capped at the idle threshold.
    fn active(&self, idle_threshold_ms: i64) -> Vec<(i64, i64)> {
        self.sessions
            .iter()
            .flat_map(|(_, times)| {
                times.windows(2).filter_map(move |pair| {
                    let end = pair[0] + (pair[1] - pair[0]).clamp(0, idle_threshold_ms);
                    (end > pair[0]).then_some((pair[0], end))
                })
            })
            .collect()
    }

    /// Each session from its first event to its last.
    fn wall(&self) -> Vec<(i64, i64)> {
        self.sessions
            .iter()
            .filter_map(|(_, times)| {
                let (first, last) = (times.first()?, times.last()?);
                (last > first).then_some((*first, *last))
            })
            .collect()
    }
}

/// Activity of events given as `(session id, time)`, ordered by session and then time.
#[must_use]
pub fn activity(events: &[(i64, UtcMillis)], idle_threshold_ms: i64) -> Activity {
    let intervals = Intervals::of(events);
    Activity {
        sessions: u64::try_from(intervals.sessions.len()).unwrap_or(u64::MAX),
        active_ms: total_length(&merged(intervals.active(idle_threshold_ms))),
        wall_ms: total_length(&merged(intervals.wall())),
    }
}

/// Activity of each local period `[starts[i], starts[i + 1])`, the last ending at `until`.
/// Intervals spanning a boundary are split at it; a session counts in every period
/// where it has an event.
#[must_use]
pub fn activity_by_period(
    events: &[(i64, UtcMillis)],
    idle_threshold_ms: i64,
    starts: &[UtcMillis],
    until: UtcMillis,
) -> Vec<Activity> {
    let intervals = Intervals::of(events);
    let bounds: Vec<i64> = starts.iter().map(|s| s.0).chain([until.0]).collect();
    let active = split_lengths(&merged(intervals.active(idle_threshold_ms)), &bounds);
    let wall = split_lengths(&merged(intervals.wall()), &bounds);
    let mut sessions = vec![0_u64; starts.len()];
    for (_, times) in &intervals.sessions {
        let mut last_period = None;
        for at in times {
            let period = bounds.partition_point(|b| *b <= *at).checked_sub(1);
            if period != last_period
                && let Some(index) = period.filter(|i| *i < starts.len())
            {
                sessions[index] += 1;
            }
            last_period = period;
        }
    }
    (0..starts.len())
        .map(|i| Activity {
            sessions: sessions[i],
            active_ms: active[i],
            wall_ms: wall[i],
        })
        .collect()
}

/// Each session's active and wall time.
#[must_use]
pub fn session_lengths(events: &[(i64, UtcMillis)], idle_threshold_ms: i64) -> Vec<SessionLength> {
    Intervals::of(events)
        .sessions
        .iter()
        .map(|(session, times)| SessionLength {
            session: *session,
            active_ms: times
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).clamp(0, idle_threshold_ms))
                .sum(),
            wall_ms: times.last().copied().unwrap_or(0) - times.first().copied().unwrap_or(0),
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
