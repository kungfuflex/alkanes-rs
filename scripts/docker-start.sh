#!/bin/bash
# Quick start script for alkanes-rs Docker environment

set -e

echo "🐳 Starting Alkanes-RS Docker Environment"
echo "=========================================="
echo ""

# Check if docker-compose is available
if ! command -v docker-compose &> /dev/null; then
    echo "❌ docker-compose not found. Please install Docker Compose."
    exit 1
fi

# Start services
echo "📦 Starting all services..."
docker-compose up -d

echo ""
echo "⏳ Waiting for services to be ready..."
sleep 5

# Wait for bitcoind to be ready
echo "⏳ Waiting for bitcoind..."
until docker-compose exec -T bitcoind bitcoin-cli -regtest -rpcuser=bitcoinrpc -rpcpassword=bitcoinrpc getblockchaininfo &> /dev/null; do
    echo "   Still waiting for bitcoind..."
    sleep 2
done
echo "✅ Bitcoind is ready"

# Check service status
echo ""
echo "📊 Service Status:"
docker-compose ps

echo ""
echo "✅ All services started!"
echo ""
echo "📝 Next Steps:"
echo ""
echo "1. Mine some blocks (regtest):"
echo "   ./scripts/docker-mine.sh 101"
echo ""
echo "2. Test the unified JSON-RPC endpoint:"
echo "   curl -X POST http://localhost:18888 -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"getblockcount\",\"params\":[],\"id\":1}'"
echo ""
echo "3. Use alkanes-cli:"
echo "   cargo build --release -p alkanes-cli"
echo "   ./target/release/alkanes-cli -p regtest --sandshrew-rpc-url http://localhost:18888 bitcoind getblockcount"
echo ""
echo "4. View logs:"
echo "   docker-compose logs -f jsonrpc"
echo ""
echo "📚 See README.docker.md for more information"
