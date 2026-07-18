# Default Factory ID Configuration

## Summary

All AMM and DataAPI commands now default to factory ID **`4:65522`** when `--factory` is not supplied.

---

## ✅ Commands with Default Factory

### 1. DataAPI Get Pools
```bash
# Both work the same:
alkanes-cli alkanes dataapi get-pools
alkanes-cli alkanes dataapi get-pools --factory 4:65522

# Use different factory:
alkanes-cli alkanes dataapi get-pools --factory 5:12345
```

**Definition** (`commands.rs:1059-1064`):
```rust
/// Get all pools (defaults to factory 4:65522)
GetPools {
    /// Factory ID in format BLOCK:TX
    #[arg(long, default_value = "4:65522")]
    factory: String,
}
```

---

### 2. Init Pool
```bash
# Both work the same:
alkanes-cli alkanes init-pool \
    --pair 2:0,32:0 \
    --liquidity 300000000:50000 \
    --to p2tr:0 \
    --from p2tr:0

alkanes-cli alkanes init-pool \
    --pair 2:0,32:0 \
    --liquidity 300000000:50000 \
    --to p2tr:0 \
    --from p2tr:0 \
    --factory 4:65522
```

**Definition** (`commands.rs:996-997`):
```rust
/// Factory ID (defaults to 4:65522)
#[arg(long, default_value = "4:65522")]
factory: String,
```

---

### 3. Swap
```bash
# Both work the same:
alkanes-cli alkanes swap \
    --path 2:0:32:0 \
    --input 1000000 \
    --minimum 100 \
    --to p2tr:0 \
    --from p2tr:0

alkanes-cli alkanes swap \
    --path 2:0:32:0 \
    --input 1000000 \
    --minimum 100 \
    --to p2tr:0 \
    --from p2tr:0 \
    --factory 4:65522
```

**Definition** (`commands.rs:1030-1031`):
```rust
/// Factory ID (defaults to 4:65522)
#[arg(long, default_value = "4:65522")]
factory: String,
```

---

## 📜 Deploy Script Updated

**File**: `scripts/deploy-regtest-diesel-pool.sh`

The deploy script now omits `--factory` flag, relying on the default:

```bash
# Before:
alkanes-cli alkanes init-pool \
    --pair "$DIESEL_ID,$FRBTC_ID" \
    --liquidity "$DIESEL_AMOUNT:$FRBTC_AMOUNT" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --factory $FACTORY \  # Explicitly passed
    --trace

# After:
alkanes-cli alkanes init-pool \
    --pair "$DIESEL_ID,$FRBTC_ID" \
    --liquidity "$DIESEL_AMOUNT:$FRBTC_AMOUNT" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --trace
# Note: --factory defaults to $FACTORY (4:65522)
```

**Query instructions updated**:
```bash
# Before:
echo "  alkanes-cli alkanes dataapi get-pools --factory $FACTORY"

# After:
echo "  alkanes-cli alkanes dataapi get-pools"
echo "  (factory defaults to $FACTORY)"
```

---

## 🎯 Benefits

1. **Simpler Commands**: Less typing for the common case
2. **Cleaner Examples**: Documentation is more concise
3. **Flexibility**: Can still override with `--factory <id>`
4. **Consistency**: All factory-related commands use same default

---

## 🧪 Testing

### Test Default Behavior
```bash
# Should work without --factory:
alkanes-cli alkanes dataapi get-pools

# Should work without --factory:
alkanes-cli alkanes init-pool \
    --pair 2:0,32:0 \
    --liquidity 300000000:50000 \
    --to p2tr:0 \
    --from p2tr:0
```

### Test Custom Factory
```bash
# Should use custom factory:
alkanes-cli alkanes dataapi get-pools --factory 5:12345

# Should use custom factory:
alkanes-cli alkanes init-pool \
    --pair 2:0,32:0 \
    --liquidity 300000000:50000 \
    --to p2tr:0 \
    --from p2tr:0 \
    --factory 5:12345
```

---

## 📝 Factory ID Reference

**Default Factory**: `4:65522`

This is the standard Alkanes AMM factory contract on:
- Mainnet
- Signet  
- Regtest

If deploying to a different network or using a different factory, supply `--factory` explicitly.

---

## ✅ Verification

All three commands checked:
- ✅ `dataapi get-pools` - Has `default_value = "4:65522"`
- ✅ `init-pool` - Has `default_value = "4:65522"`
- ✅ `swap` - Has `default_value = "4:65522"`
- ✅ Deploy script - Omits `--factory`, includes note about default

**Compilation Status**: ✅ SUCCESS
```bash
$ cargo check --package alkanes-cli
    Finished `dev` profile in 5.63s
```

---

*Updated: November 20, 2025*
*All factory-related commands now default to 4:65522*
