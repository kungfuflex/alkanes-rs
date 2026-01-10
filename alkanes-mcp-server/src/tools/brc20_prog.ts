/**
 * BRC20-Prog tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerBrc20ProgTools(): void {
  // Deploy contract
  createPositionalTool(
    'brc20_prog_deploy_contract',
    'Deploy a BRC20-prog contract from Foundry build JSON',
    ['brc20-prog', 'deploy-contract'],
    ['foundry_json_path'],
    {
      type: 'object',
      properties: {
        foundry_json_path: { type: 'string', description: 'Path to Foundry build JSON file' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'Addresses to source UTXOs from',
        },
        change: { type: 'string', description: 'Change address' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        trace: { type: 'boolean', description: 'Enable transaction tracing' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction preview' },
        no_activation: { type: 'boolean', description: 'Skip activation transaction' },
        use_slipstream: { type: 'boolean', description: 'Use MARA Slipstream service' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        rebar_tier: { type: 'number', description: 'Rebar fee tier (1 or 2)' },
        strategy: { type: 'string', description: 'Anti-frontrunning strategy' },
        resume: { type: 'string', description: 'Resume from existing commit transaction' },
      },
      required: ['foundry_json_path'],
    }
  );

  // Transact
  createSimpleTool(
    'brc20_prog_transact',
    'Call a BRC20-prog contract function',
    ['brc20-prog', 'transact'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Contract address (0x prefixed hex)' },
        signature: { type: 'string', description: 'Function signature (e.g., "transfer(address,uint256)")' },
        calldata: { type: 'string', description: 'Calldata arguments as comma-separated values' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'Addresses to source UTXOs from',
        },
        change: { type: 'string', description: 'Change address' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        trace: { type: 'boolean', description: 'Enable transaction tracing' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction preview' },
        use_slipstream: { type: 'boolean', description: 'Use MARA Slipstream service' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        rebar_tier: { type: 'number', description: 'Rebar fee tier (1 or 2)' },
        strategy: { type: 'string', description: 'Anti-frontrunning strategy' },
        resume: { type: 'string', description: 'Resume from existing commit transaction' },
      },
      required: ['address', 'signature', 'calldata'],
    }
  );

  // Wrap BTC
  createPositionalTool(
    'brc20_prog_wrap_btc',
    'Wrap BTC to frBTC (simple wrap without execution)',
    ['brc20-prog', 'wrap-btc'],
    ['amount'],
    {
      type: 'object',
      properties: {
        amount: { type: 'number', description: 'Amount of BTC to wrap (in satoshis)' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'Addresses to source UTXOs from',
        },
        change: { type: 'string', description: 'Change address' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        trace: { type: 'boolean', description: 'Enable transaction tracing' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction preview' },
        use_slipstream: { type: 'boolean', description: 'Use MARA Slipstream service' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        rebar_tier: { type: 'number', description: 'Rebar fee tier (1 or 2)' },
        resume: { type: 'string', description: 'Resume from existing commit transaction' },
      },
      required: ['amount'],
    }
  );

  // Unwrap BTC
  createSimpleTool(
    'brc20_prog_unwrap_btc',
    'Unwrap frBTC to BTC (burns frBTC and queues BTC payment)',
    ['brc20-prog', 'unwrap-btc'],
    {
      type: 'object',
      properties: {
        amount: { type: 'number', description: 'Amount of frBTC to unwrap (in satoshis)' },
        vout: { type: 'number', description: 'Vout index for the inscription output', default: 0 },
        to: { type: 'string', description: 'Recipient address for the unwrapped BTC' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'Addresses to source UTXOs from',
        },
        change: { type: 'string', description: 'Change address' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        trace: { type: 'boolean', description: 'Enable transaction tracing' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction preview' },
        use_slipstream: { type: 'boolean', description: 'Use MARA Slipstream service' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        rebar_tier: { type: 'number', description: 'Rebar fee tier (1 or 2)' },
        resume: { type: 'string', description: 'Resume from existing commit transaction' },
      },
      required: ['amount', 'to'],
    }
  );

  // Wrap and execute
  createSimpleTool(
    'brc20_prog_wrap_and_execute',
    'Wrap BTC and deploy+execute a script (wrapAndExecute)',
    ['brc20-prog', 'wrap-and-execute'],
    {
      type: 'object',
      properties: {
        amount: { type: 'number', description: 'Amount of BTC to wrap (in satoshis)' },
        script: { type: 'string', description: 'Script bytecode to deploy and execute (hex-encoded)' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'Addresses to source UTXOs from',
        },
        change: { type: 'string', description: 'Change address' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        trace: { type: 'boolean', description: 'Enable transaction tracing' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction preview' },
        use_slipstream: { type: 'boolean', description: 'Use MARA Slipstream service' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        rebar_tier: { type: 'number', description: 'Rebar fee tier (1 or 2)' },
        resume: { type: 'string', description: 'Resume from existing commit transaction' },
      },
      required: ['amount', 'script'],
    }
  );

  // Wrap and execute2
  createSimpleTool(
    'brc20_prog_wrap_and_execute2',
    'Wrap BTC and call an existing contract (wrapAndExecute2)',
    ['brc20-prog', 'wrap-and-execute2'],
    {
      type: 'object',
      properties: {
        amount: { type: 'number', description: 'Amount of BTC to wrap (in satoshis)' },
        target: { type: 'string', description: 'Target contract address for wrapAndExecute2' },
        signature: { type: 'string', description: 'Function signature to call on target' },
        calldata: { type: 'string', description: 'Calldata arguments as comma-separated values', default: '' },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'Addresses to source UTXOs from',
        },
        change: { type: 'string', description: 'Change address' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        trace: { type: 'boolean', description: 'Enable transaction tracing' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction preview' },
        use_slipstream: { type: 'boolean', description: 'Use MARA Slipstream service' },
        use_rebar: { type: 'boolean', description: 'Use Rebar Shield' },
        rebar_tier: { type: 'number', description: 'Rebar fee tier (1 or 2)' },
        resume: { type: 'string', description: 'Resume from existing commit transaction' },
      },
      required: ['amount', 'target', 'signature'],
    }
  );

  // Query tools (no wallet needed)
  createPositionalTool(
    'brc20_prog_signer_address',
    'Get FrBTC signer address for the current network',
    ['brc20-prog', 'signer-address'],
    [],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createPositionalTool(
    'brc20_prog_get_contract_deploys',
    'Get contract deployments made by an address',
    ['brc20-prog', 'get-contract-deploys'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address or address identifier' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_code',
    'Get contract bytecode (eth_getCode)',
    ['brc20-prog', 'get-code'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Contract address (0x prefixed hex)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createSimpleTool(
    'brc20_prog_call',
    'Call a contract function (eth_call)',
    ['brc20-prog', 'call'],
    {
      type: 'object',
      properties: {
        to: { type: 'string', description: 'Contract address (0x prefixed hex)' },
        data: { type: 'string', description: 'Calldata (0x prefixed hex)' },
        from: { type: 'string', description: 'From address (optional, 0x prefixed hex)' },
        block: { type: 'string', description: 'Block number or "latest" (optional)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['to', 'data'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_balance',
    'Get frBTC balance (eth_getBalance)',
    ['brc20-prog', 'get-balance'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address (0x prefixed hex)' },
        block: { type: 'string', description: 'Block number or "latest"', default: 'latest' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createSimpleTool(
    'brc20_prog_estimate_gas',
    'Estimate gas for a transaction (eth_estimateGas)',
    ['brc20-prog', 'estimate-gas'],
    {
      type: 'object',
      properties: {
        to: { type: 'string', description: 'Contract address (0x prefixed hex)' },
        data: { type: 'string', description: 'Calldata (0x prefixed hex)' },
        from: { type: 'string', description: 'From address (optional, 0x prefixed hex)' },
        block: { type: 'string', description: 'Block number or "latest" (optional)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['to', 'data'],
    }
  );

  createSimpleTool(
    'brc20_prog_block_number',
    'Get current block number (eth_blockNumber)',
    ['brc20-prog', 'block-number'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createPositionalTool(
    'brc20_prog_get_block_by_number',
    'Get block by number (eth_getBlockByNumber)',
    ['brc20-prog', 'get-block-by-number'],
    ['block'],
    {
      type: 'object',
      properties: {
        block: { type: 'string', description: 'Block number (hex or decimal) or "latest"' },
        full: { type: 'boolean', description: 'Include full transaction details' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['block'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_block_by_hash',
    'Get block by hash (eth_getBlockByHash)',
    ['brc20-prog', 'get-block-by-hash'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Block hash (0x prefixed hex)' },
        full: { type: 'boolean', description: 'Include full transaction details' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_transaction_count',
    'Get transaction count/nonce (eth_getTransactionCount)',
    ['brc20-prog', 'get-transaction-count'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address (0x prefixed hex)' },
        block: { type: 'string', description: 'Block number or "latest"', default: 'latest' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_transaction',
    'Get transaction by hash (eth_getTransactionByHash)',
    ['brc20-prog', 'get-transaction'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Transaction hash (0x prefixed hex)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_transaction_receipt',
    'Get transaction receipt (eth_getTransactionReceipt)',
    ['brc20-prog', 'get-transaction-receipt'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Transaction hash (0x prefixed hex)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createSimpleTool(
    'brc20_prog_get_storage_at',
    'Get storage at a specific location (eth_getStorageAt)',
    ['brc20-prog', 'get-storage-at'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Contract address (0x prefixed hex)' },
        position: { type: 'string', description: 'Storage position (0x prefixed hex)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address', 'position'],
    }
  );

  createSimpleTool(
    'brc20_prog_get_logs',
    'Get logs (eth_getLogs)',
    ['brc20-prog', 'get-logs'],
    {
      type: 'object',
      properties: {
        from_block: { type: 'string', description: 'From block (hex or decimal)' },
        to_block: { type: 'string', description: 'To block (hex or decimal)' },
        address: {
          type: 'array',
          items: { type: 'string' },
          description: 'Filter by address (can be specified multiple times)',
        },
        topics: { type: 'string', description: 'Filter by topics (JSON array format)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'brc20_prog_chain_id',
    'Get chain ID (eth_chainId)',
    ['brc20-prog', 'chain-id'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'brc20_prog_gas_price',
    'Get gas price (eth_gasPrice)',
    ['brc20-prog', 'gas-price'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'brc20_prog_version',
    'Get BRC20-Prog version (brc20_version)',
    ['brc20-prog', 'version'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createPositionalTool(
    'brc20_prog_get_receipt_by_inscription',
    'Get transaction receipt by inscription ID',
    ['brc20-prog', 'get-receipt-by-inscription'],
    ['inscription_id'],
    {
      type: 'object',
      properties: {
        inscription_id: { type: 'string', description: 'Inscription ID (e.g., "txid:i0")' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['inscription_id'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_inscription_by_tx',
    'Get inscription ID by transaction hash',
    ['brc20-prog', 'get-inscription-by-tx'],
    ['tx_hash'],
    {
      type: 'object',
      properties: {
        tx_hash: { type: 'string', description: 'Transaction hash (0x prefixed hex)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['tx_hash'],
    }
  );

  createPositionalTool(
    'brc20_prog_get_inscription_by_contract',
    'Get inscription ID by contract address',
    ['brc20-prog', 'get-inscription-by-contract'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Contract address (0x prefixed hex)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createSimpleTool(
    'brc20_prog_brc20_balance',
    'Get BRC20 balance (brc20_balance)',
    ['brc20-prog', 'brc20-balance'],
    {
      type: 'object',
      properties: {
        pkscript: { type: 'string', description: 'Bitcoin pkscript (hex)' },
        ticker: { type: 'string', description: 'BRC20 ticker symbol' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['pkscript', 'ticker'],
    }
  );

  createPositionalTool(
    'brc20_prog_trace_transaction',
    'Get transaction trace (debug_traceTransaction)',
    ['brc20-prog', 'trace-transaction'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'Transaction hash (0x prefixed hex)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createSimpleTool(
    'brc20_prog_txpool_content',
    'Get txpool content (txpool_content)',
    ['brc20-prog', 'txpool-content'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'brc20_prog_client_version',
    'Get client version (web3_clientVersion)',
    ['brc20-prog', 'client-version'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createSimpleTool(
    'brc20_prog_unwrap',
    'Get pending unwraps from BRC20-Prog FrBTC contract',
    ['brc20-prog', 'unwrap'],
    {
      type: 'object',
      properties: {
        block_tag: { type: 'string', description: 'Block tag to query' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        experimental_asm: { type: 'boolean', description: 'Use experimental EVM bytecode assembler' },
        experimental_sol: { type: 'boolean', description: 'Use experimental Solidity-compiled bytecode' },
      },
    }
  );
}
