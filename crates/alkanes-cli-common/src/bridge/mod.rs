//! Bridge operations — deposit stablecoins (USDT/USDC) into subfrost vault.
//!
//! This handles the EVM side of the cross-chain bridge:
//! 1. Approve vault to spend stablecoins
//! 2. Call vault.depositAndBridge(amount, protostones, outputs)
//! 3. The vault emits PaymentQueued event
//! 4. The subfrost signal engine picks it up and mints frUSD on Bitcoin

pub mod deposit;
pub mod types;
