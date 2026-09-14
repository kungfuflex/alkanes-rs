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
        /// Reserved. Local signing is not built (see the key flags above), so
        /// there is currently no signed transaction to withhold: the command
        /// prints the two calls either way.
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
            // ⚠️ IN BPS, not in whole percent. `bps / 100` is integer division,
            // so every clamp under 100bps printed "0% will become BTC" while
            // the line under it printed the real figure and the payload encoded
            // the real figure. On a pool this size a sub-100bps clamp is the
            // ordinary case, and the contradiction landed on the confirmation
            // screen of a money path whose whole claim is that the printed
            // number and the encoded number agree.
            "The pool is too shallow to convert all of this at once: {effective_bps}bps \
             ({}.{:02}%) will become BTC and the rest arrives as frUSD. Converting more in one \
             trade would be refused and ALL of it would arrive as frUSD.",
            effective_bps / 100,
            effective_bps % 100
        ),
    }
}

/// Is a direct swap inside the pool's depth cap?
///
/// Returns the share in bps so the caller can report it rather than just refuse.
///
/// ⚠️ `reserve_in` must be the reserve of the token going IN. Prefer
/// [`depth_share_bps`], which picks it from the side and cannot be handed the
/// wrong leg.
pub fn swap_depth_share_bps(amount_in: u128, reserve_in: u128) -> Option<u128> {
    if reserve_in == 0 {
        return None;
    }
    Some(amount_in.saturating_mul(BPS_DENOM) / reserve_in)
}

/// What to say about a DIRECT swap's size.
///
/// ⚠️ NOT a refusal, because nothing refuses a direct swap.
/// [`MAX_POOL_SHARE_BPS`] is the COORDINATOR's cap, and the coordinator only
/// sits in the BRIDGE path: it refuses an over-cap conversion and pays out the
/// source asset instead. A direct `exchange` on the pool has no coordinator in
/// it. It executes at whatever the curve gives, and `min_dy` is the only
/// protection the trader has.
///
/// Saying "this would be REFUSED" here teaches a protection that is not there.
/// Worse, the under-cap wording read as an all-clear: 999bps of this pool moves
/// the price about 8%, and nothing was capping it.
pub fn direct_swap_depth_note(share_bps: u128) -> String {
    format!(
        "size: {share_bps}bps of the pool's incoming leg. That is PRICE IMPACT, not a limit. \
         Nothing caps a direct swap for size: it executes at whatever the curve gives, so read \
         the effective rate below and set your own min_dy. (The {MAX_POOL_SHARE_BPS}bps cap \
         applies to BRIDGE conversions, where the coordinator refuses instead.)"
    )
}

/// USD per BTC, to two decimals, from a raw `*_price_q` and its scale.
///
/// ⚠️ THE QUOTE IS token1-PER-token0 — frBTC per frUSD — so USD/BTC is the
/// RECIPROCAL, `price_q_scale / price_q`. Getting it the right way round
/// matters more than usual here because the wrong way yields a number near
/// zero, which a reader takes for "no price" rather than for an error.
///
/// Integer math on purpose: these are U256-derived values quoted at 1e18, and a
/// double loses them silently.
pub fn usd_per_btc(price_q: u128, price_q_scale: u128) -> Option<String> {
    if price_q == 0 {
        return None;
    }
    let cents = price_q_scale.checked_mul(100)? / price_q;
    Some(format!("{}.{:02}", cents / 100, cents % 100))
}

/// The share of the pool a trade takes, measured against the leg it is paid in.
///
/// ⚠️ THE UNIT IS THE WHOLE POINT. `amount_in` is in the SOURCE token's base
/// units, and frUSD (8 decimals, thousands of them) and frBTC (8 decimals, tens
/// of millions of sats for a whole coin) are not comparable numbers. Dividing a
/// sat amount by the frUSD reserve answers ~0bps for a trade that is a tenth of
/// the pool, so the cap silently stops binding on that side — the guard still
/// runs, still prints, and never refuses.
///
/// `reserves` is `(frusd, frbtc)`, the order the index reports them in.
pub fn depth_share_bps(side: Side, amount_in: u128, reserves: (u128, u128)) -> Option<u128> {
    let reserve_in = match side {
        Side::Frusd => reserves.0,
        Side::Frbtc => reserves.1,
    };
    swap_depth_share_bps(amount_in, reserve_in)
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

// ── The burn leg: BTC → EVM ─────────────────────────────────────────────────
//
// `burnData` is NOT protobuf. It is a raw byte encoding, and it is deliberately
// permissive on the READ side: every parse failure is a reason to pay out
// PLAINLY, never to stall, because the burn is already irreversible on Bitcoin
// and the queue is strictly ordered — a stall behind one bad payload strands
// every burn behind it too.
//
// That permissiveness is exactly why the WRITE side here is strict. A payload
// we encode wrong does not error; it degrades to a plain USDC payout and the
// user's requested swap silently does not happen.

/// Wire version. A different byte here is `UnknownVersion` and degrades.
pub const BURN_DATA_VERSION: u8 = 0x01;

/// Uniswap V2 Router02 — the ONLY allowlisted `compose_targets_evm` entry.
///
/// V2 and not V3 deliberately: the vault performs a single `target.call`, and
/// V3's multicall shape does not fit that. Anything not on the allowlist is
/// refused by the coordinator and the burn pays out plainly.
pub const UNISWAP_V2_ROUTER02: [u8; 20] = [
    0x7a, 0x25, 0x0d, 0x56, 0x30, 0xB4, 0xcF, 0x53, 0x97, 0x39,
    0xdF, 0x2C, 0x5d, 0xAc, 0xb4, 0xc6, 0x59, 0xF2, 0x48, 0x8D,
];

/// `swapExactTokensForETH(uint256,uint256,address[],address,uint256)`
pub const SELECTOR_SWAP_EXACT_TOKENS_FOR_ETH: [u8; 4] = [0x18, 0xcb, 0xaf, 0xe5];

/// Canonical L1 addresses for the burn path's swap.
pub const USDC_L1: [u8; 20] = [
    0xa0, 0xb8, 0x69, 0x91, 0xc6, 0x21, 0x8b, 0x36, 0xc1, 0xd1,
    0x9d, 0x4a, 0x2e, 0x9e, 0xb0, 0xce, 0x36, 0x06, 0xeb, 0x48,
];
pub const WETH_L1: [u8; 20] = [
    0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e,
    0x5c, 0x4f, 0x27, 0xea, 0xd9, 0x08, 0x3c, 0x75, 0x6c, 0xc2,
];

/// Plain transfer to the burn's `eth_address` — the payload-free default.
pub fn burn_data_transfer() -> Vec<u8> {
    // Empty is ALSO read as Transfer, but emitting the explicit form makes the
    // intent legible on chain rather than inferred from an absence.
    vec![BURN_DATA_VERSION, 0x00]
}

/// Approve `target` for the payout and call it, refunding the remainder to the
/// burner's own `eth_address`.
pub fn burn_data_call(target: [u8; 20], calldata: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(22 + calldata.len());
    out.push(BURN_DATA_VERSION);
    out.push(0x01);
    out.extend_from_slice(&target);
    out.extend_from_slice(calldata);
    out
}

/// ABI-encode `swapExactTokensForETH(amountIn, amountOutMin, [USDC,WETH], to, deadline)`.
///
/// ⚠️ `amount_out_min` is YOURS ALONE. There is no coordinator-side slippage
/// floor on this leg — 0 is an instruction to accept any amount of ETH,
/// including a sandwich's leavings. This function refuses 0 rather than
/// encoding it.
///
/// `amount_in` is set by the coordinator from the actual payout, so the value
/// encoded here is the burner's own expectation; a mismatch is tolerated by the
/// router as long as the allowance covers it.
pub fn encode_swap_exact_tokens_for_eth(
    amount_in: u128,
    amount_out_min: u128,
    to: [u8; 20],
    deadline: u64,
) -> Result<Vec<u8>, String> {
    if amount_out_min == 0 {
        return Err(
            "amountOutMin is 0, which accepts any output at all. There is no coordinator-side \
             slippage floor on the burn leg — set a real floor."
                .into(),
        );
    }
    let mut out = Vec::with_capacity(4 + 32 * 9);
    out.extend_from_slice(&SELECTOR_SWAP_EXACT_TOKENS_FOR_ETH);
    out.extend_from_slice(&word_u128(amount_in));
    out.extend_from_slice(&word_u128(amount_out_min));
    // `path` is dynamic: the head carries its OFFSET, not its contents. Five
    // head words precede it (4 statics + this one), so the offset is 5*32.
    out.extend_from_slice(&word_u128(5 * 32));
    out.extend_from_slice(&word_addr(to));
    out.extend_from_slice(&word_u128(deadline as u128));
    // tail: path.len(), then each element
    out.extend_from_slice(&word_u128(2));
    out.extend_from_slice(&word_addr(USDC_L1));
    out.extend_from_slice(&word_addr(WETH_L1));
    Ok(out)
}

fn word_u128(v: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_addr(a: [u8; 20]) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(&a);
    w
}

/// Parse a `0x`-prefixed EVM address.
pub fn parse_evm_address(s: &str) -> Result<[u8; 20], String> {
    let t = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    if t.len() != 40 || !t.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("not a 20-byte EVM address: {s}"));
    }
    let raw = hex::decode(t).map_err(|e| e.to_string())?;
    let mut out = [0u8; 20];
    out.copy_from_slice(&raw);
    Ok(out)
}

#[cfg(test)]
mod burn_tests {
    use super::*;

    #[test]
    fn transfer_is_the_two_byte_explicit_form() {
        assert_eq!(burn_data_transfer(), vec![0x01, 0x00]);
    }

    #[test]
    fn call_payload_matches_the_coordinators_parser_layout() {
        // version | action | 20-byte target | calldata
        let cd = [0xaa, 0xbb];
        let p = burn_data_call(UNISWAP_V2_ROUTER02, &cd);
        assert_eq!(p[0], BURN_DATA_VERSION);
        assert_eq!(p[1], 0x01);
        assert_eq!(&p[2..22], &UNISWAP_V2_ROUTER02);
        assert_eq!(&p[22..], &cd);
        // The parser requires 22 bytes before calldata; a short target would
        // otherwise be padded with calldata and yield a REAL but wrong address.
        assert!(p.len() >= 22);
    }

    #[test]
    fn zero_amount_out_min_is_refused_not_encoded() {
        // There is no coordinator-side floor on this leg.
        let e = encode_swap_exact_tokens_for_eth(1_000, 0, [0u8; 20], 1).unwrap_err();
        assert!(e.contains("slippage floor"), "{e}");
    }

    #[test]
    fn swap_calldata_has_the_right_selector_and_dynamic_offset() {
        let to = parse_evm_address("0x1111111111111111111111111111111111111111").unwrap();
        let cd = encode_swap_exact_tokens_for_eth(1_000_000, 900_000, to, 1_800_000_000).unwrap();
        assert_eq!(&cd[..4], &SELECTOR_SWAP_EXACT_TOKENS_FOR_ETH);
        // 4 selector + 5 head words + 3 tail words
        assert_eq!(cd.len(), 4 + 32 * 8);
        // The `path` head word must be the OFFSET (5*32 = 160), not a value —
        // encoding the array inline here silently shifts every later argument.
        let off = u128::from_be_bytes(cd[4 + 64 + 16..4 + 96].try_into().unwrap());
        assert_eq!(off, 160, "path must be encoded as a dynamic offset");
        // and the tail says length 2
        let len = u128::from_be_bytes(cd[4 + 160 + 16..4 + 192].try_into().unwrap());
        assert_eq!(len, 2);
    }

    #[test]
    fn the_router_constant_is_uniswap_v2_router02() {
        assert_eq!(
            format!("0x{}", hex::encode(UNISWAP_V2_ROUTER02)).to_lowercase(),
            "0x7a250d5630b4cf539739df2c5dacb4c659f2488d"
        );
    }

    #[test]
    fn evm_addresses_are_parsed_strictly() {
        assert!(parse_evm_address("0x1111111111111111111111111111111111111111").is_ok());
        for bad in ["", "0x11", "0xzz11111111111111111111111111111111111111"] {
            assert!(parse_evm_address(bad).is_err(), "should refuse {bad:?}");
        }
    }
}

// ── Arm B: EVM → BTC. The deposit's two calls and the payload they carry ────
//
// `depositAndBridge`'s third argument is OPAQUE TO THE VAULT: it stores the
// bytes verbatim and the coordinator interprets them later. So a malformed
// payload still SUCCEEDS on Ethereum, and the coordinator then pays the
// operator's fallback recipient rather than the depositor. There is no error on
// either chain and nothing to watch for.
//
// That is why every check in this section runs before the calldata is emitted,
// and why the encoders return `Result` rather than doing their best.

/// The frUSD vault on L1 (ERC1967 proxy). `assets(0) = USDC`, `assets(1) = USDT`.
pub const FRUSD_VAULT_L1: [u8; 20] = [
    0x95, 0x77, 0x9e, 0x7e, 0x1c, 0x94, 0x30, 0x42, 0x25, 0x5b,
    0x8a, 0x78, 0x27, 0x3f, 0xe6, 0xde, 0x48, 0x23, 0xcf, 0x06,
];

/// Tether on L1. (`USDC_L1` is declared with the burn leg above.)
pub const USDT_L1: [u8; 20] = [
    0xda, 0xc1, 0x7f, 0x95, 0x8d, 0x2e, 0xe5, 0x23, 0xa2, 0x20,
    0x62, 0x06, 0x99, 0x45, 0x97, 0xc1, 0x3d, 0x83, 0x1e, 0xc7,
];

/// `approve(address,uint256)`
pub const SELECTOR_APPROVE: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];
/// `depositAndBridge(uint8,uint256,bytes)`
pub const SELECTOR_DEPOSIT_AND_BRIDGE: [u8; 4] = [0xbe, 0xdb, 0x65, 0xee];

/// The ERC20 the vault holds under a given asset id.
///
/// The ids are the VAULT'S registration order, not ours, and they are what the
/// calldata carries. Approving one token and depositing against the other's
/// accounting is a swapped-id bug that both chains accept.
pub fn stable_token_l1(asset_id: u8) -> Option<[u8; 20]> {
    match asset_id {
        0 => Some(USDC_L1),
        1 => Some(USDT_L1),
        _ => None,
    }
}

/// A MAINNET Bitcoin address as its scriptPubKey.
///
/// Addresses, not script hex, at the user boundary: a person can recognise
/// `bc1p…` and cannot proofread 34 bytes. The network requirement is the load
/// bearing part — a testnet address parses fine and yields a well-formed script
/// that mainnet pays into a hole.
pub fn mainnet_script_pubkey(addr: &str) -> Result<Vec<u8>, String> {
    use std::str::FromStr;
    let parsed = bitcoin::Address::from_str(addr.trim())
        .map_err(|e| format!("not a Bitcoin address: {addr:?} ({e})"))?;
    let checked = parsed.require_network(bitcoin::Network::Bitcoin).map_err(|_| {
        format!("{addr:?} is not a MAINNET address — the script would be well formed and unpayable")
    })?;
    Ok(checked.script_pubkey().to_bytes())
}

/// Is this a scriptPubKey the coordinator will MINT frUSD to?
///
/// DELIBERATELY STRICTER than [`is_payable_script`]. The two fields answer to
/// different authorities: `recipient_script` is minted to by the coordinator and
/// is restricted to the five spendable forms, while `btc_destination` is paid by
/// the frBTC signing group, which relays every witness version v0..v16. Using
/// one classifier for both either refuses a valid destination or accepts a
/// recipient the coordinator will not mint to.
///
/// Mirrors `is_standard_script_pubkey` in the coordinator.
pub fn is_standard_recipient_script(script: &[u8]) -> bool {
    match script.len() {
        // P2WPKH: OP_0 PUSH20
        22 => script[0] == 0x00 && script[1] == 0x14,
        // P2WSH: OP_0 PUSH32 · P2TR: OP_1 PUSH32
        34 => (script[0] == 0x00 || script[0] == 0x51) && script[1] == 0x20,
        // P2SH: OP_HASH160 PUSH20 <20> OP_EQUAL
        23 => script[0] == 0xa9 && script[1] == 0x14 && script[22] == 0x87,
        // P2PKH: OP_DUP OP_HASH160 PUSH20 <20> OP_EQUALVERIFY OP_CHECKSIG
        25 => {
            script[0] == 0x76
                && script[1] == 0xa9
                && script[2] == 0x14
                && script[23] == 0x88
                && script[24] == 0xac
        }
        _ => false,
    }
}

/// proto3 varint.
fn varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while v > 0x7f {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
    out
}

/// `<tag> <len varint> <bytes>`
fn length_delimited(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    out.extend_from_slice(&varint(body.len() as u64));
    out.extend_from_slice(body);
    out
}

/// `BridgeData` for a plain DIRECT_TRANSFER: mint the frUSD straight to
/// `recipient_script`.
///
/// ⚠️ EMPTY bytes are not a no-op. `frusd-bridge-data::parse` maps them to
/// `BridgeIntent::Unspecified` and the coordinator mints to the OPERATOR'S
/// configured fallback recipient. Nothing here can return an empty buffer.
pub fn encode_direct_transfer(recipient_script: &[u8]) -> Result<Vec<u8>, String> {
    if !is_standard_recipient_script(recipient_script) {
        return Err(format!(
            "recipient is not a standard scriptPubKey ({} bytes); the coordinator would refuse \
             the mint and the deposit could not be recovered",
            recipient_script.len()
        ));
    }
    // proto3 omits zero-valued scalars, so `action` (DIRECT_TRANSFER = 0) never
    // appears on the wire and field 2 is the whole message.
    Ok(length_delimited(0x12, recipient_script))
}

/// `BridgeData` with field 4: convert `convert_bps` of the mint to native BTC at
/// `btc_destination`, settle the rest as frUSD at `recipient_script`.
///
/// REFUSES ANYTHING THE COORDINATOR WOULD DEGRADE, and the asymmetry is the
/// whole point. A fault in the OUTER message stalls the deposit — loud, someone
/// investigates. A fault in this INNER message never errors at all:
/// `parse_btc_conversion` degrades every fault to a plain frUSD payout with a
/// log line. The depositor is paid, just not what they asked for.
///
/// ⚠️ Field 4 is MUTUALLY EXCLUSIVE with field 3 (`recipient_protostone`). Both
/// set is an instruction naming two things to do with the same money and the
/// coordinator does neither. This function cannot emit field 3 at all, which is
/// how the exclusion is enforced rather than checked.
pub fn encode_btc_conversion(
    recipient_script: &[u8],
    convert_bps: u32,
    btc_destination: &[u8],
    max_slippage_bps: u32,
) -> Result<Vec<u8>, String> {
    if !is_standard_recipient_script(recipient_script) {
        return Err(format!(
            "recipient is not a standard scriptPubKey ({} bytes); the coordinator would refuse \
             the mint and the deposit could not be recovered",
            recipient_script.len()
        ));
    }
    if convert_bps == 0 || convert_bps as u128 > BPS_DENOM {
        // 0 is excluded on purpose: it is a DirectTransfer and must take that
        // code path rather than a zero-sized version of this one. Above the
        // denominator the coordinator refuses rather than clamping — someone who
        // wrote 20000 meant something, and it was not "100%".
        return Err(format!(
            "convert_bps must be 1..={BPS_DENOM}, got {convert_bps}; use a plain deposit for no \
             conversion"
        ));
    }
    if !is_payable_script(btc_destination) {
        return Err(format!(
            "btc_destination is not a payable scriptPubKey ({} bytes); frBTC's burn() would copy \
             it into the payment record and destroy the supply against it",
            btc_destination.len()
        ));
    }
    if max_slippage_bps as u128 > BPS_DENOM {
        return Err(format!("max_slippage_bps must be 0..={BPS_DENOM}, got {max_slippage_bps}"));
    }

    // Inner `BtcConversion`. Fields ascend and proto3 defaults are OMITTED: a
    // `max_slippage_bps` of 0 emits no field 3, and absent means "the
    // coordinator's own bound" rather than "no tolerance".
    let mut inner = vec![0x08];
    inner.extend_from_slice(&varint(convert_bps as u64));
    inner.extend_from_slice(&length_delimited(0x12, btc_destination));
    if max_slippage_bps > 0 {
        inner.push(0x18);
        inner.extend_from_slice(&varint(max_slippage_bps as u64));
    }

    let mut out = length_delimited(0x12, recipient_script);
    out.extend_from_slice(&length_delimited(0x22, &inner));
    Ok(out)
}

/// `approve(spender, amount)` calldata.
///
/// ⚠️ Callers must pass the EXACT deposit amount. A standing infinite allowance
/// on a proxy the depositor does not control outlives the single transaction it
/// was granted for.
pub fn encode_approve(spender: [u8; 20], amount: u128) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 64);
    out.extend_from_slice(&SELECTOR_APPROVE);
    out.extend_from_slice(&word_addr(spender));
    out.extend_from_slice(&word_u128(amount));
    out
}

/// `depositAndBridge(assetId, amount, bridgeData)` calldata.
pub fn encode_deposit_and_bridge(asset_id: u8, amount: u128, bridge_data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 * 4 + bridge_data.len() + 32);
    out.extend_from_slice(&SELECTOR_DEPOSIT_AND_BRIDGE);
    out.extend_from_slice(&word_u128(asset_id as u128));
    out.extend_from_slice(&word_u128(amount));
    // `bytes` is dynamic: the head carries its OFFSET past the three head words,
    // not its contents. Inlining it here shifts every argument silently.
    out.extend_from_slice(&word_u128(3 * 32));
    out.extend_from_slice(&word_u128(bridge_data.len() as u128));
    out.extend_from_slice(bridge_data);
    // right-pad the tail to a whole word
    let rem = bridge_data.len() % 32;
    if rem != 0 {
        out.extend(std::iter::repeat(0u8).take(32 - rem));
    }
    out
}

/// Everything the two Ethereum calls need, decided before either is emitted.
#[derive(Debug, Clone)]
pub struct DepositPlan {
    /// The vault's own asset id. Carried in the calldata.
    pub asset_id: u8,
    /// The ERC20 to `approve`. NOT the vault.
    pub token: [u8; 20],
    pub amount: u128,
    pub recipient_script: Vec<u8>,
    pub btc_destination_script: Option<Vec<u8>>,
    /// What will ACTUALLY convert, after the pool's depth cap. May be less than
    /// requested, and may be 0.
    pub effective_bps: u32,
    /// Empty when nothing was clamped. Non-empty MUST be shown to the user.
    pub note: String,
    pub bridge_data: Vec<u8>,
    pub approve_calldata: Vec<u8>,
    pub deposit_calldata: Vec<u8>,
}

/// Decide a deposit completely, or refuse it.
///
/// Pure on purpose: every refusal on this path is a refusal the coordinator
/// would NOT make — it degrades instead, paying the depositor something other
/// than what they asked for with no error anywhere. Keeping the decision in one
/// function with no I/O is what makes those refusals testable.
///
/// `frusd_reserve` is `None` when the pool's depth could not be read. That is
/// not fatal: the conversion is sent as requested and the caller says so.
#[allow(clippy::too_many_arguments)]
pub fn plan_deposit(
    stable: &str,
    amount: u128,
    recipient: &str,
    convert_bps: u32,
    btc_destination: Option<&str>,
    max_slippage_bps: u32,
    frusd_reserve: Option<u128>,
) -> Result<DepositPlan, String> {
    let asset_id = match stable.trim().to_ascii_lowercase().as_str() {
        "usdc" => 0u8,
        "usdt" => 1u8,
        other => {
            return Err(format!(
                "unknown stable {other:?} — the vault registered usdc(0) and usdt(1)"
            ))
        }
    };
    let token = stable_token_l1(asset_id).ok_or("no token registered for that asset id")?;
    if amount == 0 {
        return Err("amount is zero: the vault would take nothing and nothing would be minted".into());
    }
    if convert_bps as u128 > BPS_DENOM {
        return Err(format!("convert_bps must be 0..={BPS_DENOM}, got {convert_bps}"));
    }
    // Checked here rather than only where it is encoded: on the DirectTransfer
    // branch the value is never looked at, so an absurd one used to pass in
    // silence and do nothing.
    if max_slippage_bps as u128 > BPS_DENOM {
        return Err(format!("max_slippage_bps must be 0..={BPS_DENOM}, got {max_slippage_bps}"));
    }
    let recipient_script = mainnet_script_pubkey(recipient)?;

    // The destination is required by the REQUESTED conversion, not the effective
    // one. Clamping to zero is a property of today's pool depth; asking to
    // convert without saying where the BTC goes is a property of the request.
    let destination_script = match (convert_bps > 0, btc_destination) {
        (true, None) => {
            return Err("--btc-destination is required when converting to BTC".into())
        }
        // ⚠️ CONTRADICTORY INPUT, refused rather than half-honoured.
        // `--convert-bps` defaults to 0, so naming a destination and nothing
        // else is an easy mistake, and it is the flow the flag advertises:
        // paying a third party in BTC out of an EVM balance. Encoding a plain
        // DirectTransfer mints 100% as frUSD to `--recipient` and says nothing
        // about the destination that was typed and dropped.
        (false, Some(_)) => {
            return Err(
                "--btc-destination was given but --convert-bps is 0, so nothing would be \
                 converted and the destination would be ignored: all of it would mint as frUSD \
                 to --recipient. Set --convert-bps, or drop --btc-destination."
                    .into(),
            )
        }
        (true, Some(d)) => Some(mainnet_script_pubkey(d)?),
        (false, None) => None,
    };

    let plan = plan_conversion(convert_bps, amount, frusd_reserve);

    let bridge_data = match (plan.effective_bps, destination_script.as_ref()) {
        // Clamped to nothing, or never asked for: a plain mint. NOT a zero-bps
        // conversion — the coordinator reads those as different intents.
        (0, _) | (_, None) => encode_direct_transfer(&recipient_script)?,
        (bps, Some(dest)) => {
            encode_btc_conversion(&recipient_script, bps, dest, max_slippage_bps)?
        }
    };

    Ok(DepositPlan {
        asset_id,
        token,
        amount,
        recipient_script,
        btc_destination_script: destination_script,
        effective_bps: plan.effective_bps,
        note: plan.note,
        approve_calldata: encode_approve(FRUSD_VAULT_L1, amount),
        deposit_calldata: encode_deposit_and_bridge(asset_id, amount, &bridge_data),
        bridge_data,
    })
}

#[cfg(test)]
mod deposit_tests {
    use super::*;

    /// GOLDEN VECTORS, copied verbatim from
    /// `subzero-rs/crates/frusd-bridge-data/fixtures/bridge_data_vectors.json`
    /// by way of the client that already mirrors them
    /// (`subfrost-app lib/bridge/__tests__/fixtures/`).
    ///
    /// The fixture file's own header is the reason these are pinned here rather
    /// than reasoned about: the coordinator declares its messages with prost's
    /// derive macros instead of generating them from the `.proto`, so nothing
    /// mechanically forces schema and code to agree. A client in any language is
    /// meant to be SHOWN byte-identical against these, not assumed so.
    const P2TR_AB: &str = "5120abababababababababababababababababababababababababababababababab";
    const P2WPKH_CD: &str = "0014cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
    const P2PKH_EF: &str = "76a914efefefefefefefefefefefefefefefefefefefef88ac";

    fn unhex(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    #[test]
    fn direct_transfer_matches_the_golden_vectors() {
        // `direct_transfer_p2tr`
        assert_eq!(
            hex::encode(encode_direct_transfer(&unhex(P2TR_AB)).unwrap()),
            format!("1222{P2TR_AB}")
        );
        // `live_mainnet_deposit` — a REAL deposit that must keep decoding as a
        // plain DirectTransfer for as long as this bridge exists.
        let live = "51201b18312df38aa8203a1b589c9f45852f55325488df00f52024280cf9240fe574";
        assert_eq!(
            hex::encode(encode_direct_transfer(&unhex(live)).unwrap()),
            format!("1222{live}")
        );
    }

    #[test]
    fn btc_conversion_matches_the_golden_vectors() {
        // `btc_conversion_full`: 10000 bps, 100 bps slippage, P2WPKH destination.
        assert_eq!(
            hex::encode(
                encode_btc_conversion(&unhex(P2TR_AB), 10_000, &unhex(P2WPKH_CD), 100).unwrap()
            ),
            format!("1222{P2TR_AB}221d08904e1216{P2WPKH_CD}1864")
        );
        // `btc_conversion_partial`: 4000 bps, slippage left to the coordinator.
        assert_eq!(
            hex::encode(
                encode_btc_conversion(&unhex(P2TR_AB), 4_000, &unhex(P2TR_AB), 0).unwrap()
            ),
            format!("1222{P2TR_AB}222708a01f1222{P2TR_AB}")
        );
        // `btc_conversion_p2pkh_tight`: the shortest recipient and the longest
        // destination, so a length byte written as a constant shows up here.
        assert_eq!(
            hex::encode(
                encode_btc_conversion(&unhex(P2WPKH_CD), 6_000, &unhex(P2PKH_EF), 25).unwrap()
            ),
            format!("1216{P2WPKH_CD}222008f02e1219{P2PKH_EF}1819")
        );
    }

    #[test]
    fn a_zero_max_slippage_omits_field_three_because_proto3_omits_defaults() {
        // Absent means "use the coordinator's own bound", NOT "no tolerance".
        // Writing an explicit 0 would say something different from what the
        // fixture says, and the fixture is what the coordinator reads.
        let without = encode_btc_conversion(&unhex(P2TR_AB), 4_000, &unhex(P2TR_AB), 0).unwrap();
        let with_one = encode_btc_conversion(&unhex(P2TR_AB), 4_000, &unhex(P2TR_AB), 1).unwrap();
        // A slippage of 1 costs exactly the two bytes `18 01`, and the inner
        // length byte grows by two with it. Nothing else may move.
        assert_eq!(with_one.len(), without.len() + 2);
        assert_eq!(&with_one[with_one.len() - 2..], &[0x18, 0x01]);
    }

    #[test]
    fn convert_bps_out_of_range_is_refused_never_clamped() {
        // Someone who wrote 20000 meant something, and it was not "100%".
        assert!(encode_btc_conversion(&unhex(P2TR_AB), 20_000, &unhex(P2TR_AB), 0).is_err());
        // 0 is a DirectTransfer and must take that path, not a zero-sized
        // version of this one.
        assert!(encode_btc_conversion(&unhex(P2TR_AB), 0, &unhex(P2TR_AB), 0).is_err());
        assert!(encode_btc_conversion(&unhex(P2TR_AB), 4_000, &unhex(P2TR_AB), 20_000).is_err());
    }

    #[test]
    fn an_unpayable_btc_destination_is_refused_before_anything_is_signed() {
        // frBTC's `burn()` never checks the script it pays to: it copies it into
        // the payment record and burns the supply. This encoder is the last
        // place that can say no.
        let op_return = unhex("6a0400000000");
        let e = encode_btc_conversion(&unhex(P2TR_AB), 4_000, &op_return, 0).unwrap_err();
        assert!(e.contains("payable"), "{e}");
    }

    #[test]
    fn the_recipient_allowlist_is_stricter_than_the_destination_one() {
        // The two fields answer to different authorities: `recipient_script` is
        // minted to by the coordinator (five spendable forms only), while
        // `btc_destination` is paid by the frBTC signing group, which relays
        // every witness version. Reusing one classifier for both either refuses
        // a valid destination or accepts an unmintable recipient.
        let witness_v1_short = unhex("5114cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd");
        assert!(is_payable_script(&witness_v1_short), "the signing group relays v1..v16");
        assert!(
            !is_standard_recipient_script(&witness_v1_short),
            "the coordinator mints only to the five standard forms"
        );
        // And the five standard forms are accepted by both.
        for s in [P2TR_AB, P2WPKH_CD, P2PKH_EF] {
            assert!(is_standard_recipient_script(&unhex(s)), "should accept {s}");
            assert!(is_payable_script(&unhex(s)), "should accept {s}");
        }
        assert!(encode_direct_transfer(&witness_v1_short).is_err());
    }

    #[test]
    fn a_mainnet_address_becomes_its_script_pubkey() {
        // BIP-173 test vector. Addresses are what a person can proofread; 34
        // bytes of script hex are not, so the boundary takes an address.
        assert_eq!(
            hex::encode(mainnet_script_pubkey("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4").unwrap()),
            "0014751e76e8199196d454941c45d1b3a323f1433bd6"
        );
    }

    #[test]
    fn a_non_mainnet_address_is_refused_rather_than_paid_into_a_hole() {
        // A testnet address parses fine and yields a well-formed script that
        // mainnet pays into nothing. The network check is the only thing
        // between the two, because the shapes are identical.
        for addr in [
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080",
        ] {
            assert!(mainnet_script_pubkey(addr).is_err(), "should refuse {addr}");
        }
        assert!(mainnet_script_pubkey("not an address").is_err());
    }

    #[test]
    fn approve_encodes_the_erc20_selector_and_the_exact_amount() {
        let cd = encode_approve(FRUSD_VAULT_L1, 100_000_000);
        assert_eq!(&cd[..4], &SELECTOR_APPROVE);
        assert_eq!(cd.len(), 4 + 64, "approve is two static words");
        assert_eq!(&cd[4 + 12..4 + 32], &FRUSD_VAULT_L1, "spender must be the vault");
        let amount = u128::from_be_bytes(cd[4 + 48..4 + 64].try_into().unwrap());
        assert_eq!(amount, 100_000_000, "an unlimited allowance outlives the deposit");
    }

    #[test]
    fn deposit_and_bridge_abi_encodes_the_bytes_as_a_dynamic_tail() {
        let bridge = encode_direct_transfer(&unhex(P2TR_AB)).unwrap();
        let cd = encode_deposit_and_bridge(1, 100_000_000, &bridge);
        assert_eq!(&cd[..4], &SELECTOR_DEPOSIT_AND_BRIDGE);
        // head: assetId, amount, offset-to-bytes. The offset is three words in,
        // and encoding the bytes inline instead silently shifts every argument.
        let asset = u128::from_be_bytes(cd[4 + 16..4 + 32].try_into().unwrap());
        assert_eq!(asset, 1, "usdt is assetId 1; a swapped id deposits the other token");
        let amount = u128::from_be_bytes(cd[4 + 48..4 + 64].try_into().unwrap());
        assert_eq!(amount, 100_000_000);
        let offset = u128::from_be_bytes(cd[4 + 80..4 + 96].try_into().unwrap());
        assert_eq!(offset, 96, "bytes must be a dynamic offset, not inline");
        // tail: length then the payload, right-padded to a whole word.
        let len = u128::from_be_bytes(cd[4 + 112..4 + 128].try_into().unwrap()) as usize;
        assert_eq!(len, bridge.len());
        assert_eq!(&cd[4 + 128..4 + 128 + len], &bridge[..]);
        assert_eq!((cd.len() - 4) % 32, 0, "the tail must be padded to a word boundary");
    }

    #[test]
    fn an_empty_bridge_data_is_refused_because_it_pays_the_operator() {
        // `frusd-bridge-data::parse` maps empty bytes to `BridgeIntent::Unspecified`
        // and the coordinator then mints to the OPERATOR'S fallback recipient.
        // The deposit succeeds on both chains and the depositor is not paid.
        assert!(encode_direct_transfer(&[]).is_err());
    }

    /// The live pool at height 962,502: 18,812.14 frUSD and 0.29521902 frBTC.
    const LIVE: (u128, u128) = (1_881_214_389_078, 29_521_902);

    #[test]
    fn a_frbtc_trade_is_measured_against_the_FRBTC_reserve() {
        // Selling 0.03 frBTC is ~10% of this pool and moves the price 8.5%.
        // Measured against the frUSD reserve — base units of a DIFFERENT token
        // with a different scale — it reports 0bps and the cap never binds.
        // The guard reads plausible either way; only the unit tells them apart.
        assert_eq!(depth_share_bps(Side::Frbtc, 3_000_000, LIVE), Some(1_016));
        assert!(depth_share_bps(Side::Frbtc, 3_000_000, LIVE).unwrap() > MAX_POOL_SHARE_BPS);
        // The frUSD side keeps the number it already reported: $1,000 = 531bps,
        // measured against this same pool by the shipped command.
        assert_eq!(depth_share_bps(Side::Frusd, 100_000_000_000, LIVE), Some(531));
    }

    #[test]
    fn a_zero_reserve_is_none_rather_than_a_share_of_nothing() {
        assert_eq!(depth_share_bps(Side::Frbtc, 1, (LIVE.0, 0)), None);
        assert_eq!(depth_share_bps(Side::Frusd, 1, (0, LIVE.1)), None);
    }

    #[test]
    fn the_clamp_note_states_the_same_number_the_payload_carries() {
        // `bps / 100` is integer division, so ANY clamp under 100bps printed
        // "0% will become BTC" while the line under it printed the real bps and
        // the payload encoded the real bps. Sub-100bps clamps are the ORDINARY
        // case on a pool this size, not an edge.
        let reserve = 5_854_00_000_000u128;
        let p = plan_conversion(10_000, 100_000_000_000, Some(reserve));
        assert_eq!(p.effective_bps, 58, "this is the clamp the old note printed as 0%");
        assert!(
            p.note.contains("58bps"),
            "the note must carry the number the bytes carry, said: {}",
            p.note
        );
        assert!(!p.note.contains("0%"), "said: {}", p.note);
    }

    #[test]
    fn a_destination_without_a_conversion_is_refused_rather_than_dropped() {
        // `--convert-bps` defaults to 0, so supplying only `--btc-destination`
        // is an easy and entirely reasonable mistake: the flag's whole purpose
        // is paying a third party in BTC out of an EVM balance. Encoding a
        // plain DirectTransfer here mints 100% as frUSD to `--recipient` and
        // says nothing about the destination that was typed and ignored.
        let addr = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        let e = plan_deposit("usdc", 1, addr, 0, Some(addr), 0, None).unwrap_err();
        assert!(e.contains("--convert-bps"), "{e}");
    }

    #[test]
    fn an_out_of_range_slippage_is_refused_even_when_no_conversion_is_encoded() {
        // Otherwise the value is only checked on the branch that uses it, and
        // an absurd one passes silently on the other.
        let addr = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        assert!(plan_deposit("usdc", 1, addr, 0, None, 20_000, None).is_err());
    }

    #[test]
    fn the_direct_swap_depth_note_does_not_promise_a_refusal_that_cannot_happen() {
        // `MAX_POOL_SHARE_BPS` is the COORDINATOR's cap on a bridge conversion.
        // A direct `exchange` has no coordinator in the path: nothing refuses
        // it, it executes at whatever the curve gives, and `min_dy` is the only
        // protection. Telling a trader it "would be REFUSED" describes a
        // mechanism that is not there, and the under-cap wording read as a
        // green light for a size that moves this pool ~8%.
        for bps in [531u128, 999, 1_016, 2_657] {
            let note = direct_swap_depth_note(bps);
            let lower = note.to_lowercase();
            // The claim that must never be made about THIS trade. The note may
            // still explain where a refusal does exist, which is the bridge.
            assert!(!lower.contains("this trade would be refused"), "at {bps}bps: {note}");
            assert!(!lower.contains("pay out in the source asset."), "at {bps}bps: {note}");
            assert!(note.contains(&format!("{bps}bps")), "at {bps}bps: {note}");
            // Every size, over or under the cap, must be framed as impact —
            // the old under-cap wording read as an all-clear.
            assert!(lower.contains("price impact"), "at {bps}bps: {note}");
        }
    }

    #[test]
    fn a_deposit_plan_carries_the_calldata_for_both_calls() {
        let p = plan_deposit(
            "usdc",
            100_000_000,
            "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
            0,
            None,
            0,
            None,
        )
        .unwrap();
        assert_eq!(p.asset_id, 0);
        assert_eq!(p.token, USDC_L1);
        // approve goes to the TOKEN and names the VAULT as spender, for exactly
        // the deposit amount.
        assert_eq!(&p.approve_calldata[..4], &SELECTOR_APPROVE);
        assert_eq!(&p.approve_calldata[4 + 12..4 + 32], &FRUSD_VAULT_L1);
        // depositAndBridge goes to the vault and carries the payload verbatim.
        assert_eq!(&p.deposit_calldata[..4], &SELECTOR_DEPOSIT_AND_BRIDGE);
        assert!(
            p.deposit_calldata
                .windows(p.bridge_data.len())
                .any(|w| w == p.bridge_data.as_slice()),
            "the calldata must carry the bridgeData it was planned with"
        );
    }

    #[test]
    fn a_plain_deposit_is_a_direct_transfer_not_a_zero_sized_conversion() {
        let p = plan_deposit(
            "usdt",
            1,
            "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
            0,
            None,
            0,
            None,
        )
        .unwrap();
        assert_eq!(p.asset_id, 1);
        // Field 2 only: no field 4 tag anywhere.
        assert_eq!(p.bridge_data[0], 0x12);
        assert_eq!(p.bridge_data.len(), 2 + 22);
    }

    #[test]
    fn a_clamped_conversion_encodes_THE_CLAMPED_BPS_not_the_requested_one() {
        // The pool's depth clamp already existed and was already printed. What
        // matters is that the BYTES carry the clamped number too: a payload that
        // asks for more than the screen said is refused by the coordinator, and
        // refusal here means the whole mint arrives as frUSD.
        let addr = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        let reserve = 5_854_00_000_000u128; // ~$5,854 a side
        let p = plan_deposit("usdc", 100_000_000_000, addr, 10_000, Some(addr), 0, Some(reserve))
            .unwrap();
        assert!(p.effective_bps < 10_000, "this deposit is far past the cap");
        assert!(!p.note.is_empty(), "a clamp the user is not told about is a silent degrade");
        let expected = encode_btc_conversion(
            &mainnet_script_pubkey(addr).unwrap(),
            p.effective_bps,
            &mainnet_script_pubkey(addr).unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(p.bridge_data, expected);
    }

    #[test]
    fn a_conversion_clamped_to_nothing_becomes_a_plain_deposit() {
        // `plan_conversion` can clamp to 0. Encoding a 0-bps conversion is
        // refused by the encoder, so the plan must fall back to DirectTransfer
        // rather than fail — the deposit itself is still perfectly valid.
        let addr = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        let p = plan_deposit("usdc", 100_000_000_000_000, addr, 10_000, Some(addr), 0, Some(1))
            .unwrap();
        assert_eq!(p.effective_bps, 0);
        assert_eq!(p.bridge_data[0], 0x12);
        assert_eq!(p.bridge_data.len(), 2 + 22, "must be field 2 alone");
    }

    #[test]
    fn a_deposit_refuses_what_the_coordinator_would_silently_degrade() {
        let addr = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        // testnet recipient
        assert!(plan_deposit(
            "usdc",
            1,
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            0,
            None,
            0,
            None
        )
        .is_err());
        // unknown stable — the vault registered usdc(0) and usdt(1) only
        assert!(plan_deposit("dai", 1, addr, 0, None, 0, None).is_err());
        // zero amount: the vault takes nothing and nothing is minted
        assert!(plan_deposit("usdc", 0, addr, 0, None, 0, None).is_err());
        // converting with nowhere to send the BTC
        assert!(plan_deposit("usdc", 1, addr, 5_000, None, 0, None).is_err());
    }

    #[test]
    fn the_vault_and_the_stables_are_the_deployed_addresses() {
        assert_eq!(
            format!("0x{}", hex::encode(FRUSD_VAULT_L1)),
            "0x95779e7e1c943042255b8a78273fe6de4823cf06"
        );
        // asset ids are the VAULT'S own registration order, not ours.
        assert_eq!(stable_token_l1(0).unwrap(), USDC_L1);
        assert_eq!(stable_token_l1(1).unwrap(), USDT_L1);
        // An unregistered id has no token; answering with one would approve a
        // contract the vault never registered.
        assert!(stable_token_l1(2).is_none());
        assert_eq!(
            format!("0x{}", hex::encode(USDT_L1)).to_lowercase(),
            "0xdac17f958d2ee523a2206206994597c13d831ec7"
        );
    }
}
