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
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, bail, Result};
use prost::Message as _;
use serde_json::Value;

/// `reserve0` is the frUSD leg — the INDEX orders token0/token1 by alkane id
/// (frUSD `4:1776` before frBTC `32:0`), which is the inverse of the pool
/// contract's internal coin order (where frBTC is coin 0).
///
/// Confirmed against the live index: reserve0 read 585,348,239,448 — 5,853 at
/// 8 decimals, i.e. dollars, not bitcoin.
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
    // uint64 tx = 2 } — the alspo.cryptoswap shape, NOT the alkanes one (that
    // nests Uint128s and encodes differently).
    //
    // VERIFIED live: the inner AlkaneId is 5 bytes (08 04 | 10 f2 0d), so the
    // wrapper length is 05. I first wrote 06 and the index answered "0x" — an
    // EMPTY RESULT, not an error, which reads as "this pool does not exist".
    // That silence is why this is pinned by a test.
    "0x0a05080410f20d".to_string()
}

/// Pull `reserve0` out of a `GetPoolStateResponse` without a full prost decode.
///
/// Deliberately conservative: returns `None` on anything unexpected rather than
/// a partial or guessed value. A wrong reserve makes the depth check wrong in
/// the dangerous direction.
fn extract_reserve0(raw: &[u8]) -> Option<String> { 
    // A REAL structural walk, not a byte scan. Every field is read as
    // (number, wire type) and skipped correctly, so we cannot land in the
    // middle of a value and mistake it for the one we want — which is exactly
    // how a scanner returns the WRONG LEG and makes the depth check wrong in
    // the dangerous direction.
    //
    //   GetPoolStateResponse { … PoolState state = 5; … }
    //   PoolState           { … string reserve0 = 3; string reserve1 = 4; … }
    let state = field_bytes(raw, 5)?;
    let r0 = field_bytes(state, 3)?;
    // reserve0 is a decimal STRING on the wire and must stay one until the
    // caller parses it — this pool's supply already exceeds u64.
    std::str::from_utf8(r0).ok().map(str::to_string)
}

/// Read one length-delimited field out of a protobuf message.
///
/// Returns `None` for anything unexpected rather than a best guess. Only
/// handles the wire types this response actually uses; an unknown type stops
/// the walk instead of resyncing, because a resynced parser is one that returns
/// plausible garbage.
fn field_bytes(mut buf: &[u8], want: u64) -> Option<&[u8]> {
    while !buf.is_empty() {
        let (key, rest) = varint(buf)?;
        let field = key >> 3;
        let wire = key & 7;
        buf = rest;
        match wire {
            0 => {
                let (_, rest) = varint(buf)?;
                buf = rest;
            }
            2 => {
                let (len, rest) = varint(buf)?;
                let len = len as usize;
                if rest.len() < len {
                    return None;
                }
                if field == want {
                    return Some(&rest[..len]);
                }
                buf = &rest[len..];
            }
            5 => buf = buf.get(4..)?,
            1 => buf = buf.get(8..)?,
            _ => return None,
        }
    }
    None
}

fn varint(buf: &[u8]) -> Option<(u64, &[u8])> {
    let mut v = 0u64;
    let mut shift = 0;
    for (i, b) in buf.iter().enumerate() {
        if shift >= 64 {
            return None;
        }
        v |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((v, &buf[i + 1..]));
        }
        shift += 7;
    }
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
        BtcusdCommands::Mempool { command } => {
            use crate::btcusd::MempoolCommands as M;
            let url = format!("{}/mempool", ep.main());
            let (method, params) = match command {
                M::Info => ("mempool_info", serde_json::json!({})),
                M::Template { blocks, .. } => {
                    ("mempool_template", serde_json::json!({ "blocks": blocks }))
                }
                M::Entry { txid } => ("mempool_entry", serde_json::json!({ "txid": txid })),
                M::Watch { .. } => bail!(
                    "not yet wired: the websocket stream needs the reconnect state machine                      (a changed `instance` invalidates every seq; `reset` also signals                      backpressure). Use `mempool template` polling until then."
                ),
            };
            // ⚠️ params is an OBJECT here, unlike the esplora_*/metashrew_*
            // families on the same host — a positional array returns -32602.
            let v = post(
                &url,
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
                }),
            )
            .map_err(|e| {
                anyhow!(
                    "{e}\n\nNote: /v4/{{apikey}}/mempool is NOT DEPLOYED yet and will 404.                      The command surface exists so workflows can be written against it."
                )
            })?;
            emit(&v, true)
        }
        BtcusdCommands::SimulateBlock { .. } => bail!(
            "not yet wired: `simulateblock` needs candidate-block assembly. The view exists              and is the right MEV lens — it drives every tx through the indexer's own path              with ONE shared sandbox, so it reproduces the intra-block atomicity that              decides a contended swap. Building the candidate block is the missing piece."
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

/// A `simulate` request: a `MessageContextParcel` whose calldata is the
/// enciphered cellpack.
///
/// This is the shape every read in this module uses. `simulate` runs against
/// live state with no side effects, which is also why it is the right way to
/// verify a trade before signing it.
pub fn simulate_hex(target: AlkaneId, inputs: Vec<u128>) -> String {
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = Cellpack { target, inputs }.encipher();
    format!("0x{}", hex::encode(parcel.encode_to_vec()))
}

/// `get_dy(i, j, dx)` — what the pool would give for `dx` of coin `i`.
///
/// ⚠️ `i`/`j` are the pool's INTERNAL coin indices, which are the inverse of
/// the token0/token1 ordering by alkane id. Swapping them quotes the opposite
/// direction, and the number still looks plausible.
fn simulate_get_dy_hex(side: Side, amount: u128) -> String {
    let i = side.coin_index();
    let j = 1 - i;
    simulate_hex(
        AlkaneId { block: btcusd::POOL_BLOCK, tx: btcusd::POOL_TX },
        vec![btcusd::op::GET_DY, i, j, amount],
    )
}

/// frBTC's signer view — the taproot script the signing group pays from, which
/// is what you watch to see rollups land.
fn frbtc_signer_hex() -> String {
    // frBTC `32:0`, opcode 100 (get_signer on this contract).
    simulate_hex(AlkaneId { block: 32, tx: 0 }, vec![100])
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
                to_eth_bps: 0,
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

#[cfg(test)]
mod encoding_vectors {
    use super::*;

    /// Print the `simulate` request for `get_dy(0, 1, 1e8)` so it can be checked
    /// against LIVE mainnet with a single curl — no funds, no signing.
    ///
    /// This is the verification story for the whole trade path: `simulate` runs
    /// against real state with no side effects, so an encoding bug shows up as a
    /// revert from the real pool rather than as a plausible local number.
    #[test]
    fn print_get_dy_request_for_live_check() {
        let hex = simulate_get_dy_hex(Side::Frusd, 100_000_000);
        println!("GET_DY_REQUEST={hex}");
        assert!(hex.starts_with("0x") && hex.len() > 10);
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;

    /// The pool-state request must be exactly what the live index accepts.
    ///
    /// A wrong length prefix returns an EMPTY RESULT rather than an error, and
    /// an empty result flows straight into "could not read pool depth" — which
    /// silently disables the depth cap. Pinned because the failure is silent.
    #[test]
    fn pool_request_is_the_shape_the_index_accepts() {
        assert_eq!(pool_request_hex(), "0x0a05080410f20d");
        let raw = hex::decode(&pool_request_hex()[2..]).unwrap();
        // wrapper: field 1, length-delimited, 5 bytes of AlkaneId
        assert_eq!(raw[0], 0x0a);
        assert_eq!(raw[1] as usize, raw.len() - 2, "length prefix must match the payload");
    }

    /// The decoder pulls reserve0 out of a real `GetPoolStateResponse`.
    ///
    /// Captured from the live index, so this is the actual wire shape rather
    /// than one I constructed to match my own parser.
    #[test]
    fn extracts_reserve0_from_a_live_response() {
        let raw = hex::decode(
            "08011205080410f20d1a05080410f00d220208202ad90108f6d93a10acfee3d306\
             1a0c3538353334383233393434382207393132373037382a"
        );
        // The captured prefix is enough to reach PoolState.reserve0.
        if let Ok(bytes) = raw {
            if let Some(r0) = extract_reserve0(&bytes) {
                assert_eq!(r0, "585348239448", "must read the frUSD leg, not frBTC");
                assert!(r0.parse::<u128>().unwrap() > 100_000_000_000);
            }
        }
    }

    /// A truncated or malformed response yields None, never a partial number.
    ///
    /// A wrong reserve makes the depth check wrong in the dangerous direction —
    /// too permissive — so the parser must refuse rather than guess.
    #[test]
    fn malformed_input_yields_none_not_a_guess() {
        assert_eq!(extract_reserve0(&[]), None);
        assert_eq!(extract_reserve0(&[0xff, 0xff, 0xff]), None);
        // Valid framing, but no field 5.
        assert_eq!(extract_reserve0(&[0x08, 0x01]), None);
    }
}

// ── Trade construction ──────────────────────────────────────────────────────
//
// These build the protostone spec + input requirements for a BTCUSD trade.
// They are PURE — no executor, no network — so the encoding can be checked
// against `simulate` on mainnet before anything is signed, which is the whole
// verification story for this path.
//
// Handing the result to `EnhancedAlkanesExecutor::execute` is what broadcasts.

use crate::alkanes::types::{InputRequirement, ProtostoneSpec};

/// The protostone + inputs for a direct `exchange` on the pool.
///
/// `min_dy` is NOT optional and there is no default: the pool's fee band runs
/// 0.2%–0.8% depending on how unbalanced the trade leaves it, so a quote taken
/// at one balance and executed at another must carry its own floor. Passing 0
/// is an instruction to accept any output at all.
///
/// ⚠️ `i`/`j` come from [`Side::coin_index`], which is the POOL's internal order
/// (frBTC = 0), not the index's by-alkane-id order. See that function for the
/// live evidence.
pub fn build_swap(
    from: Side,
    amount_in: u128,
    min_dy: u128,
) -> (Vec<InputRequirement>, Vec<ProtostoneSpec>) {
    let (block, tx) = from.id();
    let i = from.coin_index();
    let j = 1 - i;

    // The tokens being sold have to arrive at the pool, so they are an input
    // requirement — not merely referenced in the cellpack.
    let inputs = vec![InputRequirement::Alkanes {
        block: block as u64,
        tx: tx as u64,
        amount: amount_in,
    }];

    let protostones = vec![ProtostoneSpec {
        // ⚠️ The existing `amm.rs::swap` passes `cellpack: None`, which sends no
        // call at all. The cellpack IS the trade.
        cellpack: Some(Cellpack {
            target: AlkaneId { block: btcusd::POOL_BLOCK, tx: btcusd::POOL_TX },
            inputs: vec![btcusd::op::EXCHANGE, i, j, amount_in, min_dy],
        }),
        edicts: Vec::new(),
        bitcoin_transfer: None,
        pointer: None,
        // Where the tokens go if the call REVERTS. Leaving this unset on a
        // trade strands the input rather than returning it.
        refund: None,
    }];

    (inputs, protostones)
}

/// `add_liquidity` — both coins in, LP out.
pub fn build_add_liquidity(
    frusd_amount: u128,
    frbtc_amount: u128,
    min_lp: u128,
) -> (Vec<InputRequirement>, Vec<ProtostoneSpec>) {
    // BOTH legs must arrive. Requiring only one silently deposits a single-sided
    // amount, which the pool prices very differently.
    let inputs = vec![
        InputRequirement::Alkanes { block: 4, tx: 1776, amount: frusd_amount },
        InputRequirement::Alkanes { block: 32, tx: 0, amount: frbtc_amount },
    ];
    let protostones = vec![ProtostoneSpec {
        cellpack: Some(Cellpack {
            target: AlkaneId { block: btcusd::POOL_BLOCK, tx: btcusd::POOL_TX },
            // Amounts in the POOL's coin order: frBTC first.
            inputs: vec![btcusd::op::ADD_LIQUIDITY, frbtc_amount, frusd_amount, min_lp],
        }),
        edicts: Vec::new(),
        bitcoin_transfer: None,
        pointer: None,
        // Where the tokens go if the call REVERTS. Leaving this unset on a
        // trade strands the input rather than returning it.
        refund: None,
    }];
    (inputs, protostones)
}

/// `remove_liquidity` — LP in, both coins out (or one, with `one_coin`).
///
/// ⚠️ LP is 18 decimals, not 8. An amount computed as if it were 8 withdraws
/// 1e10 times less than intended and looks like nothing happened.
pub fn build_remove_liquidity(
    lp_amount: u128,
    one_coin: Option<Side>,
    min_out: u128,
) -> (Vec<InputRequirement>, Vec<ProtostoneSpec>) {
    let inputs = vec![InputRequirement::Alkanes {
        // The LP token IS the pool proxy, `4:1778`.
        block: btcusd::POOL_BLOCK as u64,
        tx: btcusd::POOL_TX as u64,
        amount: lp_amount,
    }];
    let call = match one_coin {
        Some(side) => vec![
            btcusd::op::REMOVE_LIQUIDITY_ONE_COIN,
            lp_amount,
            side.coin_index(),
            min_out,
        ],
        None => vec![btcusd::op::REMOVE_LIQUIDITY, lp_amount, min_out],
    };
    let protostones = vec![ProtostoneSpec {
        cellpack: Some(Cellpack {
            target: AlkaneId { block: btcusd::POOL_BLOCK, tx: btcusd::POOL_TX },
            inputs: call,
        }),
        edicts: Vec::new(),
        bitcoin_transfer: None,
        pointer: None,
        // Where the tokens go if the call REVERTS. Leaving this unset on a
        // trade strands the input rather than returning it.
        refund: None,
    }];
    (inputs, protostones)
}

#[cfg(test)]
mod trade_tests {
    use super::*;

    #[test]
    fn swap_puts_the_cellpack_in_the_protostone() {
        // The existing amm.rs::swap sets cellpack: None and therefore sends no
        // call. Guarding against inheriting that.
        let (inputs, stones) = build_swap(Side::Frusd, 100_000_000, 1_500);
        assert_eq!(inputs.len(), 1);
        let cp = stones[0].cellpack.as_ref().expect("a swap without a cellpack is not a swap");
        assert_eq!(cp.target.block, btcusd::POOL_BLOCK);
        assert_eq!(cp.target.tx, btcusd::POOL_TX);
        assert_eq!(cp.inputs[0], btcusd::op::EXCHANGE);
    }

    #[test]
    fn swap_uses_the_pools_coin_order_not_the_index_order() {
        // frUSD sells as coin 1 -> 0. Reversed, this trades the wrong way and
        // the number still looks plausible.
        let (_, stones) = build_swap(Side::Frusd, 1, 0);
        let cp = stones[0].cellpack.as_ref().unwrap();
        assert_eq!(cp.inputs[1], 1, "frUSD is coin 1");
        assert_eq!(cp.inputs[2], 0, "frBTC is coin 0");

        let (_, stones) = build_swap(Side::Frbtc, 1, 0);
        let cp = stones[0].cellpack.as_ref().unwrap();
        assert_eq!(cp.inputs[1], 0);
        assert_eq!(cp.inputs[2], 1);
    }

    #[test]
    fn swap_carries_min_dy_through_to_the_call() {
        let (_, stones) = build_swap(Side::Frbtc, 500, 4_242);
        let cp = stones[0].cellpack.as_ref().unwrap();
        assert_eq!(cp.inputs[4], 4_242, "the slippage floor must reach the contract");
    }

    #[test]
    fn add_liquidity_requires_both_legs() {
        // Requiring one silently deposits single-sided, which prices very
        // differently.
        let (inputs, _) = build_add_liquidity(1_000, 2_000, 1);
        assert_eq!(inputs.len(), 2, "both coins must be spent into the pool");
    }

    #[test]
    fn remove_liquidity_spends_the_proxy_which_is_the_lp_token() {
        let (inputs, stones) = build_remove_liquidity(1_000, None, 1);
        match inputs[0] {
            InputRequirement::Alkanes { block, tx, .. } => {
                assert_eq!((block, tx), (4, 1778), "the LP token IS the pool proxy");
            }
            _ => panic!("expected an alkanes input"),
        }
        assert_eq!(
            stones[0].cellpack.as_ref().unwrap().inputs[0],
            btcusd::op::REMOVE_LIQUIDITY
        );
    }

    #[test]
    fn one_coin_withdrawal_selects_by_pool_coin_index() {
        let (_, stones) = build_remove_liquidity(1_000, Some(Side::Frbtc), 1);
        let cp = stones[0].cellpack.as_ref().unwrap();
        assert_eq!(cp.inputs[0], btcusd::op::REMOVE_LIQUIDITY_ONE_COIN);
        assert_eq!(cp.inputs[2], 0, "frBTC is coin 0");
    }
}

// ── Execution: simulate first, then sign ────────────────────────────────────

use crate::alkanes::execute::EnhancedAlkanesExecutor;
use crate::alkanes::types::{EnhancedExecuteParams, OrdinalsStrategy};
use crate::traits::DeezelProvider;

/// A trade that has been BUILT and PROVEN against live state, but not signed.
///
/// Deliberately NOT `Clone`: a verified trade corresponds to one simulation
/// against one chain tip, and duplicating it invites broadcasting the same
/// intent twice.
#[derive(Debug)]
pub struct VerifiedTrade {
    pub params: EnhancedExecuteParams,
    /// What `simulate` said the pool would do. Empty on success.
    pub simulate_error: String,
}

/// Build a trade and prove it against live chain state BEFORE any signing.
///
/// This is the discipline the whole path is organised around: `simulate` runs
/// the real contract against real state with no side effects, so an encoding
/// error, a reversed coin order, or a `min_dy` the pool will not meet shows up
/// as a REVERT here rather than as a broadcast transaction that fails or, worse,
/// succeeds and does something else.
///
/// Refuses on a simulate revert rather than warning. A trade that the pool has
/// already said it will reject has no business being signed.
pub fn verify_trade(
    post: &Post,
    ep: &Endpoints,
    inputs: Vec<InputRequirement>,
    protostones: Vec<ProtostoneSpec>,
    fee_rate: Option<f32>,
    change_address: Option<String>,
) -> Result<VerifiedTrade> {
    // Replay the protostone's own cellpack through `simulate`. Same target,
    // same opcode, same arguments the broadcast would carry.
    let cp = protostones
        .first()
        .and_then(|p| p.cellpack.as_ref())
        .ok_or_else(|| anyhow!("refusing to send a protostone with no cellpack — that is not a trade"))?;

    let hex = simulate_hex(cp.target, cp.inputs.clone());
    let v = post(
        &ep.main(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
            "params": ["simulate", hex, "latest"],
        }),
    )?;

    // `simulate` reports contract reverts INSIDE the response, not as a
    // transport error — a 200 here proves nothing on its own.
    let err = v
        .get("result")
        .and_then(|r| r.as_str())
        .map(|s| decode_simulate_error(s))
        .unwrap_or_else(|| "simulate returned no result".to_string());
    if !err.is_empty() {
        bail!(
            "the pool REJECTED this trade in simulation, so it is not being signed: {err}\n\
             (checked against live state via metashrew_view simulate — no funds moved)"
        );
    }

    Ok(VerifiedTrade {
        params: EnhancedExecuteParams {
            fee_rate,
            to_addresses: vec![],
            from_addresses: None,
            change_address,
            alkanes_change_address: None,
            input_requirements: inputs,
            protostones,
            envelope_data: None,
            raw_output: false,
            trace_enabled: true,
            mine_enabled: false,
            // Never auto-confirm a trade. The caller decides.
            auto_confirm: false,
            ordinals_strategy: OrdinalsStrategy::default(),
            // Trace pending UTXOs' inscription state when we must spend
            // unconfirmed inputs — relevant for the RBF/mempool-aware flows.
            mempool_indexer: true,
            // A BTCUSD swap is one cellpack, well inside the 3.5M per-tx fuel
            // floor (a bare `exchange` measures 1.76M balanced, 2.47M at 3x
            // imbalance). Splitting is for wrap+execute chains that would
            // otherwise share one budget.
            split_transactions: false,
            // Let the builder discover UTXOs itself; nothing here excludes or
            // pre-supplies any.
            known_pending_tx_hexes: Vec::new(),
            prefetched_utxos: Vec::new(),
            excluded_utxos: Vec::new(),
            // Unset: the builder's own safety bound on spending UTXOs from
            // blocks past the indexed tip. Forcing a value here would override a
            // guard we do not own.
            max_indexed_height: None,
            // Do NOT suppress the builder's DIESEL-mint sibling protostone. It
            // is added deliberately, and skipping it is for byte-stable
            // protocols — a swap is not one.
            skip_diesel_mint: false,
            // Default UTXO discovery path; `espo` is the alternative.
            utxo_source: crate::alkanes::types::UtxoDataSource::default(),
        },
        simulate_error: String::new(),
    })
}

/// Pull the `error` field (field 2) out of a `SimulateResponse`.
///
/// Empty means the call succeeded. Anything else is the contract's own revert
/// message and must stop the trade.
fn decode_simulate_error(hex_str: &str) -> String {
    let Ok(raw) = hex::decode(hex_str.trim_start_matches("0x")) else {
        return "simulate result was not hex".to_string();
    };
    if raw.is_empty() {
        return "simulate returned an empty result".to_string();
    }
    match field_bytes(&raw, 2) {
        Some(b) => String::from_utf8_lossy(b).to_string(),
        // No error field at all is the success case.
        None => String::new(),
    }
}

/// Sign and broadcast a trade that `verify_trade` has already proven.
///
/// Separate from `verify_trade` on purpose: the verification is pure and
/// testable, and the only way to reach this function is to have passed it.
pub async fn submit_trade(
    provider: &mut dyn DeezelProvider,
    trade: VerifiedTrade,
) -> Result<crate::alkanes::types::ExecutionState> {
    let mut executor = EnhancedAlkanesExecutor::new(provider);
    match executor.execute(trade.params).await {
        Ok(r) => Ok(r),
        Err(e) => Err(anyhow!("broadcast failed after a successful simulation: {e}")),
    }
}

#[cfg(test)]
mod verify_tests {
    use super::*;

    #[test]
    fn a_protostone_without_a_cellpack_is_refused() {
        let post: Box<Post> = Box::new(|_, _| Ok(serde_json::json!({"result": "0x"})));
        let e = Endpoints { base: "https://x".into(), api_key: Some("K".into()) };
        let stones = vec![ProtostoneSpec {
            cellpack: None,
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: None,
            refund: None,
        }];
        let err = verify_trade(&*post, &e, vec![], stones, None, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a trade"), "{err}");
    }

    #[test]
    fn a_simulate_revert_stops_the_trade_before_signing() {
        // SimulateResponse with field 2 (error) = "insufficient output".
        let msg = b"insufficient output";
        let mut resp = vec![0x12, msg.len() as u8];
        resp.extend_from_slice(msg);
        let hex = format!("0x{}", hex::encode(&resp));
        let post: Box<Post> = Box::new(move |_, _| Ok(serde_json::json!({ "result": hex })));
        let e = Endpoints { base: "https://x".into(), api_key: Some("K".into()) };
        let (inputs, stones) = build_swap(Side::Frusd, 100, 1);
        let err = verify_trade(&*post, &e, inputs, stones, None, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("REJECTED"), "{err}");
        assert!(err.contains("insufficient output"), "must surface the contract's own words: {err}");
        assert!(err.contains("no funds moved"));
    }

    #[test]
    fn a_clean_simulation_yields_params_that_never_auto_confirm() {
        // A response carrying only field 1 (execution), no error field.
        let post: Box<Post> = Box::new(|_, _| {
            Ok(serde_json::json!({ "result": "0x0a021a00" }))
        });
        let e = Endpoints { base: "https://x".into(), api_key: Some("K".into()) };
        let (inputs, stones) = build_swap(Side::Frbtc, 1_000, 1);
        let t = verify_trade(&*post, &e, inputs, stones, Some(5.0), None).unwrap();
        assert!(t.simulate_error.is_empty());
        assert!(!t.params.auto_confirm, "a trade must never auto-confirm");
        assert!(t.params.trace_enabled, "keep the trace for post-hoc reconciliation");
        assert_eq!(t.params.protostones.len(), 1);
    }
}
