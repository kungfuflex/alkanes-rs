/**
 * DataAPI tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerDataApiTools(): void {
  createSimpleTool(
    'dataapi_get_alkanes',
    'Get all alkanes',
    ['dataapi', 'get-alkanes'],
    {
      type: 'object',
      properties: {
        limit: { type: 'number', description: 'Limit results' },
        offset: { type: 'number', description: 'Offset results' },
        sort_by: { type: 'string', description: 'Sort by field' },
        order: { type: 'string', description: 'Sort order' },
        search: { type: 'string', description: 'Search query' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );

  createPositionalTool(
    'dataapi_get_alkanes_by_address',
    'Get alkanes for an address',
    ['dataapi', 'get-alkanes-by-address'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address to query' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'dataapi_get_alkane_details',
    'Get alkane details',
    ['dataapi', 'get-alkane-details'],
    ['id'],
    {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'Alkane ID in format BLOCK:TX' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['id'],
    }
  );

  createSimpleTool(
    'dataapi_get_pools',
    'Get all pools (defaults to factory 4:65522)',
    ['dataapi', 'get-pools'],
    {
      type: 'object',
      properties: {
        factory: { type: 'string', description: 'Factory ID in format BLOCK:TX', default: '4:65522' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );

  createPositionalTool(
    'dataapi_get_pool_by_id',
    'Get pool details',
    ['dataapi', 'get-pool-by-id'],
    ['id'],
    {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'Pool ID in format BLOCK:TX' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['id'],
    }
  );

  createPositionalTool(
    'dataapi_get_pool_history',
    'Get pool history',
    ['dataapi', 'get-pool-history'],
    ['pool_id'],
    {
      type: 'object',
      properties: {
        pool_id: { type: 'string', description: 'Pool ID in format BLOCK:TX' },
        category: { type: 'string', description: 'Category filter' },
        limit: { type: 'number', description: 'Limit results' },
        offset: { type: 'number', description: 'Offset results' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['pool_id'],
    }
  );

  createSimpleTool(
    'dataapi_get_swap_history',
    'Get swap history',
    ['dataapi', 'get-swap-history'],
    {
      type: 'object',
      properties: {
        pool_id: { type: 'string', description: 'Pool ID filter' },
        limit: { type: 'number', description: 'Limit results' },
        offset: { type: 'number', description: 'Offset results' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );

  createSimpleTool(
    'dataapi_get_bitcoin_price',
    'Get Bitcoin price',
    ['dataapi', 'get-bitcoin-price'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );

  createPositionalTool(
    'dataapi_get_market_chart',
    'Get Bitcoin market chart',
    ['dataapi', 'get-market-chart'],
    ['days'],
    {
      type: 'object',
      properties: {
        days: { type: 'string', description: 'Number of days (1, 7, 14, 30, 90, 180, 365, max)' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['days'],
    }
  );

  createSimpleTool(
    'dataapi_health',
    'Health check',
    ['dataapi', 'health'],
    {
      type: 'object',
      properties: {
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );

  createPositionalTool(
    'dataapi_get_holders',
    'Get holders of an alkane token',
    ['dataapi', 'get-holders'],
    ['alkane'],
    {
      type: 'object',
      properties: {
        alkane: { type: 'string', description: 'Alkane ID in format BLOCK:TX' },
        page: { type: 'number', description: 'Page number', default: 1 },
        limit: { type: 'number', description: 'Results per page', default: 100 },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['alkane'],
    }
  );

  createPositionalTool(
    'dataapi_get_holder_count',
    'Get holder count for an alkane token',
    ['dataapi', 'get-holder-count'],
    ['alkane'],
    {
      type: 'object',
      properties: {
        alkane: { type: 'string', description: 'Alkane ID in format BLOCK:TX' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['alkane'],
    }
  );

  createPositionalTool(
    'dataapi_get_address_balances',
    'Get alkane balances for an address (with UTXO tracking)',
    ['dataapi', 'get-address-balances'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address or address identifier' },
        include_outpoints: { type: 'boolean', description: 'Include individual outpoint details' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'dataapi_get_outpoint_balances',
    'Get alkane balances for a specific outpoint',
    ['dataapi', 'get-outpoint-balances'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'Outpoint in format TXID:VOUT' },
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
      required: ['outpoint'],
    }
  );

  createSimpleTool(
    'dataapi_get_block_height',
    'Get the latest block height processed by the indexer',
    ['dataapi', 'get-block-height'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );

  createSimpleTool(
    'dataapi_get_block_hash',
    'Get the latest block hash processed by the indexer',
    ['dataapi', 'get-block-hash'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );

  createSimpleTool(
    'dataapi_get_indexer_position',
    'Get the indexer position (height and hash of latest processed block)',
    ['dataapi', 'get-indexer-position'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Output raw JSON' },
        raw_http: { type: 'boolean', description: 'Output raw HTTP response' },
      },
    }
  );
}
