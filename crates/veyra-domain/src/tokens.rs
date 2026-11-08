//! Canonical token semantics (ADR-005).
//!
//! Vendors define token fields differently (OpenAI's `input` includes cached tokens;
//! Anthropic's does not; reasoning is part of output). The normaliser converts every
//! source into these canonical fields. `None` always means "not reported", never zero.

/// Token counts of one usage event in canonical form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TokenCounts {
    /// Prompt tokens billed at the normal input rate.
    pub input_uncached: Option<u64>,
    /// Prompt tokens served from cache.
    pub cache_read: Option<u64>,
    /// Tokens written to the provider's short-lived cache (Anthropic: 5 minutes;
    /// OpenAI: 30 minutes).
    pub cache_write_short: Option<u64>,
    /// Tokens written to the provider's long-lived cache (Anthropic: 1 hour;
    /// OpenAI: 12 hours).
    pub cache_write_long: Option<u64>,
    /// Cache writes whose TTL the source does not report.
    pub cache_write_unspecified: Option<u64>,
    /// All generated tokens, **including** reasoning.
    pub output_total: Option<u64>,
    /// Reasoning tokens: a reported subset of `output_total`, never added again.
    pub reasoning: Option<u64>,
    /// Tool-related tokens where a source reports them separately.
    pub tool_tokens: Option<u64>,
}

/// Error raised when token counts violate canonical invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("reasoning tokens ({reasoning}) exceed output tokens ({output})")]
    ReasoningExceedsOutput { reasoning: u64, output: u64 },
}

impl TokenCounts {
    /// Checks the canonical invariants.
    ///
    /// # Errors
    /// Returns [`TokenError`] when reasoning exceeds total output.
    pub fn validate(&self) -> Result<(), TokenError> {
        if let (Some(reasoning), Some(output)) = (self.reasoning, self.output_total)
            && reasoning > output
        {
            return Err(TokenError::ReasoningExceedsOutput { reasoning, output });
        }
        Ok(())
    }

    /// All cache-write tokens regardless of TTL, or `None` if none were reported.
    #[must_use]
    pub fn cache_write_total(&self) -> Option<u64> {
        sum_reported(&[
            self.cache_write_short,
            self.cache_write_long,
            self.cache_write_unspecified,
        ])
    }

    /// The prompt size of the request (uncached + cache read + cache write), used for
    /// prompt-length price tiers. `None` if no input component was reported.
    #[must_use]
    pub fn prompt_tokens(&self) -> Option<u64> {
        sum_reported(&[
            self.input_uncached,
            self.cache_read,
            self.cache_write_total(),
        ])
    }

    /// Total tokens: prompt plus output. `None` if nothing was reported.
    #[must_use]
    pub fn total(&self) -> Option<u64> {
        sum_reported(&[self.prompt_tokens(), self.output_total])
    }

    /// Whether no token field was reported at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total().is_none() && self.reasoning.is_none() && self.tool_tokens.is_none()
    }
}

/// Sums the reported values; `None` only when every value is unreported.
fn sum_reported(values: &[Option<u64>]) -> Option<u64> {
    values
        .iter()
        .flatten()
        .fold(None, |acc, v| Some(acc.unwrap_or(0).saturating_add(*v)))
}

#[cfg(test)]
mod tests {
    use super::{TokenCounts, TokenError};

    #[test]
    fn unreported_fields_stay_unreported() {
        let tokens = TokenCounts::default();
        assert!(tokens.is_empty());
        assert_eq!(tokens.prompt_tokens(), None);
        assert_eq!(tokens.total(), None);
    }

    #[test]
    fn prompt_includes_all_input_components() {
        let tokens = TokenCounts {
            input_uncached: Some(100),
            cache_read: Some(1_000),
            cache_write_short: Some(10),
            cache_write_long: Some(20),
            cache_write_unspecified: Some(5),
            output_total: Some(50),
            ..TokenCounts::default()
        };
        assert_eq!(tokens.cache_write_total(), Some(35));
        assert_eq!(tokens.prompt_tokens(), Some(1_135));
        assert_eq!(tokens.total(), Some(1_185));
    }

    #[test]
    fn reasoning_is_a_subset_of_output() {
        let tokens = TokenCounts {
            output_total: Some(10),
            reasoning: Some(11),
            ..TokenCounts::default()
        };
        assert_eq!(
            tokens.validate(),
            Err(TokenError::ReasoningExceedsOutput {
                reasoning: 11,
                output: 10
            })
        );
        let ok = TokenCounts {
            reasoning: Some(10),
            ..tokens
        };
        assert_eq!(ok.validate(), Ok(()));
        assert_eq!(
            ok.total(),
            Some(10),
            "reasoning is never added on top of output"
        );
    }
}
