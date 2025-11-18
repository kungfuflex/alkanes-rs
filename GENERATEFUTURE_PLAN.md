# generatefuture Implementation Plan

## Overview
Add a new RPC command `generatefuture` to Bitcoin Core that generates a single block with a special coinbase transaction containing a Runestone with Protostone `[32, 0, 77]` to enable future claiming.

## Components

### 1. Bitcoin Core Patch
**File**: `/data/alkanes-rs/patch/bitcoin/src/rpc/mining.cpp`

Add `generatefuture()` function that:
- Takes one parameter: `address` (string)
- Generates 1 block only
- Creates coinbase with:
  - Output 0: Standard coinbase output paying to address
  - Output 1: OP_RETURN with Runestone containing Protostone

**Protostone Specification**:
```
Protocol Tag: 1 (ALKANES)
Message: [32, 0, 77] (cellpack - 3 bytes encoded as varint list)
Pointer: Some(0)
Refund: Some(0)
Edicts: [] (empty)
Burn: None
From: None
```

**Encoding Format** (based on protorune-support):
```rust
// Protostone encoding produces Vec<u128>:
// [protocol_tag, message_length, ...message_u128s...]

// For [32, 0, 77]:
// Varint encode: [32, 0, 77] -> bytes
// Split into u128s (15 bytes per u128)
// Result: [1, message_len, message_u128...]
```

**Runestone Format**:
```
OP_RETURN OP_13 
  [protocol field encoded as varints]
  [pointer field: tag=2, value=0]
```

### 2. Dockerfile Update
**File**: `/data/alkanes-rs/docker/bitcoind/Dockerfile`

Update to build from patched source:
```dockerfile
FROM debian:trixie-slim AS build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential ca-certificates cmake git \
    libboost-dev libevent-dev libsqlite3-dev \
    libzmq3-dev pkg-config clang-19 ccache \
    capnproto libcapnp-dev systemtap-sdt-dev python3

# Copy patched Bitcoin source
COPY ../patch/bitcoin /src/bitcoin
WORKDIR /src/bitcoin

# Build
RUN cmake -B build ... && cmake --build build -j$(nproc)

# Install
RUN cmake --install build

# Second stage - runtime
FROM debian:trixie-slim
COPY --from=build /opt/bitcoin /opt
...
```

### 3. CLI Command
**File**: `/data/alkanes-rs/crates/alkanes-cli/src/main.rs`

Add under `bitcoind` subcommand:
```rust
Bitcoind::Generatefuture { address } => {
    let result = provider.call_bitcoind_rpc(
        "generatefuture",
        &[address.into()]
    ).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
}
```

## Implementation Steps

### Step 1: Implement Runestone Encoding Helper in C++

Create helper functions in `mining.cpp`:
```cpp
// Encode varint (LEB128)
std::vector<uint8_t> EncodeVarint(uint64_t n);

// Encode list of varints
std::vector<uint8_t> EncodeVarintList(const std::vector<uint64_t>& values);

// Split bytes into u128s (15 bytes per u128, LE encoding)
std::vector<uint128_t> SplitBytesTo128(const std::vector<uint8_t>& bytes);

// Encode protostone message
std::vector<uint128_t> EncodeProtostoneMessage(const std::vector<uint64_t>& cellpack);

// Create runestone script with protostone
CScript CreateRunestoneWithProtostone(const std::vector<uint64_t>& cellpack);
```

### Step 2: Add generatefuture RPC Handler

```cpp
static RPCHelpMan generatefuture()
{
    return RPCHelpMan{"generatefuture",
        "Mine a single block with a future-claiming protostone in the coinbase.",
        {
            {"address", RPCArg::Type::STR, RPCArg::Optional::NO, 
             "The address to send the coinbase reward to."},
        },
        RPCResult{
            RPCResult::Type::STR_HEX, "", "hash of the generated block"
        },
        RPCExamples{
            HelpExampleCli("generatefuture", "\"myaddress\"")
        },
        [&](const RPCHelpMan& self, const JSONRPCRequest& request) -> UniValue
        {
            // Decode address
            CTxDestination destination = DecodeDestination(request.params[0].get_str());
            if (!IsValidDestination(destination)) {
                throw JSONRPCError(RPC_INVALID_ADDRESS_OR_KEY, "Invalid address");
            }

            NodeContext& node = EnsureAnyNodeContext(request.context);
            Mining& miner = EnsureMining(node);
            ChainstateManager& chainman = EnsureChainman(node);

            // Create coinbase script
            CScript coinbase_script = GetScriptForDestination(destination);

            // Create block template
            std::unique_ptr<BlockTemplate> block_template(
                miner.createNewBlock({.coinbase_output_script = coinbase_script})
            );
            CHECK_NONFATAL(block_template);

            // Get mutable block
            CBlock block = block_template->getBlock();

            // Modify coinbase transaction to add OP_RETURN with runestone
            CMutableTransaction coinbase_tx(block.vtx[0]);
            
            // Add OP_RETURN output with runestone
            std::vector<uint64_t> cellpack = {32, 0, 77};
            CScript runestone_script = CreateRunestoneWithProtostone(cellpack);
            coinbase_tx.vout.push_back(CTxOut(0, runestone_script));

            // Update block
            block.vtx[0] = MakeTransactionRef(std::move(coinbase_tx));

            // Mine the block
            uint64_t max_tries = DEFAULT_MAX_TRIES;
            std::shared_ptr<const CBlock> block_out;
            if (!GenerateBlock(chainman, std::move(block), max_tries, block_out, true)) {
                throw JSONRPCError(RPC_INTERNAL_ERROR, "Failed to generate block");
            }

            return block_out->GetHash().GetHex();
        }
    };
}
```

### Step 3: Register the RPC Command

In `RegisterMiningRPCCommands()`:
```cpp
void RegisterMiningRPCCommands(CRPCTable& t)
{
    static const CRPCCommand commands[]{
        // ... existing commands ...
        {"hidden", &generatefuture},  // Add this line
    };
    // ...
}
```

### Step 4: Update Dockerfile

Replace current simple Dockerfile with build-from-source version.

### Step 5: Update CLI

Add `generatefuture` subcommand to `alkanes-cli bitcoind`.

## Testing

```bash
# Build custom bitcoind
cd /data/alkanes-rs
docker-compose down
docker-compose build bitcoind
docker-compose up -d

# Test generatefuture
./target/release/alkanes-cli -p regtest bitcoind generatefuture bcrt1p...

# Verify runestone in coinbase
./target/release/alkanes-cli -p regtest esplora tx <blockhash>

# Verify future can be claimed
./target/release/alkanes-cli -p regtest alkanes execute "[31,0,14]" --to <address>
```

## Expected Result

The coinbase transaction should have structure:
```
vin: [...] (coinbase input)
vout:
  [0]: value=50 BTC, scriptPubKey=<address>
  [1]: value=0, scriptPubKey=OP_RETURN OP_13 <runestone_data>
```

The runestone data encodes:
```
Protocol field: [1, 3, <encoded [32,0,77]>]
Pointer: 0
Refund: 0
```

This allows calling `[31, 0, 14]` from another transaction to claim the future.

## Notes

- Runestone magic number is OP_13 (0x5d)
- Varint encoding follows LEB128 format
- u128 splitting: max 15 bytes per u128, little-endian
- Protocol tag 1 = ALKANES Metaprotocol
- [32, 0, 77] calls the CREATE_FUTURE opcode on frBTC
