# 🚀 Rebuild and Test - Futures Fix Applied!

## What I Fixed

Added `deploy_futures_from_protostones()` function to the indexer that:
1. Detects protostones with cellpack [32, 0, 77] in coinbase
2. Deploys future contract at [31:height]
3. Initializes the contract storage

## Files Modified

1. **`crates/alkanes/src/network.rs`**
   - Added `deploy_futures_from_protostones()` function (115 lines)

2. **`crates/alkanes/src/indexer.rs`**
   - Import the new function
   - Call it in `index_block()` after `setup_ftrbtc()`

## 🚀 Rebuild and Test

### Step 1: Rebuild WASM Indexer

```bash
cd ~/alkanes-rs
./build-wasm.sh
```

**Wait time:** 3-5 minutes

### Step 2: Restart Docker Services

```bash
# Stop services
docker-compose down

# Clear indexer database (fresh start)
docker volume rm alkanes-rs_metashrew-data

# Start services
docker-compose up -d
```

Wait ~30 seconds for services to be healthy.

### Step 3: Generate a Future

```bash
./target/release/alkanes-cli -p regtest bitcoind generatefuture
```

**Look for:** "Deploying future at block N" in logs!

### Step 4: Verify Future Has Bytecode

```bash
# Wait for indexer to process
sleep 5

# Check the future
BLOCK=$(./target/release/alkanes-cli -p regtest bitcoind getblockcount)
./target/release/alkanes-cli -p regtest alkanes inspect 31:$BLOCK
```

**Expected:**
```
🔍 Inspection Result for Alkane: 31:1
├── 📏 Bytecode Length: 12345 bytes  ← Should be > 0!
└── 💾 Storage Keys: ...
```

### Step 5: Generate More Futures

```bash
# Generate a few more
./target/release/alkanes-cli -p regtest bitcoind generatefuture
sleep 5
./target/release/alkanes-cli -p regtest bitcoind generatefuture
sleep 5
./target/release/alkanes-cli -p regtest bitcoind generatefuture
```

### Step 6: Check All Futures

```bash
for i in 1 2 3 4; do
  echo "Checking future [31:$i]:"
  ./target/release/alkanes-cli -p regtest alkanes inspect 31:$i | grep "Bytecode Length"
done
```

All should show bytecode > 0!

### Step 7: Test in Browser

```bash
cd ~/subfrost-app
yarn dev
```

Open: **http://localhost:3000/futures**

1. Click "Generate Future" button
2. Wait for success alert
3. Refresh page
4. **See REAL futures in the table!** 🎉

## 🎯 What Should Happen

### In the Docker Logs:

```bash
docker logs -f alkanes-rs_metashrew_1
```

You should see:
```
Deploying future at block 1
Future 1 deployed successfully
```

### In the CLI:

```bash
$ ./target/release/alkanes-cli -p regtest alkanes inspect 31:1

🔍 Inspection Result for Alkane: 31:1
├── 📏 Bytecode Length: 12345 bytes
├── 📦 Storage Keys: 15
└── 💰 Balance: 100000000 (1.00000000 BTC)
```

### In the Browser:

The Markets table should show:
- `ftrBTC[31:1]` - Call - Strike: 50000 - Expiry: Block 101
- `ftrBTC[31:2]` - Call - Strike: 51000 - Expiry: Block 102
- `ftrBTC[31:3]` - Call - Strike: 52000 - Expiry: Block 103

**Real blockchain data!** Not mock data!

## 🎊 If This Works

You'll have **COMPLETE** futures functionality:
- ✅ Generate futures
- ✅ Futures have bytecode
- ✅ Can inspect futures
- ✅ Can view in UI
- ✅ Ready for claiming ([31, 0, 14])
- ✅ Ready for trading

## 🔍 Debugging

If futures still have 0 bytes:

### Check Docker Logs
```bash
docker logs alkanes-rs_metashrew_1 --tail 100
```

Look for:
- "Deploying future at block N"
- "Future N deployed successfully"
- Any error messages

### Check the Protostone
```bash
BLOCK=$(./target/release/alkanes-cli -p regtest bitcoind getblockcount)
HASH=$(curl -s --user bitcoinrpc:bitcoinrpc \
  --data-binary "{\"method\":\"getblockhash\",\"params\":[$BLOCK]}" \
  http://localhost:18443 | jq -r '.result')
curl -s --user bitcoinrpc:bitcoinrpc \
  --data-binary "{\"method\":\"getblock\",\"params\":[\"$HASH\",2]}" \
  http://localhost:18443 | jq '.result.tx[0].vout[2]'
```

Should show the protostone OP_RETURN.

### Verify WASM Build
```bash
ls -lh target/wasm32-unknown-unknown/release/alkanes.wasm
```

Should be a file (~1-2 MB), not a directory.

## 🚀 Run the Build Now!

```bash
cd ~/alkanes-rs
./build-wasm.sh
```

Then follow the steps above to test! 🎉
