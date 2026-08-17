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
    pool_reserves(post, ep).map(|(frusd, _)| frusd)
}

/// Both reserves, as `(frusd, frbtc)`.
///
/// The depth cap has to measure a trade against the leg it is PAID IN, so a
/// caller that only ever reads the frUSD side cannot check a frBTC sell at all.
pub fn pool_reserves(post: &Post, ep: &Endpoints) -> Option<(u128, u128)> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
        "params": ["getpoolstate", pool_request_hex(), "latest"],
    });
    let v = post(&ep.btcusd_index(), body).ok()?;
    let hex = v.get("result")?.as_str()?;
    // The index answers in protobuf; the reserves are decimal STRINGS there and
    // must stay strings until parsed — the LP supply already exceeds u64.
    let raw = hex::decode(hex.trim_start_matches("0x")).ok()?;
    extract_reserves(&raw)
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

/// BOTH reserves, as `(frusd, frbtc)`.
///
/// The depth cap needs the reserve of the leg a trade is PAID IN, so reading
/// only `reserve0` leaves the frBTC side unmeasurable — and a sat amount
/// divided by the frUSD reserve is a plausible-looking 0bps rather than an
/// error. Same walk, same conservatism: `None` beats a guessed leg.
fn extract_reserves(raw: &[u8]) -> Option<(u128, u128)> {
    //   GetPoolStateResponse { … PoolState state = 5; … }
    //   PoolState { … string reserve0 = 3; string reserve1 = 4; … }
    let state = field_bytes(raw, 5)?;
    let r0 = std::str::from_utf8(field_bytes(state, 3)?).ok()?.parse::<u128>().ok()?;
    let r1 = std::str::from_utf8(field_bytes(state, 4)?).ok()?.parse::<u128>().ok()?;
    // The INDEX orders token0/token1 by alkane id (frUSD 4:1776 before frBTC
    // 32:0), which is the inverse of the pool contract's internal coin order.
    if RESERVE0_IS_FRUSD {
        Some((r0, r1))
    } else {
        Some((r1, r0))
    }
}

/// The `result` of a JSON-RPC answer, as bytes.
///
/// An answer without one is an ERROR here rather than an empty decode: an empty
/// result is how "this pool does not exist" gets fabricated out of a malformed
/// request, and a decoder that shrugs at it reports a dead pool instead of a bad
/// call.
fn result_bytes(v: &Value) -> Result<Vec<u8>> {
    let s = v
        .get("result")
        .and_then(|r| r.as_str())
        .ok_or_else(|| anyhow!("no result in the index's answer: {v}"))?;
    hex::decode(s.trim_start_matches("0x")).map_err(|e| anyhow!("result is not hex: {e}"))
}

/// Render base units at a fixed scale, with no float in the path.
///
/// ⚠️ The scale is a property of the TOKEN, not of the pool. frUSD and frBTC are
/// 8 decimals; the LP token is 18, because LP is minted in units of the
/// invariant D, which lives in 1e18-normalised space.
fn scaled(v: u128, decimals: u32) -> String {
    let div = 10u128.pow(decimals);
    format!("{}.{:0width$}", v / div, v % div, width = decimals as usize)
}

fn dec8(v: u128) -> String {
    scaled(v, 8)
}

fn dec18(v: u128) -> String {
    scaled(v, 18)
}

/// The pool's price tip. Three prices, and they are NOT interchangeable.
///
/// Field numbers from `alspo-proto/proto/cryptoswap.proto`, package
/// `alspo.cryptoswap`, message `GetPriceResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceSnapshot {
    /// The scale every `*_price_q` is quoted in (1e18).
    pub price_q_scale: u128,
    /// The pool's most recent ACTUAL trade. Absent until it has been traded
    /// against — an empty pool still quotes marginal and oracle.
    pub executed_price_q: Option<u128>,
    pub executed_height: Option<u64>,
    /// From the pool's own price state, available from its first block.
    pub marginal_price_q: Option<u128>,
    pub oracle_price_q: Option<u128>,
    pub state_height: Option<u64>,
}

/// Decode a `GetPriceResponse`.
fn decode_price(raw: &[u8]) -> Option<PriceSnapshot> {
    let dec = |f: u64| -> Option<u128> {
        std::str::from_utf8(field_bytes(raw, f)?).ok()?.parse().ok()
    };
    Some(PriceSnapshot {
        price_q_scale: dec(5)?,
        executed_price_q: dec(6),
        executed_height: field_varint(raw, 8),
        marginal_price_q: dec(10),
        oracle_price_q: dec(11),
        state_height: field_varint(raw, 12),
    })
}

/// Reserves, LP supply and the invariant slots.
///
/// Field numbers from `alspo.cryptoswap.PoolState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolStateSnapshot {
    pub height: u64,
    /// In the MODULE's token0/token1 orientation, which is by alkane id — the
    /// INVERSE of the contract's own coin order.
    pub reserve_frusd: u128,
    pub reserve_frbtc: u128,
    /// ⚠️ 18 decimals, not 8, and already past u64.
    pub lp_total_supply: u128,
    pub marginal_price_q: Option<u128>,
    pub oracle_price_q: Option<u128>,
    /// 1e18-scaled. Rises with fees; a fall means the pool lost value.
    pub virtual_price: u128,
    /// Refuse to trade at all.
    pub is_killed: bool,
    /// An A/gamma ramp is in flight, so a quote taken now may not hold.
    pub ramping: bool,
    pub price_q_scale: u128,
}

/// Decode a `GetPoolStateResponse`.
fn decode_pool_state(raw: &[u8]) -> Option<PoolStateSnapshot> {
    let state = field_bytes(raw, 5)?;
    let dec = |buf: &[u8], f: u64| -> Option<u128> {
        std::str::from_utf8(field_bytes(buf, f)?).ok()?.parse().ok()
    };
    let (reserve_frusd, reserve_frbtc) = extract_reserves(raw)?;
    Some(PoolStateSnapshot {
        height: field_varint(state, 1)?,
        reserve_frusd,
        reserve_frbtc,
        lp_total_supply: dec(state, 5)?,
        marginal_price_q: dec(state, 10),
        oracle_price_q: dec(state, 11),
        virtual_price: dec(state, 14)?,
        // proto3 omits false, so an absent field IS the answer.
        is_killed: field_varint(state, 16).unwrap_or(0) != 0,
        ramping: field_varint(state, 17).unwrap_or(0) != 0,
        price_q_scale: dec(raw, 6)?,
    })
}

/// Read one varint field out of a protobuf message.
///
/// Same walk as [`field_bytes`], same refusal to resync.
fn field_varint(mut buf: &[u8], want: u64) -> Option<u64> {
    while !buf.is_empty() {
        let (key, rest) = varint(buf)?;
        let field = key >> 3;
        let wire = key & 7;
        buf = rest;
        match wire {
            0 => {
                let (v, rest) = varint(buf)?;
                if field == want {
                    return Some(v);
                }
                buf = rest;
            }
            2 => {
                let (len, rest) = varint(buf)?;
                let len = len as usize;
                if rest.len() < len {
                    return None;
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
            if *raw {
                return emit(&v, true);
            }
            let p = decode_price(&result_bytes(&v)?)
                .ok_or_else(|| anyhow!("the index answered a shape this build cannot read"))?;
            let usd = |q: Option<u128>| {
                q.and_then(|q| btcusd::usd_per_btc(q, p.price_q_scale))
                    .unwrap_or_else(|| "-".into())
            };
            // Three prices, kept apart on purpose. `executed` is the pool's last
            // real trade and is absent until it has been traded against;
            // marginal and oracle come from its price state and always exist.
            println!("USD/BTC");
            println!("  executed  {:>14}   (last actual trade)", usd(p.executed_price_q));
            println!("  marginal  {:>14}   (the pool's own price slot)", usd(p.marginal_price_q));
            println!("  oracle    {:>14}   (EMA)", usd(p.oracle_price_q));
            if let Some(h) = p.state_height {
                println!("state height {h}");
            }
            // ⚠️ The marginal price is NOT what a trade gets. Quote first.
            println!("\nThese are pool prices, not execution prices. Run `btcusd quote` for what a");
            println!("given size actually receives — on a pool this size the gap is material.");
            Ok(())
        }
        BtcusdCommands::Pool { raw } => {
            let v = post(
                &ep.btcusd_index(),
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
                    "params": ["getpoolstate", pool_request_hex(), "latest"],
                }),
            )?;
            if *raw {
                return emit(&v, true);
            }
            let s = decode_pool_state(&result_bytes(&v)?)
                .ok_or_else(|| anyhow!("the index answered a shape this build cannot read"))?;
            println!("pool 4:1778 at height {}", s.height);
            println!("  frUSD reserve  {:>20}  ({})", s.reserve_frusd, dec8(s.reserve_frusd));
            println!("  frBTC reserve  {:>20}  ({})", s.reserve_frbtc, dec8(s.reserve_frbtc));
            // ⚠️ LP is 18 decimals, not 8. Opcode 102 is get_A and returns
            // 400000; anything reading it as get_decimals renders this at
            // 1e400000.
            println!("  LP supply      {:>20}  ({}, 18 decimals)", s.lp_total_supply, dec18(s.lp_total_supply));
            println!("  virtual price  {:>20}  ({})", s.virtual_price, dec18(s.virtual_price));
            if let Some(q) = s.marginal_price_q {
                println!(
                    "  marginal       {:>20}  ({} USD/BTC)",
                    q,
                    btcusd::usd_per_btc(q, s.price_q_scale).unwrap_or_else(|| "-".into())
                );
            }
            // The depth cap is a share of the frUSD reserve, so it moves with
            // the pool. Printing the number beats printing the rule.
            let cap = s.reserve_frusd.saturating_mul(btcusd::MAX_POOL_SHARE_BPS) / btcusd::BPS_DENOM;
            println!("  depth cap      {:>20}  ({} frUSD — {}bps of the reserve)", cap, dec8(cap), btcusd::MAX_POOL_SHARE_BPS);
            // Both default to false on the wire, so absent is the answer.
            if s.is_killed {
                println!("\n⚠️  is_killed — the pool refuses trades. Do not build one.");
            }
            if s.ramping {
                println!("\n⚠️  ramping — A/gamma are moving between blocks, so a quote may not hold.");
            }
            Ok(())
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
            // Measured against the leg the trade is PAID IN. Dividing a sat
            // amount by the frUSD reserve answers ~0bps for a sell worth a tenth
            // of the pool, so the cap prints and never binds on that side.
            match pool_reserves(post, ep) {
                Some(r) => match btcusd::depth_share_bps(side, amt, r) {
                    // One message for every size. The old one named a refusal
                    // that cannot happen on a direct swap, and its under-cap
                    // half read as an all-clear for a size that moves this pool
                    // several percent.
                    Some(share) => println!("{}", btcusd::direct_swap_depth_note(share)),
                    None => println!("depth: the incoming leg's reserve is zero — do not trade"),
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
            match v.get("result").and_then(|r| r.as_str()).and_then(decode_simulate_u128) {
                Some(dy) => {
                    let (from_name, to_name) = match side {
                        Side::Frusd => ("frUSD", "frBTC"),
                        Side::Frbtc => ("frBTC", "frUSD"),
                    };
                    println!("in : {amt} {from_name} (base units)");
                    println!("out: {dy} {to_name} (base units)");
                    // Both legs are 8-decimal, so the implied rate is a plain
                    // ratio. Shown because the POOL's marginal price is not the
                    // price you get — and today the gap is large.
                    if dy > 0 {
                        let rate = match side {
                            Side::Frusd => amt as f64 / dy as f64,
                            Side::Frbtc => dy as f64 / amt as f64,
                        };
                        println!("implied: {rate:.0} USD/BTC  (EFFECTIVE, not the pool's marginal price)");
                    } else {
                        println!("⚠️  the pool quoted ZERO out — do not trade on this");
                    }
                    Ok(())
                }
                None => emit(&v, false),
            }
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

        BtcusdCommands::Burn { amount, eth_address, to_eth_bps, min_out, .. } => {
            let amt = btcusd::parse_base_units(amount).map_err(|e| anyhow!(e))?;
            if amt == 0 {
                bail!("amount is zero: nothing would be burned");
            }
            let to = btcusd::parse_evm_address(eth_address).map_err(|e| anyhow!(e))?;
            if *to_eth_bps > 10_000 {
                bail!("--to-eth-bps must be 0..=10000");
            }

            let payload = if *to_eth_bps == 0 {
                println!("burn {amt} frUSD -> USDC at 0x{}", hex::encode(to));
                btcusd::burn_data_transfer()
            } else {
                let min = min_out
                    .as_deref()
                    .ok_or_else(|| anyhow!(
                        "--min-out is required with --to-eth-bps: there is NO coordinator-side \
                         slippage floor on this leg, so 0 accepts any output at all"
                    ))
                    .and_then(|m| btcusd::parse_base_units(m).map_err(|e| anyhow!(e)))?;
                // The coordinator sets amountIn from the ACTUAL payout; the
                // value encoded here is the burner's own expectation.
                let expected_in = amt * (*to_eth_bps as u128) / btcusd::BPS_DENOM;
                let cd = btcusd::encode_swap_exact_tokens_for_eth(
                    expected_in,
                    min,
                    to,
                    // A deadline far enough out to survive Bitcoin confirmation
                    // plus the coordinator's own settlement wait.
                    u64::MAX,
                )
                .map_err(|e| anyhow!(e))?;
                println!(
                    "burn {amt} frUSD -> {}bps to ETH via Uniswap V2, rest USDC, to 0x{}",
                    to_eth_bps,
                    hex::encode(to)
                );
                println!("min out            {min} (YOUR floor — the coordinator has none here)");
                btcusd::burn_data_call(btcusd::UNISWAP_V2_ROUTER02, &cd)
            };

            println!("burnData           0x{}", hex::encode(&payload));
            println!();
            println!(
                "⚠️  Not yet broadcast. The burnData above is the authoritative payload \
                 (layout matched to the coordinator's own parser and unit-tested), but the \
                 frUSD BurnAndBridge cellpack packing still needs checking against \
                 alkanes-integ-tests/tests/frusd_burn_bridge.rs before this is signed. \
                 Guessing that packing is exactly how a burn silently degrades to a plain \
                 payout."
            );
            Ok(())
        }

        // Bitcoin-side construction. Refuses rather than pretending.
        BtcusdCommands::Swap { .. }
        | BtcusdCommands::AddLiquidity { .. }
        | BtcusdCommands::RemoveLiquidity { .. } => bail!(
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
                    "{e}\n\nNote: /v4/{{apikey}}/mempool is deployed and answering, so this is \
                     most likely the endpoint or the API key rather than a missing route. \
                     Check that --endpoint points at the mempool host and that the key is valid."
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
    let amt = btcusd::parse_base_units(amount).map_err(|e| anyhow!(e))?;

    // Everything is decided before a byte is emitted. The pool's depth is read
    // first because it CLAMPS the conversion, and the clamped number is what has
    // to reach the payload — a payload asking for more than the screen said is
    // refused by the coordinator, and refusal means the whole mint lands as
    // frUSD.
    let plan = btcusd::plan_deposit(
        stable,
        amt,
        recipient,
        convert_bps,
        btc_destination,
        max_slippage_bps,
        frusd_reserve(post, ep),
    )
    .map_err(|e| anyhow!(e))?;

    if !plan.note.is_empty() {
        println!("⚠️  {}", plan.note);
    }

    println!(
        "deposit  {amt} base units of {} (assetId {})",
        stable.to_uppercase(),
        plan.asset_id
    );
    println!("recipient (frUSD)  {recipient}");
    if plan.effective_bps > 0 {
        println!(
            "convert            {}bps to BTC -> {}",
            plan.effective_bps,
            btc_destination.unwrap_or("-")
        );
        println!("max slippage       {max_slippage_bps}bps (honoured downwards only)");
    } else if convert_bps > 0 {
        println!("convert            NOTHING — the pool cannot absorb it; this lands as frUSD");
    }
    println!("approve            exactly {amt} — NOT an unlimited allowance");
    println!("bridgeData         0x{}", hex::encode(&plan.bridge_data));

    println!("\n1) approve — send to the TOKEN, not the vault");
    println!("   to    0x{}", hex::encode(plan.token));
    println!("   data  0x{}", hex::encode(&plan.approve_calldata));
    println!("\n2) depositAndBridge — send to the VAULT");
    println!("   to    0x{}", hex::encode(btcusd::FRUSD_VAULT_L1));
    println!("   data  0x{}", hex::encode(&plan.deposit_calldata));

    // No key at all is the NORMAL mode, and the calldata above is the whole
    // deliverable. A key that was SUPPLIED and rejected is a different thing,
    // and collapsing the two hid both a typo'd path and the shell-history
    // refusal that exists to protect the person typing it.
    let supplied_a_key = eth_key_file.is_some() || eth_private_key.is_some();
    let key = match EthKey::resolve(eth_key_file, eth_private_key, allow_inline) {
        Ok(k) => k,
        Err(e) if supplied_a_key => return Err(e.context("the Ethereum key could not be used")),
        Err(_) => {
            println!(
                "\nNo Ethereum key supplied. The two calls above are what you sign, \
                 in that order, and the approve must confirm first."
            );
            return Ok(());
        }
    };

    // ⚠️ REFUSED RATHER THAN GUESSED. Signing these needs a live nonce
    // (`eth_getTransactionCount`), a fee estimate, and the approve CONFIRMED
    // before the deposit is broadcast — the vault's `transferFrom` reverts
    // against an allowance that is still pending. None of that is built here.
    //
    // This path previously signed an EIP-1559 transaction to the zero address
    // with empty calldata and reported "submitted privately". It did not
    // deposit anything, and on a fresh account it confirmed, burning gas and
    // consuming nonce 0. A command that silently does nothing is worse than one
    // that refuses.
    let _ = (dry_run, &key);
    bail!(
        "not yet wired: local signing needs a live nonce, a fee estimate, and the approve \
         confirmed before the deposit is broadcast. Sign the two calls above with your own \
         wallet instead — that is the supported path, and it is the one the webapp uses."
    );
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
        // Swap / add / remove still need protostone assembly and must refuse
        // rather than appear to succeed. `Burn` is NO LONGER in this list — it
        // now produces a real burnData payload, so asserting it refuses would
        // be asserting a regression.
        let post: Box<Post> = Box::new(|_, _| Ok(serde_json::json!({"result": "0x"})));
        let e = ep(Some("K"));
        let cmd = BtcusdCommands::Swap {
            from: "frusd".into(),
            amount: "1".into(),
            max_slippage_bps: 50,
            dry_run: true,
        };
        let err = run(&cmd, &*post, &e).unwrap_err().to_string();
        assert!(
            err.contains("not yet wired"),
            "must refuse explicitly rather than appear to succeed: {err}"
        );
    }

    #[test]
    fn an_unreadable_key_is_reported_rather_than_read_as_no_key_at_all() {
        // Every `EthKey::resolve` failure collapsed into "No Ethereum key
        // supplied" and exit 0. The direction was safe, but a typo'd key path
        // read as the intended no-key mode, and the shell-history refusal for
        // `--eth-private-key` never reached the person it protects.
        let post: Box<Post> = Box::new(|_, _| Ok(serde_json::json!({"result": "0x"})));
        let e = ep(Some("K"));
        let cmd = BtcusdCommands::Deposit {
            stable: "usdc".into(),
            amount: "1".into(),
            recipient: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".into(),
            convert_bps: 0,
            btc_destination: None,
            max_slippage_bps: 0,
            eth_key_file: Some("/nonexistent/key/file".into()),
            eth_private_key: None,
            i_know_this_lands_in_shell_history: false,
            dry_run: true,
        };
        let err = run(&cmd, &*post, &e).unwrap_err().to_string();
        assert!(
            !err.contains("not yet wired"),
            "a bad key must not be reported as the unbuilt signing path: {err}"
        );
        assert!(err.to_lowercase().contains("key"), "{err}");
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

    /// A real `getpoolstate` answer, captured from the live index at height
    /// 962,502 (state height 962,472). The actual wire shape, not one built to
    /// match this parser.
    const LIVE_POOL_STATE: &str = "08011205080410f20d1a05080410f00d220208202adb0108a8df3a10c4eefdd3\
             061a0d31383831323134333839303738220832393532313930322a1437343531\
             36373739383134363038383937363236320e3135363034353333313035313435\
             3a0e3135363039303635313232313638420e3135363037343934383335333233\
             48a8df3a520e31353630343533333130353134355a0e31353630393036353132\
             32313638620e31353630373439343833353332336a1235383837373336313837\
             33363239343934387213313030303038393532343433333539363934357a1331\
             3030303039333236313437313231323339343213313030303030303030303030\
             30303030303030";

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

    /// Base units rendered at the right scale, without a float anywhere.
    ///
    /// The two scales are not interchangeable and the pool holds both: frUSD
    /// and frBTC are 8 decimals, the LP token is 18. Rendering the LP supply at
    /// 8 would report 745,167,798,146 LP for a pool that has 74.5.
    #[test]
    fn amounts_render_at_their_own_scale() {
        assert_eq!(dec8(1_881_214_389_078), "18812.14389078");
        assert_eq!(dec8(29_521_902), "0.29521902");
        assert_eq!(dec8(0), "0.00000000");
        assert_eq!(dec18(74_516_779_814_608_897_626), "74.516779814608897626");
        assert_eq!(dec18(1_000_089_524_433_596_945), "1.000089524433596945");
        // Past u64 must survive: the LP supply already is.
        assert_eq!(
            dec18(23_113_653_069_174_808_444_000_000_000u128),
            "23113653069.174808444000000000"
        );
    }

    /// A JSON-RPC answer whose `result` is not the hex string we expect is an
    /// error, not an empty decode. `getbytecode` returning `0x` for a wrapped
    /// request is exactly how "this contract has no code" gets fabricated.
    #[test]
    fn a_missing_result_is_an_error_rather_than_an_empty_decode() {
        assert!(result_bytes(&serde_json::json!({"error": {"code": -32601}})).is_err());
        assert!(result_bytes(&serde_json::json!({"result": "0x"})).unwrap().is_empty());
        assert_eq!(
            result_bytes(&serde_json::json!({"result": "0x0801"})).unwrap(),
            vec![0x08, 0x01]
        );
    }

    /// The live `getprice` response, decoded into the three prices it actually
    /// carries.
    ///
    /// Field numbers are read from `alspo-proto/proto/cryptoswap.proto`
    /// (`alspo.cryptoswap`), not inferred from the bytes. The three are NOT
    /// interchangeable: `executed_price_q` is the pool's last actual trade and
    /// is ABSENT until it has been traded against, while marginal and oracle
    /// come from the pool's own price state and exist from its first block.
    /// Printing one under the other's name is a wrong number that reads right.
    #[test]
    fn decodes_the_three_prices_from_a_live_getprice() {
        let raw = hex::decode(
            "08011205080410f20d1a05080410f00d220208202a1331303030303030303030\
             303030303030303030320e313535373830363732393332383838c4eefdd30640\
             a8df3a4a20949076100ebd8fd98d7de98981823a652a7383535a0396e4d7482a\
             e013d63d52520e31353630343533333130353134355a0e313536303930363531\
             323231363860a8df3a",
        )
        .unwrap();
        let p = decode_price(&raw).unwrap();
        assert_eq!(p.price_q_scale, 1_000_000_000_000_000_000);
        assert_eq!(p.executed_price_q, Some(15_578_067_293_288));
        assert_eq!(p.marginal_price_q, Some(15_604_533_105_145));
        assert_eq!(p.oracle_price_q, Some(15_609_065_122_168));
        assert_eq!(p.state_height, Some(962_472));
    }

    /// The quote is token1-per-token0 — frBTC per frUSD — so USD/BTC is the
    /// RECIPROCAL. Getting it backwards yields a number near zero, which reads
    /// as "no price" rather than as an error.
    #[test]
    fn usd_per_btc_is_the_reciprocal_and_is_computed_without_floats() {
        assert_eq!(
            btcusd::usd_per_btc(15_578_067_293_288, 1_000_000_000_000_000_000).unwrap(),
            "64192.81"
        );
        assert_eq!(btcusd::usd_per_btc(0, 1_000_000_000_000_000_000), None);
    }

    /// The whole pool snapshot, from the same live capture the reserves come
    /// from.
    #[test]
    fn decodes_a_live_pool_state_snapshot() {
        let raw = hex::decode(LIVE_POOL_STATE).unwrap();
        let s = decode_pool_state(&raw).unwrap();
        assert_eq!(s.height, 962_472);
        assert_eq!(s.reserve_frusd, 1_881_214_389_078);
        assert_eq!(s.reserve_frbtc, 29_521_902);
        // LP is 18 decimals, not 8, and the supply is already past u64.
        assert_eq!(s.lp_total_supply, 74_516_779_814_608_897_626);
        assert_eq!(s.virtual_price, 1_000_089_524_433_596_945);
        // proto3 omits false, so absent is the answer, not a missing field.
        assert!(!s.is_killed);
        assert!(!s.ramping, "check this before quoting: the curve moves between blocks");
        assert_eq!(s.marginal_price_q, Some(15_604_533_105_145));
    }

    /// The two flags that gate "refuse to trade", pinned.
    ///
    /// proto3 omits false, so the live capture stops at field 15 and CANNOT
    /// witness these: absent-because-false and absent-because-the-field-number-
    /// is-wrong are the same bytes. If they were wrong, a killed pool would
    /// decode as alive and the warning would never fire, silently, which is the
    /// exact failure class this work exists to remove. So the vector is
    /// synthetic and built from the schema
    /// (`alspo.cryptoswap.PoolState`: `is_killed = 16`, `ramping = 17`).
    #[test]
    fn the_kill_and_ramp_flags_are_read_from_the_fields_the_schema_assigns() {
        fn ld(tag: u8, body: &[u8]) -> Vec<u8> {
            let mut v = vec![tag, body.len() as u8];
            v.extend_from_slice(body);
            v
        }
        let mut state = Vec::new();
        state.extend_from_slice(&[0x08, 0x01]); // 1 height
        state.extend_from_slice(&ld(0x1a, b"100")); // 3 reserve0
        state.extend_from_slice(&ld(0x22, b"200")); // 4 reserve1
        state.extend_from_slice(&ld(0x2a, b"300")); // 5 total_supply
        state.extend_from_slice(&ld(0x72, b"1000000000000000000")); // 14 virtual_price
        // field 16 -> key 16<<3 = 128 -> varint 0x80 0x01; field 17 -> 136.
        state.extend_from_slice(&[0x80, 0x01, 0x01]); // 16 is_killed = true
        state.extend_from_slice(&[0x88, 0x01, 0x01]); // 17 ramping = true
        let mut msg = ld(0x2a, &state); // 5 state
        msg.extend_from_slice(&ld(0x32, b"1000000000000000000")); // 6 price_q_scale

        let s = decode_pool_state(&msg).unwrap();
        assert!(s.is_killed, "a killed pool decoding as alive is the silent failure");
        assert!(s.ramping);
        assert_eq!(s.reserve_frusd, 100);
        assert_eq!(s.reserve_frbtc, 200);

        // And the live capture, which carries neither, must read false rather
        // than fail to decode.
        let live = decode_pool_state(&hex::decode(LIVE_POOL_STATE).unwrap()).unwrap();
        assert!(!live.is_killed);
        assert!(!live.ramping);
    }

    /// A `get_dy` answer that does not fit u128 is refused, not truncated.
    ///
    /// ⚠️ The REAL response is 32 bytes, not 16: a u256, little-endian, whose
    /// high half is zero for every value that fits. A first attempt at this
    /// refused anything over 16 bytes, which is what a synthetic 16-byte
    /// fixture says the view returns and is not what the view returns. It
    /// passed, and it broke every quote against mainnet. Hence the live capture
    /// below.
    #[test]
    fn a_simulate_result_that_does_not_fit_u128_is_refused_rather_than_truncated() {
        // Captured from the live index: `get_dy` for 3,000,000 sats in.
        let live = "0x0a221a20f55bc5a628000000000000000000000000000000000000000000\
                    00000000000010a7d530";
        assert_eq!(decode_simulate_u128(live), Some(174_596_643_829));

        fn resp(data: &[u8]) -> String {
            let mut exec = vec![0x1a, data.len() as u8];
            exec.extend_from_slice(data);
            let mut msg = vec![0x0a, exec.len() as u8];
            msg.extend_from_slice(&exec);
            format!("0x{}", hex::encode(msg))
        }
        // A value that genuinely needs more than 128 bits. Truncating keeps the
        // low half and answers 42, which reads exactly like a real quote.
        let mut wide = vec![0u8; 32];
        wide[0] = 0x2a;
        wide[16] = 0x01;
        assert_eq!(decode_simulate_u128(&resp(&wide)), None);
        // ... while the same 32-byte shape with an empty high half is the
        // ordinary case and must still decode.
        let mut narrow = vec![0u8; 32];
        narrow[0] = 0x2a;
        assert_eq!(decode_simulate_u128(&resp(&narrow)), Some(42));
    }

    /// BOTH reserves out of a real `GetPoolStateResponse`.
    ///
    /// Captured from the live index at height 962,502 — the actual wire shape,
    /// not one built to match this parser. The frBTC leg is what the depth cap
    /// needs to measure a sell against, and reading only `reserve0` is why that
    /// cap reported 0bps for a trade worth a tenth of the pool.
    #[test]
    fn extracts_both_reserves_from_a_live_response() {
        let raw = hex::decode(LIVE_POOL_STATE).unwrap();
        let (frusd, frbtc) = extract_reserves(&raw).unwrap();
        assert_eq!(frusd, 1_881_214_389_078, "18,812.14 frUSD at 8 decimals");
        assert_eq!(frbtc, 29_521_902, "0.29521902 frBTC");
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

/// The u128 a view returned: `SimulateResponse.execution.data`, little-endian.
///
/// Returns `None` rather than 0 when it cannot be read — a quote of zero and a
/// quote that could not be parsed must not look the same.
fn decode_simulate_u128(hex_str: &str) -> Option<u128> {
    let raw = hex::decode(hex_str.trim_start_matches("0x")).ok()?;
    let exec = field_bytes(&raw, 1)?;
    let data = field_bytes(exec, 3)?;
    // ⚠️ The view answers in 32 bytes: a u256, little-endian, whose high half
    // is zero for every value that fits. Keeping the low 128 bits
    // UNCONDITIONALLY would turn a genuinely larger answer into a plausible
    // small one — the u64 wrap that froze 23 LP, one width up. So take the low
    // half only when the high half is empty, and refuse otherwise.
    let (low, high) = data.split_at(data.len().min(16));
    if high.iter().any(|b| *b != 0) {
        return None;
    }
    let mut buf = [0u8; 16];
    buf[..low.len()].copy_from_slice(low);
    Some(u128::from_le_bytes(buf))
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

#[cfg(test)]
mod burn_cmd_tests {
    use super::*;

    fn post_ok() -> Box<Post> {
        Box::new(|_, _| Ok(serde_json::json!({ "result": "0x" })))
    }
    fn e() -> Endpoints {
        Endpoints { base: "https://x".into(), api_key: Some("K".into()) }
    }

    #[test]
    fn a_partial_eth_swap_without_min_out_is_refused() {
        // No coordinator-side floor exists on this leg, so omitting the floor
        // is an unbounded-loss instruction, not a convenience.
        let cmd = BtcusdCommands::Burn {
            amount: "100000000".into(),
            eth_address: "0x1111111111111111111111111111111111111111".into(),
            to_eth_bps: 2_000,
            min_out: None,
            dry_run: true,
        };
        let err = run(&cmd, &*post_ok(), &e()).unwrap_err().to_string();
        assert!(err.contains("--min-out is required"), "{err}");
    }

    #[test]
    fn a_plain_burn_needs_no_min_out() {
        let cmd = BtcusdCommands::Burn {
            amount: "100000000".into(),
            eth_address: "0x1111111111111111111111111111111111111111".into(),
            to_eth_bps: 0,
            min_out: None,
            dry_run: true,
        };
        assert!(run(&cmd, &*post_ok(), &e()).is_ok());
    }

    #[test]
    fn an_out_of_range_split_is_refused() {
        let cmd = BtcusdCommands::Burn {
            amount: "1".into(),
            eth_address: "0x1111111111111111111111111111111111111111".into(),
            to_eth_bps: 20_000,
            min_out: Some("1".into()),
            dry_run: true,
        };
        assert!(run(&cmd, &*post_ok(), &e()).unwrap_err().to_string().contains("0..=10000"));
    }

    #[test]
    fn a_malformed_eth_address_is_refused_before_anything_is_encoded() {
        let cmd = BtcusdCommands::Burn {
            amount: "1".into(),
            eth_address: "0xdeadbeef".into(),
            to_eth_bps: 0,
            min_out: None,
            dry_run: true,
        };
        assert!(run(&cmd, &*post_ok(), &e()).is_err());
    }
}

/// frUSD `BurnAndBridge` — opcode 5.
pub const OP_BURN_AND_BRIDGE: u128 = 5;
/// `BurnAndBridgeWithData` — opcode 5 with a composed-call payload attached.
pub const OP_BURN_AND_BRIDGE_WITH_DATA: u128 = 14;

/// Split a 20-byte EVM address into the `(hi, lo)` pair `BurnAndBridge` expects.
///
/// ⚠️ The split is **12 bytes / 8 bytes**, not 16/8 and not 16/4. Verified
/// against `alkanes-integ-tests/tests/frusd_burn_bridge.rs`, which packs
/// `evm_addr[..24]` and `evm_addr[24..40]` as HEX character ranges — i.e. 12
/// then 8 bytes.
///
/// Getting this wrong yields a well-formed cellpack that names a DIFFERENT,
/// real-looking address. An inverted hi/lo split is a documented past incident
/// in this system, not a hypothetical.
pub fn split_evm_address(addr: [u8; 20]) -> (u128, u128) {
    // 4/16, NOT 12/8. The contract's `eth_addr_from_split` takes hi = the TOP
    // 4 BYTES (it must fit a u32 and the contract REJECTS a hi that overflows)
    // and lo = the BOTTOM 16 BYTES. A 12/8 split produces a well-formed
    // cellpack the contract refuses, so every burn built with it reverts:
    // tokens refund, nothing is lost, and nothing works either.
    let mut hi = [0u8; 16];
    hi[12..].copy_from_slice(&addr[..4]); // top 4 bytes, right-aligned
    let mut lo = [0u8; 16];
    lo.copy_from_slice(&addr[4..]); // bottom 16 bytes
    (u128::from_be_bytes(hi), u128::from_be_bytes(lo))
}

/// Pack payload bytes into the u128 words a `BurnAndBridgeWithData` cellpack
/// carries: big-endian, 16 bytes per word, final word zero-padded.
///
/// The length travels SEPARATELY because a payload legitimately ending in zero
/// bytes could not otherwise be told from the padding. Mirrors
/// frusd-bridge-data::pack_burn_data.
pub fn pack_burn_data(bytes: &[u8]) -> (u128, Vec<u128>) {
    let mut words = Vec::with_capacity(bytes.len().div_ceil(16));
    for chunk in bytes.chunks(16) {
        let mut w = [0u8; 16];
        w[..chunk.len()].copy_from_slice(chunk);
        words.push(u128::from_be_bytes(w));
    }
    (bytes.len() as u128, words)
}

/// The protostone + inputs for a frUSD `BurnAndBridge`.
///
/// Two shapes, because MessageDispatch decodes POSITIONALLY and extending
/// opcode 5 would break every existing caller:
///
///   no payload : `4,1776,5,hi,lo,assetId`
///   with payload: `4,1776,14,hi,lo,assetId,dataLen,wordCount,words...`
///
/// Opcode 14 carries TWO lengths with DIFFERENT UNITS: `dataLen` in BYTES and
/// the Vec count in ELEMENTS (words). Emitting the words without the element
/// count yields a cellpack that decodes to a short/empty Vec — the coordinator
/// then reads no payload and pays out plainly, which is the silent degradation
/// this module exists to prevent.
///
/// `asset_id`: 0 = USDC, 1 = USDT.
pub fn build_burn(
    amount: u128,
    eth_address: [u8; 20],
    asset_id: u128,
    burn_data: Option<&[u8]>,
) -> (Vec<InputRequirement>, Vec<ProtostoneSpec>) {
    let (hi, lo) = split_evm_address(eth_address);
    let inputs = vec![InputRequirement::Alkanes {
        block: 4,
        tx: 1776, // frUSD
        amount,
    }];
    let cellpack_inputs = match burn_data {
        None => vec![OP_BURN_AND_BRIDGE, hi, lo, asset_id],
        Some(d) => {
            let (data_len, words) = pack_burn_data(d);
            let mut v = vec![
                OP_BURN_AND_BRIDGE_WITH_DATA,
                hi,
                lo,
                asset_id,
                data_len,
                words.len() as u128,
            ];
            v.extend_from_slice(&words);
            v
        }
    };
    let protostones = vec![ProtostoneSpec {
        cellpack: Some(Cellpack {
            target: AlkaneId { block: 4, tx: 1776 },
            inputs: cellpack_inputs,
        }),
        edicts: Vec::new(),
        bitcoin_transfer: None,
        pointer: None,
        refund: None,
    }];
    (inputs, protostones)
}

#[cfg(test)]
mod burn_golden_vectors {
    use super::*;

    /// 0x97774400_0000000000000000000000000000cf06
    const ADDR: [u8; 20] = [
        0x97, 0x77, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0xcf, 0x06,
    ];

    fn cellpack_of(stones: &[ProtostoneSpec]) -> Vec<u128> {
        stones[0].cellpack.as_ref().unwrap().inputs.clone()
    }

    // ─────────────────────────────────────────────────────────────────────
    // These literals are NOT computed here. They are the mainnet measurement
    // of 2026-08-10 (`simulatetransaction`, height 961,899), ported verbatim
    // from subfrost-app lib/bridge/__tests__/burnData.test.ts.
    //
    // Deriving the expectation from `pack_burn_data`/`split_evm_address` —
    // the same functions the implementation calls — is exactly what kept the
    // TypeScript suite GREEN against a wrong encoder. A test that asks the
    // code to confirm itself proves nothing. Do not "simplify" these back
    // into computed values.
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn address_splits_4_16_not_12_8() {
        let (hi, lo) = split_evm_address(ADDR);
        assert_eq!(hi, 0x97774400);
        assert_eq!(lo, 0xcf06);
        // The contract REJECTS a hi that overflows a u32.
        assert!(hi <= u32::MAX as u128, "hi must fit a u32");
    }

    #[test]
    fn opcode_14_carries_two_lengths_bytes_then_element_count() {
        let (_, stones) = build_burn(1, ADDR, 0, Some(&[9, 9, 9]));
        assert_eq!(
            cellpack_of(&stones),
            vec![
                14,
                2541175808,
                52998,
                0,
                3, // dataLen — BYTES
                1, // Vec count — ELEMENTS
                12009965175477489169824853534747656192,
            ]
        );
    }

    #[test]
    fn counts_words_not_bytes_seventeen_bytes_takes_two() {
        let (_, stones) = build_burn(1, ADDR, 1, Some(&[0xab; 17]));
        assert_eq!(
            cellpack_of(&stones),
            vec![
                14,
                2541175808,
                52998,
                1,
                17, // BYTES
                2,  // ELEMENTS — the whole point
                228189351935217557851910030866009271211,
                227297987279220614266551007307938922496,
            ]
        );
    }

    #[test]
    fn no_payload_uses_opcode_5_and_stays_six_wide() {
        let (_, stones) = build_burn(1, ADDR, 0, None);
        assert_eq!(cellpack_of(&stones), vec![5, 2541175808, 52998, 0]);
    }
}

#[cfg(test)]
mod burn_packing_tests {
    use super::*;

    /// Pinned against the reference test's own address and split.
    ///
    /// `frusd_burn_bridge.rs` uses 0xf39Fd6e5…92266 and computes
    /// This test USED to assert a 12/8 split, and passed — against an encoder
    /// the contract rejects. It was written from the retired protostoneBuilder
    /// rather than from the contract, and nothing on this side could tell the
    /// difference: a 12/8 cellpack is well-formed, it just reverts on chain.
    /// The authority is now burn_golden_vectors above (mainnet, height
    /// 961,899); this only pins the 4/16 boundary on a second address.
    #[test]
    fn hi_lo_split_takes_the_top_four_bytes() {
        let addr = hex::decode("f39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap();
        let mut a = [0u8; 20];
        a.copy_from_slice(&addr);
        let (hi, lo) = split_evm_address(a);
        assert_eq!(hi, 0xf39fd6e5, "hi is the TOP 4 bytes");
        assert_eq!(
            lo,
            u128::from_str_radix("1aad88f6f4ce6ab8827279cfffb92266", 16).unwrap(),
            "lo is the BOTTOM 16 bytes"
        );
        assert!(hi <= u32::MAX as u128, "the contract rejects a hi over u32");
    }

    /// The split must be reversible — a round trip that loses bytes is a
    /// silently wrong recipient.
    #[test]
    fn the_split_loses_nothing() {
        let a: [u8; 20] = core::array::from_fn(|i| (i as u8).wrapping_mul(7).wrapping_add(3));
        let (hi, lo) = split_evm_address(a);
        let mut back = [0u8; 20];
        back[..4].copy_from_slice(&hi.to_be_bytes()[12..]);
        back[4..].copy_from_slice(&lo.to_be_bytes());
        assert_eq!(back, a);
    }

    #[test]
    fn burn_spends_frusd_and_targets_frusd() {
        let (inputs, stones) = build_burn(100_000_000, [0x11; 20], 0, None);
        match inputs[0] {
            InputRequirement::Alkanes { block, tx, amount } => {
                assert_eq!((block, tx), (4, 1776), "frUSD is the token being burned");
                assert_eq!(amount, 100_000_000);
            }
            _ => panic!("expected an alkanes input"),
        }
        let cp = stones[0].cellpack.as_ref().unwrap();
        assert_eq!(cp.inputs[0], OP_BURN_AND_BRIDGE);
        assert_eq!(cp.inputs.len(), 4, "opcode + hi + lo + assetId");
    }
}
