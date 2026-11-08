//! Money and rates as exact integers (ADR-004), and the three cost kinds that must
//! never be added together.

use std::iter::Sum;
use std::ops::Add;

/// An amount in billionths of a US dollar. `i64` covers ±$9.2 billion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NanoUsd(pub i64);

impl NanoUsd {
    pub const ZERO: Self = Self(0);
}

impl Add for NanoUsd {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sum for NanoUsd {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

/// Error raised when a decimal price or multiplier cannot be represented exactly.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("`{value}` must be a finite, non-negative number with at most {max_decimals} decimals")]
pub struct DecimalError {
    value: f64,
    max_decimals: u32,
}

/// Converts a decimal to an integer of `10^decimals` units, rejecting any value that
/// would lose precision.
fn to_scaled_integer(value: f64, decimals: u32) -> Result<u64, DecimalError> {
    let error = DecimalError {
        value,
        max_decimals: decimals,
    };
    if !value.is_finite() || value < 0.0 {
        return Err(error);
    }
    let scaled = value * 10f64.powi(i32::try_from(decimals).map_err(|_| error)?);
    let rounded = scaled.round();
    if (scaled - rounded).abs() > 1e-6 || rounded > 9.0e15 {
        return Err(error);
    }
    // Exact: `rounded` is a non-negative integer below 2^53.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked above"
    )]
    Ok(rounded as u64)
}

/// A price in millionths of a US dollar per one million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MicroUsdPerMTok(pub u64);

impl MicroUsdPerMTok {
    /// Converts a published "USD per 1M tokens" price, e.g. `2.50`.
    ///
    /// # Errors
    /// Returns [`DecimalError`] for negative values or more than 6 decimals.
    pub fn from_usd_per_mtok(usd: f64) -> Result<Self, DecimalError> {
        to_scaled_integer(usd, 6).map(Self)
    }
}

/// A price multiplier in basis points: `10_000` is 1.0×, `5_000` is 0.5×.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Multiplier(pub u32);

impl Multiplier {
    pub const ONE: Self = Self(10_000);

    /// Converts a factor such as `0.5` or `1.1`.
    ///
    /// # Errors
    /// Returns [`DecimalError`] for negative values or more than 4 decimals.
    pub fn from_factor(factor: f64) -> Result<Self, DecimalError> {
        let bps = to_scaled_integer(factor, 4)?;
        u32::try_from(bps).map(Self).map_err(|_| DecimalError {
            value: factor,
            max_decimals: 4,
        })
    }

    /// Combines two multipliers exactly (rounded half-even to the nearest basis point).
    #[must_use]
    pub fn then(self, other: Self) -> Self {
        let product = u64::from(self.0) * u64::from(other.0);
        let combined = div_round_half_even(u128::from(product), 10_000);
        Self(u32::try_from(combined).unwrap_or(u32::MAX))
    }
}

/// Integer division rounded half to even (banker's rounding).
#[must_use]
pub fn div_round_half_even(numerator: u128, denominator: u128) -> u128 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let twice = remainder * 2;
    if twice > denominator || (twice == denominator && quotient % 2 == 1) {
        quotient + 1
    } else {
        quotient
    }
}

macro_rules! cost_kind {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NanoUsd);

        impl $name {
            #[must_use]
            pub fn new(amount: NanoUsd) -> Self {
                Self(amount)
            }

            #[must_use]
            pub fn amount(self) -> NanoUsd {
                self.0
            }
        }

        impl Add for $name {
            type Output = Self;

            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl Sum for $name {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                Self(iter.map(Self::amount).sum())
            }
        }
    };
}

cost_kind!(
    /// What recorded token usage would cost at published API rates. An estimate, never
    /// a bill.
    ///
    /// Cost kinds cannot be added to each other (ADR-004):
    ///
    /// ```compile_fail,E0308
    /// use veyra_domain::money::{ActualCharge, ApiEquivalentCost, NanoUsd};
    /// let _ = ApiEquivalentCost::new(NanoUsd(1)) + ActualCharge::new(NanoUsd(1));
    /// ```
    ///
    /// ```compile_fail,E0308
    /// use veyra_domain::money::{ApiEquivalentCost, NanoUsd, SubscriptionSpend};
    /// let _ = ApiEquivalentCost::new(NanoUsd(1)) + SubscriptionSpend::new(NanoUsd(1));
    /// ```
    ///
    /// The same kind adds normally:
    ///
    /// ```
    /// use veyra_domain::money::{ApiEquivalentCost, NanoUsd};
    /// let total = ApiEquivalentCost::new(NanoUsd(1)) + ApiEquivalentCost::new(NanoUsd(2));
    /// assert_eq!(total.amount(), NanoUsd(3));
    /// ```
    ApiEquivalentCost
);
cost_kind!(
    /// A charge confirmed by verified billing information.
    ActualCharge
);
cost_kind!(
    /// A subscription payment, entered manually or reported by a provider.
    SubscriptionSpend
);

#[cfg(test)]
mod tests {
    use super::{MicroUsdPerMTok, Multiplier, div_round_half_even};

    #[test]
    fn converts_published_prices_exactly() {
        assert_eq!(
            MicroUsdPerMTok::from_usd_per_mtok(2.5),
            Ok(MicroUsdPerMTok(2_500_000))
        );
        assert_eq!(
            MicroUsdPerMTok::from_usd_per_mtok(0.01),
            Ok(MicroUsdPerMTok(10_000))
        );
        assert_eq!(
            MicroUsdPerMTok::from_usd_per_mtok(0.075),
            Ok(MicroUsdPerMTok(75_000))
        );
        assert!(MicroUsdPerMTok::from_usd_per_mtok(-1.0).is_err());
        assert!(MicroUsdPerMTok::from_usd_per_mtok(0.000_000_1).is_err());
        assert!(MicroUsdPerMTok::from_usd_per_mtok(f64::NAN).is_err());
    }

    #[test]
    fn multipliers_are_basis_points() {
        assert_eq!(Multiplier::from_factor(0.5), Ok(Multiplier(5_000)));
        assert_eq!(Multiplier::from_factor(1.1), Ok(Multiplier(11_000)));
        assert_eq!(
            Multiplier(5_000).then(Multiplier(11_000)),
            Multiplier(5_500)
        );
        assert_eq!(Multiplier::ONE.then(Multiplier::ONE), Multiplier::ONE);
    }

    #[test]
    fn rounds_half_to_even() {
        assert_eq!(div_round_half_even(15, 10), 2);
        assert_eq!(div_round_half_even(25, 10), 2);
        assert_eq!(div_round_half_even(26, 10), 3);
        assert_eq!(div_round_half_even(24, 10), 2);
        assert_eq!(div_round_half_even(0, 10), 0);
    }
}
