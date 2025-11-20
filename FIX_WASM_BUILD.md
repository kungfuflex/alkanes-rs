# 🔧 Fix WASM Build - Step by Step

## What I Fixed

1. Added `getrandom_backend="custom"` configuration to `.cargo/config.toml`
2. Updated build script to build ONLY the alkanes crate (not the entire workspace)
3. This avoids building CLI and other crates that use tokio (incompatible with WASM)

## 🚀 Run This Command

```bash
cd ~/alkanes-rs
./build-wasm.sh
```

This will:
1. Clean the wasm32 target directory
2. Build ONLY the `alkanes` crate with `cargo build -p alkanes`
3. Skip all non-WASM compatible crates (CLI, jsonrpc, etc.)

**Wait time**: 3-5 minutes (much faster since we're not building the whole workspace)

## ✅ After Build Completes

### Step 1: Verify WASM File
```bash
ls -lh ~/alkanes-rs/target/wasm32-unknown-unknown/release/alkanes.wasm
```

Should show a file (not directory) with size around 1-2 MB.

### Step 2: Start All Docker Services
```bash
cd ~/alkanes-rs
docker-compose up -d
```

Now metashrew should start successfully!

### Step 3: Verify Services Running
```bash
docker-compose ps
```

All services should show "Up".

### Step 4: Test Generate Future
```bash
./target/release/alkanes-cli -p regtest bitcoind generatefuture
```

### Step 5: Check the Future Was Created
```bash
# Wait a moment for indexer to process
sleep 5

# Get current block height
BLOCK=$(./target/release/alkanes-cli -p regtest bitcoind getblockcount)
echo "Checking future at block $BLOCK"

# Inspect the future
./target/release/alkanes-cli -p regtest alkanes inspect 31:$BLOCK
```

Should show bytecode length > 0!

### Step 6: Test in Browser

1. Make sure app is running:
   ```bash
   cd ~/subfrost-app
   yarn dev
   ```

2. Open: http://localhost:3000/futures

3. Click "Generate Future" button

4. Should see **REAL futures** in the table!

## 🎯 If Build Fails

**Error**: "wasm32-unknown-unknown target not installed"

**Fix**:
```bash
rustup target add wasm32-unknown-unknown
```

Then retry: `./build-wasm.sh`

## 🎉 Success!

Once the build completes and docker services start, you'll have:
- ✅ Real blockchain data
- ✅ Working futures generation
- ✅ Live indexer processing blocks
- ✅ Full integration working end-to-end!

## 🚀 Summary

**Just run**:
```bash
cd ~/alkanes-rs
./build-wasm.sh
```

Wait for it to finish, then:
```bash
docker-compose up -d
```

Then test in the browser! 🎊
