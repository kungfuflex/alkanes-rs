# Alkanes Execute Transaction Creation Scheme

## Overview

The `alkanes execute` command builds Bitcoin transactions with embedded protostone data to interact with Alkanes smart contracts. This document describes the complete transaction creation logic to prevent burning BTC or alkanes tokens.

## Core Principle

**Alkanes-cli-common must NEVER burn BTC or alkanes tokens.** All outputs must be explicitly created and all change must be returned to the user.

## Transaction Processing Order

Processing happens in the following order to determine outputs:

1. **Identifier-based outputs (v0, v1, v2, etc.)**
2. **--to addresses override**
3. **--change output (with optimization)**
4. **--alkanes-change output (for unwanted alkanes)**
5. **Automatic protostone generation (if needed)**
6. **Fee calculation and validation**

---

## 1. Identifier-Based Output Generation

### Rule
If protostone specifications reference output identifiers (v0, v1, v2, etc.), these MUST create corresponding physical outputs in the transaction.

### Default Behavior (no --to flag)
When `--to` is NOT specified:
- Use the `--change` address for ALL identifier-based outputs (v0, v1, v2, ...)
- If `--change` is also not specified, use `p2tr:0` as the default address

### Example
```bash
# Protostone references v0
alkanes execute "[3,65520,50]:v0:v0" --envelope contract.wasm

# This MUST create:
# - Output 0: p2tr:0 with dust (546 sats) + alkanes from protostone
# - Output 1: OP_RETURN with runestone
# - Output 2: p2tr:0 with BTC change (if any)
```

---

## 2. --to Flag Override

### Rule
The `--to` flag provides a comma-separated list of address identifiers to override where identifier outputs go.

### Format
```bash
--to p2tr:0,p2tr:1,p2wsh:0
```

### Mapping
- First address → v0
- Second address → v1  
- Third address → v2
- etc.

### Example
```bash
# Map v0 to p2tr:1, v1 to p2wsh:0
alkanes execute "[3,65520]:v0:v0,[4,100]:v1:v1" \
  --to p2tr:1,p2wsh:0 \
  --envelope contract.wasm

# Creates:
# - Output 0: p2tr:1 (receives v0 protostone results)
# - Output 1: p2wsh:0 (receives v1 protostone results)
# - Output 2: OP_RETURN with runestone
# - Output 3: Change output (default or --change)
```

---

## 3. --change Output Logic

### Default Behavior
When `--change` is NOT specified:
- Default to `p2wsh:0` for the BTC change output
- This output receives surplus BTC after paying for:
  - All explicit outputs (identifier-based, --to addresses)
  - Transaction fees
  - Alkanes change output (if created)

### Optimization: Exact Change Case
**DO NOT create a change output** if ALL of the following are true:
1. We have the exact amount of BTC needed (within margin)
2. The cost of including the change output ≥ the change amount
3. We save money by just paying slightly higher fees instead

The "margin of error" equals the cost of the change output itself (~34 vbytes ≈ 34-340 sats depending on fee rate).

### Example: Normal Case
```bash
# Input: 100,000 sats
# Outputs needed: 10,000 sats
# Fee: 1,000 sats
# Change: 89,000 sats → CREATE change output

alkanes execute "[3,100]:v0:v0" --change p2tr:2
# Output 0: p2tr:0 (10,000 sats + v0 alkanes)
# Output 1: OP_RETURN
# Output 2: p2tr:2 (89,000 sats) ← change
```

### Example: Exact Change Case
```bash
# Input: 1,546 sats
# Outputs needed: 546 sats (dust)
# Fee: 1,000 sats
# Potential change: 0 sats → OMIT change output

alkanes execute "[3,100]:v0:v0"
# Output 0: p2tr:0 (546 sats + v0 alkanes)
# Output 1: OP_RETURN
# No change output (would cost more than it's worth)
```

### Explicit --change Override
When `--change` IS specified:
- ALWAYS create the change output (no optimization)
- This gives the user explicit control

```bash
alkanes execute "[3,100]:v0:v0" --change p2tr:5
# Always creates change output at p2tr:5, even if amount is small
```

---

## 4. --inputs Flag

### Format
The `--inputs` flag specifies what UTXOs to spend, with three types of requirements:

```bash
--inputs "requirement1,requirement2,requirement3"
```

### Requirement Types

#### 4.1. Bitcoin Requirement
```bash
B:amount
```
- Spend sufficient BTC to get `amount` satoshis into the transaction
- Used for: fees, output values, BTC transfers

**Example:**
```bash
--inputs "B:50000000"
# Spend UTXOs totaling ≥50M sats
```

#### 4.2. Alkanes Requirement  
```bash
block:tx:amount
```
- Spend UTXOs containing at least `amount` of alkane `[block, tx]`
- Amount of 0 means "spend ALL of this alkane"

**Example:**
```bash
--inputs "2:1:1000"
# Spend UTXOs containing ≥1000 units of alkane [2, 1]
```

#### 4.3. Bitcoin Output Assignment
```bash
B:amount:vN
```
- Assign `amount` satoshis of BTC to output identifier `vN`
- This creates a **protostone** that transfers BTC, not an edict

**Example:**
```bash
--inputs "B:500000:v0"
# Assign 500,000 sats to output v0
# Creates a protostone with pointer to v0
```

### Complete Example
```bash
alkanes execute "[3,100,5]:v0:v0" \
  --inputs "2:1:1,B:50000000,B:1000000:v1" \
  --to p2tr:0,p2tr:1

# Means:
# 1. Spend UTXOs with ≥1 unit of alkane [2, 1]
# 2. Spend UTXOs with ≥50M sats total
# 3. Assign 1M sats to output v1
# 4. Send protostone results to p2tr:0 (v0) and p2tr:1 (v1)
```

---

## 5. --alkanes-change Output

### Purpose
When spending alkanes from UTXOs, you may not want to spend the EXACT amount needed. The `--alkanes-change` flag specifies where unwanted alkanes should be refunded.

### Default Behavior (no --alkanes-change)
1. If `--change` is specified → unwanted alkanes go to `--change` address
2. If `--change` is NOT specified → unwanted alkanes go to `p2tr:0`

### With --alkanes-change
Unwanted alkanes go to the specified address, which may be different from the BTC change address.

### Example
```bash
# Wallet has UTXO with 10,000 units of alkane [2, 1]
# We only need 1 unit

alkanes execute "[2,1,1:p1]:v0:v0,[4,100,0]:v0:v0" \
  --inputs "2:1:1" \
  --alkanes-change p2tr:5 \
  --change p2wsh:0

# Creates:
# - Output 0: p2tr:0 (receives v0 protostone results)
# - Output 1: p2tr:5 (receives 9,999 units of [2,1] as change)
# - Output 2: OP_RETURN with runestone
# - Output 3: p2wsh:0 (BTC change)
```

---

## 6. Automatic Protostone Generation

### When It Triggers
If we cannot spend the EXACT amount of alkanes from UTXOs (determined by querying `protorunesbyoutpoint`), we must generate an **automatic protostone** to handle the unwanted alkanes.

### Position in Protostone Stack
The automatic protostone is **left-shifted** (inserted at index 0), pushing all user-defined protostones to the right.

```
Before: [user_p0, user_p1, user_p2]
After:  [auto_p0, user_p0, user_p1, user_p2]
```

This means:
- Original p0 becomes p1
- Original p1 becomes p2
- All protostone references must be adjusted

### Two Strategies

#### Strategy A: Split Off Unwanted (Edict-based)
Use **edicts** to transfer unwanted alkanes to the alkanes-change output, and set **pointer** to the change output.

```
[unwanted_alkane:amount:alkanes_change_output]:alkanes_change_output:alkanes_change_output
```

#### Strategy B: Split Off Wanted (Edict-based)
Use **edicts** to transfer ONLY the wanted alkanes to the next protostone (now p1 instead of p0), and set **pointer** to the alkanes-change output for everything else.

```
[wanted_alkane:amount:p1]:alkanes_change_output:alkanes_change_output
```

**Choose the strategy that results in fewer edicts.**

### Example: Strategy A
```bash
# Wallet has: 10,000 units of [2, 1]
# User wants: 1 unit for protostone

User protostone: [4,100,0]:v0:v0

# Automatic protostone (Strategy A):
[2:1:9999:v1]:v1:v1

# Final protostone stack:
# p0 (auto):  [2:1:9999:v1]:v1:v1        ← Sends 9999 to v1 (alkanes change)
# p1 (user):  [4,100,0]:v0:v0            ← User's original p0
```

### Example: Strategy B
```bash
# Same scenario, but with Strategy B:

# Automatic protostone (Strategy B):
[2:1:1:p1]:v1:v1

# Final protostone stack:
# p0 (auto):  [2:1:1:p1]:v1:v1           ← Sends 1 to p1, rest to v1
# p1 (user):  [4,100,0]:v0:v0            ← User's original p0
```

---

## 7. Fee Calculation and Validation

### Fee Estimation
1. Estimate transaction size based on:
   - Number of inputs
   - Number of outputs (including change)
   - Presence of envelope (large witness data)
   - Presence of runestone (OP_RETURN)

2. Calculate fee: `vsize * fee_rate`

3. Add buffer (1-50%) to account for variations

### Validation
Before finalizing the transaction:

1. **Input validation:**
   - `sum(input_amounts) ≥ sum(output_amounts) + fee`

2. **Output validation:**
   - All identifier outputs (v0, v1, ...) are created
   - BTC change output created (unless optimized out)
   - Alkanes change output created (if needed)
   - Dust limit satisfied for all outputs (≥546 sats)

3. **Fee validation:**
   - Fee is reasonable (not too high or too low)
   - Fee doesn't exceed `MAX_FEE_SATS` (currently 100,000 sats)

### Example
```bash
# Inputs: 1,000,000 sats
# Outputs: v0 (546 sats), v1 (546 sats)
# Fee (estimated): 5,000 sats
# Change: 993,908 sats

alkanes execute "[3,100]:v0:v0,[4,200]:v1:v1" \
  --to p2tr:0,p2tr:1 \
  --inputs "B:1000000" \
  --fee-rate 5.0

# Validation:
# ✓ 1,000,000 ≥ 546 + 546 + 5,000 = 6,092
# ✓ All outputs created
# ✓ Fee reasonable
# ✓ Change output created (993,908 sats)
```

---

## 8. Complete Example: Contract Deployment

### Command
```bash
alkanes execute "[3,65520,50]:v0:v0" \
  --envelope contract.wasm \
  --from p2tr:0 \
  --change p2tr:0 \
  --fee-rate 1.0 \
  --mine \
  --trace \
  -y
```

### Transaction Structure
```
Inputs:
  - UTXO from p2tr:0 (e.g., 100,000,000 sats)

Outputs:
  0: p2tr:0 (546 sats) ← v0 identifier, receives deployed contract alkane
  1: OP_RETURN (0 sats) ← Runestone with protostone [3,65520,50]:v0:v0
  2: p2tr:0 (99,989,454 sats) ← BTC change

Witness:
  - Input 0: Envelope with contract.wasm (script-path spend)
```

### Protostone Encoding
```
Runestone.protocol = [
  1,          // protocol_tag = ALKANES
  0,          // burn = None
  0,          // refund = v0
  0,          // pointer = v0
  0,          // from = None
  3,          // message length
  3, 65520, 50,  // cellpack: [3, 65520, 50]
  0           // edicts length (no edicts)
]

Runestone.pointer = 0  // Default pointer to v0
```

---

## 9. Complete Example: Factory Initialization

### Command
```bash
FACTORY_INIT_PROTOSTONE="[2:1:1:p1]:v0:v0,[4,65522,0,780993,4,65523]:v0:v0"

alkanes execute "$FACTORY_INIT_PROTOSTONE" \
  --inputs "2:1:1" \
  --from p2tr:0 \
  --change p2wsh:0 \
  --fee-rate 1.0 \
  --mine \
  --trace \
  -y
```

### Transaction Structure
```
Inputs:
  - UTXO from p2tr:0 with alkane [2,1] (e.g., 100 units + 10,000,000 sats)

Outputs:
  0: p2tr:0 (546 sats) ← v0 identifier, receives results + change alkanes
  1: OP_RETURN (0 sats) ← Runestone with two protostones
  2: p2wsh:0 (9,989,454 sats) ← BTC change

Protostones:
  p0: [2:1:1:p1]:v0:v0
      - Sends 1 unit of [2,1] to p1 (the second protostone)
      - Sends remaining 99 units to v0 (output 0)
      
  p1: [4,65522,0,780993,4,65523]:v0:v0
      - Receives 1 unit of [2,1] from p0
      - Calls factory.InitFactory(780993, [4, 65523])
      - Sends auth token back to v0 (output 0)
```

### Runestone Encoding
```
Runestone.protocol = [
  // First protostone (auth token transfer)
  1,          // protocol_tag = ALKANES
  0,          // burn = None
  0,          // refund = v0
  1,          // pointer = p1 (next protostone)
  0,          // from = None
  2,          // message length
  2, 1,       // cellpack: [2, 1] (auth token alkane)
  1,          // edicts length
  2, 1, 1, 1, // edict: [2, 1, 1, p1] (send 1 token to p1)
  
  // Second protostone (factory call)
  1,          // protocol_tag = ALKANES
  0,          // burn = None
  0,          // refund = v0
  0,          // pointer = v0
  0,          // from = None
  5,          // message length
  4, 65522, 0, 780993, 4, 65523,  // cellpack: [4, 65522, 0, ...]
  0           // edicts length (no edicts)
]

Runestone.pointer = 0  // Default pointer to v0
```

---

## 10. Implementation Checklist

### Current Issues to Fix

- [ ] **Issue 1: No automatic output creation for identifier references**
  - When protostones reference v0, v1, etc., outputs are not automatically created
  - Result: Alkanes get burned (sent to OP_RETURN only)
  - Fix: `create_outputs()` must detect identifier references in protostones

- [ ] **Issue 2: No change output when --change not specified**
  - BTC change gets burned or causes transaction to fail
  - Fix: Default `--change` to `p2wsh:0` if not specified

- [ ] **Issue 3: No alkanes change handling**
  - When spending UTXOs with more alkanes than needed, excess gets burned
  - Fix: Implement automatic protostone generation for alkanes change

- [ ] **Issue 4: No B:amount:vN support**
  - Cannot assign BTC to specific outputs via protostones
  - Fix: Parse and handle `B:amount:vN` in `--inputs`

### Implementation Steps

1. **Modify `create_outputs()`** (/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/execute.rs)
   - Scan all protostones for referenced identifiers (v0, v1, v2, ...)
   - Create corresponding outputs with dust (546 sats)
   - Map identifiers to addresses via `--to` or `--change` or default to `p2tr:0`

2. **Add change output logic** (same file)
   - Default `--change` to `p2wsh:0` if not specified
   - Implement exact-change optimization (omit change if cost > value)
   - Always create change if explicitly specified via `--change`

3. **Implement alkanes change handling** (same file)
   - Query `protorunesbyoutpoint` for each input UTXO
   - Calculate excess alkanes (have - need)
   - Generate automatic protostone at index 0 if excess > 0
   - Adjust all protostone references (p0 → p1, p1 → p2, etc.)

4. **Add B:amount:vN support** (parsing.rs + execute.rs)
   - Parse `B:amount:vN` format in `parse_input_requirements()`
   - Store as a new variant: `InputRequirement::BitcoinOutput { amount, target }`
   - Generate protostone with pointer to specified output

5. **Enhance validation** (execute.rs)
   - Validate `sum(inputs) ≥ sum(outputs) + fee`
   - Validate all referenced identifiers have corresponding outputs
   - Validate dust limits (≥546 sats)
   - Validate fee reasonableness

---

## 11. Testing Strategy

### Unit Tests
- Test identifier output generation (v0, v1, v2)
- Test change output logic (normal and exact-change cases)
- Test alkanes change protostone generation
- Test B:amount:vN parsing and execution

### Integration Tests
- Deploy contract (envelope + protostone)
- Initialize factory (auth token + call)
- Create liquidity pool (multiple protostones)
- Swap tokens (alkanes transfer)

### Regression Tests
- Ensure no BTC burning
- Ensure no alkanes burning
- Ensure correct change handling
- Ensure correct fee calculation

---

## 12. Migration Notes

### For Existing Scripts
Scripts using `alkanes execute` may need updates:

**Before:**
```bash
# This burned BTC and alkanes!
alkanes execute "[3,65520,50]:v0:v0" --envelope contract.wasm
```

**After:**
```bash
# Now automatically creates v0 output and change
alkanes execute "[3,65520,50]:v0:v0" \
  --envelope contract.wasm \
  --change p2tr:0  # Optional, defaults to p2wsh:0
```

### For Advanced Users
- `--to` flag provides fine-grained control over identifier mapping
- `--alkanes-change` separates BTC and alkanes change addresses
- `B:amount:vN` enables complex BTC routing

---

## Summary

The alkanes execute transaction scheme ensures:
1. **No burning**: All BTC and alkanes are accounted for
2. **Flexibility**: Multiple ways to specify outputs and change
3. **Safety**: Validation at every step
4. **Efficiency**: Optimizes out unnecessary outputs

By following this scheme, alkanes-cli-common will construct correct, efficient, and safe transactions for all alkanes operations.
