# free-mint

These are the sources for the template deployed at [4, 797].

This alkane is adapted from earlier testing versions by the same name, but suitable for production usage. It enables semantics similar to what we are used to in runes ecosystem mints, but on the ALKANES metaprotocol. This template can be spawned using factory cellpacks and a data segment can be appended (such as a graphic), but other parameters can be supplied for an initial premine, a mint quantity per mint transaction, a cap, and a name/symbol, supplied with the initialization vector in the alkanes protocol message.

## 🔒 Deterministic Builds & Verification

This contract supports **reproducible builds** for bytecode verification (similar to Etherscan source verification). Anyone can verify that deployed bytecode matches the published source code.

**Quick Start:**
```bash
# Build deterministically (requires Docker)
./scripts/build-reproducible.sh

# Verify against published checksum
./scripts/verify-build.sh <sha256-hash>
```

**Documentation:**
- 📖 [Quick Start Guide](./VERIFICATION_QUICK_START.md) - TL;DR for busy developers
- 📚 [Complete Documentation](./DETERMINISTIC_BUILDS.md) - Technical deep-dive
- 📋 [Changes Summary](./CHANGES_SUMMARY.md) - What was implemented

---

## Overview

This contract implements a token with free mint capabilities using the alkane framework. It includes security features such as:

- Proper initialization guard via observe_initialization()
- Transaction hash validation to enforce one mint per transaction
- Comprehensive overflow protection
- Supply cap enforcement

## Features

- Standard token functionality (name, symbol, total supply)
- Free mint capabilities with configurable parameters
- Transaction-based mint limits
- Supply cap enforcement
- Comprehensive view functions

## Storage Pattern

The contract follows the established storage pattern using StoragePointer::from_keyword for all persistent storage with the following keys:

- `/name` - Token name
- `/symbol` - Token symbol
- `/totalsupply` - Total supply tracking (Total supply in circulation. Not max supply)
- `/minted` - Total mints counter
- `/value-per-mint` - Value per mint configuration
- `/cap` - Maximum supply cap (This the maximum amount of times it can be minted) 
- `/data` - Additional token data
- `/initialized` - Initialization guard
- `/tx-hashes` - Transaction hash tracking for mint limits

## Opcodes

The contract implements all required opcodes:

- 0: Initialize(token_units, value_per_mint, cap, name, symbol)
     - token_units : Initial pre-mine tokens to be received on deployer's address
     - value_per_mint: Amount of tokens to be received on each successful mint
     - cap: Max amount of times the token can be minted
     - name: Token name
     - symbol: Token symbol
- 77: MintTokens()
- 88: SetNameAndSymbol(name, symbol)
- 99: GetName() -> String
- 100: GetSymbol() -> String
- 101: GetTotalSupply() -> u128
- 102: GetCap() -> u128
- 103: GetMinted() -> u128
- 104: GetValuePerMint() -> u128
- 1000: GetData() -> Vec<u8>

## Security Patterns

The contract implements several security patterns:

1. **Initialization Guard**: Prevents multiple initializations of the contract.
   ```rust
   fn observe_initialization(&self) -> Result<()> {
       let mut pointer = StoragePointer::from_keyword("/initialized");
       if pointer.get().len() == 0 {
           pointer.set_value::<u8>(0x01);
           Ok(())
       } else {
           Err(anyhow!("already initialized"))
       }
   }
   ```

2. **Transaction Hash Validation**: Enforces one mint per transaction.
   ```rust
   // Check if a transaction hash has been used for minting
   pub fn has_tx_hash(&self, txid: &Txid) -> bool {
       StoragePointer::from_keyword("/tx-hashes/")
           .select(&txid.as_byte_array().to_vec())
           .get_value::<u8>() == 1
   }
   
   // Add a transaction hash to the used set
   pub fn add_tx_hash(&self, txid: &Txid) -> Result<()> {
       StoragePointer::from_keyword("/tx-hashes/")
           .select(&txid.as_byte_array().to_vec())
           .set_value::<u8>(0x01);
       Ok(())
   }
   ```

3. **Overflow Protection**: Prevents integer overflow vulnerabilities.
   ```rust
   fn increase_total_supply(&self, v: u128) -> Result<()> {
       self.set_total_supply(overflow_error(self.total_supply().checked_add(v))
           .map_err(|_| anyhow!("total supply overflow"))?);
       Ok(())
   }
   ```

4. **Cap Enforcement**: Prevents minting beyond the supply cap.
   ```rust
   // Check if minting would exceed cap
   if self.minted() >= self.cap() {
       return Err(anyhow!("Supply cap reached: {} of {}", self.minted(), self.cap()));
   }
   ```

## Building

To build the contract:

```bash
cargo build
```

To build for WebAssembly:

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Testing

The project includes a comprehensive test suite that can be run with:

```bash
cargo test
```

The tests use the `test-utils` feature of the alkanes-runtime crate to provide a mock environment for testing.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
