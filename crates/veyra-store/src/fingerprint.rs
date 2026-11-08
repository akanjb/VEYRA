//! A stable content fingerprint, so re-ingesting an unchanged event is a no-op.
//!
//! Uses FNV-1a 64 rather than the standard library's hasher, whose output may change
//! between Rust releases. A change in the derived `Hash` layout would only cause a
//! one-time rewrite of identical data, never incorrect results.

use std::hash::{Hash, Hasher};

use veyra_domain::event::UsageEvent;
use veyra_domain::pricing::EventCost;

struct Fnv1a(u64);

impl Hasher for Fnv1a {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// Fingerprint of an event and its cost, as a SQLite-storable integer.
pub fn fingerprint(event: &UsageEvent, cost: &EventCost) -> i64 {
    let mut hasher = Fnv1a(0xcbf2_9ce4_8422_2325);
    event.hash(&mut hasher);
    cost.hash(&mut hasher);
    i64::from_ne_bytes(hasher.finish().to_ne_bytes())
}
