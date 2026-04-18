/**
 * Prompts API - Provides pre-configured prompts with best practices and gotchas
 */

export interface Prompt {
  name: string;
  description: string;
  arguments?: Array<{
    name: string;
    description: string;
    required: boolean;
  }>;
}

export interface PromptMessage {
  role: 'user' | 'assistant';
  content: {
    type: 'text';
    text: string;
  };
}

export interface GetPromptResult {
  messages: PromptMessage[];
}

/**
 * Get list of all available prompts
 */
export function listPrompts(): Prompt[] {
  return [
    {
      name: 'debug-alkanes-contract',
      description: 'Debug a failing Alkanes contract with common gotchas in mind',
      arguments: [
        {
          name: 'error',
          description: 'The error message or symptom',
          required: true,
        },
      ],
    },
    {
      name: 'build-token-contract',
      description: 'Build an Alkanes token contract following best practices',
      arguments: [
        {
          name: 'token_type',
          description: 'Token type: owned, auth, or custom',
          required: true,
        },
        {
          name: 'features',
          description: 'Additional features needed (optional)',
          required: false,
        },
      ],
    },
    {
      name: 'deploy-contract',
      description: 'Deploy an Alkanes contract with proper addressing and initialization',
      arguments: [
        {
          name: 'deployment_type',
          description: 'Deployment type: sequential, custom-address, or factory',
          required: true,
        },
      ],
    },
    {
      name: 'optimize-swap-path',
      description: 'Find optimal swap path between tokens using AMM pools',
      arguments: [
        {
          name: 'from_token',
          description: 'Source token AlkaneId (e.g., "2:0")',
          required: true,
        },
        {
          name: 'to_token',
          description: 'Destination token AlkaneId (e.g., "2:1")',
          required: true,
        },
        {
          name: 'amount',
          description: 'Amount to swap',
          required: true,
        },
      ],
    },
    {
      name: 'write-txscript',
      description: 'Write an AssemblyScript tx-script for batch operations',
      arguments: [
        {
          name: 'operation',
          description: 'What operation to batch (e.g., "fetch pool details")',
          required: true,
        },
      ],
    },
  ];
}

/**
 * Get a specific prompt by name
 */
export function getPrompt(name: string, args?: Record<string, string>): GetPromptResult {
  switch (name) {
    case 'debug-alkanes-contract':
      return getDebugContractPrompt(args?.error || 'unknown error');

    case 'build-token-contract':
      return getBuildTokenPrompt(args?.token_type || 'owned', args?.features);

    case 'deploy-contract':
      return getDeployContractPrompt(args?.deployment_type || 'sequential');

    case 'optimize-swap-path':
      return getOptimizeSwapPrompt(
        args?.from_token || '',
        args?.to_token || '',
        args?.amount || ''
      );

    case 'write-txscript':
      return getWriteTxScriptPrompt(args?.operation || '');

    default:
      throw new Error(`Prompt not found: ${name}`);
  }
}

/**
 * Debug contract prompt
 */
function getDebugContractPrompt(error: string): GetPromptResult {
  return {
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: `Debug this Alkanes contract error: ${error}

Before investigating, check these CRITICAL gotchas:

1. **Double Indexing** - Did you call index_block multiple times for the same block? This causes:
   - Unexpected token IDs
   - Swapped balances
   - Inconsistent state

2. **Auth Token Initialization** - For owned tokens, did you initialize with auth token units?
   \`\`\`rust
   inputs: vec![0, 100, 1000] // opcode, auth_units, token_units
   \`\`\`

3. **AlkaneId Addressing** - Is your AlkaneId correct?
   - [1, 0] → deploys to [2, next_sequence]
   - [3, n] → deploys to [4, n]
   - block parameter is NOT block height!

4. **Cellpack Format** - Is the first input the opcode?
   \`\`\`rust
   inputs: [opcode, param1, param2, ...]
   \`\`\`

5. **Table Population** - Are the required tables populated?
   - OUTPOINTS_FOR_ADDRESS - populated for all txs
   - RUNE_ID_TO_OUTPOINTS - only for runestone txs

6. **Protobuf Encoding** - For RPC calls, is Uint128 encoded as length-delimited?

7. **Fuel Metering** - Is there adequate fuel for the operation?

Check the gotchas documentation (alkanes://docs/gotchas) for detailed solutions.

Now let's investigate your specific error...`,
        },
      },
    ],
  };
}

/**
 * Build token contract prompt
 */
function getBuildTokenPrompt(tokenType: string, features?: string): GetPromptResult {
  const featuresText = features ? `\nAdditional features: ${features}` : '';

  return {
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: `Build an Alkanes ${tokenType} token contract following best practices.${featuresText}

Key requirements for ${tokenType} tokens:

${tokenType === 'owned' ? `
**Owned Token Checklist**:
1. Implement AlkaneResponder trait
2. Implement AuthenticatedResponder trait
3. Opcode 0 (initialize):
   - Deploy auth token with deploy_auth_token()
   - Store auth token ID at /auth
   - Return both auth token and owned token
4. Opcode 77 (mint):
   - Call only_owner() first
   - Mint new tokens
   - Return minted tokens
5. CRITICAL: Initialize with both auth_token_units and token_units

Example initialization:
\`\`\`rust
inputs: vec![
    0,    // opcode: initialize
    100,  // auth_token_units (REQUIRED)
    1000, // token_units
]
\`\`\`
` : ''}

${tokenType === 'auth' ? `
**Auth Token Checklist**:
1. Opcode 0 (initialize):
   - Mint initial supply
   - Return auth tokens to caller
2. Opcode 1 (authenticate):
   - Verify exactly 1 incoming alkane
   - Verify it's the auth token
   - Verify at least 1 unit
   - Return auth token to maintain ownership
` : ''}

${tokenType === 'custom' ? `
**Custom Token Checklist**:
1. Implement AlkaneResponder trait
2. Define your opcodes (0 should be initialize)
3. Handle state management with load/store
4. Return proper CallResponse with alkanes
5. Consider if you need authentication
` : ''}

**Common Pitfalls**:
- Don't forget to return alkanes in the response
- Use shift_or_err() to safely extract inputs
- Always handle the default case in opcode match
- Test with simulation before deployment

Read the contract development guide (alkanes://docs/contract-development) for full details.`,
        },
      },
    ],
  };
}

/**
 * Deploy contract prompt
 */
function getDeployContractPrompt(deploymentType: string): GetPromptResult {
  return {
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: `Deploy an Alkanes contract using ${deploymentType} deployment.

**Deployment Types**:

${deploymentType === 'sequential' ? `
**Sequential Deployment ([1, 0] → [2, n])**:
1. Use target [1, 0] in your cellpack
2. Contract will be deployed to [2, next_sequence]
3. Sequence starts at 1 (0 is reserved for genesis ALKANE)

\`\`\`bash
alkanes-cli alkanes execute \\
  --target 1:0 \\
  --inputs 0,100,1000 \\
  --envelope my_contract.wasm \\
  --fee 50000
\`\`\`
` : ''}

${deploymentType === 'custom-address' ? `
**Custom Address Deployment ([3, n] → [4, n])**:
1. Choose a unique u128 value for n
2. Use target [3, n] in your cellpack
3. Contract will be deployed to [4, n]

\`\`\`bash
alkanes-cli alkanes execute \\
  --target 3:0xffddffee \\
  --inputs 0,100,1000 \\
  --envelope my_contract.wasm \\
  --fee 50000
\`\`\`

This allows deploying to predictable addresses.
` : ''}

${deploymentType === 'factory' ? `
**Factory Deployment (Clone Pattern)**:

Option 1: Clone from sequential ([5, n]):
\`\`\`bash
# Deploy template to [2, n]
alkanes-cli alkanes execute --target 1:0 --envelope template.wasm --fee 50000

# Clone it (assuming it's at [2, 5])
alkanes-cli alkanes execute --target 5:5 --inputs 0,500 --fee 30000
\`\`\`

Option 2: Clone from custom address ([6, n]):
\`\`\`bash
# Deploy template to [4, 0xffddffee]
alkanes-cli alkanes execute --target 3:0xffddffee --envelope template.wasm --fee 50000

# Clone it
alkanes-cli alkanes execute --target 6:0xffddffee --inputs 0,500 --fee 30000
\`\`\`

Factory pattern is ideal for contracts deployed many times (tokens, pools, etc).
` : ''}

**Deployment Checklist**:
1. ✅ Simulate first: \`alkanes-cli alkanes simulate ...\`
2. ✅ Verify WASM compiles correctly
3. ✅ Check initialization inputs are correct
4. ✅ Set adequate fee for contract size
5. ✅ Verify deployment address after execution

**CRITICAL**: The block parameter is NOT blockchain block height - it's the deployment selector!

Read the contract development guide (alkanes://docs/contract-development) for more details.`,
        },
      },
    ],
  };
}

/**
 * Optimize swap path prompt
 */
function getOptimizeSwapPrompt(fromToken: string, toToken: string, amount: string): GetPromptResult {
  return {
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: `Find the optimal swap path from ${fromToken} to ${toToken} for amount ${amount}.

**Approach**:

1. **Query available pools**:
\`\`\`bash
alkanes-cli alkanes simulate <factory_id> --inputs 3
\`\`\`

2. **Build liquidity graph**:
   - For each pool, get token pair
   - Build adjacency list of token connections

3. **Search for paths** (1-4 hops):
   - Use BFS/DFS to find all paths
   - Consider liquidity thresholds
   - Calculate output for each path

4. **Select optimal path**:
   - Compare output amounts
   - Consider price impact
   - Factor in gas costs

**Using tx-scripts for efficiency**:

Instead of N+1 RPC calls, use a WASM tx-script:

\`\`\`bash
# Compile the path optimizer
cd crates/alkanes-cli-common/src/alkanes/asc/optimize-path
npm run build

# Run with envelope
alkanes-cli alkanes simulate 1:0 \\
  --inputs <factory_block>,<factory_tx>,<from_block>,<from_tx>,<to_block>,<to_tx>,<amount>,<max_hops> \\
  --envelope build/release.wasm
\`\`\`

This batches all pool queries and path calculation into a single RPC call.

**Execute the swap**:
\`\`\`bash
alkanes-cli alkanes swap \\
  --path ${fromToken},<intermediate>,...,${toToken} \\
  --input ${amount} \\
  --min-output <calculated_min> \\
  --fee 20000
\`\`\`

Read the AssemblyScript guide (alkanes://docs/assemblyscript-txscripts) for tx-script development.`,
        },
      },
    ],
  };
}

/**
 * Write tx-script prompt
 */
function getWriteTxScriptPrompt(operation: string): GetPromptResult {
  return {
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: `Write an AssemblyScript tx-script to: ${operation}

**Tx-Script Architecture**:

Tx-scripts are WASM programs that run in the Alkanes simulation environment to batch multiple operations into a single RPC call.

**Setup**:
\`\`\`bash
cd crates/alkanes-cli-common/src/alkanes/asc
mkdir my-txscript
cd my-txscript
npm init -y
npm install --save-dev assemblyscript
\`\`\`

**Basic Template**:
\`\`\`typescript
import {
  ExecutionContext,
  staticcallWithInputs,
  AlkaneId,
  ExtendedCallResponse
} from '../alkanes-asm-common/assembly/index';

export function __execute(): i32 {
  // 1. Load context and parse inputs
  const ctx = ExecutionContext.load();
  const input0 = ctx.getInput(0);
  const input1 = ctx.getInput(1);

  // 2. Make staticcalls to gather data
  const target = new AlkaneId(input0, input1);
  const response1 = staticcallWithInputs(target, [3]); // opcode 3

  // 3. Process response data
  // ... parse and compute ...

  // 4. Build and return response
  const response = new ExtendedCallResponse();
  response.writeU128(result);
  return response.finalize();
}
\`\`\`

**Memory Layout Convention**:
\`\`\`
0-1023:       Context
2048-2095:    Cellpack 1
3200-3215:    Empty AlkaneTransferParcel
3300-3303:    Empty StorageMap
4096-4143:    Cellpack 2
8192-12287:   Return buffer 1
12288-16383:  Return buffer 2
16384+:       Final response
\`\`\`

**Build & Use**:
\`\`\`bash
# Build
npm run asbuild:release

# Use with CLI
alkanes-cli alkanes simulate 1:0 \\
  --inputs <params> \\
  --envelope build/release.wasm
\`\`\`

**Performance Tips**:
1. Minimize allocations - reuse buffers
2. Use inline functions
3. Avoid String operations
4. Pre-calculate sizes
5. Batch staticcalls

Read the full guide (alkanes://docs/assemblyscript-txscripts) for detailed patterns and examples.`,
        },
      },
    ],
  };
}
