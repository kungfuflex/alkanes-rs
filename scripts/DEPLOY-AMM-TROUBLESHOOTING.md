# OYL AMM Deployment Troubleshooting Guide

This document explains the auth token authentication pattern used in the OYL AMM deployment and provides troubleshooting steps for the factory initialization.

## Overview

The OYL AMM factory requires authentication via an auth token to prevent unauthorized initialization. This is implemented using a two-protostone pattern that passes an auth token from the first protostone to the second.

## Auth Token Pattern Explained

### The Problem

The factory's `InitFactory` opcode (opcode 0) needs to be authenticated. Only the auth token holder should be able to initialize the factory configuration.

### The Solution

A two-protostone transaction where:
1. First protostone sends auth token to the second protostone
2. Second protostone uses the auth token to authenticate the factory call
3. Auth token is returned to the wallet after authentication

### Protostone Format

```bash
[2:1:1:p1]:v0:v0,[4,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0
```

Breaking this down:

#### First Protostone: `[2:1:1:p1]:v0:v0`
- `[2:1:1:p1]` - Cellpack notation
  - `2:1` - Auth token alkane ID (block 2, tx 1)
  - `1` - Amount to send (1 unit)
  - `p1` - Destination: physical output 1 (the next protostone in the same TX)
- `:v0:v0` - Pointer and refund outputs
  - First `v0` - Pointer target (virtual output 0)
  - Second `v0` - Refund target (virtual output 0, for change)

#### Second Protostone: `[4,$AMM_FACTORY_PROXY_TX,0,$POOL_BEACON_PROXY_TX,4,$POOL_UPGRADEABLE_BEACON_TX]:v0:v0`
- `[4,$AMM_FACTORY_PROXY_TX,0,...]` - Cellpack notation for factory call
  - `4:$AMM_FACTORY_PROXY_TX` - Factory alkane ID (block 4, tx 65522)
  - `0` - Opcode for InitFactory
  - `$POOL_BEACON_PROXY_TX` - Pool beacon proxy ID (tx 780993)
  - `4:$POOL_UPGRADEABLE_BEACON_TX` - Pool beacon ID (block 4, tx 65523)
- `:v0:v0` - Returns auth token to virtual output 0 after the call

### Transaction Structure

```
TX Inputs:
  - Input 0: Bitcoin UTXO (for fees)
  - Input 1: Auth token UTXO [2:1] with 1+ units
  
TX Outputs:
  - Output 0 (v0 → p0): Change/refund (Bitcoin + remaining auth tokens)
  - Output 1 (p1): Receives 1 unit of auth token, consumed by protostone 2
  - Output 2+: Protocol messages from factory initialization
```

### Key CLI Flags

```bash
--inputs 2:1:1
```
This tells the wallet to include a UTXO containing at least 1 unit of auth token `[2:1]` in the transaction inputs.

## Common Issues and Solutions

### Issue 1: Auth Token Not Found

**Symptom:**
```
Error: Insufficient balance for alkane 2:1
```

**Diagnosis:**
The wallet doesn't have any auth tokens.

**Solution:**
1. Check if auth token factory deployed correctly:
   ```bash
   alkanes-cli -p regtest alkanes getbytecode 4:65517
   ```
   Should return non-empty bytecode.

2. Check wallet balance:
   ```bash
   alkanes-cli -p regtest --wallet-file ~/.alkanes/wallet.json \
     --passphrase testtesttest alkanes getbalance
   ```
   Should show `[2:1]` in the balance list.

3. If no auth tokens, you may need to mint them (check auth token factory documentation).

### Issue 2: Auth Token Not Reaching Second Protostone

**Symptom:**
Factory call fails with authentication error, but auth token exists in wallet.

**Diagnosis:**
The auth token is not being properly passed from protostone 1 to protostone 2.

**Potential Causes:**
1. **Output routing issue**: The `p1` routing might not be working correctly
2. **UTXO selection**: The `--inputs 2:1:1` might not be selecting the right UTXO
3. **Protostone order**: Protostones might be processed in wrong order

**Solution:**
1. Add `--trace` flag to see execution details:
   ```bash
   alkanes-cli -p regtest ... alkanes execute "..." --trace --mine -y
   ```

2. Check the transaction outputs after execution to verify auth token routing.

3. Verify the protostone syntax is exactly:
   ```
   [2:1:1:p1]:v0:v0,[4,65522,0,780993,4,65523]:v0:v0
   ```
   Note the comma separating the two protostones.

### Issue 3: Factory Not Accepting Auth Token

**Symptom:**
Auth token is passed correctly but factory still rejects the initialization.

**Diagnosis:**
Factory's authentication logic might not recognize the auth token.

**Solution:**
1. Verify factory expects auth token at `[2:1]`:
   ```bash
   # Check factory bytecode for auth token references
   alkanes-cli -p regtest alkanes inspect 4:65522
   ```

2. Check if factory was deployed with correct configuration.

3. Review factory source code to confirm auth token ID.

### Issue 4: Incorrect Physical Output Reference

**Symptom:**
Transaction succeeds but auth token goes to wrong output or gets burned.

**Diagnosis:**
The `p1` reference might not be pointing to the correct physical output.

**Understanding Physical Outputs:**
- Virtual outputs (v0, v1, etc.) are converted to physical outputs based on protocol messages
- `p1` means "physical output 1" which should receive the auth token
- The second protostone should consume from `p1` as an input

**Solution:**
1. Verify output order matches expectation:
   - p0: Change (v0 from first protostone)
   - p1: Auth token for second protostone
   - p2+: Protocol messages

2. Try alternative routing if p1 doesn't work:
   ```bash
   # Try using explicit output index
   [2:1:1:1]:v0:v0,[4,65522,0,780993,4,65523]:v0:v0
   ```

### Issue 5: Transaction Fails to Build

**Symptom:**
CLI returns error before transaction is broadcast.

**Common Causes:**
1. Malformed protostone syntax
2. Missing wallet UTXOs
3. Insufficient Bitcoin for fees

**Solution:**
1. Validate protostone syntax - common mistakes:
   - Missing commas between protostones
   - Wrong number of arguments
   - Incorrect alkane ID format (should be `block:tx` or just `tx`)

2. Check wallet has mature Bitcoin UTXOs:
   ```bash
   alkanes-cli -p regtest --wallet-file ~/.alkanes/wallet.json \
     --passphrase testtesttest wallet utxos p2tr:0
   ```

3. Ensure regtest has mined enough blocks (need 100+ for coinbase maturity).

## Debugging Steps

### Step-by-Step Debugging Process

1. **Verify Pre-conditions**
   ```bash
   # Check all contracts deployed
   for tx in 65517 780993 65524 65520 65522 65523; do
     echo "Checking 4:$tx..."
     alkanes-cli -p regtest alkanes getbytecode 4:$tx | head -c 50
     echo ""
   done
   
   # Check auth token exists
   alkanes-cli -p regtest alkanes getbytecode 2:1
   
   # Check wallet balance
   alkanes-cli -p regtest --wallet-file ~/.alkanes/wallet.json \
     --passphrase testtesttest alkanes getbalance
   ```

2. **Test Auth Token Transfer**
   Try a simple auth token transfer first:
   ```bash
   # Send auth token to self to verify it can be spent
   alkanes-cli -p regtest --wallet-file ~/.alkanes/wallet.json \
     --passphrase testtesttest alkanes execute "[2:1:1]:v0:v0" \
     --from p2tr:0 --inputs 2:1:1 --fee-rate 1 --mine -y
   ```

3. **Run Factory Initialization with Trace**
   ```bash
   alkanes-cli -p regtest --wallet-file ~/.alkanes/wallet.json \
     --passphrase testtesttest \
     alkanes execute "[2:1:1:p1]:v0:v0,[4,65522,0,780993,4,65523]:v0:v0" \
     --from p2tr:0 --inputs 2:1:1 --fee-rate 1 --mine --trace -y
   ```

4. **Examine Transaction After Mining**
   ```bash
   # Get recent transactions
   alkanes-cli -p regtest --wallet-file ~/.alkanes/wallet.json \
     --passphrase testtesttest wallet history
   
   # Inspect specific transaction
   alkanes-cli -p regtest bitcoind getrawtransaction <txid> 1
   ```

5. **Verify Factory State**
   ```bash
   # Check if factory was initialized
   # (depends on factory contract's state inspection methods)
   alkanes-cli -p regtest alkanes inspect 4:65522
   ```

## Alternative Approaches

If the two-protostone pattern continues to fail, consider these alternatives:

### Alternative 1: Separate Transactions

Instead of one transaction with two protostones, use two separate transactions:

1. **Transaction 1**: Send auth token to a specific address
   ```bash
   alkanes-cli ... execute "[2:1:1]:v0:v0" --mine -y
   ```

2. **Transaction 2**: Use that UTXO with factory call
   ```bash
   alkanes-cli ... execute "[4,65522,0,780993,4,65523]:v0:v0" \
     --inputs 2:1:1 --mine -y
   ```

### Alternative 2: Pre-funded Factory

If possible, modify factory to accept initialization without auth token for regtest environments, or use a different authentication mechanism.

### Alternative 3: Direct Physical Output Index

Try using direct output index instead of `p1`:
```bash
[2:1:1:1]:v0:v0,[4,65522,0,780993,4,65523]:v0:v0
```

## Reference Implementation

See these files for working examples:
- `crates/alkanes-cli-common/src/alkanes/amm_cli.rs` - AMM CLI implementation with auth token pattern
- `reference/oyl-amm/deploy-oyl-amm.sh` - Reference deployment script
- `scripts/deploy-regtest.sh` - Full regtest deployment with OYL AMM

## Getting Help

If you're still stuck:

1. Check the trace output carefully - it shows exactly what happened
2. Compare your transaction structure to working examples in tests
3. Verify auth token factory source code for correct token ID
4. Check factory source code for authentication logic

## Technical Details

### Why Two Protostones?

The two-protostone pattern is necessary because:
1. Alkanes authentication requires the auth token to be present as an input to the called contract
2. We need to dynamically route the auth token to the factory call
3. The first protostone "prepares" the auth token in the right output
4. The second protostone "consumes" it for authentication
5. After authentication, the token is returned (not burned)

### Auth Token Lifecycle

```
1. Auth token created by factory: [2:1] exists in protocol state
2. Wallet has UTXO with auth token: Input has pointer to [2:1]
3. First protostone: Sends [2:1] to p1 (physical output 1)
4. Second protostone: Receives [2:1] from p1, uses for auth
5. Factory validates: Checks caller has [2:1]
6. Second protostone: Returns [2:1] to v0
7. Wallet receives: Auth token back in change output
```

### Protostone Syntax Reference

```
Cellpack: [alkane_id:opcode,arg1,arg2,...] or [alkane_id,arg1,arg2,...]
Pointer:  :vN (virtual output N)
Refund:   :vN (virtual output N for change)
Edict:    [alkane_id:amount:destination]

Destinations:
  - vN: Virtual output N (becomes physical output after protocol messages)
  - pN: Physical output N (direct reference)
  - 0: Burn/protocol
```

## Success Indicators

You'll know it worked when:
1. Transaction broadcasts successfully
2. Transaction gets mined (visible in wallet history)
3. No error in trace output
4. Factory state shows initialized configuration
5. Auth token returns to wallet (check balance after)
6. You can create pools using the factory

Example success output:
```
[SUCCESS] OYL Factory initialized successfully!
Waiting for metashrew to index factory initialization (5 seconds)...

🎉 OYL AMM deployment complete!
```
