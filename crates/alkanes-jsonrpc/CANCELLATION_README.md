# Lua Execution Cancellation and Timeout

This document describes the cancellation and timeout mechanisms implemented for Lua script execution in alkanes-jsonrpc.

## Overview

Long-running Lua scripts are now protected by two cancellation mechanisms:

1. **Connection-based cancellation**: When the HTTP connection closes, Lua execution is immediately cancelled
2. **Timeout-based cancellation**: After 5 minutes, Lua execution is forcibly terminated

## Implementation Details

### 1. Lua VM Debug Hooks

The implementation uses Lua's debug hook system to periodically check for cancellation and timeout:

- A hook function is installed that runs every **10,000 Lua VM instructions**
- The hook checks two conditions:
  - Whether the `CancellationToken` has been cancelled (connection closed)
  - Whether the execution time has exceeded 5 minutes
- If either condition is true, the hook returns an error, which immediately terminates Lua execution
- This works for **all Lua code**, including pure synchronous infinite loops

**Key advantage**: The Lua VM itself interrupts execution at the instruction level, so even tight infinite loops like `while true do end` will be terminated.

### 2. Connection Drop Detection

When an HTTP request is received:
- A `CancellationToken` is created for the request
- A `CancellationGuard` holds the token and is tied to the request handler's scope
- When the HTTP handler exits (normally or due to connection drop), the guard is dropped
- Dropping the guard triggers the cancellation token
- The Lua executor checks this token and aborts execution

**Code path:**
```
main.rs::handle_jsonrpc()
  → Creates CancellationGuard
  → Passes CancellationToken to handler
  → handler.rs::handle_request_with_storage()
    → Passes token to sandshrew::handle_sandshrew_method()
      → Passes token to lua_executor::execute_lua_script()
        → Uses tokio::select! to race execution vs cancellation
```

### 3. Timeout Protection (5 minutes)

The `execute_lua_script()` function uses `tokio::select!` to race three conditions:
- Lua script completes normally
- Cancellation token is triggered (connection closed)
- 5-minute timeout expires

Whichever completes first wins, and the other branches are cancelled.

**Timeout behavior:**
- After 5 minutes, `tokio::time::sleep(LUA_EXECUTION_TIMEOUT)` completes
- The `tokio::select!` macro drops the Lua execution future
- An error is returned: "Lua execution timeout: exceeded 5 minute limit"
- A warning is logged with the timeout duration

### 4. RPC Call Cancellation

All RPC calls made from Lua (via `_RPC.*` methods) check the cancellation token before executing:

```rust
async fn call_rpc(&self, method: &str, params: Vec<Value>) -> Result<Value> {
    // Check if cancelled before making RPC call
    if self.cancel_token.is_cancelled() {
        return Err(anyhow!("Lua execution cancelled: connection closed"));
    }
    // ... proceed with RPC call
}
```

This ensures that even if a Lua script is making many RPC calls, it will stop promptly when cancelled.

## How It Works: Connection Close

1. Client makes HTTP POST request with `lua_evalscript` method
2. Server creates `CancellationToken` and `CancellationGuard`
3. Lua script starts executing
4. **Client disconnects** (closes connection)
5. HTTP handler function is dropped
6. `CancellationGuard` destructor runs, calling `token.cancel()`
7. Two things happen simultaneously:
   - `tokio::select!` detects cancellation and exits
   - Any pending RPC calls check `is_cancelled()` and abort
8. Server returns error immediately

**Timeline:**
```
T+0ms:    Request received, Lua starts
T+1000ms: Client disconnects
T+1001ms: Guard dropped, token cancelled
T+1002ms: Lua execution aborted
```

## How It Works: Timeout

1. Lua script starts executing with complex computation or infinite loop
2. `tokio::select!` races three futures:
   - `lua.load(script).eval_async().await` (the Lua execution)
   - `cancel_token.cancelled()` (connection close detection)
   - `tokio::time::sleep(LUA_EXECUTION_TIMEOUT)` (5 minute timer)
3. After 5 minutes, the sleep completes first
4. `tokio::select!` drops the other futures
5. Server returns error: "Lua execution timeout: exceeded 5 minute limit"

**Timeline:**
```
T+0s:     Request received, Lua starts
T+30s:    Still executing...
T+300s:   Still executing (5 minutes elapsed)
T+300s:   Timeout fires, execution aborted
T+300s:   Error returned to client (if still connected)
```

## How Interruption Works for Different Lua Code Patterns

### Pure Synchronous Infinite Loops

**Even tight infinite loops are interruptible** thanks to the Lua VM hook mechanism:

```lua
-- This WILL be interrupted within milliseconds
while true do
    local x = 1 + 1
end
```

**Why it works:**
- Every 10,000 Lua instructions, the VM calls our hook function
- Even a simple loop like `x = 1 + 1` consumes multiple VM instructions
- After ~10,000 iterations, the hook fires and checks for cancellation
- If cancelled or timed out, the hook returns an error that terminates execution

**Interruption latency**: Typically 1-10 milliseconds for most code, depending on how fast the VM reaches 10,000 instructions.

### Lua Code with RPC Calls

RPC calls provide additional cancellation checkpoints:

```lua
while true do
    local result = _RPC.btc_getblockcount()  -- Checks cancellation BEFORE RPC call
    -- If cancelled, this RPC call will abort immediately
end
```

This code has **two** cancellation mechanisms:
1. VM hook every 10,000 instructions
2. Explicit check before each RPC call

### Long-Running Computations

```lua
local sum = 0
for i = 1, 10000000000 do
    sum = sum + i  -- Will be interrupted at VM instruction boundaries
end
```

This will be interrupted by the hook mechanism after ~10,000 VM instructions, not after 10,000 loop iterations.

## Error Messages

| Scenario | Error Message |
|----------|--------------|
| Connection closed | `Lua execution cancelled: connection closed` |
| 5-minute timeout | `Lua execution timeout: exceeded 5 minute limit` |
| Lua script error | `Lua execution error: <error details>` |
| RPC call after cancel | `Lua execution cancelled: connection closed` |

## Configuration

Two constants control the cancellation behavior in `lua_executor.rs`:

```rust
/// Maximum allowed execution time for Lua scripts (5 minutes)
const LUA_EXECUTION_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Lua VM hook check interval (every 10,000 instructions)
const LUA_HOOK_INSTRUCTION_COUNT: u32 = 10_000;
```

**Timeout duration**: Change `LUA_EXECUTION_TIMEOUT` to adjust the maximum script execution time.

**Hook frequency**: Change `LUA_HOOK_INSTRUCTION_COUNT` to adjust interruption responsiveness:
- **Lower values** (e.g., 1,000): More responsive cancellation, but higher CPU overhead
- **Higher values** (e.g., 100,000): Less CPU overhead, but slower cancellation response
- **Recommended**: 10,000 provides good balance between responsiveness and performance

## Testing

To test connection drop cancellation:
1. Start the server
2. Send a long-running Lua script request
3. Close the HTTP connection before it completes
4. Check server logs for "Request cancelled due to connection drop or completion"

To test timeout:
1. Send a Lua script with an infinite loop or very long computation
2. Wait 5 minutes
3. Server should return timeout error
4. Check logs for "Lua execution timeout after 300 seconds"

## Benefits

✅ **Complete interruption**: Even infinite loops are terminated via VM hooks  
✅ **Fast response**: Cancellation happens within 1-10ms in most cases  
✅ **Resource protection**: Server won't be consumed by runaway scripts  
✅ **Connection awareness**: Disconnected clients don't waste server resources  
✅ **Multi-layered**: Three cancellation mechanisms (hooks, RPC checks, timeout)  
✅ **Clear error messages**: Users know exactly why their script was terminated  
✅ **Configurable**: Both timeout and hook frequency can be tuned  
✅ **Low overhead**: 10,000 instruction intervals have minimal performance impact  

## Future Improvements

Potential enhancements:
- Per-user or per-script timeout configuration
- Dynamic hook frequency based on script complexity
- Metrics/monitoring for timeout and cancellation frequency
- Graceful warnings before timeout (e.g., warning at 4 minutes)
- Script complexity analysis to auto-tune hook frequency
