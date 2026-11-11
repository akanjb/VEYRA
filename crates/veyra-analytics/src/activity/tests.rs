use veyra_domain::time::UtcMillis;

use super::{
    Activity, DURATION_BOUNDS_MS, activity, activity_by_period, duration_bin, session_active_ms,
    session_lengths,
};

const MIN: i64 = 60_000;
const T: i64 = 5 * MIN;

fn at(minutes: i64) -> UtcMillis {
    UtcMillis(minutes * MIN)
}

#[test]
fn caps_gaps_at_the_idle_threshold_but_not_wall_time() {
    // Gaps of 2, 30 and 4 minutes: 2 + 5 + 4 = 11 active minutes, 36 wall minutes.
    let events = [(1, at(0)), (1, at(2)), (1, at(32)), (1, at(36))];
    assert_eq!(
        activity(&events, T),
        Activity {
            sessions: 1,
            active_ms: 11 * MIN,
            wall_ms: 36 * MIN,
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
            active_ms: 6 * MIN,
            wall_ms: 6 * MIN,
        }
    );
}

#[test]
fn a_single_event_is_a_session_without_active_or_wall_time() {
    assert_eq!(
        activity(&[(9, at(10))], T),
        Activity {
            sessions: 1,
            active_ms: 0,
            wall_ms: 0,
        }
    );
    assert_eq!(activity(&[], T), Activity::default());
}

#[test]
fn periods_split_intervals_at_their_boundaries_and_add_up() {
    // Periods of one hour. Session 1 runs 50 → 70 minutes, across the boundary at 60;
    // session 2 has one event at 130 minutes, in the third period.
    let events = [
        (1, at(50)),
        (1, at(54)),
        (1, at(58)),
        (1, at(70)),
        (2, at(130)),
    ];
    let starts = [at(0), at(60), at(120)];
    let periods = activity_by_period(&events, T, &starts, at(180));
    assert_eq!(
        periods,
        [
            Activity {
                sessions: 1,
                active_ms: 10 * MIN,
                wall_ms: 10 * MIN,
            },
            Activity {
                sessions: 1,
                active_ms: 3 * MIN,
                wall_ms: 10 * MIN,
            },
            Activity {
                sessions: 1,
                active_ms: 0,
                wall_ms: 0,
            },
        ]
    );
    // The periods' times add up to the whole; sessions count once per period they touch.
    let whole = activity(&events, T);
    assert_eq!(
        periods.iter().map(|p| p.active_ms).sum::<i64>(),
        whole.active_ms
    );
    assert_eq!(
        periods.iter().map(|p| p.wall_ms).sum::<i64>(),
        whole.wall_ms
    );
}

#[test]
fn session_lengths_and_their_bins() {
    let events = [(1, at(0)), (1, at(2)), (1, at(32)), (2, at(5))];
    let lengths = session_lengths(&events, T);
    assert_eq!(
        lengths
            .iter()
            .map(|l| (l.session, l.active_ms, l.wall_ms))
            .collect::<Vec<_>>(),
        [(1, 7 * MIN, 32 * MIN), (2, 0, 0)]
    );
    assert_eq!(duration_bin(0), 0);
    assert_eq!(duration_bin(59_999), 0);
    assert_eq!(duration_bin(MIN), 1);
    assert_eq!(duration_bin(32 * MIN), 4);
    assert_eq!(duration_bin(10 * 3_600_000), DURATION_BOUNDS_MS.len() - 1);
}
