/**
 * ESPO tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerEspoTools(): void {
  createSimpleTool(
    'espo_height',
    'Get current ESPO indexer height',
    ['espo', 'height'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createPositionalTool(
    'espo_balances',
    'Get alkanes balances for an address',
    ['espo', 'balances'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address to query balances for' },
        include_outpoints: { type: 'boolean', description: 'Include outpoint details in response' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'espo_outpoints',
    'Get outpoints containing alkanes for an address',
    ['espo', 'outpoints'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address to query outpoints for' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'espo_outpoint',
    'Get alkanes balances at a specific outpoint',
    ['espo', 'outpoint'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'Outpoint (format: txid:vout)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['outpoint'],
    }
  );

  createPositionalTool(
    'espo_holders',
    'Get holders of an alkane token',
    ['espo', 'holders'],
    ['alkane_id'],
    {
      type: 'object',
      properties: {
        alkane_id: { type: 'string', description: 'Alkane ID (format: block:tx)' },
        page: { type: 'number', description: 'Page number', default: 1 },
        limit: { type: 'number', description: 'Items per page', default: 100 },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['alkane_id'],
    }
  );

  createPositionalTool(
    'espo_holders_count',
    'Get holder count for an alkane',
    ['espo', 'holders-count'],
    ['alkane_id'],
    {
      type: 'object',
      properties: {
        alkane_id: { type: 'string', description: 'Alkane ID (format: block:tx)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['alkane_id'],
    }
  );

  createPositionalTool(
    'espo_keys',
    'Get storage keys for an alkane contract',
    ['espo', 'keys'],
    ['alkane_id'],
    {
      type: 'object',
      properties: {
        alkane_id: { type: 'string', description: 'Alkane ID (format: block:tx)' },
        page: { type: 'number', description: 'Page number', default: 1 },
        limit: { type: 'number', description: 'Items per page', default: 100 },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['alkane_id'],
    }
  );

  createSimpleTool(
    'espo_ping',
    'Ping the ESPO server',
    ['espo', 'ping'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'espo_ammdata_ping',
    'Ping the AMM Data module',
    ['espo', 'ammdata-ping'],
    {
      type: 'object',
      properties: {},
    }
  );

  createPositionalTool(
    'espo_candles',
    'Get OHLCV candlestick data for a pool',
    ['espo', 'candles'],
    ['pool'],
    {
      type: 'object',
      properties: {
        pool: { type: 'string', description: 'Pool ID (format: block:tx)' },
        timeframe: { type: 'string', description: 'Timeframe (e.g., "10m", "1h", "1d", "1w", "1M")' },
        side: { type: 'string', description: 'Side ("base" or "quote")' },
        limit: { type: 'number', description: 'Items per page' },
        page: { type: 'number', description: 'Page number' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['pool'],
    }
  );

  createPositionalTool(
    'espo_trades',
    'Get trade history for a pool',
    ['espo', 'trades'],
    ['pool'],
    {
      type: 'object',
      properties: {
        pool: { type: 'string', description: 'Pool ID (format: block:tx)' },
        limit: { type: 'number', description: 'Items per page' },
        page: { type: 'number', description: 'Page number' },
        side: { type: 'string', description: 'Side ("base" or "quote")' },
        filter_side: { type: 'string', description: 'Filter side ("buy", "sell", or "all")' },
        sort: { type: 'string', description: 'Sort field' },
        dir: { type: 'string', description: 'Direction ("asc" or "desc")' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['pool'],
    }
  );

  createSimpleTool(
    'espo_pools',
    'Get all pools with pagination',
    ['espo', 'pools'],
    {
      type: 'object',
      properties: {
        limit: { type: 'number', description: 'Items per page' },
        page: { type: 'number', description: 'Page number' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'espo_find_best_swap_path',
    'Find the best swap path between two tokens',
    ['espo', 'find-best-swap-path'],
    {
      type: 'object',
      properties: {
        token_in: { type: 'string', description: 'Input token (format: block:tx)' },
        token_out: { type: 'string', description: 'Output token (format: block:tx)' },
        mode: { type: 'string', description: 'Mode ("exact_in", "exact_out", or "implicit")' },
        amount_in: { type: 'string', description: 'Amount in (as string to preserve precision)' },
        amount_out: { type: 'string', description: 'Amount out (as string to preserve precision)' },
        amount_out_min: { type: 'string', description: 'Minimum amount out' },
        amount_in_max: { type: 'string', description: 'Maximum amount in' },
        available_in: { type: 'string', description: 'Available amount in' },
        fee_bps: { type: 'number', description: 'Fee in basis points' },
        max_hops: { type: 'number', description: 'Maximum number of hops' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['token_in', 'token_out'],
    }
  );

  createPositionalTool(
    'espo_get_best_mev_swap',
    'Find the best MEV swap opportunity for a token',
    ['espo', 'get-best-mev-swap'],
    ['token'],
    {
      type: 'object',
      properties: {
        token: { type: 'string', description: 'Token (format: block:tx)' },
        fee_bps: { type: 'number', description: 'Fee in basis points' },
        max_hops: { type: 'number', description: 'Maximum number of hops' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['token'],
    }
  );
}
