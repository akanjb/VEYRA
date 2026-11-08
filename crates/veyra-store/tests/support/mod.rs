//! Shared helpers for store integration tests: temporary databases, synthetic events and
//! a deterministic random generator (no external test dependencies).

#![allow(dead_code, reason = "each test binary uses a different subset")]
#![allow(
    clippy::unwrap_used,
    reason = "test support: a failure aborts the test"
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::event::{
    Attribution, DedupKey, EventContext, Granularity, ServiceTier, SourceRef, UsageEvent,
};
use veyra_domain::identity::{AppId, ConnectorId, Interface, ModelId, ProviderId};
use veyra_domain::pricing::{PriceCatalog, price_event};
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;
use veyra_store::{PricedEvent, Store};

/// 2026-11-08T00:00:00Z
pub const NOV_8: i64 = 1_794_096_000_000;
pub const HOUR: i64 = 3_600_000;

/// A database in a fresh temporary directory, deleted on drop.
pub struct TempDb {
    dir: PathBuf,
    pub store: Store,
}

impl TempDb {
    pub fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "veyra-store-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir.join("veyra.db")).unwrap();
        Self { dir, store }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A Claude Sonnet 5.5 request with the given tokens.
pub fn event(key: &str, at: i64, tokens: TokenCounts) -> UsageEvent {
    UsageEvent {
        dedup_key: DedupKey::new(key).unwrap(),
        occurred_at: UtcMillis(at),
        source: SourceRef {
            connector: ConnectorId::new("test").unwrap(),
            parser_version: 1,
            tool_version: Some("1.0.0".to_owned()),
        },
        attribution: Attribution {
            provider: Some(ProviderId::new("anthropic").unwrap()),
            model: Some(ModelId::new("claude-sonnet-5-5").unwrap()),
            raw_model: Some("claude-sonnet-5-5".to_owned()),
            app: AppId::new("claude-code").unwrap(),
            interface: Interface::Cli,
            gateway: None,
        },
        context: EventContext {
            session_key: Some("claude-code:s1".to_owned()),
            parent_session_key: None,
            project_key: Some("/work/veyra".to_owned()),
            device_key: Some("laptop".to_owned()),
            is_subagent: false,
        },
        tokens,
        requests: Some(1),
        reported_cost: None,
        accuracy: AccuracyMap::uniform(Accuracy::Exact),
        granularity: Granularity::Request,
        service_tier: ServiceTier::Standard,
        inference_geo_us: false,
    }
}

pub fn tokens(input: u64, output: u64) -> TokenCounts {
    TokenCounts {
        input_uncached: Some(input),
        output_total: Some(output),
        ..TokenCounts::default()
    }
}

pub fn priced(event: UsageEvent, catalog: &PriceCatalog) -> PricedEvent {
    let cost = price_event(&event, catalog);
    PricedEvent { event, cost }
}

pub fn bundled_catalog() -> PriceCatalog {
    veyra_registry::Registry::bundled()
        .unwrap()
        .catalog()
        .clone()
}

/// xorshift64*: small, deterministic, good enough for synthetic data.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[usize::try_from(self.below(items.len() as u64)).unwrap()]
    }
}
