pub mod balance;
pub mod amm;

#[cfg(feature = "postgres")]
pub mod optimized_balance;

#[cfg(feature = "postgres")]
pub mod optimized_amm;

pub use balance::{ValueTransferExtractor, BalanceTracker, BalanceChange, AddressBalance};
pub use amm::{TradeEventExtractor, AmmTracker, TradeEvent, ReserveSnapshot, Candle};

#[cfg(feature = "postgres")]
pub use optimized_balance::{OptimizedBalanceTracker, OptimizedBalanceProcessor};

#[cfg(feature = "postgres")]
pub use optimized_amm::OptimizedAmmTracker;
