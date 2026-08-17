---
name: btcusd-trading
description: Trade BTC/USD on SUBFROST — the frUSD/frBTC CryptoSwap pool on alkanes, and the EVM bridge arms (USDC/USDT/ETH ↔ BTC). Use when reading BTCUSD market data, quoting or executing a swap, building a bridge deposit or burn, or watching mempool/rollup activity on either chain.
---

# Trading BTC/USD on SUBFROST

You are operating on **real money on mainnet**. There is no testnet deployment of
this pool or this vault. Every number below was measured, not assumed, and the
warnings mark places where the system **succeeds while doing the wrong thing** —
those are the ones that will cost a user funds, because nothing errors.

## The one rule that overrides convenience

**Read chain state with `metashrew_view`. Never with an `alkanes_*` JSON-RPC
method.** The `alkanes_*` wrappers are a different code path from what production
reads, and they diverge. `alkanes-cli-common` already speaks the correct surface
everywhere (33 `getbytecode` call sites, 26 `simulate`) — follow it.

Also: never use mempool.space. Use `/v4/{apikey}` endpoints.

---

## 1. Endpoints, and the API-key warning you must surface

| endpoint | serves |
|---|---|
| `/v4/{apikey}` | the main JSON-RPC: `metashrew_view`, `metashrew_height`, `esplora_*`, `btc_*` |
| `/v4/{apikey}/btcusd` | the dedicated BTCUSD index (protobuf both directions, package `alspo.cryptoswap`) |
| `/v4/{apikey}/espo` | espo-shaped `<module>.<suffix>` routes, e.g. `ammdata.get_pools` |
| `/v4/{apikey}/mempool` + `/mempool/ws` | mempool JSON-RPC and change stream — **not live yet**, see §7 |

> ⚠️ **If the user has no API key you will fall back to `/v4/jsonrpc`, and you
> MUST say so out loud.** State plainly: *"Using the shared default endpoint
> `/v4/jsonrpc`. It is rate-limited and shared; get a key at api.subfrost.io for
> full throughput."* Do not silently use it — an unattended agent that never
> mentions it will look like it is running normally right up until it is
> throttled mid-trade.

**Throttle to the cap rather than failing.** When rate-limited, slow down and
keep going; do not abort a workflow. A slower correct run beats a half-finished
one, and a partially-executed trade sequence is the worst outcome available.

---

## 2. The pool

| | |
|---|---|
| **Address callers use** | `4:1778` — an `Upgradeable` **PROXY**, and also the frBTCUSD **LP token** |
| **Implementation** | `4:1786` — the CryptoSwap (Curve V2) math |
| **token0** | frUSD `4:1776` (itself a proxy) |
| **token1** | frBTC `32:0` |
| **Curve** | A = 400000, gamma = 1.45e14, precision = 1e10 both legs |
| **Fees** | mid 0.2%, out 0.8% |

### Opcodes

```
0   init_pool            100  get_virtual_price
1   add_liquidity        101  get_balances
2   remove_liquidity     102  get_A          ← NOT decimals. See below.
3   remove_liq_one_coin  103  get_gamma
5   exchange             104  price_oracle
                         105  price_scale
                         106  lp_price       ← the CryptoSwap discriminator
                         107  get_dy
                         108  calc_token_amount
```

The proxy has its OWN opcodes in a high range so they cannot collide:
`initialize = 32767`, `initialize_with_auth = 32764`, `upgrade = 32766`,
`forward = 36863`. Everything else falls through to the implementation via
delegatecall.

> ⚠️ **`4:1778` is 18 decimals, not 8**, and it does not implement the token ABI.
> Opcode 99 (`get_name`) REVERTS; opcode 100 returns a 1e18 number
> (`virtual_price`), not a symbol; **opcode 102 returns 400000 — the
> amplification `A`** — so anything treating 102 as `get_decimals` renders every
> LP balance at 1e400000. Identify a CryptoSwap pool by **`lp_price` (106)
> answering**, which reverts on ordinary tokens.
>
> 18 is a property of the math, not a declaration: LP is minted in units of the
> invariant D, which lives in 1e18-normalised space.

### Live constraints — check these before quoting anything

**The invariants (these do not move):**

* **Depth cap.** A single conversion may take at most **10% of the pool's frUSD
  reserve** (`MAX_POOL_SHARE_BPS = 1000`). Over that, the coordinator **refuses
  the swap and pays out plain frUSD instead** — the deposit still succeeds, no
  error on either chain. So the cap is a fraction, and its dollar value moves
  with the pool.
* **The pool's price is set by its own reserves, not by the market**, and this is
  a shallow pool. The gap between the two can be any size and in either
  direction, and arbitrage will not reliably close it — the profit available is
  too small against fees at this depth.

**Therefore: never quote either number from memory. Measure both, then warn on
any trade above the depth cap and quote the effective execution price, not the
pool's marginal price.**

> ⚠️ **The measured values below are a SNAPSHOT and they go stale fast.** An
> earlier version of this file carried numbers from 2026-08-10 and, one week
> later, understated the pool by 4.2× and claimed the pool was "8% over market"
> when it was *under*. That error runs in the dangerous direction: it talks a
> user out of a fair trade, and it would just as easily talk one into a bad one
> after the next move. **Re-measure before quoting.**
>
> Note what actually changed: the pool's own `price_scale` barely moved (64,164 →
> 64,069). The market rose to meet it. So the "stale by construction" framing was
> right about the mechanism and wrong to assume the gap stays on one side.

**Snapshot read 2026-08-17** (chain tip 962,947 via `metashrew_height`; the pool
state itself is unchanged since block **962,598**, its last event, on 2026-08-15):

| | |
|---|---|
| frUSD reserve | 24,304.66 |
| frBTC reserve | 0.38092710 |
| **pool price** (`marginal_price_q`) | **~$64,075 / BTC** |
| `price_scale` | ~$64,069 / BTC |
| reserve ratio (sanity check only) | ~$63,804 / BTC |
| depth cap (10% of frUSD) | ~$2,430 |
| market, same reading | ~$64,342 → pool ~0.4% **under** |

Re-measure with one call:

```bash
curl -s https://mainnet.subfrost.io/v4/$KEY/btcusd \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"metashrew_view",
       "params":["getpoolstate","0x0a05080410f20d","latest"]}'
```

> ⚠️ **The response is prost hex, not JSON.** `GetPoolStateResponse`: `token0`=f3,
> `token1`=f4, `state`=f5, and inside that `reserve0`/`reserve1` are decimal
> strings at 8 decimals (keep them strings — see §3). Read `token0`/`token1` from
> the response rather than assuming the order. The reference decoder is
> `decode_pool_state` in `alkanes-cli-common/src/btcusd_exec.rs`.

> 🔴 **Take the pool's price from `marginal_price_q` (state f10), as
> `price_q_scale / marginal_price_q` — the reciprocal rule in §3. Do NOT divide
> the reserves.** In CryptoSwap the balances sit near `price_scale`, not at
> parity, so the reserve ratio is not the tradable price: at this reading the two
> differ by 0.42%, which is more than the pool's own mid fee. It is a sanity
> check, nothing more. And for an actual trade, quote with `get_dy` (§4) rather
> than any of these — none of them include your size.

---

## 3. Reading market data

Prefer the dedicated index; it is protobuf in both directions.

```bash
curl -s https://mainnet.subfrost.io/v4/$KEY/btcusd \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"metashrew_view",
       "params":["getprice","<prost-hex>","latest"]}'
```

Views: `getprice`, `getpoolstate`, `getreserves`, `getcandles`, `getpools`,
`indexheight`. Buckets for candles: **3600 and 86400 only**.

> ⚠️ **Amounts are decimal STRINGS and must stay strings.** `total_supply` is
> `96211459799069359794` at the 2026-08-17 reading — past `u64`. Parsing as a double or u64 silently
> corrupts it. This is not hypothetical: a u64 parse is what froze 23.0137 LP as
> unspendable.
>
> ⚠️ **Prices are token1-per-token0 (frBTC per frUSD), so USD/BTC is the
> RECIPROCAL** — `price_q_scale / price_q`. Getting it backwards yields a number
> near zero, which reads as "no price" rather than as an error.
>
> ⚠️ **`indexheight` is a tautology, not a liveness signal.** `/index_height` is
> rewritten every block with that block's height, so a view read at height H
> always returns H — it will happily return a height for a block that does not
> exist. Use `metashrew_height` to tell fresh from stalled.

Reading state directly instead: `metashrew_view("simulate", …)` with a
`MessageContextParcel` whose `calldata` is the enciphered cellpack.

`getbytecode` takes a **`BytecodeRequest` that WRAPS an `AlkaneId`** — passing a
bare `AlkaneId` returns an **empty result, not an error**, which reads as "this
contract has no code."

---

## 4. Arm A — direct swap, frUSD ↔ frBTC

Both assets already on alkanes. Call `exchange` (opcode 5) on `4:1778`.

1. Quote with `get_dy` (107) — do not compute the curve yourself.
2. Check the trade against the depth cap (§2) and refuse or warn.
3. Set a slippage bound. The pool's own fee band is 0.2%–0.8% depending on how
   unbalanced the trade leaves it, so a quote taken at one balance and executed at
   another must tolerate the difference.
4. Build the protostone, sign, broadcast.

---

## 5. Arm B — EVM → BTC (deposit)

User holds USDC/USDT on Ethereum, wants BTC.

**The wallet cannot sign this.** The Ethereum key is the user's. Prepare the
transaction and hand it off; never claim it is sent.

Vault: `0x95779e7e1c943042255b8a78273fe6de4823cf06` (ERC1967 proxy, L1).
Assets: `assets(0) = USDC`, `assets(1) = USDT`, both 6-decimal.

```
approve(vault, amount)            → then
depositAndBridge(uint8 assetId, uint256 amount, bytes bridgeData)
selector 0xbedb65ee
```

`bridgeData` is a protobuf (`frusd.bridge.BridgeData`):

```
field 1  action            = 0 (DIRECT_TRANSFER) — always
field 2  recipient_script  = the BTC scriptPubKey the frUSD settles at
field 3  recipient_protostone  (optional, MUTUALLY EXCLUSIVE with 4)
field 4  btc_conversion        (optional) = BtcConversion{
             1 convert_bps      0..=10000
             2 btc_destination  required when convert_bps > 0
             3 max_slippage_bps honoured DOWNWARDS only }
```

Dispatch is by **field presence**, not by `action`.

> ⚠️ **Every failure on this path is silent.** A malformed `bridgeData` still
> SUCCEEDS on Ethereum — the vault stores the bytes verbatim — and the
> coordinator then pays the **operator's fallback recipient** instead of the
> depositor. Validate before signing, because nothing downstream will.
>
> ⚠️ **`btc_destination` is the most dangerous field in the system.** frBTC's
> `burn()` validates the unwrap pointer's INDEX but never the SCRIPT at it. It
> copies the script into the payment record and burns the frBTC immediately — no
> escrow, no expiry, no reclaim. A non-standard destination destroys the supply
> against an address that can never be paid, and the contract will not stop it.
> **Accept only P2PKH, P2SH, or witness v0–v16, with v0 restricted to the 20- and
> 32-byte programs.** Refuse anything else; do not "clean it up".
>
> ⚠️ **`convert_bps` out of range is REFUSED, never clamped** — someone who wrote
> 20000 meant something, and it was not 100%.
>
> ⚠️ **Approve exactly the deposit amount, not `uint256::MAX`.** A standing
> infinite allowance on a proxy the user does not control outlives the single
> transaction it was granted for.

Use addresses, not script hex, at the user boundary — a person can recognise
`bc1p…` and cannot proofread 34 bytes. Convert and **require mainnet**: a testnet
address parses fine and yields a well-formed script that mainnet pays into a hole.

### Loading an Ethereum key

Never ask a user to paste a private key into a chat. Prefer, in order:

1. **Hand off to their wallet** — build the calldata and give them a link or an
   EIP-681 URI; MetaMask signs.
2. **A keystore file** they unlock locally (`--keystore path --password-file …`).
3. **An env var** (`ETH_PRIVATE_KEY`) for unattended runs only, and say clearly
   that it is in the process environment.

Never log the key, never echo it, never put it in a command line that lands in
shell history.

---

## 6. Arm C — BTC → EVM (burn)

`burnData` is **NOT protobuf** — it is a raw byte encoding:

```
empty  or  [0x01, 0x00]                            → plain Transfer
[0x01, 0x01, <20-byte target>, <calldata…>]        → Call
```

Only allowlisted target: **Uniswap V2 Router02**
`0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D`, calldata
`swapExactTokensForETH(amountIn, amountOutMin, path, to, deadline)` with
`path = [USDC, WETH]`.

> ⚠️ **`amountOutMin` is YOURS to set. There is no coordinator-side slippage
> floor on this leg.** Passing 0 is an unbounded-loss instruction.

V2 and not V3 deliberately: one `target.call`, no multicall.

---

## 7. Watching: mempool, rollups, and the user's own activity

* **Signer addresses.** Track the frBTC/frUSD signing group's taproot addresses
  to see rollups land. Query with `metashrew_view("protorunesbyaddress", …)`.
* **⚠️ `protorunesbyaddress` is currently slow and marginal** — measured ~10.5s
  against a 15s pool timeout on a busy address. Budget for it, cache it, and do
  not put it on a tight poll loop. It has already caused an outage.
* **Both chains.** A bridge trade is not done when the Bitcoin side settles;
  check the Ethereum side too via the configured ETH RPC.
* **Keep a local view.** Persist what you submitted — txids, intents, expected
  outcomes — so a resumed session can reconcile rather than re-submit. Duplicate
  submission on a bridge path is a real loss, not an inconvenience.
* **`/v4/{apikey}/mempool/*` is documented but NOT DEPLOYED yet.** It will 404.
  Do not build a workflow that depends on it without checking first.

---

## 8. Before you execute anything

1. Is the trade within the **depth cap**? If not, say what will actually happen
   (frUSD, not BTC).
2. Have you **measured** the pool against the market today, and quoted the
   **effective execution price** including whatever that gap costs — in whichever
   direction it currently runs?
3. For a bridge: is `btc_destination` a **standard, payable mainnet** script?
4. For a bridge: is the approve **exactly** the amount?
5. Are amounts still **strings**, not floats?
6. Did you tell the user you are on the **shared `/v4/jsonrpc`** if you are?
7. Have you said clearly that **you are not signing the Ethereum side** — they
   are?

If you cannot answer all seven, do not broadcast. Explain what is missing.
