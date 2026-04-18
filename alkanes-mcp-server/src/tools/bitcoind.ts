/**
 * Bitcoind tools - Bitcoin Core RPC commands
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerBitcoindTools(): void {
  // Simple tools with no arguments
  createSimpleTool(
    'bitcoind_getblockcount',
    'Get current block count',
    ['bitcoind', 'getblockcount'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'bitcoind_getblockchaininfo',
    'Get blockchain information',
    ['bitcoind', 'getblockchaininfo'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'bitcoind_getnetworkinfo',
    'Get network information',
    ['bitcoind', 'getnetworkinfo'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'bitcoind_generatefuture',
    'Generate a single block with a future-claiming protostone in the coinbase (regtest only)',
    ['bitcoind', 'generatefuture'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'bitcoind_getchaintips',
    'Get chain tips',
    ['bitcoind', 'getchaintips'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'bitcoind_getmempoolinfo',
    'Get mempool information',
    ['bitcoind', 'getmempoolinfo'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'bitcoind_getrawmempool',
    'Get raw mempool',
    ['bitcoind', 'getrawmempool'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  // Tools with positional arguments
  createPositionalTool(
    'bitcoind_generatetoaddress',
    'Generate blocks to an address (regtest only)',
    ['bitcoind', 'generatetoaddress'],
    ['nblocks', 'address'],
    {
      type: 'object',
      properties: {
        nblocks: { type: 'number', description: 'Number of blocks to generate' },
        address: { type: 'string', description: 'Address to generate to' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['nblocks', 'address'],
    }
  );

  createPositionalTool(
    'bitcoind_getrawtransaction',
    'Get raw transaction',
    ['bitcoind', 'getrawtransaction'],
    ['txid'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'Transaction ID' },
        block_hash: { type: 'string', description: 'Block hash (optional)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid'],
    }
  );

  createPositionalTool(
    'bitcoind_getblock',
    'Get block',
    ['bitcoind', 'getblock'],
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
    'bitcoind_getblockhash',
    'Get block hash for a given height',
    ['bitcoind', 'getblockhash'],
    ['height'],
    {
      type: 'object',
      properties: {
        height: { type: 'number', description: 'Block height' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['height'],
    },
    (args) => {
      const flags: string[] = [];
      if (args.raw) flags.push('--raw');
      return flags;
    }
  );

  createPositionalTool(
    'bitcoind_getblockheader',
    'Get block header',
    ['bitcoind', 'getblockheader'],
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
    'bitcoind_getblockstats',
    'Get block statistics',
    ['bitcoind', 'getblockstats'],
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
    'bitcoind_decoderawtransaction',
    'Decode a raw transaction',
    ['bitcoind', 'decoderawtransaction'],
    ['hex'],
    {
      type: 'object',
      properties: {
        hex: { type: 'string', description: 'Raw transaction hex' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hex'],
    }
  );

  createPositionalTool(
    'bitcoind_decodepsbt',
    'Decode a PSBT',
    ['bitcoind', 'decodepsbt'],
    ['psbt'],
    {
      type: 'object',
      properties: {
        psbt: { type: 'string', description: 'PSBT as base64 string' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['psbt'],
    }
  );

  createPositionalTool(
    'bitcoind_gettxout',
    'Get transaction output',
    ['bitcoind', 'gettxout'],
    ['txid', 'vout'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'Transaction ID' },
        vout: { type: 'number', description: 'Output index' },
        include_mempool: { type: 'boolean', description: 'Include mempool' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid', 'vout'],
    }
  );

  // Special case for sendrawtransaction
  createSimpleTool(
    'bitcoind_sendrawtransaction',
    'Send a raw transaction',
    ['bitcoind', 'sendrawtransaction'],
    {
      type: 'object',
      properties: {
        tx_hex: { type: 'string', description: 'Transaction hex' },
        from_file: { type: 'string', description: 'Read transaction hex from file' },
        use_slipstream: { type: 'boolean', description: 'Use MARA Slipstream service' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    },
    (args) => {
      const flags: string[] = [];
      if (args.from_file) {
        flags.push('--from-file', String(args.from_file));
      } else if (args.tx_hex) {
        flags.push(String(args.tx_hex));
      }
      if (args.use_slipstream) flags.push('--use-slipstream');
      if (args.use_rebar) flags.push('--use-rebar');
      return flags;
    }
  );
}
