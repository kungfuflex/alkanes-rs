pub mod constants;
pub mod errors;
pub mod exports;
pub mod executor;
pub mod extcall;
pub mod fuel;
pub mod host_functions;
pub mod host_functions_safe;
pub mod instance;
pub mod memory;
pub mod runtime;
pub mod state;
pub mod state_safe;
pub mod utils;
pub mod validation;

// Re-export the safe interface
pub use executor::AlkanesExecutor;
pub use errors::{IndexerError, IndexerResult};
pub use state_safe::AlkanesStateSafe;
pub use validation::ValidationLayer;

use self::constants::*;
use self::exports::*;
use self::extcall::*;
use self::host_functions::*;
use self::instance::*;
use self::runtime::*;
use self::state::*;
use self::utils::*;
