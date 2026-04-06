//! BTC ↔ USDC/USDT atomic synth-swap.
//!
//! ## BTC → USDC (single Bitcoin TX, 3 protostones)
//!
//! User builds one Bitcoin transaction containing 3 chained protostones:
//!   p0: [32:0, 77]          wrap BTC→frBTC       pointer→p1  refund→v1
//!   p1: [4:POOL, 5]         swap frBTC→frUSD     pointer→p2  refund→v1
//!   p2: [4:frUSD, 5]        burn frUSD (bridge)  pointer→v2  refund→v2
//!
//! The frUSD bridge signing group detects the burn and releases USDC on EVM.
//!
//! ## USDC → BTC (EVM deposit + 2 signing groups)
//!
//! User calls vault.depositAndBridge(amount, protostones, outputs) on EVM.
//! The protostones encode the full path:
//!   p0: [4:frUSD, 1, ...]   mint frUSD           pointer→p1  refund→v0
//!   p1: [4:POOL, 5]         swap frUSD→frBTC     pointer→p2  refund→v0
//!   p2: [32:0, 78, V, AMT]  unwrap frBTC→BTC     pointer→v1  refund→v0
//!
//! The frUSD signing group builds a TX with these protostones. The frBTC
//! unwrap creates a pending payment that the frBTC signing group fulfills.

use serde::{Deserialize, Serialize};

// ─── BTC → USDC ─────────────────────────────────────────────────────────────

/// Parameters for a BTC → stablecoin synth swap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtcToStableParams {
    /// Amount of BTC to swap (in satoshis).
    pub btc_amount_sats: u64,
    /// The frBTC signer P2TR address (TWEAKED, via `tr(pubkey)` not `rawtr`).
    pub frbtc_signer_address: String,
    /// Synth pool alkane ID.
    pub pool_block: u128,
    pub pool_tx: u128,
    /// frUSD token alkane ID.
    pub frusd_block: u128,
    pub frusd_tx: u128,
    /// User's Bitcoin refund address (for failed intermediate steps).
    pub refund_address: String,
    /// Fee rate in sat/vB.
    pub fee_rate: f64,
}

/// Build the CLI execute command for BTC → USDC.
///
/// Returns (protostones_csv, to_addresses, inputs).
pub fn btc_to_stable_args(params: &BtcToStableParams) -> (String, Vec<String>, String) {
    let protostones = format!(
        "[32,0,77]:p1:v1,[{},{},5]:p2:v1,[{},{},5]:v2:v2",
        params.pool_block, params.pool_tx,
        params.frusd_block, params.frusd_tx,
    );

    let to_addresses = vec![
        params.frbtc_signer_address.clone(),   // v0: signer (BTC payment for wrap)
        params.refund_address.clone(),          // v1: refund (frBTC if swap fails)
        params.refund_address.clone(),          // v2: refund (frUSD if bridge fails)
    ];

    let inputs = format!("B:{}:v0", params.btc_amount_sats);

    (protostones, to_addresses, inputs)
}

// ─── USDC → BTC ─────────────────────────────────────────────────────────────

/// Parameters for a stablecoin → BTC bridge deposit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableToBtcParams {
    /// USDC/USDT amount (raw, 6 decimals).
    pub stable_amount: u64,
    /// The frBTC signer P2TR address (for the unwrap output).
    pub frbtc_signer_address: String,
    /// frBTC signer script hex (for the unwrap output).
    pub frbtc_signer_script: String,
    /// Synth pool alkane ID.
    pub pool_block: u128,
    pub pool_tx: u128,
    /// frUSD token alkane ID.
    pub frusd_block: u128,
    pub frusd_tx: u128,
    /// The user's Bitcoin P2TR address (where BTC will be sent after unwrap).
    pub btc_recipient_address: String,
    /// The user's Bitcoin P2TR script hex.
    pub btc_recipient_script: String,
    /// EVM vault address.
    pub vault_address: String,
    /// EVM USDC/USDT token address.
    pub stablecoin_address: String,
    /// EVM chain ID.
    pub chain_id: u64,
}

/// Build the protostones bytes for the EVM depositAndBridge call.
///
/// These protostones encode the full mint→swap→unwrap chain that the
/// frUSD signing group will include in its Bitcoin mint TX.
///
/// Returns (protostones_text, outputs) for the depositAndBridge call.
pub fn stable_to_btc_deposit_args(params: &StableToBtcParams) -> (String, Vec<(u64, String)>) {
    // Estimate the frUSD mint amount (roughly = stablecoin amount scaled to 18 decimals)
    // The exact amount is determined by the vault's fee structure.
    // For the protostones, we use a placeholder that the engine will fill in.
    let mint_amount = params.stable_amount as u128;

    // Protostones for the mint TX:
    // p0 is the mint protostone (built by the engine's build_mint)
    // p1 is the swap frUSD→frBTC
    // p2 is the unwrap frBTC (burns frBTC, creates pending BTC payment)
    //
    // The engine's build_mint prepends the mint protostone (p0) and appends user protostones.
    // So we only specify p1 (swap) and p2 (unwrap) here.
    let protostones = format!(
        "[{},{},5]:p2:v0,[32,0,78,0,{}]:v1:v0",
        params.pool_block, params.pool_tx,    // p1: swap frUSD→frBTC
        mint_amount,                           // p2: unwrap frBTC
    );

    // Outputs for the Bitcoin TX (specified in the EVM deposit):
    // v0: signer address (frBTC signer for the unwrap spendable output)
    // v1: user's BTC address (where the unwrap payment goes)
    let outputs = vec![
        (546u64, params.frbtc_signer_script.clone()),   // v0: signer (for unwrap spendable)
        (546u64, params.btc_recipient_script.clone()),   // v1: user BTC recipient
    ];

    (protostones, outputs)
}

/// Build the full depositAndBridge calldata for the EVM vault.
///
/// Returns (protostones_hex, outputs_for_solidity) ready for the vault call.
pub fn stable_to_btc_evm_args(params: &StableToBtcParams) -> (String, Vec<(u64, String)>) {
    let (protostones_text, outputs) = stable_to_btc_deposit_args(params);

    // Hex-encode the protostones text for the Solidity bytes parameter
    let protostones_hex = format!("0x{}", hex::encode(protostones_text.as_bytes()));

    (protostones_hex, outputs)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btc_to_stable_args() {
        let params = BtcToStableParams {
            btc_amount_sats: 50_000_000,
            frbtc_signer_address: "bcrt1psigner".into(),
            pool_block: 4,
            pool_tx: 43602,
            frusd_block: 4,
            frusd_tx: 43601,
            refund_address: "bcrt1puser".into(),
            fee_rate: 1.0,
        };

        let (protos, to, inputs) = btc_to_stable_args(&params);
        assert_eq!(protos, "[32,0,77]:p1:v1,[4,43602,5]:p2:v1,[4,43601,5]:v2:v2");
        assert_eq!(to.len(), 3);
        assert_eq!(to[0], "bcrt1psigner");
        assert_eq!(inputs, "B:50000000:v0");
    }

    #[test]
    fn test_stable_to_btc_args() {
        let params = StableToBtcParams {
            stable_amount: 100_000_000, // 100 USDC
            frbtc_signer_address: "bcrt1psigner".into(),
            frbtc_signer_script: "5120aa".into(),
            pool_block: 4,
            pool_tx: 43602,
            frusd_block: 4,
            frusd_tx: 43601,
            btc_recipient_address: "bcrt1puser".into(),
            btc_recipient_script: "5120bb".into(),
            vault_address: "0xvault".into(),
            stablecoin_address: "0xusdc".into(),
            chain_id: 31337,
        };

        let (protos, outputs) = stable_to_btc_deposit_args(&params);
        assert!(protos.contains("[4,43602,5]:p2:v0"));
        assert!(protos.contains("[32,0,78,0,100000000]:v1:v0"));
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].0, 546); // dust for signer
        assert_eq!(outputs[1].1, "5120bb"); // user script
    }

    #[test]
    fn test_evm_args_hex_encoding() {
        let params = StableToBtcParams {
            stable_amount: 50_000_000,
            frbtc_signer_address: "bcrt1psigner".into(),
            frbtc_signer_script: "5120aa".into(),
            pool_block: 4,
            pool_tx: 100,
            frusd_block: 4,
            frusd_tx: 200,
            btc_recipient_address: "bcrt1puser".into(),
            btc_recipient_script: "5120bb".into(),
            vault_address: "0xvault".into(),
            stablecoin_address: "0xusdc".into(),
            chain_id: 31337,
        };

        let (proto_hex, outputs) = stable_to_btc_evm_args(&params);
        assert!(proto_hex.starts_with("0x"));
        // Decode and verify
        let decoded = hex::decode(&proto_hex[2..]).unwrap();
        let text = String::from_utf8(decoded).unwrap();
        assert!(text.contains("[4,100,5]"));
        assert!(text.contains("[32,0,78,0,50000000]"));
        assert_eq!(outputs.len(), 2);
    }
}
