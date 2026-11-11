//! A validated set of price rules with a version label.

use std::collections::HashSet;

use super::rule::PriceRule;
use crate::identity::ModelId;
use crate::time::UtcMillis;

/// Error raised when a catalogue is internally inconsistent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    #[error("duplicate price rule id `{0}`")]
    DuplicateId(String),
    #[error("rules `{0}` and `{1}` cover the same models over overlapping dates")]
    Overlap(String, String),
    #[error("rule `{0}`: prompt tiers must be strictly ascending")]
    TiersNotAscending(String),
    #[error("rule `{0}`: effective range is empty")]
    EmptyRange(String),
}

/// An immutable, validated price catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceCatalog {
    version: String,
    rules: Vec<PriceRule>,
}

impl PriceCatalog {
    /// Validates and creates a catalogue.
    ///
    /// # Errors
    /// Returns [`CatalogError`] for duplicate ids, empty ranges, unsorted tiers, or two
    /// rules with the same model match whose dates overlap.
    pub fn new(version: impl Into<String>, rules: Vec<PriceRule>) -> Result<Self, CatalogError> {
        let mut ids = HashSet::new();
        for rule in &rules {
            if !ids.insert(rule.id.as_str()) {
                return Err(CatalogError::DuplicateId(rule.id.clone()));
            }
            if let (Some(from), Some(until)) = (rule.effective.from, rule.effective.until)
                && from >= until
            {
                return Err(CatalogError::EmptyRange(rule.id.clone()));
            }
            let ascending = rule
                .tiers
                .windows(2)
                .all(|pair| pair[0].above_prompt_tokens < pair[1].above_prompt_tokens);
            if !ascending {
                return Err(CatalogError::TiersNotAscending(rule.id.clone()));
            }
        }
        for (i, a) in rules.iter().enumerate() {
            for b in &rules[i + 1..] {
                if a.model == b.model && a.effective.overlaps(&b.effective) {
                    return Err(CatalogError::Overlap(a.id.clone(), b.id.clone()));
                }
            }
        }
        Ok(Self {
            version: version.into(),
            rules,
        })
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn rules(&self) -> &[PriceRule] {
        &self.rules
    }

    /// The most specific rule covering `model` at instant `at`.
    /// The rule with this id.
    #[must_use]
    pub fn rule(&self, id: &str) -> Option<&PriceRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    #[must_use]
    pub fn resolve(&self, model: &ModelId, at: UtcMillis) -> Option<&PriceRule> {
        self.rules
            .iter()
            .filter(|rule| rule.model.matches(model) && rule.effective.contains(at))
            .max_by_key(|rule| rule.model.specificity())
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogError, PriceCatalog};
    use crate::identity::{ModelId, ProviderId};
    use crate::money::MicroUsdPerMTok;
    use crate::pricing::rule::{
        EffectiveRange, ModelMatch, Modifiers, PriceRule, PromptTier, RateCard,
    };
    use crate::time::{CivilDate, UtcMillis};

    fn rates(input: u64) -> RateCard {
        RateCard {
            input: MicroUsdPerMTok(input),
            output: MicroUsdPerMTok(input * 5),
            cache_read: None,
            cache_write_short: None,
            cache_write_long: None,
            reasoning: None,
        }
    }

    fn rule(id: &str, model: ModelMatch, from: Option<i64>, until: Option<i64>) -> PriceRule {
        PriceRule {
            id: id.to_owned(),
            provider: ProviderId::new("acme").unwrap(),
            model,
            effective: EffectiveRange {
                from: from.map(UtcMillis),
                until: until.map(UtcMillis),
            },
            base: rates(1),
            tiers: Vec::new(),
            modifiers: Modifiers::default(),
            source_url: "https://example.com/pricing".to_owned(),
            verified_at: CivilDate::new(2026, 11, 8).unwrap(),
            history_assumed: false,
        }
    }

    fn exact(model: &str) -> ModelMatch {
        ModelMatch::Exact(ModelId::new(model).unwrap())
    }

    #[test]
    fn resolves_by_date_and_specificity() {
        let catalog = PriceCatalog::new(
            "test",
            vec![
                rule("old", exact("m-1"), None, Some(100)),
                rule("new", exact("m-1"), Some(100), None),
                rule("family", ModelMatch::Prefix("m-".to_owned()), None, None),
            ],
        )
        .unwrap();
        let m1 = ModelId::new("m-1").unwrap();
        assert_eq!(
            catalog.resolve(&m1, UtcMillis(99)).map(|r| r.id.as_str()),
            Some("old")
        );
        assert_eq!(
            catalog.resolve(&m1, UtcMillis(100)).map(|r| r.id.as_str()),
            Some("new")
        );
        let m2 = ModelId::new("m-2").unwrap();
        assert_eq!(
            catalog.resolve(&m2, UtcMillis(0)).map(|r| r.id.as_str()),
            Some("family")
        );
        let other = ModelId::new("other").unwrap();
        assert!(catalog.resolve(&other, UtcMillis(0)).is_none());
    }

    #[test]
    fn rejects_overlapping_rules_for_the_same_models() {
        let result = PriceCatalog::new(
            "test",
            vec![
                rule("a", exact("m-1"), None, Some(101)),
                rule("b", exact("m-1"), Some(100), None),
            ],
        );
        assert_eq!(
            result,
            Err(CatalogError::Overlap("a".to_owned(), "b".to_owned()))
        );
    }

    #[test]
    fn rejects_duplicates_empty_ranges_and_unsorted_tiers() {
        assert!(matches!(
            PriceCatalog::new(
                "t",
                vec![
                    rule("a", exact("x"), None, None),
                    rule("a", exact("y"), None, None)
                ]
            ),
            Err(CatalogError::DuplicateId(_))
        ));
        assert!(matches!(
            PriceCatalog::new("t", vec![rule("a", exact("x"), Some(5), Some(5))]),
            Err(CatalogError::EmptyRange(_))
        ));
        let mut tiered = rule("t", exact("x"), None, None);
        tiered.tiers = vec![
            PromptTier {
                above_prompt_tokens: 200,
                rates: rates(2),
            },
            PromptTier {
                above_prompt_tokens: 100,
                rates: rates(3),
            },
        ];
        assert!(matches!(
            PriceCatalog::new("t", vec![tiered]),
            Err(CatalogError::TiersNotAscending(_))
        ));
    }
}
