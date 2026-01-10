/**
 * Esplora API tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerEsploraTools(): void {
  createSimpleTool(
    'esplora_blocks_tip_hash',
    'Get blocks tip hash',
    ['esplora', 'blocks-tip-hash'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'esplora_blocks_tip_height',
    'Get blocks tip height',
    ['esplora', 'blocks-tip-height'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'esplora_blocks',
    'Get blocks',
    ['esplora', 'blocks'],
    {
      type: 'object',
      properties: {
        start_height: { type: 'number', description: 'Start height' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createPositionalTool(
    'esplora_block_height',
    'Get block by height',
    ['esplora', 'block-height'],
    ['height'],
    {
      type: 'object',
      properties: {
        height: { type: 'number', description: 'Block height' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['height'],
    }
  );

  createPositionalTool(
    'esplora_block',
    'Get block',
    ['esplora', 'block'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Block hash' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'esplora_block_status',
    'Get block status',
    ['esplora', 'block-status'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Block hash' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'esplora_block_txids',
    'Get block transaction IDs',
    ['esplora', 'block-txids'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Block hash' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'esplora_block_header',
    'Get block header',
    ['esplora', 'block-header'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Block hash' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'esplora_block_raw',
    'Get raw block',
    ['esplora', 'block-raw'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Block hash' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'esplora_address',
    'Get address information',
    ['esplora', 'address'],
    ['params'],
    {
      type: 'object',
      properties: {
        params: { type: 'string', description: 'Address parameters' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['params'],
    }
  );

  createPositionalTool(
    'esplora_address_txs',
    'Get address transactions',
    ['esplora', 'address-txs'],
    ['params'],
    {
      type: 'object',
      properties: {
        params: { type: 'string', description: 'Address parameters' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        exclude_coinbase: { type: 'boolean', description: 'Exclude coinbase transactions' },
        runestone_trace: { type: 'boolean', description: 'Trace runestones' },
      },
      required: ['params'],
    }
  );

  createPositionalTool(
    'esplora_address_utxo',
    'Get address UTXOs',
    ['esplora', 'address-utxo'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'esplora_tx',
    'Get transaction',
    ['esplora', 'tx'],
    ['txid'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'Transaction ID' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid'],
    }
  );

  createPositionalTool(
    'esplora_tx_hex',
    'Get transaction hex',
    ['esplora', 'tx-hex'],
    ['txid'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'Transaction ID' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid'],
    }
  );

  createPositionalTool(
    'esplora_tx_status',
    'Get transaction status',
    ['esplora', 'tx-status'],
    ['txid'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'Transaction ID' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid'],
    }
  );

  createPositionalTool(
    'esplora_broadcast',
    'Broadcast transaction',
    ['esplora', 'broadcast'],
    ['tx_hex'],
    {
      type: 'object',
      properties: {
        tx_hex: { type: 'string', description: 'Transaction hex' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['tx_hex'],
    }
  );

  createSimpleTool(
    'esplora_mempool',
    'Get mempool',
    ['esplora', 'mempool'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'esplora_fee_estimates',
    'Get fee estimates',
    ['esplora', 'fee-estimates'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );
}
