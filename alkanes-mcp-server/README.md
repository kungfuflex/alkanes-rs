# Alkanes CLI MCP Server

A comprehensive Model Context Protocol (MCP) server that exposes all `alkanes-cli` functionality as callable tools for AI agents. This server provides complete access to the Alkanes ecosystem, including Bitcoin operations, wallet management, smart contract interactions, and more.

## Features

- **Complete CLI Coverage**: All `alkanes-cli` commands are available as MCP tools
- **Multi-Environment Support**: Configure and switch between regtest, signet, and mainnet
- **Secure Credential Management**: Environment variable support and encrypted passphrase handling
- **Comprehensive Tool Set**: 200+ tools covering:
  - Bitcoin Core RPC operations
  - Wallet management and transactions
  - Alkanes protocol operations
  - BRC20-Prog contract interactions
  - Data API queries
  - Ord/Inscription operations
  - OPI indexer queries
  - And much more

## Installation

```bash
cd alkanes-mcp-server
npm install
npm run build
```

## Configuration

The MCP server supports configuration via the MCP server configuration or environment variables.

### Configuration Schema

```json
{
  "environments": {
    "regtest": {
      "cli_path": "/path/to/alkanes-cli",
      "provider": "regtest",
      "wallet_file": "~/.alkanes/wallet.json",
      "passphrase": "${WALLET_PASSPHRASE}",
      "jsonrpc_url": "https://regtest.subfrost.io/v4/...",
      "data_api": "https://regtest.subfrost.io/v4/...",
      "bitcoin_rpc_url": "http://localhost:18443",
      "esplora_api_url": "http://localhost:50010",
      "ord_server_url": "http://localhost:8090",
      "metashrew_rpc_url": "http://localhost:8080"
    },
    "signet": {
      "cli_path": "/path/to/alkanes-cli",
      "provider": "signet",
      "wallet_file": "~/.alkanes/wallet-signet.json",
      "passphrase": "${WALLET_PASSPHRASE}",
      "jsonrpc_url": "https://signet.subfrost.io/v4/...",
      "data_api": "https://signet.subfrost.io/v4/..."
    },
    "mainnet": {
      "cli_path": "/path/to/alkanes-cli",
      "provider": "mainnet",
      "wallet_file": "~/.alkanes/wallet-mainnet.json",
      "passphrase": "${WALLET_PASSPHRASE}",
      "jsonrpc_url": "https://mainnet.subfrost.io/v4/...",
      "data_api": "https://mainnet-api.oyl.gg"
    }
  },
  "default_environment": "regtest",
  "timeout_seconds": 600
}
```

### Environment Variables

- `environments`: JSON string containing the environments configuration (automatically serialized by MCP clients)
- `default_environment`: Name of the default environment to use
- `timeout_seconds`: Optional timeout for command execution (default: 600)
- `ALKANES_CLI_PATH`: Path to the alkanes-cli binary (fallback if not in config)
- `WALLET_PASSPHRASE`: Wallet passphrase (can be referenced in config as `${WALLET_PASSPHRASE}`)

## Usage

### Running the Server

```bash
npm start
```

Or for development:

```bash
npm run dev
```

### MCP Client Configuration

Add to your MCP client configuration (e.g., Cursor, Claude Desktop):

**Recommended Format:**

The configuration structure can be placed directly in the `env` object. Your MCP client will automatically serialize nested objects to JSON strings when passing them as environment variables:

```json
{
  "mcpServers": {
    "alkanes-cli": {
      "command": "node",
      "args": ["/path/to/alkanes-mcp-server/dist/index.js"],
      "env": {
        "environments": {
          "regtest": {
            "cli_path": "/path/to/alkanes-cli",
            "provider": "regtest",
            "wallet_file": "~/.alkanes/wallet.json",
            "passphrase": "${WALLET_PASSPHRASE}",
            "jsonrpc_url": "https://regtest.subfrost.io/v4/...",
            "data_api": "https://regtest.subfrost.io/v4/...",
            "bitcoin_rpc_url": "http://localhost:18443",
            "esplora_api_url": "http://localhost:50010",
            "ord_server_url": "http://localhost:8090",
            "metashrew_rpc_url": "http://localhost:8080"
          },
          "signet": {
            "cli_path": "/path/to/alkanes-cli",
            "provider": "signet",
            "wallet_file": "~/.alkanes/wallet-signet.json",
            "passphrase": "${WALLET_PASSPHRASE}",
            "jsonrpc_url": "https://signet.subfrost.io/v4/...",
            "data_api": "https://signet.subfrost.io/v4/..."
          },
          "mainnet": {
            "cli_path": "/path/to/alkanes-cli",
            "provider": "mainnet",
            "wallet_file": "~/.alkanes/wallet-mainnet.json",
            "passphrase": "${WALLET_PASSPHRASE}",
            "jsonrpc_url": "https://mainnet.subfrost.io/v4/...",
            "data_api": "https://mainnet-api.oyl.gg"
          }
        },
        "default_environment": "regtest",
        "timeout_seconds": "600",
        "WALLET_PASSPHRASE": "your-passphrase-here"
      }
    }
  }
}
```

## Available Tools

### Bitcoin Core RPC Tools

- `bitcoind_getblockcount` - Get current block count
- `bitcoind_generatetoaddress` - Generate blocks (regtest only)
- `bitcoind_getblockchaininfo` - Get blockchain information
- `bitcoind_getrawtransaction` - Get raw transaction
- `bitcoind_getblock` - Get block
- `bitcoind_sendrawtransaction` - Send raw transaction
- And 11 more...

### Wallet Tools

- `wallet_create` - Create a new wallet
- `wallet_addresses` - Get addresses from wallet
- `wallet_utxos` - List UTXOs
- `wallet_send` - Send transaction
- `wallet_balance` - Get balance
- `wallet_sign` - Sign PSBT
- And 12 more...

### Alkanes Tools

- `alkanes_execute` - Execute an alkanes transaction
- `alkanes_inspect` - Inspect an alkanes contract
- `alkanes_trace` - Trace an alkanes transaction
- `alkanes_simulate` - Simulate an alkanes transaction
- `alkanes_wrap_btc` - Wrap BTC to frBTC
- `alkanes_swap` - Execute a swap on the AMM
- `alkanes_init_pool` - Initialize a liquidity pool
- And 14 more...

### BRC20-Prog Tools

- `brc20_prog_deploy_contract` - Deploy a BRC20-prog contract
- `brc20_prog_transact` - Call a BRC20-prog contract function
- `brc20_prog_wrap_btc` - Wrap BTC to frBTC
- `brc20_prog_get_code` - Get contract bytecode
- `brc20_prog_call` - Call a contract function
- `brc20_prog_get_balance` - Get frBTC balance
- And 25+ more...

### DataAPI Tools

- `dataapi_get_alkanes` - Get all alkanes
- `dataapi_get_alkane_details` - Get alkane details
- `dataapi_get_pools` - Get all pools
- `dataapi_get_pool_by_id` - Get pool details
- `dataapi_get_holders` - Get holders of a token
- And 11 more...

### Additional Tool Categories

- **Ord Tools**: Inscription queries, address info, rune info
- **OPI Tools**: BRC-20, Runes, Bitmap, POW20, SNS indexer queries
- **Esplora Tools**: Block and transaction queries
- **Metashrew Tools**: Block height, state root queries
- **Lua Tools**: Lua script execution
- **Protorunes Tools**: Protorune queries
- **Runestone Tools**: Runestone analysis
- **Subfrost Tools**: frBTC unwrap utilities
- **ESPO Tools**: Alkanes balance indexer queries

## Examples

### Get Block Count

```json
{
  "name": "bitcoind_getblockcount",
  "arguments": {}
}
```

### Create Wallet

```json
{
  "name": "wallet_create",
  "arguments": {
    "output": "~/.alkanes/my-wallet.json"
  }
}
```

### Get Wallet Balance

```json
{
  "name": "wallet_balance",
  "arguments": {
    "raw": true
  }
}
```

### Execute Alkanes Transaction

```json
{
  "name": "alkanes_execute",
  "arguments": {
    "to": ["p2tr:0"],
    "protostones": ["B:1000"],
    "auto_confirm": true
  }
}
```

### Deploy BRC20-Prog Contract

```json
{
  "name": "brc20_prog_deploy_contract",
  "arguments": {
    "foundry_json_path": "./out/MyToken.sol/MyToken.json",
    "auto_confirm": true
  }
}
```

## Security Considerations

1. **Passphrase Handling**:

   - Use environment variables for passphrases (e.g., `${WALLET_PASSPHRASE}`)
   - Never hardcode passphrases in configuration files
   - Passphrases are never logged

2. **Configuration Validation**:

   - All URLs are validated for proper format
   - File paths are sanitized to prevent directory traversal
   - Network/provider consistency is verified

3. **Command Execution**:
   - All tool parameters are validated
   - Command injection is prevented
   - Timeouts are enforced (default: 10 minutes)

## Error Handling

The server provides comprehensive error handling:

- `CONFIG_ERROR`: Configuration issues
- `EXECUTION_ERROR`: Command execution failures
- `TIMEOUT_ERROR`: Command timeouts
- `VALIDATION_ERROR`: Parameter validation failures

All errors include detailed messages and context for debugging.

## Development

### Building

```bash
npm run build
```

### Development Mode

```bash
npm run dev
```

### Type Checking

```bash
npx tsc --noEmit
```

## License

MIT

## Support

For issues and questions, please refer to the main Alkanes project documentation.
