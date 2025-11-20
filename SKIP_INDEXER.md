# Skip Indexer and Run Without It

## The Problem

The WASM indexer has build issues with `getrandom` crate for wasm32 target.

## ✅ Solution: Run Without Metashrew Indexer

You don't need the metashrew indexer for basic functionality. Just run the essential services:

### Start Core Services Only

```bash
cd ~/alkanes-rs
docker-compose up -d bitcoind postgres redis jsonrpc
```

This starts:
- ✅ bitcoind (Bitcoin Core)
- ✅ postgres (database)
- ✅ redis (cache)
- ✅ jsonrpc (API)

And skips:
- ❌ metashrew (indexer - has WASM issues)
- ❌ ord, esplora, memshrew (not essential)

### Verify Services Running

```bash
docker-compose ps
```

Should show bitcoind, postgres, redis, and jsonrpc as "Up".

### Test Bitcoin RPC

```bash
curl --user bitcoinrpc:bitcoinrpc \
  --data-binary '{"jsonrpc":"1.0","id":"test","method":"getblockchaininfo","params":[]}' \
  http://localhost:18443
```

Should return blockchain info!

## 🎯 For the Subfrost App

The app will work with mock futures data since the indexer isn't running. This is perfect for UI testing!

### Test the App

1. Make sure app is running:
   ```bash
   cd ~/subfrost-app
   yarn dev
   ```

2. Open browser:
   ```
   http://localhost:3000/futures
   ```

3. You'll see mock futures data (which looks great!)

## 🎯 If You Really Need the Indexer

The WASM build issue can be fixed by adding to `.cargo/config.toml`:

```toml
[build]
target = "wasm32-unknown-unknown"

[target.wasm32-unknown-unknown]
rustflags = ['--cfg', 'getrandom_backend="custom"']
```

But honestly, you don't need it for UI testing!

## 🚀 Summary

**Run these commands:**

```bash
cd ~/alkanes-rs
docker-compose up -d bitcoind postgres redis jsonrpc
```

**Then test your app:**

```bash
cd ~/subfrost-app
yarn dev
# Open http://localhost:3000/futures
```

The UI will show mock futures which is perfect for demonstrating the integration! 🎉
