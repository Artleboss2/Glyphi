//! Token accounting.
//!
//! The four categories are kept apart on purpose. Cached input and cache creation
//! are billed differently from fresh input, reasoning tokens are reported by some
//! providers as part of the output and by others separately, and a total that
//! merely adds everything is not enough to price a day.

use serde::{Deserialize, Serialize};

/// What one request (or one day) consumed, in the provider's own categories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
}

impl TokenCounts {
    pub const ZERO: Self = Self {
        input_tokens: 0,
        cached_input_tokens: 0,
        cache_creation_input_tokens: 0,
        output_tokens: 0,
    };

    /// Everything the model read or wrote, whatever the cache did.
    pub fn total(&self) -> u64 {
        self.input_tokens
            + self.cached_input_tokens
            + self.cache_creation_input_tokens
            + self.output_tokens
    }

    /// True when nothing was reported. A zero-token request is not a measurement,
    /// so callers must not turn this into "the user spent nothing".
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            input_tokens: self.input_tokens.saturating_add(other.input_tokens),
            cached_input_tokens: self
                .cached_input_tokens
                .saturating_add(other.cached_input_tokens),
            cache_creation_input_tokens: self
                .cache_creation_input_tokens
                .saturating_add(other.cache_creation_input_tokens),
            output_tokens: self.output_tokens.saturating_add(other.output_tokens),
        }
    }
}

/// A day's display totals: the token counts, how many requests produced them and
/// what they are believed to have cost.
///
/// The cost is held in integer micro-dollars. Floating point is fine for a single
/// estimate and wrong for a sum of ten thousand of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    #[serde(flatten)]
    pub tokens: TokenCounts,
    pub requests: u64,
    /// Tokens no price was known for; they are counted but not priced.
    pub unpriced_tokens: u64,
    pub estimated_cost_micros_usd: Option<i64>,
}

impl Default for UsageTotals {
    fn default() -> Self {
        Self {
            tokens: TokenCounts::ZERO,
            requests: 0,
            unpriced_tokens: 0,
            estimated_cost_micros_usd: None,
        }
    }
}

impl UsageTotals {
    pub fn of(tokens: TokenCounts, requests: u64) -> Self {
        Self {
            tokens,
            requests,
            ..Self::default()
        }
    }

    /// Adds two totals. A cost stays `None` only while neither side has one: an
    /// unpriced day added to a priced one must not silently become free.
    pub fn saturating_add(self, other: Self) -> Self {
        let cost = match (self.estimated_cost_micros_usd, other.estimated_cost_micros_usd) {
            (None, None) => None,
            (left, right) => Some(left.unwrap_or(0).saturating_add(right.unwrap_or(0))),
        };
        Self {
            tokens: self.tokens.saturating_add(other.tokens),
            requests: self.requests.saturating_add(other.requests),
            unpriced_tokens: self
                .unpriced_tokens
                .saturating_add(other.unpriced_tokens),
            estimated_cost_micros_usd: cost,
        }
    }

    /// The dollars a screen shows. Rounded at the last moment, never accumulated.
    pub fn estimated_cost_usd(&self) -> Option<f64> {
        self.estimated_cost_micros_usd
            .map(|micros| micros as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_counts_every_category() {
        let counts = TokenCounts {
            input_tokens: 10,
            cached_input_tokens: 20,
            cache_creation_input_tokens: 5,
            output_tokens: 7,
        };
        assert_eq!(counts.total(), 42);
        assert!(!counts.is_empty());
        assert!(TokenCounts::ZERO.is_empty());
    }

    #[test]
    fn adding_totals_keeps_a_cost_only_when_one_side_has_it() {
        let priced = UsageTotals {
            estimated_cost_micros_usd: Some(2_500_000),
            ..UsageTotals::of(TokenCounts::ZERO, 1)
        };
        let unpriced = UsageTotals::of(TokenCounts::ZERO, 1);
        assert_eq!(
            priced.saturating_add(unpriced).estimated_cost_micros_usd,
            Some(2_500_000)
        );
        assert_eq!(unpriced.saturating_add(unpriced).estimated_cost_micros_usd, None);
        assert_eq!(priced.estimated_cost_usd(), Some(2.5));
    }
}
