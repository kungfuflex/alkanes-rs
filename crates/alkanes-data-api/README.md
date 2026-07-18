# Alkanes Data API

REST API server for alkanes blockchain data and AMM statistics.

## Features

- **Alkanes Queries**: Get alkanes by address, search, and detailed information
- **AMM Pool Data**: Pool details, liquidity positions, and token pairs
- **Transaction History**: Complete AMM transaction history (swaps, mints, burns, wraps)
- **Bitcoin Data**: Address balances, UTXOs, and transaction history
- **BTC Price Feed**: Real-time Bitcoin price from Uniswap V3 WBTC/USDC pool via alloy-rs

## Prerequisites

- Rust 1.70+
- PostgreSQL database (with alkanes-contract-indexer schema)
- Redis server
- Bitcoin Core RPC access
- Ethereum RPC endpoint (Infura provided by default)

## Configuration

Create a `.env` file based on `.env.example`:

```bash
cp .env.example .env
```

Edit the `.env` file with your configuration:

```env
HOST=0.0.0.0
PORT=3000
API_KEY=your_secret_api_key
DATABASE_URL=postgresql://user:password@localhost:5432/alkanes
REDIS_URL=redis://localhost:6379
BITCOIN_RPC_URL=http://localhost:8332
BITCOIN_RPC_USER=bitcoin
BITCOIN_RPC_PASSWORD=password
INFURA_ENDPOINT=https://mainnet.infura.io/v3/099fc58e0de9451d80b18d7c74caa7c1
```

## Building

```bash
cargo build --release -p alkanes-data-api
```

The binary will be available at `./target/release/alkanes-data-api` (approximately 15MB).

## Running

### Option 1: Run with Cargo

```bash
cargo run --release -p alkanes-data-api
```

### Option 2: Run the Binary Directly

```bash
./target/release/alkanes-data-api
```

### Option 3: Run with Docker

1. Build the Docker image:
```bash
docker build -f crates/alkanes-data-api/Dockerfile -t alkanes-data-api:latest .
```

2. Run the container:
```bash
docker run -d \
  --name alkanes-data-api \
  -p 3000:3000 \
  -e DATABASE_URL=postgresql://postgres:password@postgres:5432/alkanes \
  -e REDIS_URL=redis://redis:6379 \
  -e SANDSHREW_URL=http://sandshrew:8080 \
  -e NETWORK_ENV=mainnet \
  -e ALKANE_FACTORY_ID=840000:1 \
  -e ETHEREUM_RPC_URL=https://mainnet.infura.io/v3/YOUR_INFURA_PROJECT_ID \
  alkanes-data-api:latest
```

### Option 4: Run with Docker Compose

Add the following service to your `docker-compose.yaml`:

```yaml
  alkanes-data-api:
    build:
      context: .
      dockerfile: crates/alkanes-data-api/Dockerfile
    container_name: alkanes-data-api
    environment:
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/alkanes
      - REDIS_URL=redis://redis:6379
      - SANDSHREW_URL=http://sandshrew:8080
      - NETWORK_ENV=mainnet
      - ALKANE_FACTORY_ID=840000:1
      - ETHEREUM_RPC_URL=${ETHEREUM_RPC_URL}
      - HOST=0.0.0.0
      - PORT=3000
      - RUST_LOG=info,alkanes_data_api=debug
    ports:
      - "3000:3000"
    depends_on:
      - postgres
      - redis
      - sandshrew
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/api/v1/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
```

Then run:
```bash
docker-compose up -d alkanes-data-api
```

## API Endpoints

**Note**: API key authentication has been removed. All endpoints are publicly accessible.

### Health Check
- `GET /api/v1/health` - Health check endpoint

### Bitcoin Price
- `POST /api/v1/get-bitcoin-price` - Get current BTC price in USD
- `POST /api/v1/get-bitcoin-market-chart` - Get historical price data
- `POST /api/v1/get-bitcoin-market-weekly` - Get 52-week high/low data
- `POST /api/v1/get-bitcoin-markets` - Get market summary

### Alkanes
- `POST /api/v1/get-alkanes` - List all alkanes
- `POST /api/v1/get-alkanes-by-address` - Get alkanes for an address
- `POST /api/v1/get-alkane-details` - Get details for a specific alkane
- `POST /api/v1/get-alkanes-utxo` - Get alkane UTXOs
- `POST /api/v1/get-amm-utxos` - Get AMM-spendable UTXOs
- `POST /api/v1/global-alkanes-search` - Search alkanes

### Pools
- `POST /api/v1/get-pools` - List all pools
- `POST /api/v1/get-pool-details` - Get pool details
- `POST /api/v1/get-all-pools-details` - Get all pool details
- `POST /api/v1/address-positions` - Get liquidity positions
- `POST /api/v1/get-all-token-pairs` - Get all token pairs
- `POST /api/v1/get-token-pairs` - Get pairs for a token
- `POST /api/v1/get-alkane-swap-pair-details` - Get swap paths

### History
- `POST /api/v1/get-pool-swap-history` - Pool swap history
- `POST /api/v1/get-token-swap-history` - Token swap history
- `POST /api/v1/get-pool-mint-history` - Liquidity mint history
- `POST /api/v1/get-pool-burn-history` - Liquidity burn history
- `POST /api/v1/get-pool-creation-history` - Pool creation history
- `POST /api/v1/get-address-swap-history-for-pool` - Address swap history
- `POST /api/v1/get-address-swap-history-for-token` - Address token swaps
- `POST /api/v1/get-address-wrap-history` - Wrap transaction history
- `POST /api/v1/get-address-unwrap-history` - Unwrap transaction history
- `POST /api/v1/get-all-wrap-history` - All wrap transactions
- `POST /api/v1/get-all-unwrap-history` - All unwrap transactions
- `POST /api/v1/get-total-unwrap-amount` - Total unwrapped amount
- `POST /api/v1/get-address-pool-creation-history` - Pools created by address
- `POST /api/v1/get-address-pool-mint-history` - Liquidity adds by address
- `POST /api/v1/get-address-pool-burn-history` - Liquidity removes by address
- `POST /api/v1/get-all-address-amm-tx-history` - All AMM txs for address
- `POST /api/v1/get-all-amm-tx-history` - All AMM transactions

### Bitcoin/UTXOs
- `POST /api/v1/get-address-balance` - Get address balance
- `POST /api/v1/get-taproot-balance` - Get taproot balance
- `POST /api/v1/get-address-utxos` - Get address UTXOs
- `POST /api/v1/get-account-utxos` - Get account UTXOs
- `POST /api/v1/get-account-balance` - Get account balance
- `POST /api/v1/get-taproot-history` - Get transaction history
- `POST /api/v1/get-intent-history` - Get transaction intents

## Example Request

```bash
curl -X POST http://localhost:3000/api/v1/get-bitcoin-price \
  -H "Content-Type: application/json"
```

Response:
```json
{
  "statusCode": 200,
  "data": {
    "bitcoin": {
      "usd": 98765.43
    }
  }
}
```

## Architecture

- **actix-web**: High-performance async web framework
- **sqlx**: Async PostgreSQL database access
- **redis**: Caching layer
- **bitcoincore-rpc**: Bitcoin Core RPC client
- **alloy**: Ethereum library for Uniswap price feed

## Price Feed

The BTC price is fetched from the Uniswap V3 WBTC/USDC pool on Ethereum mainnet using the alloy-rs library. The price is cached for 60 seconds to minimize RPC calls.

## Development

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Docker Support

Coming soon.

## License

MIT
