//! BTC/USD trading against the SUBFROST CryptoSwap pool, and the EVM bridge arms.
//!
//! This module is the CLI's view of everything BTCUSD: market data, quoting,
//! direct frUSD↔frBTC swaps, liquidity, both bridge directions, and the watch
//! surface for tracking trades across two chains.
//!
//! # What is here and what is deliberately not
//!
//! The COMMAND SURFACE and the MONEY-PATH VALIDATION are here and tested. The
//! signing and broadcast paths reuse the CLI's existing wallet/provider
//! machinery rather than growing a second copy — see `commands.rs` for how
//! `frbtc-wrap` and friends do it.
//!
//! The validation is the part worth having in one place, because every failure
//! on these paths **succeeds and then goes wrong**:
//!
//! | mistake | what actually happens |
//! |---|---|
//! | conversion over the pool's depth cap | swap refused, whole mint pays out as frUSD — no error |
//! | non-standard `btc_destination` | frBTC burned against an address that can never be paid |
//! | `amountOutMin = 0` on the burn leg | unbounded loss; there is no coordinator-side floor |
//! | amount parsed as f64 | silent corruption past 2^53; LP supply is already past u64 |
//!
//! None of these raise. That is why they are checked before a transaction is
//! built rather than after one is broadcast.

use serde::{Deserialize, Serialize};

/// The canonical pool. `4:1778` is an `Upgradeable` PROXY and is also the
/// frBTCUSD LP token; the CryptoSwap math lives in `4:1786` behind it.
pub const POOL_BLOCK: u128 = 4;
pub const POOL_TX: u128 = 1778;
/// frUSD — token0 by alkane id ordering.
pub const FRUSD: (u128, u128) = (4, 1776);
/// frBTC — token1.
pub const FRBTC: (u128, u128) = (32, 0);

/// Pool opcodes. ⚠️ 102 is `get_A`, NOT `get_decimals` — it returns the
/// amplification (400000), and treating it as decimals renders LP at 1e400000.
/// `lp_price` (106) is the discriminator that identifies this contract class.
pub mod op {
    pub const ADD_LIQUIDITY: u128 = 1;
    pub const REMOVE_LIQUIDITY: u128 = 2;
    pub const REMOVE_LIQUIDITY_ONE_COIN: u128 = 3;
    pub const EXCHANGE: u128 = 5;
    pub const GET_VIRTUAL_PRICE: u128 = 100;
    pub const GET_BALANCES: u128 = 101;
    pub const GET_A: u128 = 102;
    pub const PRICE_ORACLE: u128 = 104;
    pub const PRICE_SCALE: u128 = 105;
    pub const LP_PRICE: u128 = 106;
    pub const GET_DY: u128 = 107;
    pub const CALC_TOKEN_AMOUNT: u128 = 108;
}

/// The largest share of the pool's frUSD reserve one conversion may take.
///
/// Mirrors `frusd_bridge_data::MAX_POOL_SHARE_BPS`. Beyond it the coordinator
/// refuses the swap and pays out plain frUSD — successfully, with no error on
/// either chain. Duplicated rather than imported because subzero-rs is not a
/// dependency of this repo; drift is safe in one direction only, and it is the
/// loud one (a lowered cap makes our clamp too generous and the conversion
/// visibly degrades).
pub const MAX_POOL_SHARE_BPS: u128 = 1_000;
pub const BPS_DENOM: u128 = 10_000;

/// frUSD carries 8 decimals; USDC and USDT carry 6. The vault scales by
/// `10^(8-6)`, so one stable base unit mints exactly 100 frUSD base units.
pub const FRUSD_PER_STABLE_UNIT: u128 = 100;

// ── Command surface ─────────────────────────────────────────────────────────

#[derive(clap::Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum BtcusdCommands {
    /// Spot price and USD-per-BTC, from the dedicated index.
    Price {
        /// Emit raw JSON rather than a table.
        #[arg(long)]
        raw: bool,
    },
    /// Reserves, LP supply, virtual price, and whether the curve is ramping or
    /// the pool is killed.
    ///
    /// Check `ramping` before quoting: the curve is moving between blocks, so a
    /// quote taken now may not hold. `is_killed` means refuse to trade at all.
    Pool {
        #[arg(long)]
        raw: bool,
    },
    /// OHLC candles. Buckets: 3600 or 86400 ONLY — anything else answers
    /// `ok:false` with the supported list rather than erroring.
    Candles {
        #[arg(long, default_value = "3600")]
        bucket: u32,
        #[arg(long)]
        from: Option<u64>,
        #[arg(long)]
        to: Option<u64>,
        #[arg(long, default_value = "100")]
        limit: u32,
        #[arg(long)]
        raw: bool,
    },
    /// Quote a swap WITHOUT sending it: expected output, effective execution
    /// price, and whether the size is inside the pool's depth cap.
    ///
    /// Always quote before swapping. The pool's marginal price is not the price
    /// you get, and today the gap is large (see `pool`).
    Quote {
        /// `frusd` or `frbtc`.
        #[arg(long)]
        from: String,
        /// Amount in the source token's BASE units, as a decimal STRING.
        /// Never a float: LP supply already exceeds u64.
        #[arg(long)]
        amount: String,
    },
    /// Execute a direct frUSD ↔ frBTC swap on the pool.
    Swap {
        #[arg(long)]
        from: String,
        #[arg(long)]
        amount: String,
        /// Slippage ceiling in bps. Required — there is no safe default for
        /// somebody else's money.
        #[arg(long)]
        max_slippage_bps: u32,
        /// Print the transaction instead of broadcasting it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Add liquidity (both coins) and receive LP.
    AddLiquidity {
        #[arg(long)]
        frusd: String,
        #[arg(long)]
        frbtc: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Burn LP and withdraw. ⚠️ LP is 18 decimals, not 8.
    RemoveLiquidity {
        #[arg(long)]
        lp_amount: String,
        /// Withdraw entirely into one coin instead of both.
        #[arg(long)]
        one_coin: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },

    // ── Bridge: EVM → BTC ───────────────────────────────────────────────────
    /// Prepare a USDC/USDT deposit that mints frUSD on Bitcoin, optionally
    /// converting a share to native BTC.
    ///
    /// Signing is OPTIONAL. With no key flags this prints the approve +
    /// `depositAndBridge` calldata for the user's own wallet — the right
    /// default for an interactive user. With a key it signs locally and submits
    /// through `/v4/{apikey}/ethereum-builder`, which keeps the transaction OUT
    /// OF THE PUBLIC MEMPOOL.
    ///
    /// ⚠️ That privacy matters for this specific trade: the calldata carries the
    /// BTC destination and the conversion split, and the Bitcoin leg's `min_dy`
    /// is visible in an OP_RETURN until it confirms. A public broadcast hands a
    /// searcher advance notice of a swap against a shallow pool.
    Deposit {
        /// `usdc` or `usdt`. Maps to the vault's own asset ids (0 and 1) — a
        /// swapped id approves one token and deposits against the other's
        /// accounting.
        #[arg(long)]
        stable: String,
        /// BASE units (both stables are 6-decimal), decimal string.
        #[arg(long)]
        amount: String,
        /// Mainnet Bitcoin address the minted frUSD settles at.
        ///
        /// MAY BE SOMEONE ELSE'S, same as `--btc-destination`. Paying frUSD
        /// straight to a third party out of an EVM balance is a supported flow.
        /// Validated for payability, never for ownership.
        ///
        /// Note the two can differ: `--recipient` takes the frUSD residue and
        /// `--btc-destination` takes the converted BTC, so one deposit can seed
        /// two different identities in two different assets.
        #[arg(long)]
        recipient: String,
        /// Share to convert to native BTC, in bps. 0 = plain frUSD deposit,
        /// which takes a different and byte-identical-to-mainnet code path.
        #[arg(long, default_value = "0")]
        convert_bps: u32,
        /// Where the native BTC lands. Required when `convert_bps > 0`.
        ///
        /// MAY BE SOMEONE ELSE'S ADDRESS, and that is a supported flow, not an
        /// accident: this is how a fresh BTC identity gets seeded, or how you
        /// pay a third party directly out of an EVM balance. It is validated
        /// for payability but NOT for ownership — we cannot know whose it is.
        ///
        /// ⚠️ There is no recovery. frBTC's `burn()` copies this script into the
        /// payment record and burns the supply immediately; a wrong-but-payable
        /// address pays a stranger, permanently.
        #[arg(long)]
        btc_destination: Option<String>,
        /// Your own slippage ceiling, honoured DOWNWARDS only — the coordinator
        /// clamps to min(this, its own cap). 0 takes the coordinator's.
        #[arg(long, default_value = "0")]
        max_slippage_bps: u32,

        // ── EVM signing. Omit all three to get calldata for your own wallet. ──
        /// Sign locally with the hex key in this file (mode 0600, please) and
        /// submit privately. Without it, `deposit` only PRINTS the calldata.
        #[arg(long)]
        eth_key_file: Option<String>,
        /// Key inline. Refused unless you also pass the acknowledgement flag,
        /// because it lands in shell history and the process table.
        #[arg(long)]
        eth_private_key: Option<String>,
        #[arg(long)]
        i_know_this_lands_in_shell_history: bool,
        /// Print the signed transaction instead of submitting it.
        #[arg(long)]
        dry_run: bool,
    },

    // ── Bridge: BTC → EVM ───────────────────────────────────────────────────
    /// Burn frUSD and bridge out to USDC on Ethereum, optionally swapping to
    /// ETH through Uniswap V2 on arrival.
    Burn {
        /// frUSD BASE units (8-decimal), decimal string.
        #[arg(long)]
        amount: String,
        /// Destination EVM address.
        #[arg(long)]
        eth_address: String,
        /// Share of the USDC output to swap onward into ETH, in bps.
        ///
        /// 0 = all USDC. 10000 = all ETH. Anything between splits, which is the
        /// point: a burner can land most of it as a stablecoin and take enough
        /// ETH to pay gas on the receiving account — otherwise a fresh EVM
        /// identity receives tokens it cannot move.
        #[arg(long, default_value = "0")]
        to_eth_bps: u32,
        /// ⚠️ REQUIRED whenever `--to-eth-bps > 0`. There is NO coordinator-side
        /// slippage floor on this leg — `amountOutMin` is yours alone, and 0 is
        /// an unbounded-loss instruction. Uniswap V2 Router02 is the ONLY
        /// allowlisted target; anything else degrades to a plain payout.
        #[arg(long)]
        min_out: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },

    // ── Observation ─────────────────────────────────────────────────────────
    /// The frBTC/frUSD signing group addresses, so you can watch rollups land.
    Signers {
        #[arg(long)]
        raw: bool,
    },
    /// Watch mempool and confirmed activity for addresses — the pool, the
    /// signers, or your own.
    ///
    /// ⚠️ `protorunesbyaddress` is currently slow (~10.5s against a 15s pool
    /// timeout on a busy address) and has caused an outage. Do not tighten the
    /// poll interval without measuring.
    Watch {
        /// Addresses to watch. Defaults to the signing group plus the pool.
        #[arg(long)]
        address: Vec<String>,
        #[arg(long, default_value = "15")]
        interval_secs: u64,
        /// Stop after N polls rather than running forever.
        #[arg(long)]
        max_polls: Option<u32>,
    },
    /// Mempool-aware reads: what is pending, and what the next blocks look like.
    ///
    /// Backed by `/v4/{apikey}/mempool`. ⚠️ NOT DEPLOYED YET — expect 404 until
    /// the route ships. Written now so a workflow can be built against it.
    Mempool {
        #[command(subcommand)]
        command: MempoolCommands,
    },
    /// Simulate a whole BLOCK to see where fees land and what ordering does.
    ///
    /// This is the MEV lens: `simulateblock` drives every transaction through
    /// the same per-tx path the indexer uses, with ONE shared sandbox carrying
    /// writes across transaction boundaries — so it reproduces the intra-block
    /// atomicity that decides who wins a contended swap.
    ///
    /// Use it to answer: does my transaction still get the quote I expect if it
    /// lands after the pending set? What fee gets me ahead of the trade that
    /// would move the price against me?
    SimulateBlock {
        /// Raw block hex. Omit to build a candidate block from the current
        /// mempool template.
        #[arg(long)]
        block_hex: Option<String>,
        /// Insert this raw transaction at a given position and report what it
        /// receives — the direct "where should I bid" question.
        #[arg(long)]
        insert_tx: Option<String>,
        /// Position to insert at. Omit for the template's own ordering.
        #[arg(long)]
        at_index: Option<u32>,
        #[arg(long)]
        raw: bool,
    },
    /// Dry-run a trade against current chain state without broadcasting —
    /// `simulate`, and for mempool-aware work `simulatetransaction` /
    /// `simulateblock`.
    ///
    /// This is the RBF and mempool-trading entry point: simulate against the
    /// pending set, not just the confirmed tip.
    Simulate {
        /// Raw transaction hex to simulate, or omit to simulate the pool's
        /// current view.
        #[arg(long)]
        tx_hex: Option<String>,
        /// Include the mempool's pending transactions in the simulated state.
        #[arg(long)]
        with_mempool: bool,
        #[arg(long)]
        raw: bool,
    },
}

/// Mempool reads. See the module docs on the deployment status.
#[derive(clap::Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum MempoolCommands {
    /// Size, fee coverage, and the sequence number to attach a stream at.
    ///
    /// ⚠️ Read `fee_coverage` before trusting any fee number derived from this.
    /// A thin mempool and a genuinely cheap one quote IDENTICALLY.
    Info,
    /// Projected next blocks — the template a miner would build.
    Template {
        #[arg(long, default_value = "8")]
        blocks: u32,
        #[arg(long)]
        raw: bool,
    },
    /// One transaction's mempool entry, including its projected block.
    Entry {
        txid: String,
    },
    /// Follow the change stream over websocket.
    ///
    /// Reconnect handling is not optional: a changed `instance` invalidates
    /// every `seq` you hold, and `reset` is also how backpressure is signalled.
    Watch {
        /// Resume from this sequence number rather than a fresh snapshot.
        #[arg(long)]
        since_seq: Option<u64>,
    },
}

// ── Money-path validation ───────────────────────────────────────────────────

/// Which side of the pool a token name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Frusd,
    Frbtc,
}

impl Side {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "frusd" | "usd" => Ok(Side::Frusd),
            "frbtc" | "btc" => Ok(Side::Frbtc),
            other => Err(format!(
                "unknown token {other:?} — this pool holds frusd and frbtc only"
            )),
        }
    }
    pub fn id(self) -> (u128, u128) {
        match self {
            Side::Frusd => FRUSD,
            Side::Frbtc => FRBTC,
        }
    }
    /// Coin INDEX as the POOL orders them internally — **frBTC is 0, frUSD is
    /// 1**. This is the INVERSE of the token0/token1 ordering by alkane id that
    /// the index and this module's constants use, and getting it backwards
    /// quotes the opposite direction with a number that still looks plausible.
    ///
    /// VERIFIED against the live pool rather than reasoned about, because I got
    /// it wrong the first time by following the index's ordering:
    ///   `get_dy(1, 0, 1e8)` -> 1555, and 1e8/1555 implies ~64,308 USD/BTC,
    ///   which matches the pool's price. So coin 1 is the USD leg.
    ///   `get_dy(0, 1, 1e8)` -> 532,877,077,035, i.e. ~91% of the entire frUSD
    ///   reserve for one whole frBTC — consistent, since the pool holds only
    ///   0.0913 frBTC.
    pub fn coin_index(self) -> u128 {
        match self {
            Side::Frbtc => 0,
            Side::Frusd => 1,
        }
    }
}

/// A base-unit amount, as digits only.
///
/// Refuses `1e8`, `100_000`, `100.5` and negatives rather than coercing them. A
/// laxer reader deposits an amount the user did not type, and past 2^53 a float
/// parse corrupts silently — the pool's own LP supply
/// (23113653069174808444) is already past u64.
pub fn parse_base_units(s: &str) -> Result<u128, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("amount is required (base units, decimal string)".into());
    }
    if !t.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "amount must be base units as decimal digits only, got {t:?} — \
             not scientific notation, separators, or a decimal point"
        ));
    }
    t.parse::<u128>()
        .map_err(|e| format!("amount {t:?} does not fit a u128: {e}"))
}

/// Is this scriptPubKey one the frBTC signing group will actually pay?
///
/// Accepted: P2PKH, P2SH, witness v0–v16, with v0 restricted to the 20- and
/// 32-byte programs. Everything else is refused.
///
/// ⚠️ This is not defensive tidiness. frBTC's `burn()` validates the unwrap
/// pointer's INDEX but never the SCRIPT at that index — it copies the script
/// into the payment record and burns the frBTC immediately, with no escrow,
/// expiry or reclaim. A non-standard destination destroys supply against an
/// address that can never be paid, and the contract will not stop it.
pub fn is_payable_script(script: &[u8]) -> bool {
    if script.len() == 25
        && script[0] == 0x76
        && script[1] == 0xa9
        && script[2] == 0x14
        && script[23] == 0x88
        && script[24] == 0xac
    {
        return true; // P2PKH
    }
    if script.len() == 23 && script[0] == 0xa9 && script[1] == 0x14 && script[22] == 0x87 {
        return true; // P2SH
    }
    if script.len() >= 4 && script.len() <= 42 {
        let ver = script[0];
        let is_ver_op = ver == 0x00 || (0x51..=0x60).contains(&ver);
        let prog_len = script[1] as usize;
        if is_ver_op && prog_len == script.len() - 2 && (2..=40).contains(&prog_len) {
            // v0 is defined ONLY for 20 and 32 bytes. Any other length is
            // unspendable by consensus, so paying to it burns the output.
            if ver == 0x00 {
                return prog_len == 20 || prog_len == 32;
            }
            return true;
        }
    }
    false
}

/// What a requested conversion will ACTUALLY do, given the pool's depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionPlan {
    pub requested_bps: u32,
    pub effective_bps: u32,
    /// Empty when nothing was clamped. Non-empty MUST be shown to the user.
    pub note: String,
}

/// Clamp a conversion to what the pool can absorb.
///
/// Over `MAX_POOL_SHARE_BPS` of the frUSD reserve the coordinator refuses the
/// swap and pays out the whole mint as frUSD — successfully, silently, in the
/// wrong asset. Clamping turns that into a number the user is shown.
///
/// Fees are deliberately not modelled: they only shrink the mint, so ignoring
/// them clamps slightly harder than required. Erring the other way puts the
/// swap over the cap and produces exactly the failure this prevents.
pub fn plan_conversion(
    requested_bps: u32,
    stable_amount: u128,
    frusd_reserve: Option<u128>,
) -> ConversionPlan {
    if requested_bps == 0 {
        return ConversionPlan { requested_bps, effective_bps: 0, note: String::new() };
    }
    let Some(reserve) = frusd_reserve.filter(|r| *r > 0) else {
        return ConversionPlan {
            requested_bps,
            effective_bps: requested_bps,
            note: "Could not read the pool's depth, so the conversion is sent as requested. \
                   If the pool is too shallow, SUBFROST pays out frUSD instead of BTC."
                .into(),
        };
    };
    let mint = stable_amount.saturating_mul(FRUSD_PER_STABLE_UNIT);
    if mint == 0 {
        return ConversionPlan { requested_bps, effective_bps: 0, note: String::new() };
    }
    let max_swap = reserve.saturating_mul(MAX_POOL_SHARE_BPS) / BPS_DENOM;
    let requested_swap = mint.saturating_mul(requested_bps as u128) / BPS_DENOM;
    if requested_swap <= max_swap {
        return ConversionPlan { requested_bps, effective_bps: requested_bps, note: String::new() };
    }
    // Round DOWN: at the boundary the coordinator's own recomputation could
    // land a unit over and refuse the whole thing.
    let capped = max_swap.saturating_mul(BPS_DENOM) / mint;
    let effective_bps = u32::try_from(capped).unwrap_or(0).min(requested_bps);
    if effective_bps == 0 {
        return ConversionPlan {
            requested_bps,
            effective_bps: 0,
            note: "This deposit is far larger than the pool can convert; none of it can \
                   become BTC. It will arrive as frUSD."
                .into(),
        };
    }
    ConversionPlan {
        requested_bps,
        effective_bps,
        note: format!(
            "The pool is too shallow to convert all of this at once: {}% will become BTC \
             and the rest arrives as frUSD. Converting more in one trade would be refused \
             and ALL of it would arrive as frUSD.",
            effective_bps / 100
        ),
    }
}

/// Is a direct swap inside the pool's depth cap?
///
/// Returns the share in bps so the caller can report it rather than just refuse.
pub fn swap_depth_share_bps(amount_in: u128, reserve_in: u128) -> Option<u128> {
    if reserve_in == 0 {
        return None;
    }
    Some(amount_in.saturating_mul(BPS_DENOM) / reserve_in)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_are_digits_only_so_a_typo_cannot_become_a_different_trade() {
        assert_eq!(parse_base_units("100000000").unwrap(), 100_000_000);
        assert_eq!(parse_base_units("  42 ").unwrap(), 42);
        for bad in ["1e8", "100_000", "100.5", "-1", "0x64", "", "1,000"] {
            assert!(parse_base_units(bad).is_err(), "should refuse {bad:?}");
        }
        // Past u64 must survive — the pool's LP supply already is.
        assert_eq!(
            parse_base_units("23113653069174808444").unwrap(),
            23_113_653_069_174_808_444u128
        );
    }

    #[test]
    fn side_parsing_and_coin_order() {
        assert_eq!(Side::parse("frusd").unwrap().id(), FRUSD);
        assert_eq!(Side::parse("BTC").unwrap().id(), FRBTC);
        assert!(Side::parse("eth").is_err());
        // The POOL's internal order, verified against live mainnet: frBTC is
        // coin 0, frUSD is coin 1 — the inverse of the by-alkane-id ordering.
        // Swapping these quotes the opposite direction, plausibly.
        assert_eq!(Side::Frbtc.coin_index(), 0);
        assert_eq!(Side::Frusd.coin_index(), 1);
    }

    #[test]
    fn payable_scripts_match_the_signing_groups_allowlist() {
        let ok = [
            "76a914efefefefefefefefefefefefefefefefefefefef88ac", // P2PKH
            "a914efefefefefefefefefefefefefefefefefefefef87",     // P2SH
            "0014cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",       // P2WPKH
            "0020cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd", // P2WSH
            "5120abababababababababababababababababababababababababababababababab", // P2TR
        ];
        for s in ok {
            assert!(is_payable_script(&hex::decode(s).unwrap()), "should accept {s}");
        }
        let bad = [
            "6a0400000000",                         // OP_RETURN — burns the payment
            "0010cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd", // v0 with a 16-byte program: unspendable
            "76a914efefefefefefefefefefefefefefefefefefefef88ad", // wrong terminator
        ];
        for s in bad {
            assert!(!is_payable_script(&hex::decode(s).unwrap()), "should refuse {s}");
        }
        assert!(!is_payable_script(&[]));
    }

    /// The pool on 2026-08-09: ~5,854 frUSD a side at 8 decimals.
    const RESERVE: u128 = 5_854_00_000_000;

    #[test]
    fn a_small_deposit_converts_in_full() {
        let p = plan_conversion(10_000, 100_000_000, Some(RESERVE)); // $100
        assert_eq!(p.effective_bps, 10_000);
        assert!(p.note.is_empty());
    }

    #[test]
    fn an_oversized_conversion_is_clamped_and_explained_not_degraded() {
        // $1000 at 100% would swap $1000 against a ~$585 ceiling. Unclamped,
        // the coordinator refuses and pays out ALL frUSD.
        let p = plan_conversion(10_000, 1_000_000_000, Some(RESERVE));
        assert!(p.effective_bps < 10_000 && p.effective_bps > 0);
        assert!(p.note.contains("frUSD"), "the user must be told: {}", p.note);
        let mint = 1_000_000_000u128 * FRUSD_PER_STABLE_UNIT;
        assert!(mint * p.effective_bps as u128 / BPS_DENOM <= RESERVE * MAX_POOL_SHARE_BPS / BPS_DENOM);
    }

    #[test]
    fn an_unreadable_pool_sends_as_requested_and_says_so() {
        // Our index being down is not a reason for us to decide what happens to
        // someone's money.
        let p = plan_conversion(10_000, 1_000_000_000, None);
        assert_eq!(p.effective_bps, 10_000);
        assert!(p.note.contains("frUSD instead of BTC"));
    }

    #[test]
    fn zero_bps_is_a_plain_deposit_and_never_warns() {
        let p = plan_conversion(0, 1_000_000_000_000, Some(RESERVE));
        assert_eq!(p.effective_bps, 0);
        assert!(p.note.is_empty(), "a plain frUSD deposit is not a degraded conversion");
    }

    #[test]
    fn depth_share_reports_rather_than_just_refusing() {
        // 10% of the reserve is exactly the cap.
        let tenth = RESERVE / 10;
        assert_eq!(swap_depth_share_bps(tenth, RESERVE), Some(MAX_POOL_SHARE_BPS));
        assert_eq!(swap_depth_share_bps(1, 0), None, "a zero reserve is not a 100% share");
    }
}
