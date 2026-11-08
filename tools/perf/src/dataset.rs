//! Deterministic synthetic usage: a year of activity across tools, models, projects and
//! devices, with realistic skew (busy working hours, dominant models, heavy caching).

use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{
    Attribution, DedupKey, EventContext, Granularity, ServiceTier, SourceRef, UsageEvent,
};
use veyra_domain::identity::{AppId, ConnectorId, Interface, ModelId, ProviderId};
use veyra_domain::pricing::{PriceCatalog, price_event};
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;
use veyra_store::{PricedEvent, Store, StoreError};

/// Data ends at 2026-11-08T00:00Z and spans the preceding 365 days.
pub const END: UtcMillis = UtcMillis(1_794_096_000_000);
const SPAN_MINUTES: u64 = 365 * 24 * 60;
const BATCH: usize = 5_000;
/// Events per synthetic session.
const SESSION_EVENTS: usize = 40;

/// The order in which events reach the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Session by session, in time order: how log files are actually ingested.
    Chronological,
    /// Every event at a random time: the worst case for bucket rebuilds.
    Scattered,
}

/// (provider, model, weight): skewed towards a few dominant models.
const MODELS: [(&str, &str, u64); 8] = [
    ("anthropic", "claude-sonnet-5-5", 40),
    ("anthropic", "claude-opus-5-5", 15),
    ("anthropic", "claude-haiku-5-5", 8),
    ("openai", "gpt-6.1-sol", 15),
    ("openai", "gpt-5.3-codex", 7),
    ("google", "gemini-4-argon", 8),
    ("google", "gemini-3.8-flash", 5),
    ("openai", "unlisted-preview-model", 2),
];
const APPS: [(&str, Interface); 5] = [
    ("claude-code", Interface::Cli),
    ("claude-desktop", Interface::Desktop),
    ("codex", Interface::Cli),
    ("gemini-cli", Interface::Cli),
    ("opencode", Interface::Cli),
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Deterministic per-session attribute (splitmix64 of session and salt).
fn session_value(session: usize, salt: u64, below: u64) -> u64 {
    let mut z = (session as u64).wrapping_add(salt.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) % below
}

fn weighted_model(roll: u64) -> (&'static str, &'static str) {
    let total: u64 = MODELS.iter().map(|m| m.2).sum();
    let mut pick = roll % total;
    for (provider, model, weight) in MODELS {
        if pick < weight {
            return (provider, model);
        }
        pick -= weight;
    }
    (MODELS[0].0, MODELS[0].1)
}

/// Minutes before [`END`] at which event `index` of `count` occurs.
fn minutes_before_end(index: usize, count: usize, order: Order, rng: &mut Rng) -> u64 {
    match order {
        Order::Scattered => rng.below(SPAN_MINUTES),
        Order::Chronological => {
            let sessions = (count / SESSION_EVENTS).max(1) as u64;
            let session = (index / SESSION_EVENTS) as u64;
            let start = session * SPAN_MINUTES / sessions;
            let offset = (index % SESSION_EVENTS) as u64 * 2;
            SPAN_MINUTES.saturating_sub(start + offset + 1)
        }
    }
}

fn synthetic_event(
    index: usize,
    count: usize,
    order: Order,
    rng: &mut Rng,
) -> Result<UsageEvent, Box<dyn std::error::Error>> {
    // Sessions keep one application, project and device and mostly one model; 15% of
    // requests use another model (subagents, model switches), as in real usage.
    let session = index / SESSION_EVENTS;
    let roll = if rng.below(100) < 15 {
        rng.next()
    } else {
        session_value(session, 1, u64::MAX)
    };
    let (provider, model) = weighted_model(roll);
    let (app, interface) = APPS[usize::try_from(session_value(session, 2, APPS.len() as u64))?];
    let minute = i64::try_from(minutes_before_end(index, count, order, rng))?;
    let context_size = 2_000 + rng.below(150_000);
    let cached = context_size * rng.below(95) / 100;
    let output = 50 + rng.below(4_000);
    Ok(UsageEvent {
        dedup_key: DedupKey::new(format!("perf:{index}"))?,
        occurred_at: UtcMillis(END.0 - minute * 60_000),
        source: SourceRef {
            connector: ConnectorId::new("synthetic")?,
            parser_version: 1,
            tool_version: None,
        },
        attribution: Attribution {
            provider: Some(ProviderId::new(provider)?),
            model: Some(ModelId::new(model)?),
            raw_model: Some(model.to_owned()),
            app: AppId::new(app)?,
            interface,
            gateway: None,
        },
        context: EventContext {
            session_key: Some(format!("perf:s{}", index / SESSION_EVENTS)),
            parent_session_key: None,
            project_key: Some(format!("/work/project-{}", session_value(session, 3, 40))),
            device_key: Some(format!("device-{}", session_value(session, 4, 3))),
            is_subagent: rng.below(10) == 0,
        },
        tokens: TokenCounts {
            input_uncached: Some(context_size - cached),
            cache_read: Some(cached),
            cache_write_short: Some(rng.below(3_000)),
            cache_write_long: None,
            cache_write_unspecified: None,
            output_total: Some(output),
            reasoning: Some(output / 4),
            tool_tokens: None,
        },
        requests: Some(1),
        reported_cost: None,
        accuracy: AccuracyMap::uniform(if rng.below(20) == 0 {
            Accuracy::Estimated
        } else {
            Accuracy::Exact
        }),
        granularity: Granularity::Request,
        service_tier: ServiceTier::Standard,
        inference_geo_us: false,
    })
}

/// Ingests `count` events in batches. Returns `(events written, buckets rebuilt)`.
pub fn ingest(
    store: &Store,
    catalog: &PriceCatalog,
    count: usize,
    order: Order,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let mut rng = Rng(0x00ef_1c1e_a7ed_5eed);
    let mut written = 0;
    let mut buckets = 0;
    let mut batch = Vec::with_capacity(BATCH);
    for index in 0..count {
        let event = synthetic_event(index, count, order, &mut rng)?;
        let cost = price_event(&event, catalog);
        batch.push(PricedEvent { event, cost });
        if batch.len() == BATCH || index + 1 == count {
            let summary = store
                .write_events(&batch, END)
                .map_err(|e: StoreError| e.to_string())?;
            written += summary.inserted;
            buckets += summary.buckets_rebuilt;
            batch.clear();
        }
    }
    Ok((written, buckets))
}
