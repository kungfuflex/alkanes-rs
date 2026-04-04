//! Validation state types for structured error reporting.
//!
//! Maps to: `src/consensus/validation.h` in Bitcoin Core.
//!
//! Provides [`ValidationState`], a generic state tracker that records whether
//! validation passed, and if not, why it failed with a machine-readable result
//! code and human-readable reason string.

use std::fmt;

/// Result codes for transaction validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxValidationResult {
    /// Initial value. Tx has not yet been rejected.
    Unset,
    /// Invalid by consensus rules.
    Consensus,
    /// Invalid by our policy rules (but valid by consensus).
    /// Not necessarily the tx's fault.
    RecentConsensusChange,
    /// Tx was not validated because we didn't connect the block (e.g. pruned).
    NotValidated,
    /// Transaction inputs missing/spent.
    MissingInputs,
    /// Not rejected for consensus, but rejected by mempool policy.
    NotStandard,
    /// Rejected because of resource limits.
    ResourceLimit,
}

/// Result codes for block validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockValidationResult {
    /// Initial value.
    Unset,
    /// Invalid by consensus rules (excluding any below reasons).
    Consensus,
    /// This block was valid according to the rules active when it was mined,
    /// but is no longer valid.
    RecentConsensusChange,
    /// Didn't fully validate because not on active chain.
    CachedInvalid,
    /// Header is valid, but block body is invalid.
    InvalidHeader,
    /// The block's data didn't match the data committed to by the PoW.
    MutatedBlock,
    /// Block timestamp was > 2 hours in the future.
    TimeFuture,
    /// The block failed to meet one of our checkpoints.
    Checkpoint,
}

/// Generic validation state that tracks whether validation succeeded
/// and if not, why it failed.
///
/// Equivalent to `ValidationState<R>` in Bitcoin Core (`src/consensus/validation.h`).
/// Parameterized by a result code type (`R`), typically [`TxValidationResult`]
/// or [`BlockValidationResult`].
#[derive(Debug, Clone)]
pub struct ValidationState<R: Clone + fmt::Debug> {
    mode: ValidationMode,
    result: R,
    reject_reason: String,
    debug_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ValidationMode {
    Valid,
    Invalid,
    Error,
}

impl<R: Clone + fmt::Debug + Default> ValidationState<R> {
    /// Create a new validation state in the `Valid` mode with a default result code.
    pub fn new() -> Self {
        ValidationState {
            mode: ValidationMode::Valid,
            result: R::default(),
            reject_reason: String::new(),
            debug_message: String::new(),
        }
    }
}

impl<R: Clone + fmt::Debug> ValidationState<R> {
    /// Mark as invalid with a reason and result code.
    pub fn invalid(&mut self, result: R, reject_reason: &str, debug_message: &str) -> bool {
        self.mode = ValidationMode::Invalid;
        self.result = result;
        self.reject_reason = reject_reason.to_string();
        self.debug_message = debug_message.to_string();
        false
    }

    /// Mark as error (internal processing error, not a validation failure).
    pub fn error(&mut self, reject_reason: &str) -> bool {
        self.mode = ValidationMode::Error;
        self.reject_reason = reject_reason.to_string();
        false
    }

    /// Check if validation passed.
    pub fn is_valid(&self) -> bool {
        self.mode == ValidationMode::Valid
    }

    /// Check if validation explicitly failed.
    pub fn is_invalid(&self) -> bool {
        self.mode == ValidationMode::Invalid
    }

    /// Check if there was an internal error.
    pub fn is_error(&self) -> bool {
        self.mode == ValidationMode::Error
    }

    /// Get the result code.
    pub fn get_result(&self) -> &R {
        &self.result
    }

    /// Get the reject reason string.
    pub fn get_reject_reason(&self) -> &str {
        &self.reject_reason
    }

    /// Get the debug message.
    pub fn get_debug_message(&self) -> &str {
        &self.debug_message
    }
}

impl Default for TxValidationResult {
    fn default() -> Self {
        TxValidationResult::Unset
    }
}

impl Default for BlockValidationResult {
    fn default() -> Self {
        BlockValidationResult::Unset
    }
}

/// Type alias for transaction validation state.
pub type TxValidationState = ValidationState<TxValidationResult>;

/// Type alias for block validation state.
pub type BlockValidationState = ValidationState<BlockValidationResult>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_state() {
        let state: TxValidationState = ValidationState::new();
        assert!(state.is_valid());
        assert!(!state.is_invalid());
    }

    #[test]
    fn test_invalid_state() {
        let mut state: TxValidationState = ValidationState::new();
        state.invalid(
            TxValidationResult::Consensus,
            "bad-txns",
            "duplicate inputs",
        );
        assert!(!state.is_valid());
        assert!(state.is_invalid());
        assert_eq!(state.get_reject_reason(), "bad-txns");
    }
}
