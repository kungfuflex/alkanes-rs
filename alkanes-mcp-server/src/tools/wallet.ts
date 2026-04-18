/**
 * Wallet tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerWalletTools(): void {
  // Wallet create
  createSimpleTool(
    'wallet_create',
    'Create a new wallet',
    ['wallet', 'create'],
    {
      type: 'object',
      properties: {
        mnemonic: { type: 'string', description: 'Optional mnemonic phrase to restore from' },
        output: { type: 'string', description: 'Output file path for the wallet' },
      },
    }
  );

  // Wallet addresses
  createSimpleTool(
    'wallet_addresses',
    'Get addresses from the wallet',
    ['wallet', 'addresses'],
    {
      type: 'object',
      properties: {
        ranges: {
          type: 'array',
          items: { type: 'string' },
          description: 'Address range specifications (e.g., "p2tr:0-1000")',
        },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        all_networks: { type: 'boolean', description: 'Show addresses for all networks' },
      },
    }
  );

  // Wallet UTXOs
  createSimpleTool(
    'wallet_utxos',
    'List UTXOs in the wallet',
    ['wallet', 'utxos'],
    {
      type: 'object',
      properties: {
        addresses: { type: 'string', description: 'Address specifications' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        include_frozen: { type: 'boolean', description: 'Include frozen UTXOs' },
      },
    }
  );

  // Wallet freeze
  createPositionalTool(
    'wallet_freeze',
    'Freeze a UTXO',
    ['wallet', 'freeze'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'The outpoint of the UTXO to freeze' },
      },
      required: ['outpoint'],
    }
  );

  // Wallet unfreeze
  createPositionalTool(
    'wallet_unfreeze',
    'Unfreeze a UTXO',
    ['wallet', 'unfreeze'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'The outpoint of the UTXO to unfreeze' },
      },
      required: ['outpoint'],
    }
  );

  // Wallet sign
  createPositionalTool(
    'wallet_sign',
    'Sign a PSBT',
    ['wallet', 'sign'],
    ['psbt'],
    {
      type: 'object',
      properties: {
        psbt: { type: 'string', description: 'The PSBT to sign, as a base64 string' },
      },
      required: ['psbt'],
    }
  );

  // Wallet send
  createPositionalTool(
    'wallet_send',
    'Send a transaction',
    ['wallet', 'send'],
    ['address', 'amount'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'The address to send to' },
        amount: { type: 'string', description: 'The amount to send in BTC (e.g., 0.0001)' },
        fee_rate: { type: 'number', description: 'The fee rate in sat/vB' },
        send_all: { type: 'boolean', description: 'Send all funds' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'The addresses to send from',
        },
        lock_alkanes: { type: 'boolean', description: 'Skip UTXOs that have alkanes on them' },
        change_address: { type: 'string', description: 'The change address' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        rebar_tier: { type: 'number', description: 'Rebar fee tier (1 or 2)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction' },
      },
      required: ['address', 'amount'],
    }
  );

  // Wallet balance
  createSimpleTool(
    'wallet_balance',
    'Get the balance of the wallet',
    ['wallet', 'balance'],
    {
      type: 'object',
      properties: {
        addresses: {
          type: 'array',
          items: { type: 'string' },
          description: 'The addresses to get the balance for',
        },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  // Wallet history
  createSimpleTool(
    'wallet_history',
    'Get the history of the wallet',
    ['wallet', 'history'],
    {
      type: 'object',
      properties: {
        count: { type: 'number', description: 'The number of transactions to get', default: 10 },
        address: { type: 'string', description: 'The address to get the history for' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  // Wallet create-tx
  createPositionalTool(
    'wallet_create_tx',
    'Create a transaction',
    ['wallet', 'create-tx'],
    ['address', 'amount'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'The address to send to' },
        amount: { type: 'number', description: 'The amount to send in satoshis' },
        fee_rate: { type: 'number', description: 'The fee rate in sat/vB' },
        send_all: { type: 'boolean', description: 'Send all funds' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'The addresses to send from',
        },
        change_address: { type: 'string', description: 'The change address' },
      },
      required: ['address', 'amount'],
    }
  );

  // Wallet sign-tx
  createSimpleTool(
    'wallet_sign_tx',
    'Sign a transaction',
    ['wallet', 'sign-tx'],
    {
      type: 'object',
      properties: {
        tx_hex: { type: 'string', description: 'The transaction hex to sign' },
        from_file: { type: 'string', description: 'Read transaction hex from file' },
        truncate_excess_vsize: { type: 'string', description: 'Truncate excess inputs to fit size limit' },
        split_max_vsize: { type: 'string', description: 'Split transaction into multiple transactions' },
      },
    },
    (args) => {
      const flags: string[] = [];
      if (args.from_file) {
        flags.push('--from-file', String(args.from_file));
      } else if (args.tx_hex) {
        flags.push(String(args.tx_hex));
      }
      if (args.truncate_excess_vsize) {
        flags.push('--truncate-excess-vsize', String(args.truncate_excess_vsize));
      }
      if (args.split_max_vsize) {
        flags.push('--split-max-vsize', String(args.split_max_vsize));
      }
      return flags;
    }
  );

  // Wallet decode-tx
  createSimpleTool(
    'wallet_decode_tx',
    'Decode a transaction to view its details',
    ['wallet', 'decode-tx'],
    {
      type: 'object',
      properties: {
        tx_hex: { type: 'string', description: 'Transaction hex to decode' },
        file: { type: 'string', description: 'Read transaction hex from file' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    },
    (args) => {
      const flags: string[] = [];
      if (args.file) {
        flags.push('--file', String(args.file));
      } else if (args.tx_hex) {
        flags.push(String(args.tx_hex));
      }
      return flags;
    }
  );

  // Wallet broadcast-tx
  createPositionalTool(
    'wallet_broadcast_tx',
    'Broadcast a transaction',
    ['wallet', 'broadcast-tx'],
    ['tx_hex'],
    {
      type: 'object',
      properties: {
        tx_hex: { type: 'string', description: 'The transaction hex to broadcast' },
      },
      required: ['tx_hex'],
    }
  );

  // Wallet estimate-fee
  createSimpleTool(
    'wallet_estimate_fee',
    'Estimate the fee for a transaction',
    ['wallet', 'estimate-fee'],
    {
      type: 'object',
      properties: {
        target: { type: 'number', description: 'The target number of blocks for confirmation', default: 6 },
      },
    }
  );

  // Wallet fee-rates
  createSimpleTool(
    'wallet_fee_rates',
    'Get the current fee rates',
    ['wallet', 'fee-rates'],
    {
      type: 'object',
      properties: {},
    }
  );

  // Wallet sync
  createSimpleTool(
    'wallet_sync',
    'Sync the wallet with the blockchain',
    ['wallet', 'sync'],
    {
      type: 'object',
      properties: {},
    }
  );

  // Wallet backup
  createSimpleTool(
    'wallet_backup',
    'Backup the wallet',
    ['wallet', 'backup'],
    {
      type: 'object',
      properties: {},
    }
  );

  // Wallet mnemonic
  createSimpleTool(
    'wallet_mnemonic',
    'Get the mnemonic for the wallet',
    ['wallet', 'mnemonic'],
    {
      type: 'object',
      properties: {},
    }
  );
}
