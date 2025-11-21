# Alkane Context Logging Enhancement

## Overview

Enhanced the wasmi alkane context `__log` host function to automatically prefix all log messages with the alkane ID `[m:n]` that is emitting the log. This provides instant visibility into which alkane contract is producing which logs during execution.

## Implementation

### Location
`/crates/alkanes/src/vm/host_functions.rs` - `AlkanesHostFunctionsImpl::log()`

### Changes

**Before:**
```rust
pub(super) fn log<'a>(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
    let mem = get_memory(caller)?;
    let message = {
        let data = mem.data(&caller);
        read_arraybuffer(data, v)?
    };
    print!("{}", String::from_utf8(message)?);
    Ok(())
}
```

**After:**
```rust
pub(super) fn log<'a>(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
    use crate::logging::{LogTree, log_tree, LogStyle};
    
    let mem = get_memory(caller)?;
    let (alkane_id, message) = {
        let data = mem.data(&caller);
        let msg = read_arraybuffer(data, v)?;
        let id = caller.data_mut().context.lock().unwrap().myself.clone();
        (id, msg)
    };
    
    let msg_str = String::from_utf8(message)?;
    
    // Check if message has multiple lines - if so, use tree format
    let lines: Vec<&str> = msg_str.lines().collect();
    if lines.len() > 1 {
        let mut tree = LogTree::new(format!("[{}:{}]", alkane_id.block, alkane_id.tx));
        for (i, line) in lines.iter().enumerate() {
            if i == lines.len() - 1 {
                tree.add_last(line.to_string());
            } else {
                tree.add(line.to_string());
            }
        }
        log_tree(LogStyle::VM, &tree);
    } else {
        // Single line - simple format
        println!("⚙️ [VM] [{}:{}] {}", alkane_id.block, alkane_id.tx, msg_str);
    }
    
    Ok(())
}
```

## Features

### 1. **Automatic Alkane ID Prefix**

Every log from an alkane contract is automatically prefixed with `[block:tx]`:

```
⚙️ [VM] [2:0] Processing transfer
⚙️ [VM] [2:0] Balance updated: 1000000
⚙️ [VM] [31:0] frBTC minting started
```

### 2. **Multi-line Log Support**

When an alkane logs multiple lines, they're displayed in an elegant tree structure:

**Single-line:**
```
⚙️ [VM] [2:0] Simple log message
```

**Multi-line:**
```
🔮 [WASM] ⚙️ [VM]
🔮 [WASM] [2:0]
🔮 [WASM] ├─ Processing complex operation
🔮 [WASM] ├─ Step 1: Validation complete
🔮 [WASM] ├─ Step 2: Balance updated
🔮 [WASM] └─ Step 3: Event emitted
```

### 3. **Integration with WASM Logging**

All alkane logs are intercepted by the WASM logging system, so they appear with the 🔮 [WASM] prefix:

```
🔮 [WASM] ⚙️ [VM] [2:0] Contract execution started
🔮 [WASM] ⚙️ [VM] [2:0] Calling external function
🔮 [WASM] ⚙️ [VM] [31:0] frBTC contract invoked
🔮 [WASM] ⚙️ [VM] [31:0] Minting 1000 tokens
🔮 [WASM] ⚙️ [VM] [2:0] External call completed
```

## Example Output

### Before Enhancement
```
Processing transfer
Validating balance
Balance updated: 1000000
Calling frBTC
frBTC minting started
frBTC minting complete
Transfer complete
```

**Problem:** No way to tell which contract is logging what!

### After Enhancement
```
🔮 [WASM] ⚙️ [VM] [2:0] Processing transfer
🔮 [WASM] ⚙️ [VM] [2:0] Validating balance
🔮 [WASM] ⚙️ [VM] [2:0] Balance updated: 1000000
🔮 [WASM] ⚙️ [VM] [2:0] Calling frBTC
🔮 [WASM] ⚙️ [VM] [31:0] frBTC minting started
🔮 [WASM] ⚙️ [VM] [31:0] frBTC minting complete
🔮 [WASM] ⚙️ [VM] [2:0] Transfer complete
```

**Solution:** Clear visibility of contract execution flow!

## Use Cases

### 1. **Debug Complex Transactions**

When multiple contracts interact, you can see exactly which contract is doing what:

```
🔮 [WASM] ⚙️ [VM] [100:5] DEX: Swap initiated
🔮 [WASM] ⚙️ [VM] [100:5] DEX: Checking token A balance
🔮 [WASM] ⚙️ [VM] [2:0] TokenA: balance() called
🔮 [WASM] ⚙️ [VM] [2:0] TokenA: returning balance 5000
🔮 [WASM] ⚙️ [VM] [100:5] DEX: Checking token B balance
🔮 [WASM] ⚙️ [VM] [3:0] TokenB: balance() called
🔮 [WASM] ⚙️ [VM] [3:0] TokenB: returning balance 10000
🔮 [WASM] ⚙️ [VM] [100:5] DEX: Executing swap
🔮 [WASM] ⚙️ [VM] [2:0] TokenA: transfer() called
🔮 [WASM] ⚙️ [VM] [3:0] TokenB: transfer() called
🔮 [WASM] ⚙️ [VM] [100:5] DEX: Swap completed
```

### 2. **Track Nested Calls**

Follow the execution path through nested contract calls:

```
🔮 [WASM] ⚙️ [VM] [50:1] Wallet: Initiating payment
🔮 [WASM] ⚙️ [VM] [50:1] Wallet: Calling token contract
🔮 [WASM] ⚙️ [VM] [2:0] Token: Transfer requested
🔮 [WASM] ⚙️ [VM] [2:0] Token: Checking allowance
🔮 [WASM] ⚙️ [VM] [2:0] Token: Calling auth contract
🔮 [WASM] ⚙️ [VM] [25:3] Auth: Verifying signature
🔮 [WASM] ⚙️ [VM] [25:3] Auth: Signature valid
🔮 [WASM] ⚙️ [VM] [2:0] Token: Allowance confirmed
🔮 [WASM] ⚙️ [VM] [2:0] Token: Transfer executed
🔮 [WASM] ⚙️ [VM] [50:1] Wallet: Payment completed
```

### 3. **Identify Problem Contracts**

Quickly spot which contract is causing errors:

```
🔮 [WASM] ⚙️ [VM] [100:5] Starting complex operation
🔮 [WASM] ⚙️ [VM] [100:5] Step 1 complete
🔮 [WASM] ⚙️ [VM] [100:5] Calling helper contract
🔮 [WASM] ⚙️ [VM] [200:10] Helper: Processing request
🔮 [WASM] ❌ [ERROR]
🔮 [WASM] Transaction Error
🔮 [WASM] ├─ Txid: a00fdccfe...
🔮 [WASM] ├─ Target: [200:10]  ← Problem is here!
🔮 [WASM] └─ Error: Division by zero
```

## Benefits

1. **🔍 Instant Debugging**: Immediately see which contract is logging
2. **📊 Execution Flow**: Visualize the call graph between contracts
3. **🎯 Problem Isolation**: Quickly identify problematic contracts
4. **📈 Performance Analysis**: Track which contracts are most verbose
5. **🧪 Testing**: Validate contract interactions during development
6. **📝 Audit Trail**: Complete history of contract execution
7. **🔗 Call Tracing**: Follow nested calls across multiple contracts

## Technical Details

### Data Source

The alkane ID comes from `AlkanesRuntimeContext`:

```rust
pub struct AlkanesRuntimeContext {
    pub myself: AlkaneId,     // ← The contract that's currently executing
    pub caller: AlkaneId,     // ← The contract that called this one
    pub incoming_alkanes: AlkaneTransferParcel,
    pub returndata: Vec<u8>,
    pub inputs: Vec<u128>,
    pub message: Box<MessageContextParcel>,
    pub trace: Trace,
}
```

The `myself` field always contains the ID of the currently executing contract, making it perfect for log attribution.

### Log Formatting Rules

1. **Single-line messages**: `⚙️ [VM] [m:n] <message>`
2. **Multi-line messages**: Tree structure with `[m:n]` as root
3. **All lines**: Wrapped with `🔮 [WASM]` prefix via WASM interceptor

## Compilation Status

✅ Builds successfully:
- `cargo build --release -p alkanes --target wasm32-unknown-unknown` ✅

## Future Enhancements

- Add call depth visualization (indentation based on call stack depth)
- Add execution time per contract
- Add fuel consumption per contract
- Color-code different contracts for easier visual distinction
- Support for contract name resolution (show name instead of just ID)
