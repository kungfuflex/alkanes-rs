//! Amount type for satoshi values.
//! Maps to: src/consensus/amount.h
//!
//! Consensus-critical constants and validation.

use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

/// Number of satoshis per BTC (1 BTC = 100,000,000 satoshis).
///
/// Equivalent to `COIN` in Bitcoin Core's `consensus/amount.h`.
pub const COIN: i64 = 100_000_000;

/// Maximum valid amount in satoshis (consensus-critical).
///
/// 21 million BTC = 2,100,000,000,000,000 satoshis. No valid transaction output
/// may exceed this value. Equivalent to `MAX_MONEY` in Bitcoin Core.
pub const MAX_MONEY: i64 = 21_000_000 * COIN;

/// Amount in satoshis. Can be negative (for representing fee deltas, etc.).
///
/// This is a newtype around i64, matching Bitcoin Core's `CAmount` typedef.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Amount(i64);

impl Amount {
    /// Zero satoshis.
    pub const ZERO: Amount = Amount(0);
    /// One satoshi (the smallest indivisible unit).
    pub const ONE_SAT: Amount = Amount(1);
    /// One BTC (100,000,000 satoshis).
    pub const ONE_BTC: Amount = Amount(COIN);
    /// The maximum valid monetary amount (21 million BTC).
    pub const MAX: Amount = Amount(MAX_MONEY);

    /// Create from satoshis.
    pub const fn from_sat(satoshis: i64) -> Self {
        Amount(satoshis)
    }

    /// Create from BTC (whole number).
    pub const fn from_btc(btc: i64) -> Self {
        Amount(btc * COIN)
    }

    /// Get raw satoshi value.
    pub const fn to_sat(self) -> i64 {
        self.0
    }

    /// Converts to BTC as `f64` (for display purposes only, not for consensus calculations).
    pub fn to_btc(self) -> f64 {
        self.0 as f64 / COIN as f64
    }

    /// Returns `true` if this amount is in the valid consensus range `[0, MAX_MONEY]`.
    pub fn in_money_range(self) -> bool {
        self.0 >= 0 && self.0 <= MAX_MONEY
    }
}

/// Returns `true` if the raw satoshi `value` is in the valid money range `[0, MAX_MONEY]`.
///
/// Direct port of Bitcoin Core's `MoneyRange()` function.
pub fn money_range(value: i64) -> bool {
    value >= 0 && value <= MAX_MONEY
}

impl Add for Amount {
    type Output = Amount;
    fn add(self, rhs: Amount) -> Amount {
        Amount(self.0 + rhs.0)
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Amount) {
        self.0 += rhs.0;
    }
}

impl Sub for Amount {
    type Output = Amount;
    fn sub(self, rhs: Amount) -> Amount {
        Amount(self.0 - rhs.0)
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Amount) {
        self.0 -= rhs.0;
    }
}

impl Neg for Amount {
    type Output = Amount;
    fn neg(self) -> Amount {
        Amount(-self.0)
    }
}

impl From<i64> for Amount {
    fn from(sat: i64) -> Self {
        Amount(sat)
    }
}

impl From<Amount> for i64 {
    fn from(amount: Amount) -> i64 {
        amount.0
    }
}

impl fmt::Debug for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Amount({} sat)", self.0)
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let btc = self.0 / COIN;
        let sat = (self.0 % COIN).abs();
        if self.0 < 0 {
            write!(f, "-{}.{:08} BTC", btc.abs(), sat)
        } else {
            write!(f, "{}.{:08} BTC", btc, sat)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(COIN, 100_000_000);
        assert_eq!(MAX_MONEY, 2_100_000_000_000_000);
    }

    #[test]
    fn test_money_range() {
        assert!(money_range(0));
        assert!(money_range(1));
        assert!(money_range(MAX_MONEY));
        assert!(!money_range(-1));
        assert!(!money_range(MAX_MONEY + 1));
    }

    #[test]
    fn test_amount_from_btc() {
        let a = Amount::from_btc(1);
        assert_eq!(a.to_sat(), COIN);
    }

    #[test]
    fn test_amount_arithmetic() {
        let a = Amount::from_sat(100);
        let b = Amount::from_sat(200);
        assert_eq!((a + b).to_sat(), 300);
        assert_eq!((b - a).to_sat(), 100);
        assert_eq!((-a).to_sat(), -100);
    }

    #[test]
    fn test_amount_display() {
        let a = Amount::from_sat(123456789);
        assert_eq!(format!("{}", a), "1.23456789 BTC");
    }

    #[test]
    fn test_amount_in_range() {
        assert!(Amount::ZERO.in_money_range());
        assert!(Amount::MAX.in_money_range());
        assert!(!Amount::from_sat(-1).in_money_range());
        assert!(!Amount::from_sat(MAX_MONEY + 1).in_money_range());
    }
}
