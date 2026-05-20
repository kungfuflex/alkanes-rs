/**
 * Alkanes tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerAlkanesTools(): void {
  // Alkanes execute
  createSimpleTool(
    'alkanes_execute',
    'Execute an alkanes transaction',
    ['alkanes', 'execute'],
    {
      type: 'object',
      properties: {
        inputs: { type: 'string', description: 'Input requirements (format: "B:amount", "B:amount:vN", "block:tx:amount")' },
        to: {
          type: 'array',
          items: { type: 'string' },
          description: 'Recipient addresses',
        },
        from: {
          type: 'array',
          items: { type: 'string' },
          description: 'Addresses to source UTXOs from',
        },
        change: { type: 'string', description: 'Change address for BTC' },
        alkanes_change: { type: 'string', description: 'Change address for unwanted alkanes' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        envelope: { type: 'string', description: 'Path to the envelope file (for contract deployment)' },
        protostones: {
          type: 'array',
          items: { type: 'string' },
          description: 'Protostone specifications',
        },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        trace: { type: 'boolean', description: 'Enable transaction tracing' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        auto_confirm: { type: 'boolean', description: 'Automatically confirm the transaction preview' },
      },
      required: ['to'],
    }
  );

  // Alkanes inspect
  createPositionalTool(
    'alkanes_inspect',
    'Inspect an alkanes contract',
    ['alkanes', 'inspect'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'The outpoint of the contract' },
        disasm: { type: 'boolean', description: 'Disassemble the contract bytecode' },
        fuzz: { type: 'boolean', description: 'Fuzz the contract with a range of opcodes' },
        fuzz_ranges: { type: 'string', description: 'The range of opcodes to fuzz' },
        meta: { type: 'boolean', description: 'Show contract metadata' },
        codehash: { type: 'boolean', description: 'Show the contract code hash' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['outpoint'],
    }
  );

  // Alkanes trace
  createPositionalTool(
    'alkanes_trace',
    'Trace an alkanes transaction',
    ['alkanes', 'trace'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'The outpoint of the transaction to trace' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['outpoint'],
    }
  );

  // Alkanes simulate
  createPositionalTool(
    'alkanes_simulate',
    'Simulate an alkanes transaction',
    ['alkanes', 'simulate'],
    ['alkane_id'],
    {
      type: 'object',
      properties: {
        alkane_id: { type: 'string', description: 'The alkane ID to simulate (format: block:tx:arg1:arg2:...)' },
        inputs: { type: 'string', description: 'Input alkanes as comma-separated triplets' },
        height: { type: 'number', description: 'Block height for simulation' },
        block: { type: 'string', description: 'Block hex data (0x prefixed)' },
        transaction: { type: 'string', description: 'Transaction hex data (0x prefixed)' },
        envelope: { type: 'string', description: 'Path to binary file (e.g., WASM) to pack into transaction witness' },
        pointer: { type: 'number', description: 'Pointer value', default: 0 },
        txindex: { type: 'number', description: 'Transaction index', default: 1 },
        refund: { type: 'number', description: 'Refund pointer', default: 0 },
        block_tag: { type: 'string', description: 'Block tag to query (e.g., "latest" or a block height)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['alkane_id'],
    }
  );

  // Alkanes tx-script
  createSimpleTool(
    'alkanes_tx_script',
    'Execute a tx-script with WASM bytecode',
    ['alkanes', 'tx-script'],
    {
      type: 'object',
      properties: {
        envelope: { type: 'string', description: 'Path to WASM file' },
        inputs: { type: 'string', description: 'Cellpack inputs as comma-separated u128 values' },
        block_tag: { type: 'string', description: 'Block tag to query' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['envelope'],
    }
  );

  // Alkanes sequence
  createSimpleTool(
    'alkanes_sequence',
    'Get the sequence for an outpoint',
    ['alkanes', 'sequence'],
    {
      type: 'object',
      properties: {
        block_tag: { type: 'string', description: 'Block tag to query' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  // Alkanes spendables
  createPositionalTool(
    'alkanes_spendables',
    'Get spendable outpoints for an address',
    ['alkanes', 'spendables'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'The address to get spendables for' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  // Alkanes traceblock
  createPositionalTool(
    'alkanes_traceblock',
    'Trace a block',
    ['alkanes', 'traceblock'],
    ['height'],
    {
      type: 'object',
      properties: {
        height: { type: 'number', description: 'The height of the block to trace' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['height'],
    }
  );

  // Alkanes getbytecode
  createPositionalTool(
    'alkanes_getbytecode',
    'Get the bytecode for an alkane',
    ['alkanes', 'getbytecode'],
    ['alkane_id'],
    {
      type: 'object',
      properties: {
        alkane_id: { type: 'string', description: 'The alkane ID to get the bytecode for' },
        block_tag: { type: 'string', description: 'Block tag to query' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['alkane_id'],
    }
  );

  // Alkanes getbalance
  createSimpleTool(
    'alkanes_getbalance',
    'Get the balance of an address',
    ['alkanes', 'getbalance'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'The address to get the balance for' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  // Alkanes wrap-btc
  createPositionalTool(
    'alkanes_wrap_btc',
    'Wrap BTC to frBTC and lock in vault',
    ['alkanes', 'wrap-btc'],
    ['amount'],
    {
      type: 'object',
      properties: {
        amount: { type: 'number', description: 'Amount of BTC to wrap (in satoshis)' },
        to: { type: 'string', description: 'Address to receive the frBTC tokens' },
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
      },
      required: ['amount', 'to'],
    }
  );

  // Alkanes unwrap
  createSimpleTool(
    'alkanes_unwrap',
    'Get pending unwraps',
    ['alkanes', 'unwrap'],
    {
      type: 'object',
      properties: {
        block_tag: { type: 'string', description: 'Block tag to query' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  // Alkanes backtest
  createPositionalTool(
    'alkanes_backtest',
    'Backtest a transaction by simulating it in a block',
    ['alkanes', 'backtest'],
    ['txid'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'Transaction ID to backtest' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid'],
    }
  );

  // Alkanes get-all-pools
  createSimpleTool(
    'alkanes_get_all_pools',
    'Get all pools from an AMM factory contract (defaults to 4:65522)',
    ['alkanes', 'get-all-pools'],
    {
      type: 'object',
      properties: {
        factory: { type: 'string', description: 'Factory alkane ID (format: block:tx)', default: '4:65522' },
        pool_details: { type: 'boolean', description: 'Also fetch detailed information for each pool' },
        experimental_asm: { type: 'boolean', description: 'Use experimental AssemblyScript WASM' },
        experimental_batch_asm: { type: 'boolean', description: 'Use experimental WASM-based batch optimization' },
        experimental_asm_parallel: { type: 'boolean', description: 'Use experimental parallel WASM fetching' },
        chunk_size: { type: 'number', description: 'Chunk size for batch fetching', default: 30 },
        max_concurrent: { type: 'number', description: 'Maximum concurrent requests', default: 10 },
        range: { type: 'string', description: 'Specific range to fetch (format: "0-50")' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  // Alkanes all-pools-details
  createPositionalTool(
    'alkanes_all_pools_details',
    'Get all pools with detailed information from an AMM factory contract',
    ['alkanes', 'all-pools-details'],
    ['factory_id'],
    {
      type: 'object',
      properties: {
        factory_id: { type: 'string', description: 'Factory alkane ID (format: block:tx)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['factory_id'],
    }
  );

  // Alkanes pool-details
  createPositionalTool(
    'alkanes_pool_details',
    'Get details for a specific pool',
    ['alkanes', 'pool-details'],
    ['pool_id'],
    {
      type: 'object',
      properties: {
        pool_id: { type: 'string', description: 'Pool alkane ID (format: block:tx)' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['pool_id'],
    }
  );

  // Alkanes reflect-alkane
  createPositionalTool(
    'alkanes_reflect_alkane',
    'Reflect metadata for an alkane by calling standard view opcodes',
    ['alkanes', 'reflect-alkane'],
    ['alkane_id'],
    {
      type: 'object',
      properties: {
        alkane_id: { type: 'string', description: 'Alkane ID to reflect (format: block:tx)' },
        concurrency: { type: 'number', description: 'Maximum concurrent RPC calls', default: 30 },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['alkane_id'],
    }
  );

  // Alkanes reflect-alkane-range
  createPositionalTool(
    'alkanes_reflect_alkane_range',
    'Reflect metadata for a range of alkanes',
    ['alkanes', 'reflect-alkane-range'],
    ['block', 'start_tx', 'end_tx'],
    {
      type: 'object',
      properties: {
        block: { type: 'number', description: 'Starting block number' },
        start_tx: { type: 'number', description: 'Starting transaction index' },
        end_tx: { type: 'number', description: 'Ending transaction index' },
        concurrency: { type: 'number', description: 'Maximum concurrent RPC calls', default: 30 },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['block', 'start_tx', 'end_tx'],
    }
  );

  // Alkanes init-pool
  createSimpleTool(
    'alkanes_init_pool',
    'Initialize a new liquidity pool',
    ['alkanes', 'init-pool'],
    {
      type: 'object',
      properties: {
        pair: { type: 'string', description: 'Token pair in format: BLOCK:TX,BLOCK:TX' },
        liquidity: { type: 'string', description: 'Initial liquidity amounts in format: AMOUNT0:AMOUNT1' },
        to: { type: 'string', description: 'Recipient address identifier (e.g., p2tr:0)' },
        from: { type: 'string', description: 'Sender address identifier (e.g., p2tr:0)' },
        change: { type: 'string', description: 'Change address identifier' },
        minimum: { type: 'number', description: 'Minimum LP tokens to receive' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        trace: { type: 'boolean', description: 'Show trace after transaction confirms' },
        factory: { type: 'string', description: 'Factory ID', default: '4:1' },
        auto_confirm: { type: 'boolean', description: 'Auto-confirm transaction without prompting' },
      },
      required: ['pair', 'liquidity', 'to', 'from'],
    }
  );

  // Alkanes swap
  createSimpleTool(
    'alkanes_swap',
    'Execute a swap on the AMM',
    ['alkanes', 'swap'],
    {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Swap path as comma-separated alkane IDs' },
        input: { type: 'number', description: 'Input token amount' },
        minimum_output: { type: 'number', description: 'Minimum output amount' },
        slippage: { type: 'number', description: 'Slippage percentage', default: 5.0 },
        expires: { type: 'number', description: 'Expiry block height' },
        to: { type: 'string', description: 'Recipient address identifier', default: 'p2tr:0' },
        from: { type: 'string', description: 'Sender address identifier', default: 'p2tr:0' },
        change: { type: 'string', description: 'Change address identifier' },
        fee_rate: { type: 'number', description: 'Fee rate in sat/vB' },
        trace: { type: 'boolean', description: 'Show trace after transaction confirms' },
        mine: { type: 'boolean', description: 'Mine a block after broadcasting (regtest only)' },
        factory: { type: 'string', description: 'Factory ID for path optimization', default: '4:65522' },
        no_optimize: { type: 'boolean', description: 'Skip path optimization' },
        auto_confirm: { type: 'boolean', description: 'Auto-confirm transaction without prompting' },
      },
      required: ['path', 'input'],
    }
  );
}
