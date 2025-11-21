# UTXO Selection and Transaction Building Issues

## Current Status

The `alkanes execute` command consistently fails with "An absurdly high fee rate" errors, preventing any alkane operations from working. This document outlines the root causes and required fixes.

## Root Causes

### 1. **UTXO Selection Logic Issues**

**Location:** `crates/alkanes-cli-common/src/alkanes/execute.rs::select_utxos()`

**Problems:**
- The UTXO selection doesn't properly account for the actual requirements when building fees
- Early termination of UTXO selection can leave insufficient funds for fees
- No validation that selected UTXOs can cover outputs + fees before proceeding

**Code Issues:**
```rust
// Line 382-556: select_utxos function
// Problem: Stops selecting UTXOs as soon as bitcoin_collected >= bitcoin_needed
// But bitcoin_needed doesn't include the transaction fee yet!

if bitcoin_collected >= bitcoin_needed && all_alkanes_satisfied {
    break;  // ← BREAKS TOO EARLY!
}
```

### 2. **Fee Calculation Timing**

**Location:** `crates/alkanes-cli-common/src/alkanes/execute.rs::build_single_transaction()`

**Problems:**
- Fee is calculated AFTER UTXO selection
- UTXO selection uses `bitcoin_needed` from InputRequirements, which doesn't include fees
- This creates a circular dependency: need to know fees to select UTXOs, but need UTXOs to calculate fees

**Current Flow (BROKEN):**
```
1. Parse requirements (e.g., "need 50,000 sats")
2. Select UTXOs for exactly 50,000 sats
3. Build transaction → calculate fee (e.g., 5,000 sats)
4. Total needed: 55,000 sats
5. ERROR: Only have 50,000 sats in selected UTXOs!
```

### 3. **Missing Fee Buffer in Requirements**

**Location:** `crates/alkanes-cli-common/src/alkanes/execute.rs:290-318`

**Problem:**
```rust
// Line 290-318
let estimated_reveal_amount = outputs.iter()
    .filter(|o| o.value.to_sat() > 0)
    .map(|o| o.value.to_sat())
    .sum::<u64>();

// ← No fee buffer added here!
let mut final_requirements = input_requirements.clone();
final_requirements.push(InputRequirement::Bitcoin { amount: estimated_reveal_amount });
```

The code estimates output amounts but doesn't add a fee buffer, so UTXO selection gets insufficient funds.

### 4. **High Default Fee Rate**

**Location:** `crates/alkanes-cli-common/src/alkanes/execute.rs:756`

**Problem:**
```rust
let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);  // 600 sat/vB default is very high!
```

While high fees aren't inherently wrong, combined with the UTXO selection issues, this exacerbates the problem.

### 5. **Bitcoin Core Rejection**

When the transaction is finally built:
- Selected UTXOs: 50,000 sats
- Outputs: 50,000 sats
- Fee: 0 sats (no room for fee!)
- Bitcoin Core sees: fee_rate = 0 / vsize → broadcasts → Bitcoin Core calculates actual implicit fee
- Or worse: fee_rate = (input - output) / vsize = absurdly high number

## Required Fixes

### Fix 1: Add Fee Estimation to UTXO Selection

**Approach:** Estimate fees BEFORE UTXO selection, add to requirements.

```rust
// In build_single_transaction, before select_utxos:

// Step 1: Estimate transaction size
let estimated_tx_size = estimate_transaction_vsize(
    1, // Initial estimate: 1 input
    outputs.len(),
    has_envelope,
    has_runestone,
);

// Step 2: Calculate estimated fee with buffer
let fee_rate_sat_vb = params.fee_rate.unwrap_or(10.0); // Lower default!
let estimated_fee = (fee_rate_sat_vb * estimated_tx_size as f32).ceil() as u64;
let fee_with_buffer = (estimated_fee as f64 * 1.5).ceil() as u64; // 50% buffer

// Step 3: Add fee to Bitcoin requirements
let total_bitcoin_needed = estimated_reveal_amount + fee_with_buffer;
final_requirements.push(InputRequirement::Bitcoin { amount: total_bitcoin_needed });

// Step 4: Select UTXOs with fee included
let selected_utxos = self.select_utxos(&final_requirements, &params.from_addresses).await?;

// Step 5: Recalculate actual fee with real input count
let actual_tx_size = estimate_transaction_vsize(
    selected_utxos.len(),
    outputs.len(),
    has_envelope,
    has_runestone,
);
let actual_fee = (fee_rate_sat_vb * actual_tx_size as f32).ceil() as u64;
```

### Fix 2: Iterative UTXO Selection with Fee Feedback

**Approach:** Use iterative refinement to converge on correct UTXO set.

```rust
fn select_utxos_with_fees(
    &mut self,
    requirements: &[InputRequirement],
    fee_rate: f32,
    outputs: &[TxOut],
    has_envelope: bool,
) -> Result<Vec<OutPoint>> {
    let mut selected_utxos = Vec::new();
    let mut iteration = 0;
    const MAX_ITERATIONS: usize = 5;
    
    loop {
        // Estimate fee for current UTXO count
        let estimated_vsize = estimate_transaction_vsize(
            selected_utxos.len().max(1),
            outputs.len(),
            has_envelope,
            true, // has runestone
        );
        let estimated_fee = (fee_rate * estimated_vsize as f32).ceil() as u64;
        
        // Add fee to requirements
        let mut reqs_with_fee = requirements.to_vec();
        let bitcoin_needed: u64 = requirements.iter()
            .filter_map(|r| match r {
                InputRequirement::Bitcoin { amount } => Some(*amount),
                _ => None,
            })
            .sum();
        
        reqs_with_fee.push(InputRequirement::Bitcoin { 
            amount: bitcoin_needed + estimated_fee 
        });
        
        // Select UTXOs
        let new_selection = self.select_utxos_internal(&reqs_with_fee)?;
        
        // Check if selection changed
        if new_selection.len() == selected_utxos.len() || iteration >= MAX_ITERATIONS {
            selected_utxos = new_selection;
            break;
        }
        
        selected_utxos = new_selection;
        iteration += 1;
    }
    
    Ok(selected_utxos)
}
```

### Fix 3: Validate Before Building

**Approach:** Add validation step before PSBT construction.

```rust
fn validate_utxo_selection(
    selected_utxos: &[(OutPoint, UtxoInfo)],
    outputs: &[TxOut],
    estimated_fee: u64,
) -> Result<()> {
    let total_input: u64 = selected_utxos.iter().map(|(_, u)| u.amount).sum();
    let total_output: u64 = outputs.iter().map(|o| o.value.to_sat()).sum();
    let required_total = total_output + estimated_fee;
    
    if total_input < required_total {
        return Err(AlkanesError::Wallet(format!(
            "Insufficient funds: have {} sats, need {} sats ({} outputs + {} fee)",
            total_input, required_total, total_output, estimated_fee
        )));
    }
    
    // Check for dust
    let change = total_input - required_total;
    if change > 0 && change < 546 {
        log::warn!("Change {} sats is below dust threshold, will be added to fee", change);
    }
    
    Ok(())
}
```

### Fix 4: Lower Default Fee Rate

**Change:**
```rust
// OLD:
let fee_rate_sat_vb = fee_rate.unwrap_or(600.0);

// NEW:
let fee_rate_sat_vb = fee_rate.unwrap_or(10.0);  // Reasonable for regtest
```

Or make it network-dependent:
```rust
let default_fee_rate = match self.provider.get_network() {
    Network::Bitcoin => 50.0,    // Mainnet: higher
    Network::Testnet => 10.0,    // Testnet: moderate
    Network::Regtest => 1.0,     // Regtest: minimal
    Network::Signet => 10.0,     // Signet: moderate
    _ => 10.0,
};
let fee_rate_sat_vb = fee_rate.unwrap_or(default_fee_rate);
```

### Fix 5: Better Error Messages

When UTXO selection fails, provide detailed diagnostics:

```rust
Err(AlkanesError::Wallet(format!(
    "UTXO selection failed:\n\
     Requirements:\n\
       - Bitcoin: {} sats\n\
       - Alkanes: {} types\n\
     Available:\n\
       - Total UTXOs: {}\n\
       - Spendable UTXOs: {}\n\
       - Total Bitcoin: {} sats\n\
       - Frozen: {} UTXOs\n\
       - Immature: {} UTXOs\n\
     Estimated fee: {} sats\n\
     Shortfall: {} sats",
    bitcoin_needed,
    alkanes_needed.len(),
    all_utxos.len(),
    spendable_utxos.len(),
    total_available,
    frozen_count,
    immature_count,
    estimated_fee,
    shortfall,
)))
```

## Implementation Plan

### Phase 1: Core Fixes (HIGH PRIORITY)
1. ✅ Create comprehensive test suite (`transaction_builder_tests.rs`)
2. ⬜ Implement `estimate_transaction_vsize()` helper function
3. ⬜ Add fee estimation before UTXO selection in `build_single_transaction()`
4. ⬜ Fix UTXO selection to include fee in requirements
5. ⬜ Add validation before PSBT construction
6. ⬜ Lower default fee rate and make it network-dependent

### Phase 2: Optimization (MEDIUM PRIORITY)
7. ⬜ Implement iterative UTXO selection with fee feedback
8. ⬜ Optimize UTXO selection to prefer fewer, larger UTXOs
9. ⬜ Add UTXO consolidation suggestions when fragmented
10. ⬜ Implement coin selection algorithms (e.g., Branch and Bound)

### Phase 3: Testing & Polish (MEDIUM PRIORITY)
11. ⬜ Write unit tests for all UTXO selection scenarios
12. ⬜ Write integration tests with mock provider
13. ⬜ Test edge cases (dust, frozen UTXOs, immature coinbase)
14. ⬜ Add property-based tests with quickcheck

### Phase 4: Documentation (LOW PRIORITY)
15. ⬜ Document UTXO selection algorithm
16. ⬜ Add troubleshooting guide for common errors
17. ⬜ Create examples for different transaction types

## Testing Strategy

### Unit Tests
- Test UTXO filtering (frozen, immature coinbase)
- Test fee calculation for different tx sizes
- Test change calculation and dust handling
- Test error cases (insufficient funds, no UTXOs)

### Integration Tests
- Test full transaction building flow
- Test with mock provider returning specific UTXO sets
- Test alkane balance queries
- Test multi-protostone transactions

### Regression Tests
- Test that factory init works after fixes
- Test pool creation works
- Test simple transfers work
- Test contract deployments work

## Success Criteria

After implementing these fixes:
1. ✅ `alkanes execute` commands succeed without "absurdly high fee rate" errors
2. ✅ Fee calculations are accurate and reasonable
3. ✅ UTXO selection includes sufficient funds for fees
4. ✅ Clear error messages when funds are insufficient
5. ✅ All tests pass
6. ✅ Factory initialization works
7. ✅ Pool creation works

## Related Files

- `crates/alkanes-cli-common/src/alkanes/execute.rs` - Main execution logic
- `crates/alkanes-cli-common/src/alkanes/fee_validation.rs` - Fee validation
- `crates/alkanes-cli-common/src/traits.rs` - Provider traits
- `crates/alkanes-cli-common/src/provider.rs` - Concrete provider implementation
- `crates/alkanes-cli-common/src/tests/transaction_builder_tests.rs` - Test suite

## Notes

- The current code has a fundamental architectural issue: fee estimation happens after UTXO selection
- This requires either:
  1. Iterative refinement (select → calculate fee → reselect if needed)
  2. Upfront estimation with generous buffer
  3. Hybrid approach (estimate first, refine after)
- Option 2 (upfront estimation with buffer) is simplest and most reliable for initial fix
- Option 1 (iterative) can be added later for optimization

## Current Blockers

**Factory Initialization:** Cannot proceed with factory init testing until UTXO selection is fixed, as all `alkanes execute` commands fail.

**Multi-Protostone Transactions:** Need working UTXO selection before we can test the multi-protostone approach for factory init with edicts and alkane chaining (e.g., minting alkane 2:1, then using it in factory init edict).
