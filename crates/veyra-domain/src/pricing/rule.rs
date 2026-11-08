//! A single price rule: which models it covers, when it applies, and its rates.

use crate::event::ServiceTier;
use crate::identity::{ModelId, ProviderId};
use crate::money::{MicroUsdPerMTok, Multiplier};
use crate::time::{CivilDate, UtcMillis};

/// Which models a rule applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelMatch {
    Exact(ModelId),
    /// Any canonical model id starting with this prefix (e.g. dated snapshots).
    Prefix(String),
}

impl ModelMatch {
    #[must_use]
    pub fn matches(&self, model: &ModelId) -> bool {
        match self {
            Self::Exact(id) => id == model,
            Self::Prefix(prefix) => model.as_str().starts_with(prefix.as_str()),
        }
    }

    /// Higher is more specific: exact beats any prefix; longer prefixes beat shorter.
    #[must_use]
    pub fn specificity(&self) -> (u8, usize) {
        match self {
            Self::Exact(id) => (1, id.as_str().len()),
            Self::Prefix(prefix) => (0, prefix.len()),
        }
    }
}

/// The half-open interval `[from, until)` in which a rule applies. `None` bounds are
/// open-ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveRange {
    pub from: Option<UtcMillis>,
    pub until: Option<UtcMillis>,
}

impl EffectiveRange {
    #[must_use]
    pub fn contains(&self, at: UtcMillis) -> bool {
        self.from.is_none_or(|from| at >= from) && self.until.is_none_or(|until| at < until)
    }

    /// Whether two ranges share at least one instant.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        let starts_before_other_ends = match (self.from, other.until) {
            (Some(from), Some(until)) => from < until,
            _ => true,
        };
        let other_starts_before_self_ends = match (other.from, self.until) {
            (Some(from), Some(until)) => from < until,
            _ => true,
        };
        starts_before_other_ends && other_starts_before_self_ends
    }
}

/// Rates for each priced token component. Missing optional rates make the affected
/// components unpriceable (the cost becomes `Partial`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateCard {
    pub input: MicroUsdPerMTok,
    pub output: MicroUsdPerMTok,
    pub cache_read: Option<MicroUsdPerMTok>,
    pub cache_write_short: Option<MicroUsdPerMTok>,
    pub cache_write_long: Option<MicroUsdPerMTok>,
    /// Only for models that bill reasoning separately from output.
    pub reasoning: Option<MicroUsdPerMTok>,
}

/// Rates that replace the base rates for requests whose prompt exceeds a size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptTier {
    /// The tier applies when the request's prompt tokens are strictly greater.
    pub above_prompt_tokens: u64,
    pub rates: RateCard,
}

/// How a service tier changes prices relative to standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierFactor {
    /// Applies to input, output and cache-write rates.
    pub factor: Multiplier,
    /// Cache-read factor when it differs from `factor`; e.g. Google keeps cache reads at
    /// the standard price for batch on some models (`ONE`). `None` means `factor`.
    pub cache_read: Option<Multiplier>,
}

impl TierFactor {
    pub const STANDARD: Self = Self {
        factor: Multiplier::ONE,
        cache_read: None,
    };

    #[must_use]
    pub fn cache_read_factor(self) -> Multiplier {
        self.cache_read.unwrap_or(self.factor)
    }
}

/// Multipliers applied only when the event records the matching condition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub batch: Option<TierFactor>,
    pub flex: Option<TierFactor>,
    pub priority: Option<TierFactor>,
    pub fast: Option<TierFactor>,
    pub ultrafast: Option<TierFactor>,
    pub inference_geo_us: Option<Multiplier>,
}

impl Modifiers {
    /// The factor for a service tier: standard always exists; `None` when the rule does
    /// not publish a price for that tier.
    #[must_use]
    pub fn for_tier(&self, tier: ServiceTier) -> Option<TierFactor> {
        match tier {
            ServiceTier::Standard => Some(TierFactor::STANDARD),
            ServiceTier::Batch => self.batch,
            ServiceTier::Flex => self.flex,
            ServiceTier::Priority => self.priority,
            ServiceTier::Fast => self.fast,
            ServiceTier::Ultrafast => self.ultrafast,
        }
    }
}

/// A published price, with its provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceRule {
    /// Stable identifier, e.g. `anthropic/claude-sonnet-5-5/2026-11`.
    pub id: String,
    pub provider: ProviderId,
    pub model: ModelMatch,
    pub effective: EffectiveRange,
    pub base: RateCard,
    /// Sorted by `above_prompt_tokens`, ascending.
    pub tiers: Vec<PromptTier>,
    pub modifiers: Modifiers,
    /// The official page the rates were taken from.
    pub source_url: String,
    /// When a maintainer last confirmed the rates against `source_url`.
    pub verified_at: CivilDate,
    /// True when the rule has no verified start date and is assumed to have applied
    /// unchanged before `verified_at`. Shown in the assumptions panel.
    pub history_assumed: bool,
}

impl PriceRule {
    /// The rates for a request with the given prompt size. Unknown size uses the base.
    #[must_use]
    pub fn rates_for(&self, prompt_tokens: Option<u64>) -> &RateCard {
        prompt_tokens
            .and_then(|size| {
                self.tiers
                    .iter()
                    .rev()
                    .find(|tier| size > tier.above_prompt_tokens)
            })
            .map_or(&self.base, |tier| &tier.rates)
    }
}

#[cfg(test)]
mod tests {
    use super::{EffectiveRange, ModelMatch};
    use crate::identity::ModelId;
    use crate::time::UtcMillis;

    fn range(from: Option<i64>, until: Option<i64>) -> EffectiveRange {
        EffectiveRange {
            from: from.map(UtcMillis),
            until: until.map(UtcMillis),
        }
    }

    #[test]
    fn ranges_are_half_open() {
        let r = range(Some(10), Some(20));
        assert!(!r.contains(UtcMillis(9)));
        assert!(r.contains(UtcMillis(10)));
        assert!(r.contains(UtcMillis(19)));
        assert!(!r.contains(UtcMillis(20)));
        assert!(range(None, None).contains(UtcMillis(i64::MIN)));
    }

    #[test]
    fn detects_overlap() {
        assert!(!range(Some(0), Some(10)).overlaps(&range(Some(10), None)));
        assert!(range(Some(0), Some(11)).overlaps(&range(Some(10), None)));
        assert!(range(None, Some(5)).overlaps(&range(None, None)));
        assert!(!range(None, Some(5)).overlaps(&range(Some(5), Some(9))));
    }

    #[test]
    fn exact_match_is_more_specific_than_prefix() {
        let model = ModelId::new("claude-sonnet-5-5").unwrap();
        let exact = ModelMatch::Exact(model.clone());
        let prefix = ModelMatch::Prefix("claude-sonnet".to_owned());
        assert!(exact.matches(&model) && prefix.matches(&model));
        assert!(exact.specificity() > prefix.specificity());
    }
}
