/**
 * Resources API - Exposes Alkanes documentation and guides to AI agents
 */

import { promises as fs } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = join(__dirname, '..', '..');

export interface Resource {
  uri: string;
  name: string;
  description: string;
  mimeType: string;
}

export interface ResourceContent {
  uri: string;
  mimeType: string;
  text: string;
}

/**
 * Get list of all available resources
 */
export function listResources(): Resource[] {
  return [
    // Core documentation
    {
      uri: 'alkanes://docs/tech-context',
      name: 'Alkanes Technical Context',
      description: 'Core technologies, dependencies, build system, and constraints',
      mimeType: 'text/markdown',
    },
    {
      uri: 'alkanes://docs/system-patterns',
      name: 'Alkanes System Patterns & Architecture',
      description: 'Key design patterns, table relationships, alkane addressing, and gotchas',
      mimeType: 'text/markdown',
    },
    {
      uri: 'alkanes://docs/product-context',
      name: 'Alkanes Product Context',
      description: 'What Alkanes is, key features, and use cases',
      mimeType: 'text/markdown',
    },

    // Contract development guides
    {
      uri: 'alkanes://docs/contract-development',
      name: 'Contract Development Guide',
      description: 'How to build Alkanes contracts with common pitfalls and best practices',
      mimeType: 'text/markdown',
    },
    {
      uri: 'alkanes://docs/assemblyscript-txscripts',
      name: 'AssemblyScript Tx-Scripts Guide',
      description: 'Building efficient batch query scripts with AssemblyScript',
      mimeType: 'text/markdown',
    },
    {
      uri: 'alkanes://docs/wat-templates',
      name: 'WebAssembly WAT Templates',
      description: 'On-chain WASM script development and optimization',
      mimeType: 'text/markdown',
    },

    // Common gotchas
    {
      uri: 'alkanes://docs/gotchas',
      name: 'Common Gotchas & Debugging',
      description: 'Frequent mistakes, debugging tips, and how to avoid them',
      mimeType: 'text/markdown',
    },

    // CLI and API references
    {
      uri: 'alkanes://docs/cli-overview',
      name: 'CLI Commands Overview',
      description: 'Overview of available alkanes-cli commands and usage',
      mimeType: 'text/markdown',
    },
  ];
}

/**
 * Read a specific resource by URI
 */
export async function readResource(uri: string): Promise<ResourceContent> {
  switch (uri) {
    case 'alkanes://docs/tech-context':
      return {
        uri,
        mimeType: 'text/markdown',
        text: await fs.readFile(join(repoRoot, 'memory-bank/techContext.md'), 'utf-8'),
      };

    case 'alkanes://docs/system-patterns':
      return {
        uri,
        mimeType: 'text/markdown',
        text: await fs.readFile(join(repoRoot, 'memory-bank/systemPatterns.md'), 'utf-8'),
      };

    case 'alkanes://docs/product-context':
      return {
        uri,
        mimeType: 'text/markdown',
        text: await fs.readFile(join(repoRoot, 'memory-bank/productContext.md'), 'utf-8'),
      };

    case 'alkanes://docs/contract-development':
      return {
        uri,
        mimeType: 'text/markdown',
        text: await buildContractDevelopmentGuide(),
      };

    case 'alkanes://docs/assemblyscript-txscripts':
      return {
        uri,
        mimeType: 'text/markdown',
        text: await fs.readFile(
          join(repoRoot, 'crates/alkanes-cli-common/src/alkanes/asc/README.md'),
          'utf-8'
        ),
      };

    case 'alkanes://docs/wat-templates':
      return {
        uri,
        mimeType: 'text/markdown',
        text: await fs.readFile(
          join(repoRoot, 'crates/alkanes-cli-common/src/alkanes/wat/README.md'),
          'utf-8'
        ),
      };

    case 'alkanes://docs/gotchas':
      return {
        uri,
        mimeType: 'text/markdown',
        text: buildGotchasDoc(),
      };

    case 'alkanes://docs/cli-overview':
      return {
        uri,
        mimeType: 'text/markdown',
        text: buildCliOverview(),
      };

    default:
      throw new Error(`Resource not found: ${uri}`);
  }
}

/**
 * Build contract development guide from various sources
 */
async function buildContractDevelopmentGuide(): Promise<string> {
  return `# Alkanes Contract Development Guide

This guide covers how to build smart contracts for the Alkanes protocol.

## Overview

Alkanes are executable programs in the ALKANES metaprotocol that also function as transferable assets:

- **Dual Nature**: Alkanes are both executable programs and transferable assets
- **Asset Transfer**: They conform to a standard of behavior for asset transfer consistent with runes
- **Balance Sheet**: They can hold a balance sheet of alkanes the way that a UTXO can
- **Storage and Execution**: They have the ability to read/write to storage slots they own and execute against other alkanes

## AlkaneId Structure and Addressing

**CRITICAL**: The \`block\` parameter in AlkaneId is NOT the same as the block height of the chain. It's a sequence number or specific u128 value used for addressing.

### AlkaneId Format
- Alkanes are addressed by their AlkaneId: \`[block, tx]\` where both are u128 values
- Their addresses are always \`[2, n]\` or \`[4, n]\`
- \`[2, 0]\` is the genesis ALKANE with incentives for block optimization
- The sequence number in \`[2, n]\` increases by 1 for each new alkane instantiated
- Since \`[2, 0]\` is already taken by the genesis ALKANE, the first available sequence number is 1

### Deployment Headers

When deploying an Alkane, the cellpack header determines the deployment address:

1. **[1, 0] Header**: Deploys the WASM at address \`[2, n]\`, where n is the next available sequence number
2. **[3, n] Header**: Deploys the alkane to address \`[4, n]\`, if the number is not already occupied
3. **[5, n] Header**: Clones the WASM stored at \`[2, n]\` and deploys to \`[2, next_sequence_number]\`
4. **[6, n] Header**: Clones the WASM at \`[4, n]\` and deploys to \`[2, next_sequence_number]\`

## Cellpack Structure

Cellpacks are protomessages that interact with alkanes:

- **Format**: A cellpack is a protomessage whose calldata is an encoded list of leb128 varints
- **Header**: The first two varints are either the AlkaneId targeted, or a pair with special meanings
- **Inputs**: The remaining varints after the target are considered inputs to the alkane
- **Opcodes**: By convention, the first input after the cellpack target is interpreted as an opcode
- **Initialization**: The 0 opcode following the cellpack target should call initialization logic

Example:
\`\`\`rust
let cellpack = Cellpack {
    target: AlkaneId { block: 1, tx: 0 },
    inputs: vec![
        0,    // opcode (initialization)
        100,  // parameter 1
        200,  // parameter 2
    ],
};
\`\`\`

## Standard Contract Types

### Auth Token Contract
Provides authentication and access control:
- Opcode 0: Initializes with specified amount
- Opcode 1: Verifies authentication (requires at least 1 unit of auth token)
- Returns the auth token in the response to maintain ownership

### Owned Token Contract
Token with ownership verification:
- **CRITICAL**: Must deploy an auth token during initialization
- Opcode 0: Initializes token and deploys auth token
- Opcode 77: Mints new tokens (only callable by owner)
- Uses \`only_owner()\` method for ownership verification

## Common Gotchas

### 1. Auth Token Initialization
**CRITICAL**: Owned token contracts MUST be deployed with an auth token. Without a properly initialized auth token, owned token operations will revert.

\`\`\`rust
// Correct initialization
let cellpack = Cellpack {
    target: AlkaneId { block: 1, tx: 0 },
    inputs: vec![
        0,    // opcode: initialize
        100,  // auth_token_units
        1000, // token_units
    ],
};
\`\`\`

### 2. AlkaneId Block Parameter Confusion
The \`block\` parameter is NOT block height - it's a sequence number for addressing:
- \`[2, n]\` - Sequentially assigned alkanes
- \`[4, n]\` - Custom address alkanes
- Never use actual blockchain block heights as AlkaneId.block values

### 3. Cellpack Parameter Interpretation
The first input after the target is ALWAYS the opcode:
- inputs[0] = opcode
- inputs[1..] = function parameters

Don't confuse parameter values with their semantic meaning - the contract defines what each parameter means.

### 4. Factory Patterns
Use factory opcodes for cloning templates:
\`\`\`rust
// Deploy template at static address
let template = Cellpack {
    target: AlkaneId { block: 3, tx: 0xffddffee },
    inputs: vec![/* init params */],
};

// Clone the template
let instance = Cellpack {
    target: AlkaneId { block: 6, tx: 0xffddffee },
    inputs: vec![/* instance params */],
};
\`\`\`

## WASM Contract Development

### Using the declare_alkane! Macro
\`\`\`rust
use alkanes_support::declare_alkane;

struct MyContract;

impl AlkaneResponder for MyContract {
    fn execute(&self) -> Result<CallResponse> {
        let mut inputs = self.context()?.inputs.clone();
        let opcode = shift_or_err(&mut inputs)?;

        match opcode {
            0 => self.initialize(&mut inputs),
            1 => self.do_something(&mut inputs),
            _ => Err(anyhow!("unrecognized opcode"))
        }
    }
}

declare_alkane!(MyContract);
\`\`\`

### Memory Layout for WASM Scripts
\`\`\`
0-1023:       Context buffer
1024-2047:    Working buffer for cellpacks
2048-4095:    Data buffers
4096+:        Response buffer
\`\`\`

## Testing Contracts

### CRITICAL: Avoid Double Indexing
**NEVER** call \`index_block\` multiple times for the same block - it leads to:
- Additional tokens created with unexpected IDs
- Balances swapped or duplicated
- Inconsistent state between tables

\`\`\`rust
// Correct - index once
Protorune::index_block::<AlkaneMessageContext>(block.clone(), height)?;

// Wrong - never do this
Protorune::index_block::<AlkaneMessageContext>(block.clone(), height)?;
Protorune::index_block::<AlkaneMessageContext>(block.clone(), height)?; // BAD!
\`\`\`

## CLI Usage for Contract Development

\`\`\`bash
# Deploy a contract
alkanes-cli alkanes execute \\
  --target 1:0 \\
  --inputs 0,100,1000 \\
  --fee 10000

# Simulate before deploying
alkanes-cli alkanes simulate 1:0 \\
  --inputs 0,100,1000

# Call a deployed contract
alkanes-cli alkanes execute \\
  --target 2:1 \\
  --inputs 1,500 \\
  --fee 10000
\`\`\`

## Further Reading

For more details, see:
- System Patterns documentation (alkanes://docs/system-patterns)
- Technical Context (alkanes://docs/tech-context)
- AssemblyScript Tx-Scripts Guide (alkanes://docs/assemblyscript-txscripts)
- WAT Templates Guide (alkanes://docs/wat-templates)
`;
}

/**
 * Build comprehensive gotchas documentation
 */
function buildGotchasDoc(): string {
  return `# Common Gotchas & Debugging Guide

This document covers the most common mistakes when working with Alkanes and how to avoid them.

## Critical Gotchas

### 1. Double Indexing - NEVER DO THIS
**Severity**: Critical - Causes inconsistent state

**Problem**: Calling \`index_block\` or \`Protorune::index_block\` multiple times for the same block leads to:
- Additional tokens created with unexpected IDs
- Balances swapped or duplicated
- Inconsistent state between different tables
- Confusing test results

**Solution**: Only call \`index_block\` once per block in tests
\`\`\`rust
// Correct
Protorune::index_block::<AlkaneMessageContext>(block.clone(), height)?;

// Wrong - NEVER do this
Protorune::index_block::<AlkaneMessageContext>(block.clone(), height)?;
Protorune::index_block::<AlkaneMessageContext>(block.clone(), height)?; // BAD!
\`\`\`

### 2. Auth Token Initialization Required
**Severity**: Critical - Causes contract execution to fail

**Problem**: Owned token contracts must be deployed with an auth token. Without proper initialization, \`only_owner()\` calls will revert.

**Solution**: Always initialize owned tokens with auth token units:
\`\`\`rust
// Correct - opcode 0 with auth_token_units and token_units
let cellpack = Cellpack {
    target: AlkaneId { block: 1, tx: 0 },
    inputs: vec![
        0,    // opcode: initialize
        100,  // auth_token_units (REQUIRED)
        1000, // token_units
    ],
};
\`\`\`

### 3. AlkaneId Block Parameter Confusion
**Severity**: High - Causes addressing errors

**Problem**: The \`block\` parameter in AlkaneId is NOT the blockchain block height - it's a sequence number or custom address selector.

**AlkaneId Addressing Rules**:
- \`[1, 0]\` → Deploys to \`[2, next_sequence]\`
- \`[2, n]\` → Sequentially assigned alkanes (n = 0, 1, 2, ...)
- \`[3, n]\` → Deploys to \`[4, n]\` (custom address)
- \`[4, n]\` → Custom address alkanes
- \`[5, n]\` → Clone from \`[2, n]\` → deploy to \`[2, next_sequence]\`
- \`[6, n]\` → Clone from \`[4, n]\` → deploy to \`[2, next_sequence]\`

**Example**:
\`\`\`rust
// Wrong - using block height
AlkaneId { block: 850000, tx: 0 } // This is not how it works!

// Correct - using sequence number
AlkaneId { block: 2, tx: 1 } // Second sequentially deployed alkane

// Correct - using custom address
AlkaneId { block: 4, tx: 0xffddffee } // Custom address alkane
\`\`\`

### 4. Cellpack Parameter Interpretation
**Severity**: High - Causes incorrect function calls

**Problem**: Misunderstanding what cellpack inputs represent:
- The first input is ALWAYS the opcode
- Remaining inputs are parameters for that opcode
- Parameter values are just numbers - their meaning is defined by the contract

**Example**:
\`\`\`rust
// Cellpack structure
let cellpack = Cellpack {
    target: AlkaneId { block: 2, tx: 1 },
    inputs: vec![
        77,    // [0] = opcode (mint function)
        1000,  // [1] = first parameter (amount to mint)
        5,     // [2] = second parameter (some other config)
    ],
};

// Common mistake: thinking inputs[1] means "1000 of something"
// Reality: The contract defines what 1000 means in the context of opcode 77
\`\`\`

### 5. Table Population Dependencies
**Severity**: Medium - Causes empty query results

**Problem**: Different tables are populated through different code paths:

**Table Population Paths**:
- \`OUTPOINTS_FOR_ADDRESS\` - Populated for ALL transactions with valid addresses
- \`OUTPOINT_SPENDABLE_BY\` - Populated during normal transaction indexing
- \`RUNE_ID_TO_OUTPOINTS\` - Only populated for runestone transactions
- \`OUTPOINT_TO_RUNES\` - Populated during runestone processing

**Solution**: Ensure proper indexing flow:
\`\`\`rust
// This populates OUTPOINTS_FOR_ADDRESS
index_spendables(&tx, height)?;

// This populates RUNE_ID_TO_OUTPOINTS (only for runestones)
if is_runestone {
    add_rune_outpoint(&rune_id, &outpoint)?;
}
\`\`\`

### 6. Protobuf Message Encoding
**Severity**: Medium - Causes RPC call failures

**Problem**: Incorrect encoding of nested types like \`Uint128\` in protobuf messages.

**Solution**: Encode \`Uint128\` as length-delimited fields:
\`\`\`rust
// Correct - encode as nested message
let mut protocol_tag = Uint128::new();
protocol_tag.set_value(1);
request.set_protocol_tag(protocol_tag);

// Wrong - trying to use as varint
request.set_protocol_tag(1); // Doesn't work!
\`\`\`

### 7. Context Inputs in WASM
**Severity**: Medium - Causes incorrect parameter reads

**Problem**: Context inputs are located at specific memory offsets in WASM scripts.

**Memory Layout**:
\`\`\`
Offset 0-99:   Context header
Offset 100+:   Context inputs (each input is 16 bytes / u128)
\`\`\`

**Solution**: Read inputs at correct offsets:
\`\`\`wat
;; Read first input (offset 100)
(i32.const 100)
(i64.load)

;; Read second input (offset 116 = 100 + 16)
(i32.const 116)
(i64.load)
\`\`\`

### 8. Fuel Metering
**Severity**: Low - Causes execution failures

**Problem**: Complex operations may run out of fuel.

**Solution**: Provide adequate fuel for staticcalls:
\`\`\`rust
// Low fuel - might fail for complex operations
staticcall(target, opcode, 1000)?;

// High fuel - safe for most operations
staticcall(target, opcode, 0xFFFFFFFFFFFFFFFF)?;
\`\`\`

## Debugging Tips

### 1. Enable Debug Logging
\`\`\`bash
# Set log level
export RUST_LOG=debug
alkanes-cli alkanes simulate ...
\`\`\`

### 2. Use Simulation Before Execution
Always simulate before broadcasting:
\`\`\`bash
# Simulate first
alkanes-cli alkanes simulate 1:0 --inputs 0,100,1000

# If successful, then execute
alkanes-cli alkanes execute 1:0 --inputs 0,100,1000 --fee 10000
\`\`\`

### 3. Check Storage State
Use the inspector to examine contract storage:
\`\`\`bash
alkanes-cli alkanes inspect 2:1 --storage /auth
\`\`\`

### 4. Trace Execution
Use trace functionality to see execution flow:
\`\`\`bash
alkanes-cli alkanes simulate 2:1 --inputs 1,500 --trace
\`\`\`

### 5. Verify AlkaneId Resolution
Double-check that your AlkaneIds resolve to the expected contracts:
\`\`\`bash
# List all deployed alkanes
alkanes-cli alkanes list

# Get details for specific alkane
alkanes-cli alkanes details 2:1
\`\`\`

## Error Messages and Solutions

### "did not authenticate with only the authentication token"
- **Cause**: Sending wrong alkane or multiple alkanes to \`only_owner()\`
- **Solution**: Send exactly 1 unit of the correct auth token

### "supplied alkane is not authentication token"
- **Cause**: Sending wrong AlkaneId to \`only_owner()\`
- **Solution**: Verify auth token AlkaneId stored at \`/auth\`

### "less than 1 unit of authentication token supplied"
- **Cause**: Sending 0 or fractional units of auth token
- **Solution**: Send at least 1 unit

### "unrecognized opcode"
- **Cause**: Invalid opcode for the contract
- **Solution**: Check contract ABI for supported opcodes

### Empty query results
- **Cause**: Tables not populated or using wrong token IDs
- **Solution**: Verify indexing ran correctly and check token ID mapping

## Testing Best Practices

1. **Use \`clear()\` between tests** - Ensure clean state
2. **Never double-index** - Only call \`index_block\` once per block
3. **Verify table population** - Check that expected tables are populated
4. **Test with simulation first** - Don't go straight to execution
5. **Use realistic fuel amounts** - Don't under-fuel your operations

## When to Read Documentation

If you encounter issues with:
- **Contract development** → Read "Contract Development Guide"
- **AlkaneId addressing** → Read "System Patterns" section on AlkaneId
- **Table relationships** → Read "System Patterns" section on Table Relationships
- **WASM scripts** → Read "AssemblyScript Tx-Scripts Guide" or "WAT Templates"
- **Build system** → Read "Technical Context"

## Getting Help

If you're still stuck:
1. Check the error message against this guide
2. Review the System Patterns documentation
3. Look at example contracts in \`crates/alkanes-std-*\`
4. Simulate with \`--trace\` to see execution flow
5. Check the memory-bank/ documentation files
`;
}

/**
 * Build CLI overview documentation
 */
function buildCliOverview(): string {
  return `# Alkanes CLI Commands Overview

This document provides an overview of available alkanes-cli commands and their usage.

## Core Commands

### Alkanes (Smart Contracts)

#### \`alkanes execute\`
Execute an alkanes smart contract operation (broadcasts transaction).

\`\`\`bash
alkanes-cli alkanes execute \\
  --target 2:1 \\
  --inputs 1,500,1000 \\
  --fee 10000
\`\`\`

#### \`alkanes simulate\`
Simulate alkanes execution without broadcasting (dry run).

\`\`\`bash
alkanes-cli alkanes simulate 2:1 \\
  --inputs 1,500 \\
  --trace
\`\`\`

#### \`alkanes swap\`
Swap tokens using AMM pools.

\`\`\`bash
alkanes-cli alkanes swap \\
  --path 2:0,2:1,2:2 \\
  --input 1000000 \\
  --min-output 950000
\`\`\`

### Wallet Operations

#### \`wallet balance\`
Check wallet balances for alkanes and runes.

\`\`\`bash
alkanes-cli wallet balance
\`\`\`

#### \`wallet send\`
Send alkanes or runes to another address.

\`\`\`bash
alkanes-cli wallet send \\
  --to <address> \\
  --alkane 2:1 \\
  --amount 1000
\`\`\`

### Bitcoin Operations

#### \`bitcoind getblockcount\`
Get current block height.

\`\`\`bash
alkanes-cli bitcoind getblockcount
\`\`\`

#### \`bitcoind getblock\`
Get block data by height or hash.

\`\`\`bash
alkanes-cli bitcoind getblock 850000
\`\`\`

### Data API

#### \`dataapi query\`
Query the alkanes data API.

\`\`\`bash
alkanes-cli dataapi query /alkanes/2:1
\`\`\`

### Registry

#### \`registry lookup\`
Look up alkanes in the registry.

\`\`\`bash
alkanes-cli registry lookup 2:1
\`\`\`

## MCP Integration

#### \`mcp install\`
Install and build the MCP server.

\`\`\`bash
alkanes-cli mcp install
\`\`\`

#### \`mcp configure\`
Generate MCP configuration from CLI settings.

\`\`\`bash
alkanes-cli mcp configure --editor cursor
\`\`\`

#### \`mcp status\`
Check MCP server status and configuration.

\`\`\`bash
alkanes-cli mcp status
\`\`\`

#### \`mcp setup\`
Complete automated setup (install + configure + verify).

\`\`\`bash
alkanes-cli mcp setup --editor cursor
\`\`\`

## Common Patterns

### Deploy a new contract
\`\`\`bash
# 1. Simulate first
alkanes-cli alkanes simulate 1:0 \\
  --inputs 0,100,1000 \\
  --envelope my_contract.wasm

# 2. If successful, deploy
alkanes-cli alkanes execute 1:0 \\
  --inputs 0,100,1000 \\
  --envelope my_contract.wasm \\
  --fee 50000
\`\`\`

### Call a deployed contract
\`\`\`bash
# Opcode 1, with parameter 500
alkanes-cli alkanes execute 2:1 \\
  --inputs 1,500 \\
  --fee 10000
\`\`\`

### Batch operations with tx-scripts
\`\`\`bash
# Use WASM script to batch multiple calls
alkanes-cli alkanes simulate 1:0 \\
  --inputs 0,10 \\
  --envelope batch_query.wasm
\`\`\`

## Tips

- Always simulate before executing
- Use \`--trace\` flag to debug execution
- Check \`--help\` on any command for full options
- Set appropriate fees based on network conditions
- Use MCP integration for AI-assisted development

For detailed information on specific commands, run:
\`\`\`bash
alkanes-cli <command> --help
\`\`\`
`;
}
