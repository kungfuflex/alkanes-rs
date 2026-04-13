# Alkanes Protocol Specification v2.1.8

**Audit Reference Document**

This document describes the architecture, data flow, state transitions, and security
model of the alkanes-rs v2.1.8 codebase. All file paths are relative to the repository
root. Line references use the format `file:line`.

---

## 1. Overview

Alkanes is a Bitcoin metaprotocol that enables smart contracts on top of the Runes
fungible-token standard. It operates as a **deterministic WASM indexer** that processes
Bitcoin blocks and maintains an off-chain state database derived entirely from on-chain
data.

### 1.1 Protocol Stack

```
Bitcoin Transactions
       |
   Runes Layer          (OP_RETURN Runestones: etch, mint, transfer)
       |
   Protorunes Layer     (protocol field extension: protostones, protoburns)
       |
   Alkanes Layer        (WASM smart contracts: deploy, execute, inter-contract calls)
```

### 1.2 Design Principles

- **Deterministic**: Given the same chain of Bitcoin blocks, every indexer produces
  identical state. No external data sources.
- **Layered**: Each layer processes its own concerns, passing residual balances upward.
- **UTXO-native**: Token balances are tracked per Bitcoin outpoint, inheriting the
  UTXO spend model.
- **Sandboxed execution**: Smart contracts run in a WASM VM with fuel metering,
  memory limits, and no I/O.

---

## 2. Architecture

### 2.1 Crate Structure

| Crate | Path | Purpose | Key Dependencies |
|-------|------|---------|-----------------|
| `alkanes` | `.` (root) | Top-level indexer: block processing, WASM VM, views | all below |
| `ordinals` | `crates/ordinals` | Rune/Runestone parsing, etching, minting, edicts | bitcoin |
| `protorune` | `crates/protorune` | Protorune indexer: protostone processing, protoburns, balance persistence | ordinals, protorune-support |
| `protorune-support` | `crates/protorune-support` | Shared types: BalanceSheet, Protostone, ProtoruneRuneId, protobuf definitions | ordinals |
| `alkanes-support` | `crates/alkanes-support` | Alkane-specific types: Cellpack, AlkaneId, CallResponse, StorageMap | protorune-support |
| `alkanes-runtime` | `crates/alkanes-runtime` | SDK for writing alkane smart contracts (guest-side) | alkanes-support |
| `alkanes-macros` | `crates/alkanes-macros` | Procedural macros for the alkane SDK | - |
| `alkanes-std-factory-support` | `crates/alkanes-std-factory-support` | Factory pattern support library | alkanes-support |

### 2.2 Data Flow: Block Processing Pipeline

Entry point: `src/indexer.rs:92` (`index_block`)

```
index_block(block, height)
  1. configure_network()                          -- set bech32 prefix, p2pkh/p2sh bytes
  2. clear_diesel_mints_cache()                   -- reset per-block precompile cache
  3. is_genesis(height) -> genesis()              -- one-time bootstrap (sequence=1, genesis outpoint)
  4. setup_diesel(block)                          -- deploy genesis alkane [2,0] if absent
  5. setup_frbtc(block)                           -- deploy fr-BTC contract [32,0] if absent
  6. setup_frsigil(block)                         -- deploy fr-Sigil contract [32,1] if absent
  7. check_and_upgrade_precompiled(height)         -- upgrade genesis alkane at threshold heights
  8. FuelTank::initialize(block, height)           -- compute total block fuel budget
  9. Protorune::index_block::<AlkaneMessageContext>(block, height)
     -- Process every transaction:
        a. Index outpoints and addresses
        b. Parse Runestones from OP_RETURN
        c. Process etchings, mints, edicts (Runes layer)
        d. Extract protostones from Runestone.protocol field
        e. Process protoburns (cross-protocol bridging)
        f. Process protostone edicts (within-protocol transfers)
        g. For each protostone with a message: invoke AlkaneMessageContext::handle()
        h. Persist balance sheets to outpoints
  10. unwrap::update_last_block(height)            -- advance fr-BTC unwrap tracking
  11. (optional, feature=cache) Cache wallet responses
```

### 2.3 Storage Model

The indexer uses MetaShrew's key-value store, accessed through the `IndexPointer`
abstraction. An `IndexPointer` is a composable key-path builder:

```rust
IndexPointer::from_keyword("/alkanes/")
    .select(&alkane_id_bytes)      // append binary key segment
    .keyword("/storage/")          // append string key segment
    .select(&storage_key)
```

**AtomicPointer** wraps IndexPointer with a checkpoint/commit/rollback stack,
enabling nested transactional writes. This is critical for the WASM execution model
where contract calls can revert.

Key operations:
- `checkpoint()` -- push a savepoint onto the stack
- `commit()` -- merge the current savepoint into the parent
- `rollback()` -- discard changes since the last checkpoint
- `derive()` -- create a child AtomicPointer sharing the same checkpoint stack

---

## 3. Protocol Layers

### 3.1 Runes Layer

**Source**: `crates/ordinals/src/runestone.rs`, `crates/protorune/src/lib.rs`

#### 3.1.1 Runestone Parsing

A Runestone is decoded from a Bitcoin transaction's OP_RETURN output
(`crates/ordinals/src/runestone.rs:33`, `Runestone::decipher`):

1. Find the OP_RETURN output containing the magic number `OP_PUSHNUM_13` (or `"D"` on
   Dogecoin).
2. Extract the payload bytes from subsequent data pushes.
3. Decode LEB128 varints into a `Message` containing `fields` (tag-value pairs) and
   `edicts`.
4. Parse fields into an `Etching`, `mint` RuneId, `pointer`, and `protocol` (protostone
   data).
5. If any step fails with a fatal flaw, produce a `Cenotaph` instead.

The `Runestone` struct (`crates/ordinals/src/runestone.rs:8`):
```
edicts: Vec<Edict>           -- balance transfer instructions
etching: Option<Etching>     -- new rune definition
mint: Option<RuneId>         -- rune to mint
pointer: Option<u32>         -- default output for unallocated balances
protocol: Option<Vec<u128>>  -- protostone extension data
```

#### 3.1.2 Etching

An etching creates a new rune. Validation (`crates/protorune/src/lib.rs:163-199`):

- **Name validation**: The rune name must not be reserved (below the unlock threshold
  for the current height). Names unlock over time via `constants::RESERVED_NAME`
  computation.
- **Commitment proof**: A commitment to the rune name must appear in a tapscript
  witness of one of the transaction inputs, and the input being spent must have at
  least `COMMIT_CONFIRMATIONS` (6) confirmations. In test mode, this check is bypassed.
- **Uniqueness**: The etching name must not already exist in the `ETCHINGS` table.

Persisted fields per etching: symbol, spacers, divisibility, premine, cap, amount,
height/offset start/end (mint terms).

#### 3.1.3 Minting

When a Runestone references a `mint` RuneId (`crates/protorune/src/lib.rs`, within
`index_runestone`):

1. Look up the rune's mint terms: cap, amount, height window, offset window.
2. Validate that the current height falls within the allowed mint window.
3. Check remaining mints (`MINTS_REMAINING`). If > 0, decrement and add `amount`
   to the unallocated balance pool.
4. The minted amount flows to the `pointer` output or default output.

#### 3.1.4 Edicts

Edicts are balance transfer instructions within a transaction
(`crates/protorune/src/lib.rs:89-160`, `handle_transfer_runes_to_vout`):

- Each edict specifies `(RuneId, amount, output)`.
- If `output == tx.output.len()`, the amount is spread across all non-OP_RETURN outputs.
- If `amount == 0`, transfer the entire remaining balance of that rune.
- Edicts are processed in order; each consumes from the running unallocated pool.

#### 3.1.5 Cenotaphs

A Cenotaph is a malformed Runestone. When detected:
- All rune inputs are **burned** (balances saved with `is_cenotaph = true`, which skips
  persistence of non-zero balances).
- Etchings in a cenotaph are still recorded but the rune is effectively stillborn.
- The `Flaw` enum in `crates/ordinals/src/flaw.rs` catalogs all recognized malformation
  types (e.g., `Varint`, `EdictOutput`, `EdictRuneId`, etc.).

#### 3.1.6 Pointer / Default Output

The Runestone `pointer` field designates which output receives unallocated balances
(mints, leftover after edicts). If absent, the first non-OP_RETURN output is used
(`crates/protorune/src/lib.rs:56-63`, `default_output`).

### 3.2 Protorunes Layer

**Source**: `crates/protorune/src/protostone.rs`, `crates/protorune/src/protoburn.rs`,
`crates/protorune/src/message.rs`

#### 3.2.1 Protocol Tag Isolation

Each sub-protocol is identified by a `protocol_tag` (u128). Alkanes uses tag `1`
(`src/message.rs:199`). Storage tables are namespaced per protocol via
`RuneTable::for_protocol(tag)` (`crates/protorune/src/tables.rs:87-133`).

#### 3.2.2 Protostone Extraction

Protostones are encoded in the Runestone's `protocol` field as a packed `Vec<u128>`.

Decoding (`crates/protorune-support/src/protostone.rs:301-329`, `Protostone::decipher`):
1. Join the u128 values into a byte stream, taking only 15 of each 16 bytes (the 16th
   byte is always zero padding).
2. LEB128-decode the byte stream into integers.
3. Parse as `[protocol_tag, length, ...fields]` tuples.
4. Each tuple becomes a `Protostone` with fields parsed from tag-value pairs.

The `Protostone` struct (`crates/protorune-support/src/protostone.rs:148-156`):
```
burn: Option<u128>              -- if Some, this is a protoburn (bridge from Runes)
message: Vec<u8>                -- calldata for the sub-protocol handler
edicts: Vec<ProtostoneEdict>    -- sub-protocol balance transfers
refund: Option<u32>             -- output for refunding on failure
pointer: Option<u32>            -- output for successful results
from: Option<u32>               -- edict index selector for protoburns
protocol_tag: u128              -- which sub-protocol this targets
```

#### 3.2.3 Protoburns

A protoburn bridges rune balances from the Runes layer into a sub-protocol layer
(`crates/protorune/src/protoburn.rs`).

Process (`crates/protorune/src/protoburn.rs:30-68`):
1. Identify which runestone edicts target the OP_RETURN output (the "burn" output).
2. Cycle edicts across protoburns using `BurnCycle` (round-robin assignment).
3. If a protoburn has a `from` field, only pull from those specific edict indices.
4. Copy rune metadata (name, symbol, spacers, divisibility) from the Runes table to
   the protocol-specific table.
5. Pipe the burned balances into the protocol's balance tracking for the designated
   output.

#### 3.2.4 MessageContext Trait

The `MessageContext` trait (`crates/protorune/src/message.rs:11-23`) defines the
interface each sub-protocol must implement:

```rust
trait MessageContext {
    fn handle(parcel: &MessageContextParcel) -> Result<(Vec<RuneTransfer>, BalanceSheet)>;
    fn protocol_tag() -> u128;
    fn asset_protoburned_in_protocol(id: ProtoruneRuneId) -> bool;  // default impl
}
```

The `asset_protoburned_in_protocol` method has a default implementation that checks
whether a rune ID has an etching entry in the protocol's `RUNE_ID_TO_ETCHING` table,
returning `true` if the rune was bridged via protoburn.

The `MessageContextParcel` (`crates/protorune/src/message.rs:26-39`) carries all
context needed to process a message:
- `atomic`: transactional pointer for state changes
- `runes`: incoming token balances (from protostone edict targeting this virtual output)
- `transaction`, `block`, `height`: Bitcoin context
- `pointer`, `refund_pointer`: output indices for success/failure
- `calldata`: raw bytes from the protostone `message` field
- `sheets`: the balance sheet at this protostone's virtual output (boxed)
- `txindex`: transaction index within the block
- `vout`: the virtual output index of this protostone
- `runtime_balances`: accumulated runtime balance sheet (at `u32::MAX` virtual output)

#### 3.2.5 Virtual Outputs (Shadow Vouts)

Protostones occupy virtual output indices beyond the transaction's real outputs.
For a transaction with N outputs, the first protostone is at vout N, the second
at N+1, etc. Protostone edicts and protoburns can target these virtual vouts to
direct balances into specific protostones before their messages are processed.

#### 3.2.6 Balance Sheet Persistence

After processing all protostones for a transaction, the final `balances_by_output`
map is persisted (`crates/protorune/src/balance_sheet.rs:18-36`, `PersistentRecord::save`):

For each output vout, the balance sheet is saved under:
```
/runes/proto/{tag}/byoutpoint/{outpoint_bytes}/runes     -- list of rune IDs
/runes/proto/{tag}/byoutpoint/{outpoint_bytes}/balances   -- list of u128 amounts
/runes/proto/{tag}/byoutpoint/{outpoint_bytes}/id_to_balance/{rune_bytes} -- indexed lookup
```

### 3.3 Alkanes Layer

**Source**: `src/message.rs`, `src/vm/`, `src/utils.rs`

#### 3.3.1 AlkaneMessageContext

`AlkaneMessageContext` (`src/message.rs:36`) implements `MessageContext` with
`protocol_tag() = 1`. Its `handle()` method:

1. Checks `is_active(height)` -- the sub-protocol is inactive before `GENESIS_BLOCK`.
2. Delegates to `handle_message(parcel)` (`src/message.rs:40-196`).

#### 3.3.2 Message Handling Flow

`handle_message` (`src/message.rs:40`):

1. **Decode cellpack**: Parse `parcel.calldata` as a varint list, then convert to a
   `Cellpack` (target AlkaneId + inputs).
2. **Create runtime context**: Build `AlkanesRuntimeContext` from parcel + cellpack.
3. **Derive atomic pointer**: Create a child AtomicPointer for this execution scope.
4. **Resolve target** (`run_special_cellpacks`): Handle deployment operations (CREATE,
   CREATERESERVED, FACTORY) or look up existing contract binary.
5. **Credit incoming balances**: Add the incoming runes to the target contract's
   balance (`credit_balances`).
6. **Prepare context**: Set caller and myself on the runtime context.
7. **Fuel allocation**: If this is the first call in the transaction, allocate fuel
   from the block's FuelTank.
8. **Execute WASM**: `run_after_special()` instantiates the WASM VM and calls
   `__execute`.
9. **On success**:
   - Consume fuel from the FuelTank.
   - Persist storage map changes.
   - Reconcile balance sheets (debit outgoing, validate mintable).
   - Save execution trace.
   - Return outgoing alkanes and runtime balance sheet.
10. **On failure**:
    - Drain all remaining transaction fuel.
    - Save revert trace with error data (prefixed `0x08c379a0`).
    - Propagate error (causes rollback at the protostone level).

#### 3.3.3 AlkaneId

`AlkaneId` (`crates/alkanes-support/src/id.rs:7-10`) is a pair `(block: u128, tx: u128)`.
The `block` field determines the ID's semantic meaning:

| block | Meaning | tx field |
|-------|---------|----------|
| 0 | Non-contract (null caller) | 0 |
| 1 | CREATE deployment | 0 (fixed) |
| 2 | Sequentially-assigned contract | Sequence number |
| 3 | CREATERESERVED deployment | Reserved number |
| 4 | Reserved-ID contract | Reserved number |
| 5 | Factory deployment (from seq. contract) | Parent's sequence number |
| 6 | Factory deployment (from reserved contract) | Parent's reserved number |
| 32 | System precompiled contracts | 0=fr-BTC, 1=fr-Sigil |
| 800000000 | Virtual precompiled contracts | 0=block_header, 1=coinbase_tx, 2=diesel_mints, 3=miner_fee |

Method summary (`crates/alkanes-support/src/id.rs:57-91`):
- `is_created(next_sequence)` -- true if the contract already exists
- `is_create()` -- block==1, tx==0
- `is_deployment()` -- block in {1, 3, 5, 6}
- `reserved()` -- returns Some(tx) if block==3
- `factory()` -- returns the parent AlkaneId if block==5 or block==6

#### 3.3.4 Cellpack Format

A `Cellpack` (`crates/alkanes-support/src/cellpack.rs:8-11`) is the call data encoding:

```
Cellpack {
    target: AlkaneId,     // (block, tx) -- the contract to call
    inputs: Vec<u128>,    // opcode + arguments
}
```

Encoding: LEB128-encoded list of u128 values: `[target.block, target.tx, input0, input1, ...]`

The first input (`inputs[0]`) is conventionally the **opcode** that the contract
dispatches on. Remaining inputs are arguments.

#### 3.3.5 Contract Deployment

Deployment is handled in `run_special_cellpacks` (`src/vm/utils.rs:93-187`):

**CREATE** (target = `[1, 0]`):
1. Extract WASM binary from the transaction's witness data (envelope in first input).
2. Assign the next sequence number: `[2, next_sequence]`.
3. Store the gzip-compressed binary at `/alkanes/{id_bytes}`.
4. Increment the sequence counter.
5. Map alkane ID to creation outpoint.

**CREATERESERVED** (target = `[3, N]`):
1. Extract WASM binary from witness.
2. Target becomes `[4, N]`.
3. Fail if `[4, N]` already has a binary stored.
4. Store binary and map to outpoint.

**FACTORY** (target = `[5, N]` or `[6, N]`):
1. Target becomes `[2, next_sequence]`.
2. Store a 32-byte pointer (the factory's AlkaneId bytes) instead of actual WASM.
3. When loaded, the pointer is followed to retrieve the factory's binary.
4. The new contract shares the factory's code but has its own storage and balances.

#### 3.3.6 WASM VM Execution Model

**VM Instantiation** (`src/vm/instance.rs:70-349`):

1. Create a `wasmi` engine with fuel consumption enabled.
2. Create a `Store<AlkanesState>` with:
   - `MEMORY_LIMIT = 43,554,432 bytes` (~41.5 MB) (`src/vm/constants.rs:1`)
   - Initial fuel set to `start_fuel`
3. Compile the WASM module.
4. Link all host functions (see Section 3.3.7).
5. Grow memory to minimum 512 pages (32 MB) if needed.
6. Call `__execute` export.

**Execution flow** (`src/vm/instance.rs:354-392`):
1. `checkpoint()` the atomic pointer.
2. Call `__execute` via wasmi.
3. If the export returns successfully and `had_failure` is false: `commit()`.
4. If failure: `rollback()` and return an error with revert data.

Revert data format: first 4 bytes `[0x08, 0xc3, 0x79, 0xa0]` (Solidity-style error
selector), followed by UTF-8 error message.

#### 3.3.7 Host Functions (Syscalls)

All host functions are linked into the WASM module's `"env"` namespace
(`src/vm/instance.rs:98-331`). The `SafeAlkanesHostFunctionsImpl` wrapper ensures
checkpoint depth is preserved across each call.

| Import Name | Signature | Fuel Cost | Description |
|------------|-----------|-----------|-------------|
| `abort` | `(i32, i32, i32, i32)` | 0 | Set failure flag |
| `__request_storage` | `(i32) -> i32` | `bytes * FUEL_PER_REQUEST_BYTE(1)` | Get storage value size |
| `__load_storage` | `(i32, i32) -> i32` | `bytes * FUEL_PER_LOAD_BYTE(2)` | Read storage value |
| `__request_context` | `() -> i32` | `bytes * FUEL_PER_REQUEST_BYTE(1)` | Get serialized context size |
| `__load_context` | `(i32) -> i32` | `bytes * FUEL_PER_LOAD_BYTE(2)` | Read serialized context |
| `__request_transaction` | `() -> i32` | `min(50, FUEL_LOAD_TRANSACTION/10)` | Get transaction size |
| `__load_transaction` | `(i32)` | `FUEL_LOAD_TRANSACTION(500)` | Read serialized transaction |
| `__request_block` | `() -> i32` | `min(100, FUEL_LOAD_BLOCK/10)` | Get block size |
| `__load_block` | `(i32)` | `FUEL_LOAD_BLOCK(1000)` | Read serialized block |
| `__sequence` | `(i32)` | `FUEL_SEQUENCE(5)` | Read global sequence counter |
| `__fuel` | `(i32)` | `FUEL_FUEL(5)` | Read remaining fuel |
| `__height` | `(i32)` | `FUEL_HEIGHT(10)` | Read current block height |
| `__balance` | `(i32, i32, i32)` | `FUEL_BALANCE(10)` | Query balance of (who, what) |
| `__returndatacopy` | `(i32)` | `bytes * FUEL_PER_LOAD_BYTE(2)` | Copy return data from last extcall |
| `__log` | `(i32)` | 0 | Debug log output |
| `__call` | `(i32, i32, i32, u64) -> i32` | See extcall fuel | Call another contract |
| `__delegatecall` | `(i32, i32, i32, u64) -> i32` | See extcall fuel | Delegatecall (retain caller/myself) |
| `__staticcall` | `(i32, i32, i32, u64) -> i32` | See extcall fuel | Staticcall (rollback state changes) |

**Context serialization format** (returned by `__load_context`):
Flat array of u128 LE values:
```
[myself.block, myself.tx, caller.block, caller.tx, vout,
 incoming_alkanes_count,
 incoming[0].id.block, incoming[0].id.tx, incoming[0].value,
 ...,
 input[0], input[1], ...]
```

#### 3.3.8 External Calls (Extcall)

Three call types (`src/vm/extcall.rs`):

| Type | `isdelegate` | `isstatic` | Context Change | State Handling |
|------|-------------|-----------|----------------|----------------|
| `Call` | false | false | caller=myself, myself=target | commit on success |
| `Delegatecall` | true | false | caller=caller, myself=myself (unchanged) | commit on success |
| `Staticcall` | false | false | caller=myself, myself=target | always rollback |

Extcall processing (`src/vm/host_functions.rs:530-557` `handle_extcall`,
`src/vm/host_functions.rs:723-865` `extcall`):

1. **Recursion guard**: Checkpoint depth must be < 75 (`host_functions.rs:493`).
2. **Parse inputs**: Read cellpack, incoming alkanes, and storage map from WASM memory.
3. **Deployment fuel**: If target is a deployment, charge `fuel_extcall_deploy`.
4. **Precompiled check**: If `target.block == 800_000_000`, handle as a virtual
   precompile (no WASM execution).
5. **Checkpoint**: Push a savepoint on the atomic pointer.
6. **Transfer balances**: For non-delegate calls, transfer incoming alkanes from
   caller to target contract.
7. **Create subcontext**: Clone the runtime context with updated caller/myself.
8. **Compute fuel**: `FUEL_EXTCALL(500) + storage_map_bytes * fuel_per_store_byte`.
9. **Execute**: Instantiate a new WASM VM for the target with the caller's remaining fuel.
10. **On success**: Deduct consumed fuel, save storage/balances, commit atomic pointer.
11. **On failure**: Rollback atomic pointer, store error in returndata.

Return value: positive = success (returndata length), negative = failure (negated
returndata length).

**Child revert isolation**: When a nested extcall reverts, the child's state changes
are rolled back via checkpoint, all allocated child fuel is consumed, and revert data
is stored in `returndata`. Crucially, the child's revert does **not** call `_abort()`
on the parent frame -- the parent continues executing and can inspect the negative
return value to handle the failure. This matches EVM semantics where a CALL that
reverts returns 0 to the caller without aborting the caller's execution.

#### 3.3.9 Precompiled Contracts

Virtual precompiles at `AlkaneId(800_000_000, tx)` (`src/vm/host_functions.rs:680-722`):

| tx | Function | Returns |
|----|----------|---------|
| 0 | `_get_block_header` | 80-byte Bitcoin block header |
| 1 | `_get_coinbase_tx_response` | Serialized coinbase transaction |
| 2 | `_get_number_diesel_mints` | Count of diesel mint cellpacks in the current block |
| 3 | `_get_total_miner_fee` | Sum of coinbase output values (u128 LE) |

The diesel mints count is cached per-block in `DIESEL_MINTS_CACHE` to avoid
recomputation.

#### 3.3.10 Fuel / Gas Metering

**Source**: `src/vm/fuel.rs`

Fuel operates at two levels:

**Block-level FuelTank** (`src/vm/fuel.rs:151-316`):
- `total_fuel(height)`: Total fuel budget per block. Pre-CHANGE1: 100M (mainnet).
  Post-CHANGE1: 1B (mainnet).
- `FUEL_CHANGE1_HEIGHT`: Mainnet=899,087, regtest=0.
- Each transaction receives a proportional share of block fuel based on its virtual
  size (`vfsize`).
- `minimum_fuel(height)`: Floor per transaction. Pre-CHANGE1: 350K. Post-CHANGE1: 3.5M.
- Unused fuel from a transaction is returned to the block pool via `refuel_block()`.
- On failure (`drain_fuel()`), all allocated fuel is consumed (not returned to pool).

**WASM-level fuel** (wasmi fuel counter):
- The wasmi store's fuel counter tracks per-instruction consumption.
- Host function calls charge additional fuel via `consume_fuel()`.
- Storage writes are charged per byte: `fuel_per_store_byte` (8 pre-CHANGE1,
  40 post-CHANGE1).

**Version-gated fuel enforcement** (`src/vm/fuel.rs:335-345`):
The `V217_FIX_HEIGHT` constant gates when fuel enforcement behavior is applied.
Before this height, fuel consumption calls are permitted without strict enforcement.
At and above this height, fuel is strictly enforced via `consume_fuel()`.

| Network | V217_FIX_HEIGHT |
|---------|----------------|
| Mainnet | 943,500 |
| Regtest | 0 |
| Dogecoin | 0 |
| Fractal | 0 |
| Luckycoin | 0 |
| Bellscoin | 0 |

This height also gates the `handle_transfer_runes_to_vout` edict spread behavior
(`crates/protorune/src/lib.rs:119`): pre-fix, OP_RETURN outputs could incorrectly
consume from the spread amount; post-fix, OP_RETURN outputs are skipped before
computing per-output amounts.

**Fuel cost constants** (`src/vm/fuel.rs:347-374`):
```
FUEL_PER_REQUEST_BYTE     = 1
FUEL_PER_LOAD_BYTE        = 2
FUEL_PER_STORE_BYTE_START = 8    (pre-CHANGE1)
FUEL_PER_STORE_BYTE_CHANGE1 = 40 (post-CHANGE1)
FUEL_SEQUENCE             = 5
FUEL_FUEL                 = 5
FUEL_EXTCALL              = 500
FUEL_HEIGHT               = 10
FUEL_BALANCE              = 10
FUEL_EXTCALL_DEPLOY_START = 10,000  (pre-CHANGE1)
FUEL_EXTCALL_DEPLOY_CHANGE1 = 100,000 (post-CHANGE1)
FUEL_LOAD_BLOCK           = 1,000
FUEL_LOAD_TRANSACTION     = 500
```

**Virtual fuel sizing** (`src/vm/fuel.rs:23-75`, `VirtualFuelBytes`):
Transactions containing deployment cellpacks (`[1,0]` or `block==3`) strip the
first witness before computing vsize, so deployments don't unfairly consume the
block's fuel budget due to large WASM binaries in the witness.

#### 3.3.11 Balance Operations

**Source**: `src/utils.rs`

Balance storage path:
```
/alkanes/{what_id_bytes}/balances/{who_id_bytes} -> u128 (LE)
```

Where `what` is the token being held and `who` is the holder (contract).

**credit_balances** (`src/utils.rs:66-80`):
Adds rune balances to a contract. Uses `checked_add` to prevent overflow.

**debit_balances** (`src/utils.rs:105-116`):
Subtracts rune balances from a contract after execution. Calls
`checked_debit_with_minting`.

**checked_debit_with_minting** (`src/utils.rs:83-103`):
If a contract's balance is insufficient to cover a transfer:
- If the token being transferred IS the contract itself (`transfer.id == from`),
  allow it (self-minting).
- Otherwise, return a balance underflow error.

This design intentionally allows contracts to mint their own token without limit.
It is the contract author's responsibility to implement supply control.

**transfer_from** (`src/utils.rs:118-139`):
Transfers balances between two contracts (used during extcalls). Skips if the
recipient is the null contract `[0, 0]`.

**Inventory tracking** (`src/utils.rs:44-49`):
```
/alkanes/{who_id_bytes}/inventory/ -> list of held token IDs
```
Append-only list updated during `balance_pointer` when a non-zero balance is first
detected.

#### 3.3.12 Unwrap Subsystem

**Source**: `src/unwrap.rs`

The unwrap subsystem tracks fr-BTC (AlkaneId `[32, 0]`) unwrap payments -- requests
to convert fr-BTC back to native Bitcoin.

**Storage pointers**:
- `/alkanes/{fr_btc}/storage/fulfilled` -- fulfilled payment records
- `/alkanes/{fr_btc}/storage/premium` -- unwrap premium (u128 LE)
- `/__INTERNAL/pending_unwraps` -- precomputed pending payments cache
- `/__INTERNAL/pending_unwraps_initialized` -- cache initialization flag
- `/__INTERNAL/pending_unwraps_height` -- last height the cache was updated through

**Pending cache** (`src/unwrap.rs:38-290`):
A precomputed cache of unfulfilled payments is maintained for fast view responses.
The cache is rebuilt from scratch on initialization, then incrementally updated on
each block. Cache pruning runs every 10 blocks to avoid expensive full-list rewrites.

**`update_last_block(height)`** (`src/unwrap.rs:291+`):
Called at the end of each block's indexing. Advances the `last_block` pointer past
blocks with no pending (unfulfilled) payments, ensuring both the view slow-path and
cache initialization start scanning from the correct block.

**Pending entry format**: `[block_height (8 bytes), spendable (OutPoint), output (TxOut)]`.

#### 3.3.13 Activation Guard

**Source**: `src/indexer.rs:99-110`

The `is_active(height)` function (`src/network.rs`) returns `true` only when the
current block height is at or above `GENESIS_BLOCK` for the configured network. All
alkanes-specific setup functions in the block processing pipeline are guarded behind
this check:
- `setup_diesel(block)`, `setup_frbtc(block)`, `setup_frsigil(block)`
- `check_and_upgrade_precompiled(height)`
- `FuelTank::initialize(block, height)`
- `unwrap::update_last_block(height)`

This allows the indexer to process Runes-layer data (which activates earlier, e.g.,
block 840,000 on mainnet) without crashing on alkanes-specific operations before the
alkanes genesis height (e.g., block 880,000 on mainnet).

---

## 4. Key Data Structures

### 4.1 BalanceSheet

**Source**: `crates/protorune-support/src/balance_sheet.rs`

In-memory representation: `BTreeMap<ProtoruneRuneId, u128>` (sorted by rune ID).

The `BalanceSheet<P>` struct is generic over pointer type P (for storage-backed loading):
```rust
struct BalanceSheet<P: KeyValuePointer + Clone> {
    cached: CachedBalanceSheet,    // in-memory BTreeMap
    load_ptrs: Vec<P>,             // lazy-load pointers for persistent storage
}
```

Key operations (`BalanceSheetOperations` trait, `balance_sheet.rs:180-265`):
- `get(rune)` -> u128: Returns cached value, or loads from storage via `load_ptrs`
- `set(rune, value)`: Updates the cached map
- `increase(rune, value)`: Checked addition (overflow = error)
- `decrease(rune, value)`: Saturating subtraction (underflow = false)
- `pipe(target)`: Copy all balances into `target`
- `debit(sheet)`: Subtract another sheet's balances (underflow = error)
- `merge(a, b)`: Combine two sheets additively

**Persistent storage format** (`crates/protorune/src/balance_sheet.rs:18-36`):
```
{ptr}/runes           -> append-list of ProtoruneRuneId bytes (32 bytes each)
{ptr}/balances        -> append-list of u128 values
{ptr}/id_to_balance/{rune_bytes} -> u128 (indexed lookup)
```

**MintableDebit** (`crates/protorune/src/balance_sheet.rs:90-120`):
When debiting outgoing balances, if a rune would go negative:
- Check if the rune is "mintable in protocol" (not etched via Runes and protoburned).
- If mintable, skip the debit for the excess amount (the contract minted new tokens).
- If not mintable, error.

### 4.2 ProtoruneRuneId

**Source**: `crates/protorune-support/src/balance_sheet.rs:15-21`

```rust
struct ProtoruneRuneId {
    block: u128,
    tx: u128,
}
```

Serialized as 32 bytes (two little-endian u128 values). Used as the universal token
identifier across all layers. For standard runes, `(block, tx)` maps to the Bitcoin
block height and transaction index of the etching. For alkanes, the `block` field
indicates the ID type (see Section 3.3.3).

### 4.3 Protostone

**Source**: `crates/protorune-support/src/protostone.rs:148-156`

See Section 3.2.2 for the full field listing and encoding details.

### 4.4 Cellpack

**Source**: `crates/alkanes-support/src/cellpack.rs:8-11`

See Section 3.3.4 for encoding format.

Parsing modes:
- **From varint list** (`TryFrom<Vec<u128>>`, `cellpack.rs:64-72`): First two values
  are `target.block` and `target.tx`, remainder are inputs.
- **From binary** (`Cellpack::parse`, `cellpack.rs:14-27`): Read target as two
  fixed-size u128, then consume remaining u128 values.

### 4.5 Storage Tables

#### 4.5.1 Protorune Tables (`crates/protorune/src/tables.rs`)

**RuneTable** (base Runes layer, prefix-free):

| Field | Key Path | Value Type |
|-------|----------|------------|
| `HEIGHT_TO_BLOCKHASH` | `/blockhash/byheight/{height}` | 32 bytes |
| `BLOCKHASH_TO_HEIGHT` | `/height/byblockhash/{hash}` | u64 |
| `OUTPOINT_TO_RUNES` | `/runes/byoutpoint/{outpoint}` | BalanceSheet (sub-keys) |
| `OUTPOINT_TO_HEIGHT` | `/height/byoutpoint/{outpoint}` | u64 |
| `HEIGHT_TO_TRANSACTION_IDS` | `/txids/byheight{height}` | list of txid bytes |
| `SYMBOL` | `/runes/symbol/{etching_name}` | u32 (char code) |
| `CAP` | `/runes/cap/{etching_name}` | u128 |
| `SPACERS` | `/runes/spaces/{etching_name}` | u32 |
| `OFFSETEND` | `/runes/offset/end/{etching_name}` | u64 |
| `OFFSETSTART` | `/runes/offset/start/{etching_name}` | u64 |
| `HEIGHTSTART` | `/runes/height/start/{etching_name}` | u64 |
| `HEIGHTEND` | `/runes/height/end/{etching_name}` | u64 |
| `AMOUNT` | `/runes/amount/{etching_name}` | u128 |
| `MINTS_REMAINING` | `/runes/mints-remaining/{etching_name}` | u128 |
| `PREMINE` | `/runes/premine/{etching_name}` | u128 |
| `DIVISIBILITY` | `/runes/divisibility/{etching_name}` | u8 |
| `RUNE_ID_TO_HEIGHT` | `/height/byruneid/{rune_id}` | u64 |
| `ETCHINGS` | `/runes/names` | list of etching names |
| `RUNE_ID_TO_ETCHING` | `/etching/byruneid/{rune_id}` | etching name bytes |
| `ETCHING_TO_RUNE_ID` | `/runeid/byetching/{name}` | ProtoruneRuneId bytes |
| `TXID_TO_TXINDEX` | `/txindex/byid{txid}` | u32 |

**RuneTable::for_protocol(tag)** (protocol-specific, prefixed):

| Field | Key Path | Value Type |
|-------|----------|------------|
| `OUTPOINT_TO_RUNES` | `/runes/proto/{tag}/byoutpoint/{outpoint}` | BalanceSheet |
| `HEIGHT_TO_RUNE_ID` | `/runes/proto/{tag}/byheight/{height}` | list of rune IDs |
| `RUNE_ID_TO_INITIALIZED` | `/runes/proto/{tag}/initialized/{rune_id}` | flag |
| `HEIGHT_TO_TRANSACTION_IDS` | `/runes/proto/{tag}/txids/byheight{height}` | list of txids |
| `SYMBOL` | `/runes/proto/{tag}/symbol/{name}` | u32 |
| `CAP` | `/runes/proto/{tag}/cap/{name}` | u128 |
| `SPACERS` | `/runes/proto/{tag}/spaces/{name}` | u32 |
| `DIVISIBILITY` | `/runes/proto/{tag}/divisibility/{name}` | u8 |
| `ETCHINGS` | `/runes/proto/{tag}/names` | list |
| `RUNE_ID_TO_ETCHING` | `/runes/proto/{tag}/etching/byruneid/{id}` | name bytes |
| `ETCHING_TO_RUNE_ID` | `/runes/proto/{tag}/runeid/byetching/{name}` | rune ID bytes |
| `RUNTIME_BALANCE` | `/runes/proto/{tag}/runtime/balance` | BalanceSheet |
| `INTERNAL_MINT` | `/runes/proto/{tag}/mint/isinternal` | flag |

**Standalone tables** (`crates/protorune/src/tables.rs:138-159`):

| Static | Key Path | Value Type |
|--------|----------|------------|
| `HEIGHT_TO_RUNES` | `/runes/byheight/{height}` | list of etching names |
| `OUTPOINTS_FOR_ADDRESS` | `/outpoint/byaddress/{address}` | list of outpoint bytes |
| `OUTPOINT_SPENDABLE_BY` | `/outpoint/spendableby/{outpoint}` | address bytes |
| `OUTPOINT_SPENDABLE_BY_ADDRESS` | `/outpoint/spendablebyaddress/{address}` | linked list of outpoints |
| `OUTPOINT_TO_OUTPUT` | `/output/byoutpoint/{outpoint}` | protobuf Output |
| `CACHED_WALLET_RESPONSE` | `/cached/wallet/byaddress/{address}` | serialized WalletResponse |
| `CACHED_FILTERED_WALLET_RESPONSE` | `/cached/filtered/wallet/byaddress/{address}` | serialized WalletResponse |

#### 4.5.2 Alkanes Tables (`src/tables.rs`)

| Static | Key Path | Value Type |
|--------|----------|------------|
| `TRACES` | `/trace/{outpoint}` | protobuf AlkanesTrace |
| `TRACES_BY_HEIGHT` | `/trace/{height}` | list of outpoint bytes |

#### 4.5.3 Alkanes Implicit Tables (via IndexPointer paths in code)

| Key Path | Value Type | Source |
|----------|------------|--------|
| `/alkanes/{id_bytes}` | gzip-compressed WASM binary (or 32-byte factory pointer) | `src/vm/utils.rs:122` |
| `/alkanes/{id_bytes}/storage/{key}` | arbitrary bytes | `src/utils.rs:140-147` |
| `/alkanes/{id_bytes}/balances/{who_bytes}` | u128 (LE) | `src/utils.rs:25-42` |
| `/alkanes/{id_bytes}/inventory/` | list of held token IDs | `src/utils.rs:44-49` |
| `/alkanes/sequence` | u128 (next sequence number) | `src/vm/utils.rs:48-50` |
| `/alkanes_id_to_outpoint/{id_bytes}` | consensus-encoded OutPoint | `src/vm/utils.rs:52-73` |
| `/seen-genesis` | u8 flag | `src/network.rs:202` |
| `/genesis-upgraded` | u8 flag | `src/network.rs:330` |
| `/genesis-upgraded-eoa` | u8 flag | `src/network.rs:339` |

---

## 5. State Transitions

### 5.1 Block Processing

See Section 2.2 for the high-level pipeline. Key invariants:

- Each block is processed exactly once (idempotent given the `/seen-genesis` flag).
- All state changes within a transaction are atomic (via AtomicPointer).
- The FuelTank is initialized fresh per block and drained proportionally per transaction.
- Blacklisted transactions are skipped (`BLACKLISTED_TX_HASHES` in
  `crates/protorune/src/lib.rs:39`).

### 5.2 Runestone Processing

`Protorune::index_runestone` (`crates/protorune/src/lib.rs:206`):

1. **Load input sheets**: For each transaction input, load the existing balance sheet
   from `OUTPOINT_TO_RUNES`. Clear the input's balance sheet to prevent double-spending.
2. **Process etching**: If present, validate and store the new rune's metadata.
3. **Process mint**: If present, validate terms and add minted amount to unallocated pool.
4. **Process edicts**: For each edict, transfer balances between output slots in
   `balances_by_output`.
5. **Assign unallocated**: Remaining balances go to the `pointer` output (or default).
6. **Cenotaph handling**: If the Runestone is a cenotaph, all input balances are burned.
7. **Extract protostones**: Parse `Runestone.protocol` into `Vec<Protostone>`.
8. **Process protoburns**: Bridge rune balances into sub-protocol layers.
9. **Process protostone edicts**: Transfer sub-protocol balances between virtual outputs.
10. **Process messages**: For each protostone with a message, invoke the MessageContext.
11. **Persist output sheets**: Save final balances to each output's storage location.
12. **Track addresses**: Index outpoint-to-address and address-to-outpoint mappings.

### 5.3 Protostone Message Processing

`Protostone::process_message` (`crates/protorune/src/protostone.rs:71-208`):

1. **Validate pointers**: `pointer` and `refund_pointer` must be within bounds
   (real outputs + virtual protostone outputs).
2. **Load initial sheet**: Get the balance sheet at the protostone's virtual vout.
3. **Checkpoint**: Push a savepoint.
4. **Build parcel**: Construct a `MessageContextParcel` with all context.
5. **Call handler**: `T::handle(&parcel)` where T is `AlkaneMessageContext`.
6. **On success**: Call `reconcile()` to update `balances_by_output`:
   - Remove the virtual vout's sheet.
   - Add outgoing runes to the `pointer` output's sheet.
   - Set runtime balances at `u32::MAX`.
   - Validate: initial - outgoing - runtime should be zero (modulo mintable tokens).
   - Commit the checkpoint.
7. **On failure**: Refund balances to `refund_pointer`, rollback the checkpoint.

### 5.4 Alkane Execution

`run_after_special` (`src/vm/utils.rs:250-305`):

1. Instantiate `AlkanesInstance` with the contract's WASM binary and allocated fuel.
2. Call `instance.execute()`:
   a. `checkpoint()` the atomic pointer.
   b. Invoke the `__execute` WASM export.
   c. Parse the return value as an `ExtendedCallResponse`:
      - `alkanes`: outgoing token transfers (AlkaneTransferParcel)
      - `storage`: key-value pairs to persist (StorageMap)
      - `data`: arbitrary return data bytes
   d. If `had_failure` flag is set: `rollback()`, return error.
   e. Otherwise: `commit()`, return response.
3. Compute total fuel used: `(start_fuel - remaining_fuel) + storage_bytes * fuel_per_store_byte`.
4. Return `(response, fuel_used)`.

---

## 6. Security Model

### 6.1 Balance Integrity

**Atomic transactions**: All state changes during a contract execution are wrapped in
`AtomicPointer` checkpoints. On failure, `rollback()` discards all writes.

**Nested checkpoints**: Extcalls create nested checkpoints. The depth is limited to 75
(`src/vm/host_functions.rs:493`) to prevent stack overflow attacks.

**Input clearing**: When processing a transaction, each input's balance sheet is
loaded and then cleared from storage. This prevents double-spending of rune balances
across transactions.

**Overflow protection**: All balance arithmetic uses `checked_add`/`checked_sub`
(`src/utils.rs:73-78`, `crates/protorune-support/src/balance_sheet.rs:202-225`).
Overflow produces an error that triggers rollback.

**Self-minting rule**: A contract can only mint its own token (where `transfer.id == from`).
Attempting to mint another contract's token produces a balance underflow error
(`src/utils.rs:88-103`).

**Reconciliation**: After execution, the `reconcile()` function validates that the sum
of outgoing tokens and runtime balances does not exceed the initial incoming balances
(modulo mintable tokens). Any non-zero remainder triggers a warning log
(`crates/protorune/src/balance_sheet.rs:151-155`).

### 6.2 WASM Sandbox

**Fuel limits** (`src/vm/fuel.rs`):
- Total block fuel is bounded (100M-1B depending on height and network).
- Each transaction receives proportional fuel based on virtual size.
- A minimum fuel floor prevents starvation of small transactions.
- On failure, `drain_fuel()` zeroes out remaining fuel (prevents retry abuse).

**Memory bounds**: wasmi's `StoreLimitsBuilder` enforces `MEMORY_LIMIT = 43,554,432`
bytes. Memory is pre-grown to 512 pages (32 MB). Attempts to allocate beyond the limit
cause a trap.

**No I/O access**: WASM modules have no imports for network, filesystem, or random
number generation. The only external interface is the defined set of host functions.

**Deterministic execution**: wasmi is a deterministic interpreter. Given the same
binary, fuel, and context, execution always produces the same result.

**Context safety wrapper**: `SafeAlkanesHostFunctionsImpl` (`src/vm/host_functions.rs:878-1002`)
wraps each host function with checkpoint/commit/depth-assertion to ensure the
checkpoint stack is never corrupted by a host function call.

### 6.3 Validation Rules

**Rune name validation** (`crates/protorune/src/lib.rs:142-178`):
- Names must be above the reserved threshold for the current height.
- Names must not already exist in the ETCHINGS table.
- A commitment proof with >= 6 confirmations is required (production only).

**Mint cap enforcement**: The `MINTS_REMAINING` counter is decremented atomically.
When it reaches zero, further mints of that rune are rejected.

**Edict boundary checks**: Edict outputs are validated against the transaction's output
count. Out-of-bounds edicts create a cenotaph (entire transaction's runes are burned).

**Protostone pointer validation** (`crates/protorune/src/protostone.rs:91-96`):
Pointer and refund_pointer must not exceed `num_outputs + num_protostones`.

**Virtual vout limit** (`crates/protorune/src/protostone.rs:123-126`):
Protomessage vout is capped at `num_outputs + 100` to prevent overflow attacks.

---

## 7. View Functions

All view/query endpoints. Entry points are in `src/view.rs` and `crates/protorune/src/view.rs`.

### 7.1 Alkanes View Endpoints (`src/view.rs`)

| Function | Input | Output | Description |
|----------|-------|--------|-------------|
| `protorunes_by_outpoint` | `OutpointWithProtocol` (protobuf) | `OutpointResponse` | Balances at a specific outpoint |
| `protorunes_by_address` | `ProtorunesWalletRequest` (protobuf) | `WalletResponse` | All outpoints and balances for an address |
| `protorunes_by_address2` | `ProtorunesWalletRequest` (protobuf) | `WalletResponse` | Same, using linked-list index (faster) |
| `protorunes_by_height` | `ProtorunesByHeightRequest` (protobuf) | `RunesResponse` | All protorunes etched at a height |
| `sequence` | (none) | `Vec<u8>` (u128 LE) | Current global sequence counter |
| `simulate_safe` | `MessageContextParcel`, fuel: u64 | `(ExtendedCallResponse, u64)` | Simulate a contract call (view mode) |
| `multi_simulate_safe` | `[MessageContextParcel]`, fuel: u64 | `Vec<Result<...>>` | Simulate multiple calls |
| `meta_safe` | `MessageContextParcel` | `Vec<u8>` | Call `__meta` export for ABI |
| `call_view` | AlkaneId, inputs: Vec<u128>, fuel: u64 | `Vec<u8>` | Convenience: simulate a call and return data |
| `call_multiview` | `[AlkaneId]`, `[Vec<u128>]`, fuel: u64 | `Vec<u8>` | Batch view calls |
| `alkanes_id_to_outpoint` | `AlkaneIdToOutpointRequest` (protobuf) | `AlkaneIdToOutpointResponse` | Map alkane ID to creation outpoint |
| `getinventory` | `AlkaneInventoryRequest` (protobuf) | `AlkaneInventoryResponse` | List tokens held by a contract |
| `getstorageat` | `AlkaneStorageRequest` (protobuf) | `AlkaneStorageResponse` | Read a specific storage key |
| `getbytecode` | `BytecodeRequest` (protobuf) | `Vec<u8>` | Get decompressed WASM bytecode |
| `getblock` | `BlockRequest` (protobuf) | `BlockResponse` | Get a stored block by height |
| `trace` | OutPoint | `Vec<u8>` | Get execution trace for an outpoint |
| `traceblock` | height: u32 | `Vec<u8>` | Get all traces for a block |
| `unwrap` | height: u128 | `Vec<u8>` | Get pending fr-BTC unwrap payments |

### 7.2 Protorune View Endpoints (`crates/protorune/src/view.rs`)

| Function | Input | Output | Description |
|----------|-------|--------|-------------|
| `runes_by_address` | `WalletRequest` (protobuf) | `WalletResponse` | Standard rune balances by address |
| `runes_by_outpoint` | `Outpoint` (protobuf) | `OutpointResponse` | Standard rune balances at outpoint |
| `protorunes_by_outpoint` | `OutpointWithProtocol` (protobuf) | `OutpointResponse` | Protocol-specific balances at outpoint |
| `protorunes_by_address` | `ProtorunesWalletRequest` (protobuf) | `WalletResponse` | Protocol-specific balances by address |
| `runes_by_height` | `RunesByHeightRequest` (protobuf) | `RunesResponse` | Runes etched at a height |
| `protorunes_by_height` | `ProtorunesByHeightRequest` (protobuf) | `RunesResponse` | Protorunes at a height |

### 7.3 Name/Symbol Resolution

Alkane token names and symbols are resolved dynamically by calling the contract's
view functions (`src/view.rs:152-183`):
- Opcode 99 = name
- Opcode 100 = symbol
- Results are cached in a static `STATICS_CACHE` to avoid repeated simulations.
- Limited to 300 simulate calls per balance sheet to prevent DOS.

---

## 8. Test Coverage

### 8.1 Protorune Tests (`crates/protorune/src/tests/`)

| File | Coverage Area |
|------|--------------|
| `index_runes.rs` | Basic Runes indexing: etch, mint, transfer |
| `index_runes_mint.rs` | Mint term validation, cap enforcement |
| `index_runes_edicts.rs` | Edict processing, boundary cases |
| `index_protoburns.rs` | Protoburn bridging, cycle behavior |
| `index_protomessage.rs` | Protostone message dispatch |
| `index_protorunes_by_address.rs` | Address-based balance queries |
| `index_op_return_position.rs` | OP_RETURN output position handling |
| `index_pointer_ll.rs` | Linked-list index pointer tests |
| `test_cenotaphs.rs` | Cenotaph creation and balance burning |
| `multi_protocol.rs` | Multiple sub-protocol isolation |
| `multi_block.rs` | Multi-block indexing scenarios |
| `ord_runes_parity.rs` | Parity tests with the ord indexer |
| `view_functions.rs` | View function endpoint tests |
| `test_many_outputs_bug.rs` | Edge case: transactions with many outputs |

### 8.2 Alkanes Tests (`src/tests/`)

| File | Coverage Area |
|------|--------------|
| `alkane.rs` | Basic contract deployment and execution |
| `genesis.rs` | Genesis block bootstrap |
| `genesis_upgrade.rs` | Precompiled contract upgrades |
| `fuel.rs` | Fuel metering and limits |
| `factory.rs` | Factory pattern deployment |
| `fr_btc.rs` | fr-BTC unwrap contract |
| `forge.rs` | Contract forging scenarios |
| `auth_token.rs` | Auth token contract |
| `trace.rs` | Execution trace recording |
| `crash.rs` | Error handling and revert scenarios |
| `determinism.rs` | Deterministic execution verification |
| `edict_then_message.rs` | Edict followed by message processing |
| `arbitrary_alkane_mint.rs` | Self-minting behavior |
| `special_extcall.rs` | Precompiled contract calls |
| `upgradeable.rs` | Upgradeable proxy contracts |
| `merkle_distributor.rs` | Merkle distributor contract |
| `memory_security_tests.rs` | Memory bounds and security |
| `vec_input_test.rs` | Vector input handling |
| `abi_test.rs` | ABI/meta function testing |
| `view.rs` | View function testing |
| `getstorageat.rs` | Storage-at view function testing |
| `address.rs` | Address encoding/decoding |
| `networks.rs` | Multi-network configuration |
| `helpers.rs` | Test utility functions |
| `utils.rs` | Shared test utilities |

### 8.3 Protoburn Tests (`crates/protorune/src/protoburn.rs`)

Inline tests cover:
- Single protoburn success/failure (`test_protoburn_process_success`, `test_protoburn_process_no_tag`)
- No-op burns (`test_protoburns_no_op`)
- Default output burns (`test_protoburns_default_goes_to_first_protoburn`)
- Edict cycling across burns (`test_protoburns_edicts_cycle`, `test_protoburns_edicts_cycle_two_runes`)
- Cycle loopback (`test_protoburns_edicts_cycle_loopback`)
- From-field targeting (`test_protoburns_edicts_from`, `test_protoburns_edicts_from_cycle`)
- Invalid from-field (`test_protoburns_edicts_from_invalid`)

### 8.4 Protostone Tests (`crates/protorune/src/protostone.rs`)

Inline tests cover:
- Encipher/decipher round-trip for burns, edicts, messages, and multiple protostones.

---

## 9. Configuration

### 9.1 Feature Flags (`Cargo.toml:55-85`)

| Flag | Effect |
|------|--------|
| `mainnet` | Bitcoin mainnet parameters (bech32 `bc`, genesis 880,000) |
| `testnet` | Bitcoin testnet parameters (bech32 `tb`) |
| `regtest` | (default) Regtest parameters (bech32 `bcrt`, genesis 0) |
| `dogecoin` | Dogecoin parameters (bech32 `dc`, genesis 6,000,000) |
| `luckycoin` | Luckycoin parameters (bech32 `lky`, genesis 400,000) |
| `bellscoin` | Bellscoin parameters (bech32 `bel`, genesis 500,000) |
| `fractal` | Fractal parameters (genesis 400,000) |
| `cache` | Enable wallet response caching |
| `debug-log` | Enable verbose logging in host functions and fuel |
| `test-utils` | Enable test helper utilities |
| `proxy` | Include proxy contract build |
| `owned_token` | Include owned token contract build |
| `auth_token` | Include auth token contract build |
| `genesis_alkane` | Include genesis alkane contract build |
| `genesis_protorune` | Include genesis protorune contract build |
| `amm_pool` | Include AMM pool contract build (requires auth_token) |
| `amm_factory` | Include AMM factory contract build (requires auth_token) |
| `orbital` | Include orbital contract build |
| `upgradeable` | Include upgradeable contract build |
| `refunder` | Include refunder contract build |
| `merkle_distributor` | Include merkle distributor contract build |
| `free_mint` | Include free mint contract build |

### 9.2 Network Parameters

**Genesis configuration** (`src/network.rs`):

| Network | GENESIS_BLOCK | GENESIS_OUTPOINT | UPGRADE_HEIGHT | EOA_UPGRADE_HEIGHT |
|---------|--------------|------------------|----------------|-------------------|
| Regtest | 0 | `3977b30a...` | 0 | 0 |
| Mainnet | 880,000 | `3977b30a...` | 908,888 | 917,888 |
| Fractal | 400,000 | `cf2b52ff...` | 228,194 | 228,194 |
| Dogecoin | 6,000,000 | `cf2b52ff...` | 872,101 | 872,101 |
| Luckycoin | 400,000 | `cf2b52ff...` | 872,101 | 872,101 |
| Bellscoin | 500,000 | `2c58484a...` | 288,906 | 288,906 |

**Fuel parameters by network** (`src/vm/fuel.rs`):

| Network | TOTAL_FUEL_START | TOTAL_FUEL_CHANGE1 | FUEL_CHANGE1_HEIGHT |
|---------|-----------------|-------------------|---------------------|
| Mainnet | 100,000,000 | 1,000,000,000 | 899,087 |
| Regtest | 100,000,000 | 1,000,000,000 | 0 |
| Dogecoin | 60,000,000 | 600,000,000 | 5,730,675 |
| Fractal | 50,000,000 | 500,000,000 | 759,865 |
| Luckycoin | 50,000,000 | 500,000,000 | 1,664,317 |
| Bellscoin | 50,000,000 | 500,000,000 | 533,970 |

### 9.3 System Contracts

Deployed at genesis (`src/network.rs`):

| AlkaneId | Name | Purpose |
|----------|------|---------|
| `[2, 0]` | Genesis Alkane (Diesel) | Core factory/utility contract; upgraded at GENESIS_UPGRADE_BLOCK_HEIGHT |
| `[32, 0]` | fr-BTC | Fractional Bitcoin unwrap contract |
| `[32, 1]` | fr-Sigil | Fractional Sigil contract |

---

## 10. Known Limitations and Audit Notes

### 10.1 Self-Minting Without Limit

Contracts can mint their own token without any supply cap enforcement at the protocol
level (`src/utils.rs:88-94`). This is by design -- supply control is delegated to the
contract author. Auditors should verify that user-facing contracts implement proper
mint restrictions.

### 10.2 Unsafe Global State

The `_VIEW` flag uses `static mut` with `unsafe` blocks (`src/network.rs:189-199`).
This is not thread-safe. However, the indexer runs single-threaded within the WASM
environment, so this is acceptable in practice but would be problematic if the code
were used in a multi-threaded context.

### 10.3 Blacklisted Transactions

One transaction is hardcoded as blacklisted (`crates/protorune/src/lib.rs:39`):
`5cbb0c466dd08d7af9223d45105fbbf0fdc9fb7cda4831c183d6b0cb5ba60fb0`.
This is an operational workaround; the reason should be documented.

### 10.4 Mutex Unwrap Strategy

The codebase intentionally uses `.unwrap()` on mutex locks rather than error propagation
(`src/message.rs:95-99`). The rationale is that a poisoned mutex indicates a concurrency
bug that should cause a panic and block retry, rather than silently producing an
inconsistent index.

### 10.5 Commitment Validation Bypassed in Tests

The `validate_rune_etch` function always returns `Ok(true)` in test builds
(`crates/protorune/src/lib.rs:201-203`). Auditors should verify this is compile-gated
and never reaches production.

### 10.6 Protostone Protocol Index Safety

The `PROTOCOLS` set uses `static mut` with `unsafe` (`crates/protorune/src/protostone.rs:21-37`).
Same thread-safety caveat as 10.2.

### 10.7 TRACES_BY_HEIGHT Pointer Collision

Both `TRACES` and `TRACES_BY_HEIGHT` use the same keyword `/trace/`
(`src/tables.rs:5-8`). This means trace data by outpoint and trace data by height
share the same key prefix. This works because they use different selector types
(outpoint bytes vs height value), but the shared prefix is fragile and could
cause issues if the key formats were to overlap.

### 10.8 Diesel Mints Cache Thread Safety

The `DIESEL_MINTS_CACHE` uses `LazyLock<Arc<RwLock<Option<Vec<u8>>>>>` and is cleared
per block (`src/vm/host_functions.rs:43-50`). The `try_write()` pattern silently
skips cache clearing if the lock is contended. In the single-threaded indexer this
is benign, but worth noting.

### 10.9 View Mode Global State

`set_view_mode()` (`src/network.rs:191`) sets a global flag that affects `is_genesis()`
behavior. There is no corresponding `clear_view_mode()`. In simulation contexts, once
view mode is set, it persists for the process lifetime. This is acceptable for the
WASM indexer (each invocation is isolated) but could cause issues in long-running
test processes.

### 10.10 Balance Pointer Side Effect

The `balance_pointer` function (`src/utils.rs:25-42`) has a side effect: if the
queried balance is non-zero, it appends the token to the holder's inventory list.
This means even read-only balance queries during indexing can modify state. The
append is idempotent in practice (the list is append-only and serves as a "known
tokens" set) but violates the principle of least surprise.

### 10.11 Version-Gated Edict Spread Behavior

The `handle_transfer_runes_to_vout` function (`crates/protorune/src/lib.rs:89-160`)
has three code paths for the "spread to all outputs" case (edict targeting
`output == tx.output.len()` with `amount > 0`):
- **Pre-V217_FIX_HEIGHT**: OP_RETURN outputs consume from the spread amount but
  are then skipped, silently losing tokens.
- **Post-V217_FIX_HEIGHT**: OP_RETURN outputs are skipped before computing amounts,
  preserving the full spread.

Auditors should verify that the pre-fix path is only reachable on blocks below
the activation height for each network.

### 10.12 No Turbo Flag Storage

The Runes `turbo` flag (from the ordinals specification) is parsed but not stored
in any table. This means the indexer does not track whether a rune has opted into
future protocol changes.

---

## Appendix A: Protobuf Definitions

**Source**: `crates/protorune-support/proto/protorune.proto`

Key message types:
- `uint128` -- custom type with `lo: uint64, hi: uint64` fields
- `ProtoruneRuneId` -- `height: uint128, txindex: uint128`
- `Rune` -- metadata: name, symbol, divisibility, spacers, rune_id
- `BalanceSheet` -- `repeated BalanceSheetItem` (rune + balance pairs)
- `OutpointResponse` -- balances, outpoint, output, height, txindex
- `WalletResponse` -- `repeated OutpointResponse`
- `ProtorunesWalletRequest` -- wallet address + protocol_tag

## Appendix B: WASM Contract Interface

Contracts must export:
- `__execute() -> i32` -- main entry point, returns pointer to response buffer
- `__meta() -> i32` -- (optional) returns pointer to ABI description

Response buffer format (parsed by `AlkanesExportsImpl::parse`, `src/vm/exports.rs:30-52`):
```
AlkaneTransferParcel     -- outgoing token transfers
u32                      -- number of storage entries
  [u32 key_len, key_bytes, u32 val_len, val_bytes] * N
remaining bytes          -- return data
```

The contract reads its context via `__load_context` and interacts with state via
the host functions listed in Section 3.3.7.
