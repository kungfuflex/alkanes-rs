pub mod backend;
pub mod extractor;
pub mod pipeline;
pub mod tracker;
pub mod types;
pub mod query;
pub mod trackers;
pub mod schema;

// Re-export core types
pub use backend::{StorageBackend, InMemoryBackend};
pub use extractor::TraceExtractor;
pub use pipeline::TransformPipeline;
pub use tracker::StateTracker;
pub use types::{TraceEvent, QueryParams, QueryFilter};
pub use query::QueryService;

// Re-export trackers
pub use trackers::balance::{ValueTransferExtractor, BalanceTracker, BalanceChange, AddressBalance};
pub use trackers::amm::{TradeEventExtractor, AmmTracker, TradeEvent, ReserveSnapshot, Candle};

#[cfg(feature = "postgres")]
pub use trackers::optimized_balance::{OptimizedBalanceTracker, OptimizedBalanceProcessor};

#[cfg(feature = "postgres")]
pub use trackers::optimized_amm::OptimizedAmmTracker;

#[cfg(feature = "postgres")]
pub use backend::PostgresBackend;
