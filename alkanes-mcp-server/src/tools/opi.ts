/**
 * OPI (Open Protocol Indexer) tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerOpiTools(): void {
  // BRC-20 tools
  createSimpleTool(
    'opi_block_height',
    'Get current indexed block height (BRC-20)',
    ['opi', 'block-height'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_extras_block_height',
    'Get extras indexed block height (BRC-20)',
    ['opi', 'extras-block-height'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_db_version',
    'Get database version (BRC-20)',
    ['opi', 'db-version'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_event_hash_version',
    'Get event hash version (BRC-20)',
    ['opi', 'event-hash-version'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_balance_on_block',
    'Get balance at a specific block height (BRC-20)',
    ['opi', 'balance-on-block'],
    {
      type: 'object',
      properties: {
        block_height: { type: 'number', description: 'Block height to query' },
        pkscript: { type: 'string', description: 'Pkscript of the wallet' },
        ticker: { type: 'string', description: 'BRC-20 ticker' },
      },
      required: ['block_height', 'pkscript', 'ticker'],
    }
  );

  createSimpleTool(
    'opi_activity_on_block',
    'Get all BRC-20 activity for a block',
    ['opi', 'activity-on-block'],
    {
      type: 'object',
      properties: {
        block_height: { type: 'number', description: 'Block height to query' },
      },
      required: ['block_height'],
    }
  );

  createSimpleTool(
    'opi_current_balance',
    'Get current balance of a wallet (BRC-20)',
    ['opi', 'current-balance'],
    {
      type: 'object',
      properties: {
        ticker: { type: 'string', description: 'BRC-20 ticker' },
        address: { type: 'string', description: 'Bitcoin address' },
        pkscript: { type: 'string', description: 'Pkscript of the wallet' },
      },
      required: ['ticker'],
    }
  );

  createSimpleTool(
    'opi_holders',
    'Get holders of a BRC-20 ticker',
    ['opi', 'holders'],
    {
      type: 'object',
      properties: {
        ticker: { type: 'string', description: 'BRC-20 ticker' },
      },
      required: ['ticker'],
    }
  );

  createPositionalTool(
    'opi_event',
    'Get events for an inscription (BRC-20)',
    ['opi', 'event'],
    ['inscription_id'],
    {
      type: 'object',
      properties: {
        inscription_id: { type: 'string', description: 'Inscription ID' },
      },
      required: ['inscription_id'],
    }
  );

  createSimpleTool(
    'opi_ip',
    'Get client IP (for debugging)',
    ['opi', 'ip'],
    {
      type: 'object',
      properties: {},
    }
  );

  createPositionalTool(
    'opi_raw',
    'Make a raw request to OPI endpoint',
    ['opi', 'raw'],
    ['endpoint'],
    {
      type: 'object',
      properties: {
        endpoint: { type: 'string', description: 'Endpoint path (e.g., "v1/brc20/block_height")' },
      },
      required: ['endpoint'],
    }
  );

  // Runes subcommands
  createSimpleTool(
    'opi_runes_block_height',
    'Get current indexed block height (Runes)',
    ['opi', 'runes', 'block-height'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_runes_current_balance',
    'Get current Runes balance of a wallet',
    ['opi', 'runes', 'current-balance'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Bitcoin address' },
        pkscript: { type: 'string', description: 'Pkscript of the wallet' },
      },
    }
  );

  createSimpleTool(
    'opi_runes_holders',
    'Get holders of a Rune',
    ['opi', 'runes', 'holders'],
    {
      type: 'object',
      properties: {
        rune_id: { type: 'string', description: 'Rune ID' },
      },
      required: ['rune_id'],
    }
  );

  // Bitmap subcommands
  createSimpleTool(
    'opi_bitmap_block_height',
    'Get current indexed block height (Bitmap)',
    ['opi', 'bitmap', 'block-height'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_bitmap_inscription_id',
    'Get inscription ID of a bitmap',
    ['opi', 'bitmap', 'inscription-id'],
    {
      type: 'object',
      properties: {
        bitmap: { type: 'string', description: 'Bitmap number' },
      },
      required: ['bitmap'],
    }
  );

  // POW20 subcommands
  createSimpleTool(
    'opi_pow20_block_height',
    'Get current indexed block height (POW20)',
    ['opi', 'pow20', 'block-height'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_pow20_current_balance',
    'Get current POW20 balance of a wallet',
    ['opi', 'pow20', 'current-balance'],
    {
      type: 'object',
      properties: {
        ticker: { type: 'string', description: 'POW20 ticker' },
        address: { type: 'string', description: 'Bitcoin address' },
        pkscript: { type: 'string', description: 'Pkscript of the wallet' },
      },
      required: ['ticker'],
    }
  );

  createSimpleTool(
    'opi_pow20_holders',
    'Get holders of a POW20 ticker',
    ['opi', 'pow20', 'holders'],
    {
      type: 'object',
      properties: {
        ticker: { type: 'string', description: 'POW20 ticker' },
      },
      required: ['ticker'],
    }
  );

  // SNS subcommands
  createSimpleTool(
    'opi_sns_block_height',
    'Get current indexed block height (SNS)',
    ['opi', 'sns', 'block-height'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'opi_sns_info',
    'Get info about an SNS name',
    ['opi', 'sns', 'info'],
    {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'SNS name to query' },
      },
      required: ['name'],
    }
  );
}
