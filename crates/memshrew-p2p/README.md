# memshrew

memshrew is a Bitcoin mempool tracker and analysis tool included in the metashrew framework. It provides real-time mempool monitoring, block template generation, and fee estimation services via a JSON-RPC API.

## Install

memshrew is built as part of the metashrew project:

```sh
cargo build --release -p memshrew
```

## Usage

Run memshrew with your Bitcoin Core RPC credentials:

```sh
./target/release/memshrew --daemon-rpc-url http://localhost:8332 --auth bitcoinrpc:bitcoinrpc --host 0.0.0.0 --port 8081
```

## JSON-RPC API

memshrew exposes the following RPC methods on the configured endpoint:

### memshrew_getmempooltxs

Returns detailed information about all transactions currently in the mempool, including fees, sizes, and dependency relationships.

### memshrew_getblocktemplates

Returns optimized block templates based on current mempool state, useful for mining or fee analysis.

### memshrew_estimatefees

Returns fee estimates for different confirmation target blocks (1-3 blocks), based on current mempool state.

Each estimate includes:
- target_blocks: Number of blocks to target
- fee_rate: Estimated required fee rate in sat/vB

The API follows standard JSON-RPC 2.0 format.
## Technical Details

### Real-time Updates

memshrew maintains real-time mempool state by:
- Polling the Bitcoin node every 2 seconds for mempool updates
- Regenerating block templates every 30 seconds
- Tracking transaction relationships (ancestors/descendants)
- Maintaining fee rate indexes for estimation

### Block Templates

Block templates are generated with the following constraints:
- Maximum block weight: 4,000,000 (standard Bitcoin limit)
- Minimum fee rate: 1 sat/vB
- Transaction selection optimized for fee revenue
- Ancestor sets are kept together to maintain validity
- Current block subsidy (6.25 BTC) included in reward calculations

### Fee Estimation

Fee estimates use percentile-based analysis:
- 1-block target: 95th percentile of mempool fees
- 2-block target: 80th percentile of mempool fees
- 3-block target: 50th percentile of mempool fees

This provides conservative estimates for different urgency levels.

### Security Notes

- CORS is configured to only allow localhost origins
- Authentication credentials are required for Bitcoin Core RPC access
- The service maintains a read-only connection to Bitcoin Core

## Example Responses

### Fee Estimation
```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "target_blocks": 1,
      "fee_rate": 12.5
    },
    {
      "target_blocks": 2,
      "fee_rate": 8.0
    },
    {
      "target_blocks": 3,
      "fee_rate": 5.0
    }
  ],
  "id": 1
}
```

### Mempool Transaction
```json
{
  "txid": "abc...",
  "fee": 5000,
  "vsize": 250,
  "fee_rate": 20.0,
  "ancestors": ["def...", "ghi..."],
  "descendants": ["jkl...", "mno..."]
}
```

## Configuration

### Command Line Arguments

- `--daemon-rpc-url` - Bitcoin Core RPC endpoint URL (required)
- `--auth` - RPC credentials in format username:password (optional)
- `--host` - Interface to bind to (default: 127.0.0.1)
- `--port` - Port to listen on (default: 8081)

### Environment Variables

- `HOST` - Alternative to --host flag
- `PORT` - Alternative to --port flag

## Integration

memshrew can be integrated into other systems via its JSON-RPC API. All requests should be POST requests to the root endpoint with Content-Type: application/json.

Example request:
```json
{
  "jsonrpc": "2.0",
  "method": "memshrew_estimatefees",
  "params": [],
  "id": 1
}
```

### Error Responses

Standard JSON-RPC 2.0 error format is used:
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32601,
    "message": "Method not found",
    "data": null
  },
  "id": 1
}
```

Common error codes:
- -32600: Invalid Request
- -32601: Method not found
- -32602: Invalid params
- -32603: Internal error

## Resource Usage

- Memory usage scales with mempool size
- CPU usage primarily from template generation
- Network usage depends on mempool update frequency
- Disk usage is minimal (runtime only)

For production deployments, consider monitoring resource usage and adjusting update intervals if needed.
