# Docker Setup for Alkanes-RS

This directory contains Docker configuration for running a complete Alkanes regtest environment with all required services.

## Services

The `docker-compose.yaml` sets up the following services:

1. **bitcoind** - Bitcoin Core in regtest mode (port 18443)
2. **metashrew** - Alkanes indexer (port 8080)
3. **memshrew** - Mempool indexer (port 8081)
4. **ord** - Ordinals indexer (port 8090)
5. **esplora** - Block explorer/electrs (port 50010)
6. **jsonrpc** - Unified JSON-RPC proxy built from `./crates/alkanes-jsonrpc` (port 18888)

## Quick Start

### 1. Start all services

```bash
docker-compose up -d
```

This will:
- Pull/build all required images
- Start all services in the correct order
- Create persistent volumes for blockchain data

### 2. Check service status

```bash
docker-compose ps
docker-compose logs -f jsonrpc
```

### 3. Mine some blocks (regtest)

```bash
# Create a wallet and generate an address
docker-compose exec bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc createwallet "test"
ADDRESS=$(docker-compose exec bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc getnewaddress)

# Mine 101 blocks (needed for coinbase maturity)
docker-compose exec bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc generatetoaddress 101 "$ADDRESS"
```

### 4. Test the unified JSON-RPC endpoint

```bash
# Get block count via sandshrew (unified endpoint)
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "getblockcount",
    "params": [],
    "id": 1
  }'

# Get metashrew height
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "metashrew_height",
    "params": [],
    "id": 1
  }'

# Get ord block count
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "ord_blockcount",
    "params": [],
    "id": 1
  }'
```

### 5. Test with alkanes-cli

```bash
# Build the CLI
cargo build --release -p alkanes-cli

# Use the unified endpoint
./target/release/alkanes-cli -p regtest \
  --sandshrew-rpc-url http://localhost:18888 \
  bitcoind getblockcount

./target/release/alkanes-cli -p regtest \
  --sandshrew-rpc-url http://localhost:18888 \
  metashrew height

./target/release/alkanes-cli -p regtest \
  --sandshrew-rpc-url http://localhost:18888 \
  ord block-count

./target/release/alkanes-cli -p regtest \
  --sandshrew-rpc-url http://localhost:18888 \
  esplora blocks-tip-height
```

## Service Details

### Bitcoind (Bitcoin Core)
- **Port**: 18443 (RPC)
- **User**: bitcoinrpc
- **Password**: bitcoinrpc
- **Network**: regtest
- **Volume**: `bitcoin-data` (persists blockchain)

### Metashrew (Alkanes Indexer)
- **Port**: 8080
- **Connects to**: bitcoind:18443
- **Volume**: `metashrew-data` (persists index)
- **Purpose**: Indexes alkanes, protorunes, and runestones

### Memshrew (Mempool Indexer)
- **Port**: 8081
- **Connects to**: bitcoind:18443 (RPC), bitcoind:18444 (P2P)
- **Purpose**: Real-time mempool monitoring

### Ord (Ordinals Indexer)
- **Port**: 8090
- **Connects to**: bitcoind:18443
- **Purpose**: Indexes inscriptions and ordinals

### Esplora (Block Explorer)
- **HTTP Port**: 50010
- **Electrum Port**: 50001
- **Connects to**: bitcoind:18443
- **Purpose**: Block explorer API and electrum server

### JSON-RPC Proxy (alkanes-jsonrpc)
- **Port**: 18888
- **Built from**: `./crates/alkanes-jsonrpc`
- **Purpose**: Unified JSON-RPC endpoint that routes to all services
- **Features**:
  - Routes `bitcoin_*` methods to bitcoind
  - Routes `ord_*` methods to ord
  - Routes `esplora_*` methods to esplora
  - Routes `metashrew_*` methods to metashrew
  - Routes `alkanes_*` methods to metashrew
  - Provides `sandshrew_*` high-level methods
  - Full logging and CORS support

## Managing Services

### View logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f jsonrpc
docker-compose logs -f bitcoind
docker-compose logs -f metashrew
```

### Restart a service

```bash
docker-compose restart jsonrpc
```

### Stop all services

```bash
docker-compose down
```

### Stop and remove volumes (⚠️ deletes all blockchain data)

```bash
docker-compose down -v
```

### Rebuild jsonrpc after code changes

```bash
docker-compose build jsonrpc
docker-compose up -d jsonrpc
```

## Development Workflow

1. **Make changes** to alkanes-jsonrpc or alkanes-cli
2. **Rebuild** the affected service:
   ```bash
   docker-compose build jsonrpc
   docker-compose up -d jsonrpc
   ```
3. **Test** using curl or alkanes-cli
4. **View logs**:
   ```bash
   docker-compose logs -f jsonrpc
   ```

## Troubleshooting

### Services won't start

Check if ports are already in use:
```bash
lsof -i :18443  # bitcoind
lsof -i :18888  # jsonrpc
```

### Blockchain sync issues

Reset the blockchain data:
```bash
docker-compose down -v
docker-compose up -d
```

### View service health

```bash
docker-compose ps
docker-compose exec bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc getblockchaininfo
```

### Connection refused errors

Wait for services to fully start (especially bitcoind). Check logs:
```bash
docker-compose logs bitcoind
```

## Environment Variables

You can customize the setup by creating a `.env` file:

```bash
cp .env.docker .env
# Edit .env with your custom values
```

See `.env.docker` for available configuration options.

## Production Deployment

⚠️ **This setup is for development/testing only!**

For production:
1. Change all passwords
2. Use proper SSL/TLS
3. Configure proper networking and firewalls
4. Use production-ready images
5. Set up monitoring and backups
6. Review and harden security settings

## Architecture

```
┌─────────────┐
│ alkanes-cli │
└──────┬──────┘
       │
       │ HTTP JSON-RPC
       │
       ▼
┌────────────────────────────────────────┐
│     jsonrpc (alkanes-jsonrpc)          │
│  Unified JSON-RPC Reverse Proxy        │
│  Port: 18888                           │
└─────────┬──────────────────────────────┘
          │
    ┌─────┴─────┬──────────┬──────────┬────────┐
    │           │          │          │        │
    ▼           ▼          ▼          ▼        ▼
┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
│bitcoind│ │metashrew│ │  ord   │ │esplora │ │memshrew│
│  RPC   │ │  HTTP  │ │  HTTP  │ │  HTTP  │ │  HTTP  │
│  8443  │ │  8080  │ │  8090  │ │ 50010  │ │  8081  │
└────────┘ └────────┘ └────────┘ └────────┘ └────────┘
```

## License

MIT
