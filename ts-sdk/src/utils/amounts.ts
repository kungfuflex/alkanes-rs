/**
 * Amount parsing and formatting utilities for @alkanes/ts-sdk
 *
 * Provides utilities to convert between raw string amounts (from the API)
 * and human-readable BigInt values with proper decimal handling.
 *
 * Default decimals is 8 (same as Bitcoin satoshi precision).
 */

/** Default decimal places for alkane tokens */
export const DEFAULT_DECIMALS = 8;

/**
 * Parse a raw amount string to a BigInt
 *
 * @param amount - Raw amount as string (e.g., "1000000000000000")
 * @returns BigInt representation
 *
 * @example
 * ```typescript
 * const raw = parseAmount("1000000000000000");
 * // raw = 1000000000000000n
 * ```
 */
export function parseAmount(amount: string | number | bigint): bigint {
  if (typeof amount === 'bigint') return amount;
  if (typeof amount === 'number') return BigInt(Math.floor(amount));
  // Handle hex strings
  if (amount.startsWith('0x')) {
    return BigInt(amount);
  }
  // Handle regular decimal strings
  return BigInt(amount.replace(/[^0-9-]/g, ''));
}

/**
 * Format a raw amount to a human-readable decimal string
 *
 * @param amount - Raw amount as string/bigint (e.g., "100000000")
 * @param decimals - Number of decimal places (default: 8)
 * @returns Human-readable string (e.g., "1.0")
 *
 * @example
 * ```typescript
 * formatAmount("100000000", 8) // "1.0"
 * formatAmount("150000000", 8) // "1.5"
 * formatAmount("1000000000000000", 8) // "10000000.0"
 * ```
 */
export function formatAmount(
  amount: string | number | bigint,
  decimals: number = DEFAULT_DECIMALS
): string {
  const value = parseAmount(amount);
  const divisor = 10n ** BigInt(decimals);
  const intPart = value / divisor;
  const fracPart = value % divisor;

  if (fracPart === 0n) {
    return `${intPart}.0`;
  }

  // Pad fractional part with leading zeros
  const fracStr = fracPart.toString().padStart(decimals, '0');
  // Remove trailing zeros
  const trimmed = fracStr.replace(/0+$/, '');
  return `${intPart}.${trimmed}`;
}

/**
 * Convert a human-readable amount to raw BigInt
 *
 * @param amount - Human-readable amount (e.g., "1.5")
 * @param decimals - Number of decimal places (default: 8)
 * @returns Raw BigInt amount
 *
 * @example
 * ```typescript
 * toRawAmount("1.0", 8) // 100000000n
 * toRawAmount("1.5", 8) // 150000000n
 * toRawAmount("0.00000001", 8) // 1n
 * ```
 */
export function toRawAmount(
  amount: string | number,
  decimals: number = DEFAULT_DECIMALS
): bigint {
  const strAmount = typeof amount === 'number' ? amount.toString() : amount;
  const [intPart, fracPart = ''] = strAmount.split('.');

  // Pad or truncate fractional part to match decimals
  const paddedFrac = fracPart.padEnd(decimals, '0').slice(0, decimals);
  const combined = intPart + paddedFrac;

  return BigInt(combined);
}

/**
 * Parsed alkane balance with both raw and formatted values
 */
export interface ParsedAlkaneBalance {
  /** Alkane ID in "block:tx" format */
  id: string;
  /** Block number */
  block: number;
  /** Transaction index */
  tx: number;
  /** Raw amount as BigInt */
  rawAmount: bigint;
  /** Formatted amount string (e.g., "1.5") */
  amount: string;
  /** Token name if available */
  name?: string;
  /** Token symbol if available */
  symbol?: string;
  /** Token decimals (default: 8) */
  decimals: number;
}

/**
 * Parse raw alkane balance response into typed structure with formatted amounts
 *
 * @param balance - Raw balance object from API
 * @returns Parsed balance with formatted amounts
 *
 * @example
 * ```typescript
 * const raw = { id: "2:100", amount: "1000000000000000", name: "FARTUNE100", symbol: "F100", decimals: 8 };
 * const parsed = parseAlkaneBalance(raw);
 * // parsed.amount = "10000000.0"
 * // parsed.rawAmount = 1000000000000000n
 * ```
 */
export function parseAlkaneBalance(balance: any): ParsedAlkaneBalance {
  const decimals = balance.decimals ?? DEFAULT_DECIMALS;
  const rawAmount = parseAmount(balance.amount || balance.value || '0');

  // Parse ID - can be "block:tx" string or { block, tx } object
  let block: number;
  let tx: number;
  let id: string;

  if (typeof balance.id === 'string') {
    const [b, t] = balance.id.split(':').map(Number);
    block = b;
    tx = t;
    id = balance.id;
  } else if (balance.id && typeof balance.id === 'object') {
    block = balance.id.block;
    tx = balance.id.tx;
    id = `${block}:${tx}`;
  } else {
    block = balance.block ?? 0;
    tx = balance.tx ?? 0;
    id = `${block}:${tx}`;
  }

  return {
    id,
    block,
    tx,
    rawAmount,
    amount: formatAmount(rawAmount, decimals),
    name: balance.name,
    symbol: balance.symbol,
    decimals,
  };
}

/**
 * Parse an array of alkane balances
 *
 * @param balances - Raw balance array from API
 * @returns Array of parsed balances
 */
export function parseAlkaneBalances(balances: any[]): ParsedAlkaneBalance[] {
  return balances.map(parseAlkaneBalance);
}

/**
 * Parsed reflect metadata with formatted amounts
 */
export interface ParsedReflectMetadata {
  /** Alkane ID in "block:tx" format */
  id: string;
  /** Token name */
  name: string;
  /** Token symbol */
  symbol: string;
  /** Total supply as BigInt */
  rawTotalSupply: bigint;
  /** Total supply formatted */
  totalSupply: string;
  /** Minting cap as BigInt */
  rawCap: bigint;
  /** Minting cap formatted */
  cap: string;
  /** Amount minted as BigInt */
  rawMinted: bigint;
  /** Amount minted formatted */
  minted: string;
  /** Value per mint as BigInt */
  rawValuePerMint: bigint;
  /** Value per mint formatted */
  valuePerMint: string;
  /** Premine amount as BigInt */
  rawPremine: bigint;
  /** Premine amount formatted */
  premine: string;
  /** Token decimals */
  decimals: number;
  /** Additional data */
  data?: string;
}

/**
 * Parse raw reflect metadata response into typed structure with formatted amounts
 *
 * @param metadata - Raw metadata object from API
 * @returns Parsed metadata with formatted amounts
 */
export function parseReflectMetadata(metadata: any): ParsedReflectMetadata {
  const decimals = metadata.decimals ?? DEFAULT_DECIMALS;

  const rawTotalSupply = parseAmount(metadata.total_supply || metadata.totalSupply || '0');
  const rawCap = parseAmount(metadata.cap || '0');
  const rawMinted = parseAmount(metadata.minted || '0');
  const rawValuePerMint = parseAmount(metadata.value_per_mint || metadata.valuePerMint || '0');
  const rawPremine = parseAmount(metadata.premine || '0');

  return {
    id: metadata.id || '',
    name: metadata.name || '',
    symbol: metadata.symbol || '',
    rawTotalSupply,
    totalSupply: formatAmount(rawTotalSupply, decimals),
    rawCap,
    cap: rawCap.toString(), // Cap is typically a count, not an amount
    rawMinted,
    minted: rawMinted.toString(), // Minted is typically a count
    rawValuePerMint,
    valuePerMint: formatAmount(rawValuePerMint, decimals),
    rawPremine,
    premine: formatAmount(rawPremine, decimals),
    decimals,
    data: metadata.data,
  };
}

/**
 * Parsed pool details with formatted reserves
 */
export interface ParsedPoolDetails {
  /** Pool ID in "block:tx" format */
  poolId: string;
  /** Token 0 ID */
  token0: string;
  /** Token 1 ID */
  token1: string;
  /** Reserve 0 as BigInt */
  rawReserve0: bigint;
  /** Reserve 0 formatted */
  reserve0: string;
  /** Reserve 1 as BigInt */
  rawReserve1: bigint;
  /** Reserve 1 formatted */
  reserve1: string;
  /** Total LP supply as BigInt */
  rawTotalSupply: bigint;
  /** Total LP supply formatted */
  totalSupply: string;
  /** Token 0 decimals */
  decimals0: number;
  /** Token 1 decimals */
  decimals1: number;
}

/**
 * Parse raw pool details response into typed structure with formatted amounts
 *
 * @param pool - Raw pool object from API
 * @param decimals0 - Decimals for token 0 (default: 8)
 * @param decimals1 - Decimals for token 1 (default: 8)
 * @returns Parsed pool with formatted reserves
 */
export function parsePoolDetails(
  pool: any,
  decimals0: number = DEFAULT_DECIMALS,
  decimals1: number = DEFAULT_DECIMALS
): ParsedPoolDetails {
  const rawReserve0 = parseAmount(pool.reserve0 || '0');
  const rawReserve1 = parseAmount(pool.reserve1 || '0');
  const rawTotalSupply = parseAmount(pool.total_supply || pool.totalSupply || '0');

  return {
    poolId: pool.pool_id || pool.poolId || '',
    token0: pool.token0 || '',
    token1: pool.token1 || '',
    rawReserve0,
    reserve0: formatAmount(rawReserve0, decimals0),
    rawReserve1,
    reserve1: formatAmount(rawReserve1, decimals1),
    rawTotalSupply,
    totalSupply: formatAmount(rawTotalSupply, DEFAULT_DECIMALS),
    decimals0,
    decimals1,
  };
}

/**
 * Parsed trade with formatted amounts
 */
export interface ParsedTrade {
  /** Transaction ID */
  txid: string;
  /** Output index */
  vout: number;
  /** Block height */
  blockHeight: number;
  /** Timestamp */
  timestamp: string;
  /** Trade side: "buy" | "sell" */
  side: string;
  /** Amount in as BigInt */
  rawAmountIn: bigint;
  /** Amount in formatted */
  amountIn: string;
  /** Amount out as BigInt */
  rawAmountOut: bigint;
  /** Amount out formatted */
  amountOut: string;
  /** Price */
  price: string;
}

/**
 * Parse raw trade response into typed structure with formatted amounts
 *
 * @param trade - Raw trade object from API
 * @param decimalsIn - Decimals for input token (default: 8)
 * @param decimalsOut - Decimals for output token (default: 8)
 * @returns Parsed trade with formatted amounts
 */
export function parseTrade(
  trade: any,
  decimalsIn: number = DEFAULT_DECIMALS,
  decimalsOut: number = DEFAULT_DECIMALS
): ParsedTrade {
  const rawAmountIn = parseAmount(trade.amount_in || trade.amountIn || '0');
  const rawAmountOut = parseAmount(trade.amount_out || trade.amountOut || '0');

  return {
    txid: trade.txid || '',
    vout: trade.vout ?? 0,
    blockHeight: trade.block_height || trade.blockHeight || 0,
    timestamp: trade.timestamp || '',
    side: trade.side || '',
    rawAmountIn,
    amountIn: formatAmount(rawAmountIn, decimalsIn),
    rawAmountOut,
    amountOut: formatAmount(rawAmountOut, decimalsOut),
    price: trade.price || '',
  };
}

/**
 * Satoshi to BTC conversion (8 decimals)
 */
export function satsToBtc(sats: string | number | bigint): string {
  return formatAmount(sats, 8);
}

/**
 * BTC to satoshi conversion
 */
export function btcToSats(btc: string | number): bigint {
  return toRawAmount(btc, 8);
}
