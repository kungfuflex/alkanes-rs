# Alkanes Namespace Implementation

## Overview

The `alkanes_*` JSON-RPC namespace provides a unified interface for calling Alkanes protocol methods through the JSON-RPC server. This implementation now correctly matches the TypeScript reference implementation.

## Changes Made

### Before (Incorrect)
The previous implementation was double-prefixing methods:
```rust
async fn handle_alkanes_method(...) -> Result<JsonRpcResponse> {
    let modified_request = JsonRpcRequest {
        method: format!("alkanes_{}", method),  // Wrong: adds prefix again
        params: params.to_vec(),
        ...
    };
    proxy.forward_to_metashrew(&modified_request).await
}
```

### After (Correct)
The updated implementation follows the TypeScript pattern:
```rust
async fn handle_alkanes_method(...) -> Result<JsonRpcResponse> {
    // Extract input and block_tag from params
    let input = params.get(0).cloned().unwrap_or(Value::Null);
    let block_tag = params.get(1).and_then(|v| v.as_str()).unwrap_or("latest");
    
    // Forward as metashrew_view(method_name, input, block_tag)
    let modified_request = JsonRpcRequest {
        method: "metashrew_view".to_string(),
        params: vec![
            Value::String(method.to_string()),
            input,
            Value::String(block_tag.to_string()),
        ],
        ...
    };
    proxy.forward_to_metashrew(&modified_request).await
}
```

## How It Works

When a client makes a call like:
```json
{
  "jsonrpc": "2.0",
  "method": "alkanes_getbytecode",
  "params": [{"block": 123, "tx": 456}],
  "id": 1
}
```

The server:
1. Splits the method name by `_` to get namespace (`alkanes`) and method (`getbytecode`)
2. Extracts the first param as `input` and second param as `block_tag` (defaulting to `"latest"`)
3. Forwards to metashrew as:
```json
{
  "jsonrpc": "2.0",
  "method": "metashrew_view",
  "params": ["getbytecode", {"block": 123, "tx": 456}, "latest"],
  "id": 1
}
```

## Supported Methods

Based on the TypeScript reference implementation (`/data/alkanes/src.ts/rpc.ts`), the following methods are available in the `alkanes_*` namespace:

### Block & Transaction Methods
- `alkanes_getbytecode` - Get bytecode for a contract
- `alkanes_getblock` - Get block information
- `alkanes_transactionbyid` - Get transaction by ID
- `alkanes_traceblock` - Trace all transactions in a block
- `alkanes_trace` - Trace a specific transaction

### Runes & Protorunes Methods
- `alkanes_runesbyaddress` - Get runes for an address
- `alkanes_runesbyheight` - Get runes at a specific block height
- `alkanes_runesbyoutpoint` - Get runes for a specific outpoint
- `alkanes_protorunesbyaddress` - Get protorunes for an address
- `alkanes_protorunesbyheight` - Get protorunes at a specific height
- `alkanes_protorunesbyoutpoint` - Get protorunes for a specific outpoint
- `alkanes_spendablesbyaddress` - Get spendable outputs for an address

### Alkanes-Specific Methods
- `alkanes_alkanesidtooutpoint` - Convert Alkanes ID to outpoint
- `alkanes_getinventory` - Get inventory for an Alkane
- `alkanes_getstorageat` - Get storage at a specific path
- `alkanes_getstorageatstring` - Get storage at a path (string variant)
- `alkanes_simulate` - Simulate transaction execution
- `alkanes_meta` - Get metadata for a transaction
- `alkanes_runtime` - Get runtime information
- `alkanes_sequence` - Get sequence number
- `alkanes_unwraps` - Get unwrap operations for a block

## Usage Examples

### JavaScript/TypeScript Client
```javascript
const response = await fetch('http://localhost:18888', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    method: 'alkanes_getbytecode',
    params: [{ block: 123, tx: 456 }],
    id: 1
  })
});
const result = await response.json();
```

### With Block Tag
```javascript
const response = await fetch('http://localhost:18888', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    method: 'alkanes_getbytecode',
    params: [{ block: 123, tx: 456 }, 'latest'],
    id: 1
  })
});
```

### Curl Example
```bash
curl -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "alkanes_getinventory",
    "params": [{"block": 123, "tx": 456}],
    "id": 1
  }'
```

## Parameter Format

Most methods follow this pattern:
```
alkanes_<method_name>(input_object, block_tag?)
```

Where:
- `input_object`: Method-specific parameters (e.g., `{block, tx}`, `{address}`, etc.)
- `block_tag`: Optional, defaults to `"latest"`. Can be `"latest"`, `"pending"`, or a specific block number

## References

- TypeScript implementation: `/data/alkanes/jsonrpc/src.ts/`
- Rust implementation: `/data/alkanes-rs/crates/alkanes-jsonrpc/`
- Handler logic: `/data/alkanes-rs/crates/alkanes-jsonrpc/src/handler.rs`
