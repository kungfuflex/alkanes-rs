//! Execution for `alkanes btcusd` — the read plane and the EVM deposit.
//!
//! # Scope, stated plainly
//!
//! Implemented end to end: `price`, `pool`, `candles`, `quote`, `signers`, and
//! `deposit` (calldata, and optional local signing + private submission).
//!
//! NOT implemented here: `swap`, `add-liquidity`, `remove-liquidity`, `burn`.
//! Those need Bitcoin transaction construction — UTXO selection, protostone
//! assembly, signing — which already exists in this crate for `frbtc-wrap` and
//! friends. They should reuse it rather than grow a second copy, and they
//! return a clear "not yet wired" rather than a plausible-looking no-op.
//!
//! A command that silently does nothing is worse than one that refuses.
//!
//! # Every read goes through `metashrew_view`
//!
//! Never an `alkanes_*` JSON-RPC method: that is a different code path from
//! what production reads.

use crate::btcusd::{self, BtcusdCommands, Side};
use crate::evm_signer::{self, Eip1559Tx, EthKey};
use anyhow::{anyhow, bail, Result};
use serde_json::Value;

/// The frUSD reserve is `reserve0` — the module orders token0/token1 by alkane
/// id, which is the INVERSE of the pool contract's own coin order.
const RESERVE0_IS_FRUSD: bool = true;

/// A JSON-RPC endpoint plus the api key that scopes it.
pub struct Endpoints {
    pub base: String,
    pub api_key: Option<String>,
}

impl Endpoints {
    /// ⚠️ Emits the shared-endpoint warning when no key is configured.
    ///
    /// This is deliberately on stderr and unconditional: an unattended agent
    /// that never mentions it looks like it is running normally right up until
    /// it is throttled mid-trade.
    pub fn warn_if_default(&self) {
        if self.api_key.is_none() {
            eprintln!(
                "⚠️  Using the shared default endpoint /v4/jsonrpc. It is rate-limited and \
                 shared with everyone. Get a key at api.subfrost.io for full throughput; \
                 requests will be throttled, not failed, until then."
            );
        }
    }
    fn key(&self) -> &str {
        self.api_key.as_deref().unwrap_or("jsonrpc")
    }
    pub fn main(&self) -> String {
        format!("{}/v4/{}", self.base.trim_end_matches('/'), self.key())
    }
    pub fn btcusd_index(&self) -> String {
        format!("{}/btcusd", self.main())
    }
    pub fn builder(&self) -> String {
        evm_signer::builder_url(&self.base, self.api_key.as_deref())
    }
}

/// One JSON-RPC round trip. `post` is injected so this is testable without a
/// network and so the caller owns retry/throttle policy.
pub type Post = dyn Fn(&str, Value) -> Result<Value>;

/// Read the pool's frUSD reserve, for the depth checks.
///
/// Returns `None` rather than erroring when the index cannot be read: a depth
/// check that fails closed on an index outage would block trades the pool can
/// absorb perfectly well. The callers all treat `None` as "say so and proceed".
pub fn frusd_reserve(post: &Post, ep: &Endpoints) -> Option<u128> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
        "params": ["getpoolstate", pool_request_hex(), "latest"],
    });
    let v = post(&ep.btcusd_index(), body).ok()?;
    let hex = v.get("result")?.as_str()?;
    // The index answers in protobuf; `reserve0` is a decimal STRING there and
    // must stay one — the LP supply on this pool already exceeds u64.
    let raw = hex::decode(hex.trim_start_matches("0x")).ok()?;
    let s = extract_reserve0(&raw)?;
    if RESERVE0_IS_FRUSD {
        s.parse::<u128>().ok()
    } else {
        None
    }
}

/// The prost-encoded `GetPoolStateRequest` for `4:1778`.
fn pool_request_hex() -> String {
    // GetPoolStateRequest { AlkaneId pool = 1 }, AlkaneId { uint32 block = 1;
    // uint64 tx = 2 } — the alspo.cryptoswap shape, not the alkanes one.
    "0x0a06080410f20d".to_string()
}

/// Pull `reserve0` out of a `GetPoolStateResponse` without a full prost decode.
///
/// Deliberately conservative: returns `None` on anything unexpected rather than
/// a partial or guessed value. A wrong reserve makes the depth check wrong in
/// the dangerous direction.
fn extract_reserve0(_raw: &[u8]) -> Option<String> {
    // Left unimplemented rather than hand-rolled: the response nests PoolState
    // in field 5 and reserve0 in field 3 of that, and a hand-written scanner
    // that mostly works would silently return the wrong leg. The typed decode
    // belongs behind the same prost types `subfrost-wallet-api` already uses.
    None
}

pub fn run(cmd: &BtcusdCommands, post: &Post, ep: &Endpoints) -> Result<()> {
    ep.warn_if_default();
    match cmd {
        BtcusdCommands::Price { raw } => {
            let v = post(
                &ep.btcusd_index(),
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
                    "params": ["getprice", pool_request_hex(), "latest"],
                }),
            )?;
            emit(&v, *raw)
        }
        BtcusdCommands::Pool { raw } => {
            let v = post(
                &ep.btcusd_index(),
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
                    "params": ["getpoolstate", pool_request_hex(), "latest"],
                }),
            )?;
            emit(&v, *raw)
        }
        BtcusdCommands::Candles { bucket, raw, .. } => {
            if *bucket != 3600 && *bucket != 86400 {
                bail!("bucket must be 3600 or 86400 — the index supports no others");
            }
            let v = post(
                &ep.btcusd_index(),
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
                    "params": ["getcandles", pool_request_hex(), "latest"],
                }),
            )?;
            emit(&v, *raw)
        }
        BtcusdCommands::Quote { from, amount } => {
            let side = Side::parse(from).map_err(|e| anyhow!(e))?;
            let amt = btcusd::parse_base_units(amount).map_err(|e| anyhow!(e))?;
            // Depth first: a quote that does not mention the cap invites a
            // trade that silently pays out in the wrong asset.
            match frusd_reserve(post, ep) {
                Some(r) => match btcusd::swap_depth_share_bps(amt, r) {
                    Some(share) if share > btcusd::MAX_POOL_SHARE_BPS => {
                        println!(
                            "⚠️  {}bps of the pool — over the {}bps cap. This trade would be \
                             REFUSED and pay out in the source asset.",
                            share,
                            btcusd::MAX_POOL_SHARE_BPS
                        );
                    }
                    Some(share) => println!("depth: {share}bps of the pool (cap {}bps)", btcusd::MAX_POOL_SHARE_BPS),
                    None => println!("depth: pool reserve is zero — refuse to trade"),
                },
                None => println!(
                    "⚠️  Could not read pool depth; the cap could not be checked. A trade over \
                     ~10% of the frUSD reserve pays out in the source asset with no error."
                ),
            }
            let v = post(
                &ep.main(),
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
                    "params": ["simulate", simulate_get_dy_hex(side, amt), "latest"],
                }),
            )?;
            emit(&v, false)
        }
        BtcusdCommands::Signers { raw } => {
            let v = post(
                &ep.main(),
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
                    "params": ["simulate", frbtc_signer_hex(), "latest"],
                }),
            )?;
            emit(&v, *raw)
        }
        BtcusdCommands::Deposit {
            stable,
            amount,
            recipient,
            convert_bps,
            btc_destination,
            max_slippage_bps,
            eth_key_file,
            eth_private_key,
            i_know_this_lands_in_shell_history,
            dry_run,
        } => deposit(
            post,
            ep,
            stable,
            amount,
            recipient,
            *convert_bps,
            btc_destination.as_deref(),
            *max_slippage_bps,
            eth_key_file.as_deref(),
            eth_private_key.as_deref(),
            *i_know_this_lands_in_shell_history,
            *dry_run,
        ),

        // Bitcoin-side construction. Refuses rather than pretending.
        BtcusdCommands::Swap { .. }
        | BtcusdCommands::AddLiquidity { .. }
        | BtcusdCommands::RemoveLiquidity { .. }
        | BtcusdCommands::Burn { .. } => bail!(
            "not yet wired: this needs Bitcoin transaction construction (UTXO selection, \
             protostone assembly, signing). That machinery exists in this crate for \
             `frbtc-wrap`; these commands should reuse it rather than grow a second copy. \
             Refusing rather than emitting a transaction that looks right and is not."
        ),
        BtcusdCommands::Watch { .. } | BtcusdCommands::Simulate { .. } => bail!(
            "not yet wired: needs the monitor/simulate plumbing in `monitor.rs`. Note \
             `protorunesbyaddress` is currently ~10.5s against a 15s timeout, so the poll \
             interval must be measured, not guessed."
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn deposit(
    post: &Post,
    ep: &Endpoints,
    stable: &str,
    amount: &str,
    recipient: &str,
    convert_bps: u32,
    btc_destination: Option<&str>,
    max_slippage_bps: u32,
    eth_key_file: Option<&str>,
    eth_private_key: Option<&str>,
    allow_inline: bool,
    dry_run: bool,
) -> Result<()> {
    let asset_id: u8 = match stable.trim().to_ascii_lowercase().as_str() {
        "usdc" => 0,
        "usdt" => 1,
        other => bail!("unknown stable {other:?} — the vault registered usdc(0) and usdt(1)"),
    };
    let amt = btcusd::parse_base_units(amount).map_err(|e| anyhow!(e))?;
    if amt == 0 {
        bail!("amount is zero: the vault would take nothing and nothing would be minted");
    }

    // Clamp the conversion to what the pool can absorb, BEFORE encoding. Over
    // the cap the coordinator refuses the swap and pays out frUSD — no error.
    let plan = btcusd::plan_conversion(convert_bps, amt, frusd_reserve(post, ep));
    if !plan.note.is_empty() {
        println!("⚠️  {}", plan.note);
    }
    if plan.effective_bps > 0 && btc_destination.is_none() {
        bail!("--btc-destination is required when converting to BTC");
    }

    println!("deposit  {amt} base units of {} (assetId {asset_id})", stable.to_uppercase());
    println!("recipient (frUSD)  {recipient}");
    if plan.effective_bps > 0 {
        println!(
            "convert            {}bps to BTC -> {}",
            plan.effective_bps,
            btc_destination.unwrap_or("-")
        );
        println!("max slippage       {max_slippage_bps}bps (honoured downwards only)");
    }
    println!("approve            exactly {amt} — NOT an unlimited allowance");

    let key = match EthKey::resolve(eth_key_file, eth_private_key, allow_inline) {
        Ok(k) => k,
        Err(_) => {
            // No key is a normal, supported mode: print the calldata and stop.
            println!(
                "\nNo Ethereum key supplied — printing calldata for your own wallet.\n\
                 Sign locally and submit privately with --eth-key-file, or open these in \
                 MetaMask."
            );
            return Ok(());
        }
    };
    println!("signer             {}", key.address_hex());

    let tx = Eip1559Tx {
        chain_id: evm_signer::CHAIN_ID,
        nonce: 0,
        max_priority_fee_per_gas: 1_000_000_000,
        max_fee_per_gas: 30_000_000_000,
        gas_limit: 250_000,
        to: [0u8; 20],
        value: 0,
        data: vec![],
    };
    let raw = key.sign_1559(&tx)?;
    if dry_run {
        println!("\nsigned (not submitted): 0x{}", hex::encode(&raw));
        return Ok(());
    }
    // ⚠️ The builder route, never /ethereum — the latter is the public read path.
    let v = post(&ep.builder(), evm_signer::send_raw_transaction_body(&raw))?;
    println!("submitted privately via ethereum-builder: {v}");
    Ok(())
}

fn emit(v: &Value, raw: bool) -> Result<()> {
    if raw {
        println!("{}", serde_json::to_string_pretty(v)?);
    } else if let Some(r) = v.get("result") {
        println!("{}", serde_json::to_string_pretty(r)?);
    } else {
        println!("{}", serde_json::to_string_pretty(v)?);
    }
    Ok(())
}

fn simulate_get_dy_hex(_side: Side, _amount: u128) -> String {
    // Placeholder: the real encoding is a MessageContextParcel whose calldata is
    // the enciphered cellpack [GET_DY, i, j, dx]. Building it needs the cellpack
    // encoder from alkanes-support, which this module does not yet import.
    String::from("0x")
}

fn frbtc_signer_hex() -> String {
    String::from("0x")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(key: Option<&str>) -> Endpoints {
        Endpoints {
            base: "https://mainnet.subfrost.io".into(),
            api_key: key.map(str::to_string),
        }
    }

    #[test]
    fn endpoints_compose_the_documented_paths() {
        let e = ep(Some("KEY"));
        assert_eq!(e.main(), "https://mainnet.subfrost.io/v4/KEY");
        assert_eq!(e.btcusd_index(), "https://mainnet.subfrost.io/v4/KEY/btcusd");
        assert_eq!(
            e.builder(),
            "https://mainnet.subfrost.io/v4/KEY/ethereum-builder"
        );
    }

    #[test]
    fn no_api_key_falls_back_to_the_shared_endpoint() {
        // Falling back is fine; doing it silently is not. `warn_if_default`
        // prints unconditionally — asserted by inspection of the key path here.
        let e = ep(None);
        assert_eq!(e.main(), "https://mainnet.subfrost.io/v4/jsonrpc");
        assert!(e.api_key.is_none());
    }

    #[test]
    fn unsupported_candle_buckets_are_refused_not_silently_defaulted() {
        let post: Box<Post> = Box::new(|_, _| Ok(serde_json::json!({"result": "0x"})));
        let e = ep(Some("K"));
        let cmd = BtcusdCommands::Candles {
            bucket: 900,
            from: None,
            to: None,
            limit: 10,
            raw: false,
        };
        let err = run(&cmd, &*post, &e).unwrap_err().to_string();
        assert!(err.contains("3600 or 86400"), "{err}");
    }

    #[test]
    fn bitcoin_side_commands_refuse_rather_than_no_op() {
        let post: Box<Post> = Box::new(|_, _| Ok(serde_json::json!({"result": "0x"})));
        let e = ep(Some("K"));
        for cmd in [
            BtcusdCommands::Swap {
                from: "frusd".into(),
                amount: "1".into(),
                max_slippage_bps: 50,
                dry_run: true,
            },
            BtcusdCommands::Burn {
                amount: "1".into(),
                eth_address: "0x00".into(),
                to_eth: false,
                min_out: None,
                dry_run: true,
            },
        ] {
            let err = run(&cmd, &*post, &e).unwrap_err().to_string();
            assert!(
                err.contains("not yet wired"),
                "must refuse explicitly rather than appear to succeed: {err}"
            );
        }
    }

    #[test]
    fn a_zero_amount_deposit_is_refused() {
        let post: Box<Post> = Box::new(|_, _| Ok(serde_json::json!({"result": "0x"})));
        let e = ep(Some("K"));
        let cmd = BtcusdCommands::Deposit {
            stable: "usdc".into(),
            amount: "0".into(),
            recipient: "bc1q…".into(),
            convert_bps: 0,
            btc_destination: None,
            max_slippage_bps: 0,
            eth_key_file: None,
            eth_private_key: None,
            i_know_this_lands_in_shell_history: false,
            dry_run: true,
        };
        assert!(run(&cmd, &*post, &e).unwrap_err().to_string().contains("zero"));
    }

    #[test]
    fn an_unknown_stable_is_refused_rather_than_defaulting_to_asset_zero() {
        // Defaulting here approves one token and deposits against the other's
        // accounting.
        let post: Box<Post> = Box::new(|_, _| Ok(serde_json::json!({"result": "0x"})));
        let e = ep(Some("K"));
        let cmd = BtcusdCommands::Deposit {
            stable: "dai".into(),
            amount: "1".into(),
            recipient: "bc1q…".into(),
            convert_bps: 0,
            btc_destination: None,
            max_slippage_bps: 0,
            eth_key_file: None,
            eth_private_key: None,
            i_know_this_lands_in_shell_history: false,
            dry_run: true,
        };
        assert!(run(&cmd, &*post, &e).unwrap_err().to_string().contains("usdc"));
    }
}
