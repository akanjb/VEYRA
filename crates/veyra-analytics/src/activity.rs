//! Session count and active time (PRD §7).
//!
//! - **Sessions:** distinct sessions with at least one event in the period.
//! - **Active time:** the union over sessions of the intervals
//!   `[tᵢ, tᵢ + min(tᵢ₊₁ − tᵢ, T)]` between consecutive events of a session, where `T` is the
//!   idle threshold. Gaps longer than `T` count as idle beyond `T`; parallel sessions are
//!   not double-counted. It is a *derived* measure and is labelled as such.

use veyra_domain::time::UtcMillis;

/// What the session count means (shown with every session figure).
pub const SESSIONS_DEFINITION: &str =
    "Distinct sessions with at least one request in the period; subagents count separately.";

/// What active time means (shown with every active-time figure).
pub const ACTIVE_TIME_DEFINITION: &str = "Derived from exact request times: the gaps between consecutive requests of each session, each capped at 5 minutes of idle time, with overlapping sessions counted once.";

/// Default idle threshold `T`: five minutes.
pub const DEFAULT_IDLE_THRESHOLD_MS: i64 = 5 * 60_000;

/// Activity of a set of events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Activity {
    pub sessions: u64,
    pub active_ms: i64,
}

/// Activity of events given as `(session id, time)`, ordered by session and then time.
#[must_use]
pub fn activity(events: &[(i64, UtcMillis)], idle_threshold_ms: i64) -> Activity {
    let mut intervals: Vec<(i64, i64)> = Vec::new();
    let mut sessions = 0;
    let mut previous: Option<(i64, i64)> = None;
    for &(session, UtcMillis(at)) in events {
        match previous {
            Some((last_session, last_at)) if last_session == session => {
                let end = last_at + (at - last_at).clamp(0, idle_threshold_ms);
                if end > last_at {
                    intervals.push((last_at, end));
                }
            }
            _ => sessions += 1,
        }
        previous = Some((session, at));
    }
    Activity {
        sessions,
        active_ms: union_length(intervals),
    }
}

/// Active time of one session's event times (ascending).
#[must_use]
pub fn session_active_ms(times: &[UtcMillis], idle_threshold_ms: i64) -> i64 {
    times
        .windows(2)
        .map(|pair| (pair[1].0 - pair[0].0).clamp(0, idle_threshold_ms))
        .sum()
}

/// Total length covered by possibly overlapping intervals.
fn union_length(mut intervals: Vec<(i64, i64)>) -> i64 {
    intervals.sort_unstable();
    let mut total = 0;
    let mut current: Option<(i64, i64)> = None;
    for (start, end) in intervals {
        current = match current {
            Some((s, e)) if start <= e => Some((s, e.max(end))),
            Some((s, e)) => {
                total += e - s;
                Some((start, end))
            }
            None => Some((start, end)),
        };
    }
    total + current.map_or(0, |(s, e)| e - s)
}

#[cfg(test)]
mod tests {
    use veyra_domain::time::UtcMillis;

    use super::{Activity, activity, session_active_ms};

    const MIN: i64 = 60_000;
    const T: i64 = 5 * MIN;

    fn at(minutes: i64) -> UtcMillis {
        UtcMillis(minutes * MIN)
    }

    #[test]
    fn caps_gaps_at_the_idle_threshold() {
        // Gaps of 2, 30 and 4 minutes: 2 + 5 + 4 = 11 active minutes.
        let events = [(1, at(0)), (1, at(2)), (1, at(32)), (1, at(36))];
        assert_eq!(
            activity(&events, T),
            Activity {
                sessions: 1,
                active_ms: 11 * MIN
            }
        );
        let times: Vec<UtcMillis> = events.iter().map(|(_, t)| *t).collect();
        assert_eq!(session_active_ms(&times, T), 11 * MIN);
    }

    #[test]
    fn parallel_sessions_are_not_double_counted() {
        // Session 1 covers minutes 0–4; session 2 covers minutes 2–6: union 6 minutes.
        let events = [(1, at(0)), (1, at(4)), (2, at(2)), (2, at(6))];
        assert_eq!(
            activity(&events, T),
            Activity {
                sessions: 2,
                active_ms: 6 * MIN
            }
        );
    }

    #[test]
    fn a_single_event_is_a_session_without_active_time() {
        assert_eq!(
            activity(&[(9, at(10))], T),
            Activity {
                sessions: 1,
                active_ms: 0
            }
        );
        assert_eq!(activity(&[], T), Activity::default());
    }
}
