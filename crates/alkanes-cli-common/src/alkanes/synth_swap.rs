//! BTC ↔ USDC/USDT atomic synth-swap via 3-protostone chained transaction.
//!
//! BTC → USDC flow (single Bitcoin TX):
//!   Output[0]: BTC payment to frBTC signer address (wrap amount)
//!   Output[1]: Refund address (if swap fails, frBTC lands here)
//!   Output[2]: Refund address (if bridge fails, frUSD lands here)
//!   Output[3]: BTC change
//!   Output[4]: OP_RETURN with Runestone containing 3 protostones:
//!     p0: [32:0, 77]          wrap BTC → frBTC       pointer→p1  refund→v1
//!     p1: [4:POOL, 5]         swap frBTC → frUSD     pointer→p2  refund→v1
//!     p2: [4:frUSD, 5, ...]   burn frUSD + bridge    pointer→v2  refund→v2
//!
//! The signal engine detects the burn record and processes the EVM withdrawal.

use serde::{Deserialize, Serialize};

/// Parameters for a BTC → stablecoin synth swap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthSwapParams {
    /// Amount of BTC to swap (in satoshis).
    pub btc_amount_sats: u64,

    /// The frBTC signer P2TR address (destination for the BTC payment).
    /// Derived from `[32:0]` opcode 103 (GetSigner).
    pub frbtc_signer_address: String,

    /// The frBTC signer script pubkey (hex, e.g. "5120<xonly>").
    pub frbtc_signer_script: String,

    /// Synth pool alkane ID (e.g. "4:43592" for frBTC/frUSD pool).
    pub pool_block: u128,
    pub pool_tx: u128,

    /// frUSD token alkane ID (e.g. "4:43591").
    pub frusd_block: u128,
    pub frusd_tx: u128,

    /// Swap opcode on the synth pool (typically 5).
    pub swap_opcode: u128,

    /// BurnAndBridge opcode on frUSD (typically 5).
    pub burn_opcode: u128,

    /// EVM destination details (encoded in the burn protostone).
    /// The EVM contract address that processes the withdrawal.
    pub evm_vault_address: Option<String>,

    /// The EVM recipient address for the stablecoin output.
    pub evm_recipient: Option<String>,

    /// The user's Bitcoin refund address (for failed intermediate steps).
    pub refund_address: String,

    /// Fee rate in sat/vB.
    pub fee_rate: f64,
}

/// The 3 protostones for the BTC → USDC atomic swap.
///
/// Returns a vector of protostone spec strings in the CLI format:
///   `[block,tx,opcode,...]:pointer:refund`
pub fn build_swap_protostones(params: &SynthSwapParams) -> Vec<String> {
    // p0: Wrap BTC → frBTC
    // pointer→p1 (forward frBTC to the swap)
    // refund→v1 (if wrap fails, BTC refund — but wrap can't really fail)
    let p0 = format!("[32,0,77]:p1:v1");

    // p1: Swap frBTC → frUSD via synth pool
    // pointer→p2 (forward frUSD to the burn)
    // refund→v1 (if swap fails, frBTC goes to refund output)
    let p1 = format!("[{},{},{}]:p2:v1", params.pool_block, params.pool_tx, params.swap_opcode);

    // p2: Burn frUSD and bridge to EVM
    // pointer→v2 (any remaining frUSD goes to second refund output)
    // refund→v2 (same)
    // The burn opcode encodes the EVM details in additional args
    let p2 = format!("[{},{},{}]:v2:v2", params.frusd_block, params.frusd_tx, params.burn_opcode);

    vec![p0, p1, p2]
}

/// Build the output addresses for the swap TX.
///
/// Returns: [frbtc_signer, refund_1, refund_2]
/// The change output is handled separately by the CLI.
pub fn build_swap_outputs(params: &SynthSwapParams) -> Vec<(String, u64)> {
    vec![
        // Output 0: BTC payment to frBTC signer (this is the wrap amount)
        (params.frbtc_signer_address.clone(), params.btc_amount_sats),
        // Output 1: Refund for frBTC if swap fails (dust)
        (params.refund_address.clone(), 546),
        // Output 2: Refund for frUSD if bridge fails (dust)
        (params.refund_address.clone(), 546),
    ]
}

/// Estimate the frUSD output amount from a BTC→frUSD swap.
///
/// This is a rough estimate: frBTC amount ≈ btc_sats (minus small fee),
/// then the synth pool swap applies its fee curve.
/// For accurate quotes, simulate the pool's get_dy view function.
pub fn estimate_frusd_output(btc_sats: u64, pool_fee_bps: u64) -> u64 {
    // Rough: frBTC ≈ btc_sats, then pool fee
    let frbtc = btc_sats.saturating_sub(100); // wrap fee ~100 sats
    let fee = frbtc * pool_fee_bps / 10000;
    frbtc.saturating_sub(fee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_swap_protostones() {
        let params = SynthSwapParams {
            btc_amount_sats: 50_000_000,
            frbtc_signer_address: "bcrt1p09qw7wm9j9u6zdcaaszhj09sylx7g7qxldnvu83ard5a2m0x98wqzulgv0".into(),
            frbtc_signer_script: "51207940ef3b659179a1371dec05793cb027cde47806fb66ce1e3d1b69d56de629dc".into(),
            pool_block: 4,
            pool_tx: 43592,
            frusd_block: 4,
            frusd_tx: 43591,
            swap_opcode: 5,
            burn_opcode: 5,
            evm_vault_address: None,
            evm_recipient: None,
            refund_address: "bcrt1p3rng065wgw8axe0a9ewjnd4zqdz83spn6w4m6g9y5tylyqfsdcus2swmte".into(),
            fee_rate: 1.0,
        };

        let protos = build_swap_protostones(&params);
        assert_eq!(protos.len(), 3);
        assert_eq!(protos[0], "[32,0,77]:p1:v1");
        assert_eq!(protos[1], "[4,43592,5]:p2:v1");
        assert_eq!(protos[2], "[4,43591,5]:v2:v2");
    }

    #[test]
    fn test_build_swap_outputs() {
        let params = SynthSwapParams {
            btc_amount_sats: 50_000_000,
            frbtc_signer_address: "bcrt1psigner".into(),
            frbtc_signer_script: "5120aa".into(),
            pool_block: 4,
            pool_tx: 100,
            frusd_block: 4,
            frusd_tx: 200,
            swap_opcode: 5,
            burn_opcode: 5,
            evm_vault_address: None,
            evm_recipient: None,
            refund_address: "bcrt1puser".into(),
            fee_rate: 1.0,
        };

        let outputs = build_swap_outputs(&params);
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].0, "bcrt1psigner");
        assert_eq!(outputs[0].1, 50_000_000);
        assert_eq!(outputs[1].0, "bcrt1puser");
        assert_eq!(outputs[1].1, 546); // dust refund
    }

    #[test]
    fn test_estimate_frusd_output() {
        // 0.5 BTC with 40bps pool fee
        let estimate = estimate_frusd_output(50_000_000, 40);
        assert!(estimate > 49_000_000); // should be close to input minus fees
        assert!(estimate < 50_000_000);
    }
}
