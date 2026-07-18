# alkanes-jsonrpc

A unified JSON-RPC reverse proxy server for the Alkanes ecosystem built with actix-web. This server provides a clean, well-organized interface to multiple backend services including Bitcoin Core, Ord, Esplora, Metashrew, and Memshrew.

## Features

- **Unified JSON-RPC Interface**: Single endpoint for all blockchain operations
- **Clean Environment Variables**: Well-organized configuration with sensible defaults
- **Reverse Proxy Support**: Seamlessly proxies requests to multiple backend services
- **Multiple Namespaces**: 
  - `bitcoin_*` - Bitcoin Core RPC methods
  - `ord_*` - Ord server endpoints
  - `esplora_*` - Esplora/Electrs endpoints
  - `metashrew_*` - Metashrew indexer
  - `memshrew_*` - Memshrew mempool indexer
  - `alkanes_*` - Alkanes protocol methods
  - `sandshrew_*` - High-level wallet and balance queries
- **Request Logging**: Built-in logging with X-Real-IP header support
- **CORS Support**: Cross-origin requests enabled by default

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_HOST` | `0.0.0.0` | Server bind address |
| `SERVER_PORT` | `18888` | Server port |
| `BITCOIN_RPC_URL` | `http://localhost:8332` | Bitcoin Core RPC endpoint |
| `BITCOIN_RPC_USER` | `bitcoinrpc` | Bitcoin Core RPC username |
| `BITCOIN_RPC_PASSWORD` | `bitcoinrpc` | Bitcoin Core RPC password |
| `METASHREW_URL` | `http://localhost:8080` | Metashrew indexer endpoint |
| `MEMSHREW_URL` | `http://localhost:8081` | Memshrew mempool indexer endpoint |
| `ORD_URL` | `http://localhost:8090` | Ord server endpoint |
| `ESPLORA_URL` | `http://localhost:50010` | Esplora/Electrs endpoint |

## Installation

Build from the workspace root:

```bash
cargo build --release -p alkanes-jsonrpc
```

## Usage

### Running the Server

```bash
# With default configuration
cargo run --release -p alkanes-jsonrpc

# With custom configuration
SERVER_PORT=3000 \
BITCOIN_RPC_URL=http://bitcoin-node:8332 \
ORD_URL=http://ord-server:8090 \
cargo run --release -p alkanes-jsonrpc
```

### Example Requests

#### Bitcoin Core RPC

```bash
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "bitcoin_getblockcount",
    "params": [],
    "id": 1
  }'
```

#### Ord Server

```bash
# Get inscription content
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "ord_content",
    "params": ["<inscription_id>"],
    "id": 1
  }'

# Get outputs for an address
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "ord_outputs",
    "params": ["<bitcoin_address>"],
    "id": 1
  }'
```

#### Esplora

```bash
# Get address UTXOs
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "esplora_address::utxo",
    "params": ["<bitcoin_address>"],
    "id": 1
  }'
```

#### Alkanes Protocol

```bash
# Get protorunes by address
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "alkanes_protorunesbyaddress",
    "params": [{
      "address": "<bitcoin_address>",
      "protocolTag": "1"
    }],
    "id": 1
  }'
```

#### Sandshrew Namespace

The `sandshrew_*` methods provide high-level wallet functionality:

```bash
# Get comprehensive address balance info
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "sandshrew_balances",
    "params": [{
      "address": "<bitcoin_address>",
      "protocolTag": "1"
    }],
    "id": 1
  }'

# Response includes:
# - spendable: UTXOs without assets (safe to spend)
# - assets: UTXOs with runes/inscriptions
# - pending: UTXOs not yet indexed
# - ordHeight: Current ord indexer height
# - metashrewHeight: Current metashrew indexer height
```

```bash
# Multicall - batch multiple RPC calls
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "sandshrew_multicall",
    "params": [[
      ["bitcoin_getblockcount", []],
      ["ord_blockheight", []],
      ["metashrew_height", []]
    ]],
    "id": 1
  }'
```

## Architecture

The server acts as a reverse proxy that:

1. **Receives** JSON-RPC requests on a single endpoint
2. **Routes** requests based on method namespace prefix
3. **Forwards** to the appropriate backend service
4. **Transforms** responses into standardized JSON-RPC format
5. **Returns** the result to the client

### Namespace Routing

- Methods prefixed with `bitcoin_*` → Bitcoin Core RPC
- Methods prefixed with `ord_*` → Ord server HTTP endpoints
- Methods prefixed with `esplora_*` → Esplora HTTP endpoints
- Methods prefixed with `metashrew_*` → Metashrew JSON-RPC
- Methods prefixed with `memshrew_*` → Memshrew JSON-RPC
- Methods prefixed with `alkanes_*` → Metashrew JSON-RPC (alkanes namespace)
- Methods prefixed with `sandshrew_*` → Internal high-level methods
- All other methods → Bitcoin Core RPC (with last segment as method name)

## Logging

The server uses `env_logger` for logging. Set the `RUST_LOG` environment variable to control log levels:

```bash
RUST_LOG=info cargo run --release -p alkanes-jsonrpc
RUST_LOG=debug cargo run --release -p alkanes-jsonrpc
```

Request logging includes:
- Client IP (from X-Real-IP header if available)
- Full JSON-RPC request payload

## Improvements Over Reference Implementation

1. **Cleaner Environment Variables**:
   - Single `BITCOIN_RPC_URL` instead of complex address parsing
   - Single `ORD_URL` instead of separate host/port variables
   - Explicit URLs for all services with sensible defaults

2. **Type Safety**:
   - Written in Rust with full type checking
   - Structured error handling

3. **Better Architecture**:
   - Modular design with separate modules for each concern
   - Async/await throughout with actix-web
   - Proper HTTP client with connection pooling (reqwest)

4. **Enhanced Features**:
   - Built-in CORS support
   - Better error messages with full stack traces
   - Request/response logging
   - Support for 100MB request bodies for large inscriptions

## License

MIT
