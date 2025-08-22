# frBTC Storage and Payment Analysis

This document provides a detailed analysis of the `fr-btc` Alkane's storage layout and its mechanism for handling unwraps via the `Payment` struct.

## Storage Slot Analysis

The `fr-btc` contract utilizes the `alkanes-runtime`'s `StoragePointer` to manage its state within the indexer's key-value store. All storage paths are relative to the Alkane's base path, which is `/alkanes/{fr_btc_id}/`, where `{fr_btc_id}` is the unique identifier for the `fr-btc` Alkane.

The following table maps the logical storage slots defined in the contract to their physical paths in the indexer's database:

| `fr-btc` Storage Slot | Indexer Key-Value Path | Value Type | Description |
| :--- | :--- | :--- | :--- |
| `premium_pointer` | `/alkanes/{fr_btc_id}/premium` | `Vec<u8>` (16 bytes for `u128`) | Stores the premium fee charged for wrapping BTC into frBTC. |
| `signer_pointer` | `/alkanes/{fr_btc_id}/signer` | `Vec<u8>` | Stores the `script_pubkey` of the multisig wallet authorized to sign unwrap transactions. |
| `observe_transaction` | `/alkanes/{fr_btc_id}/seen/{txid}` | `Vec<u8>` (1 byte, `0x01`) | A set-like data structure used to mark a transaction as processed, preventing replay attacks. `{txid}` is the 32-byte transaction ID. |
| `burn` (payments) | `/alkanes/{fr_btc_id}/payments/byheight/{height}` | `Vec<Vec<u8>>` | A list of serialized `Payment` structs, indexed by the block height at which the unwrap request was made. Each element in the list is a `Vec<u8>` representing a single pending payment. |

## The `Payment` Struct and Unwrap Process

The `Payment` struct is central to the unwrapping process, creating a transparent and verifiable commitment for the multisig to fulfill.

### `Payment` Struct Definition

A `Payment` object contains two key fields:

1.  **`output` (`TxOut`)**: Defines the destination for the unwrapped BTC.
    *   `script_pubkey`: The user's Bitcoin address.
    *   `value`: The amount of BTC (in satoshis) to be paid.

2.  **`spendable` (`OutPoint`)**: A pointer to a UTXO that must be spent by the multisig to fund the payment. This UTXO is created by the user in the same transaction as the unwrap request and is made spendable by the `fr-btc` contract.

### Unwrap and Payment Tracking Workflow

The process for unwrapping `frBTC` back to BTC is as follows:

1.  **Unwrap Request**: A user initiates an unwrap by calling the `unwrap` function on the `fr-btc` contract. The transaction containing this call must also create a new UTXO that is spendable by the `fr-btc` contract.

2.  **`Payment` Creation**: The `unwrap` function burns the user's `frBTC` and constructs a `Payment` struct. The `spendable` field is set to the `OutPoint` of the UTXO created in the previous step.

3.  **On-Chain Storage**: The newly created `Payment` struct is serialized and appended to the list at the storage path `/payments/byheight/{height}`, where `{height}` is the current block height.

4.  **Off-Chain Monitoring**: An off-chain service, operated by the multisig members, continuously monitors this storage path for new `Payment` entries.

5.  **Transaction Fulfillment**: Upon detecting a new `Payment`, the off-chain service constructs a Bitcoin transaction that:
    *   Spends the `spendable` `OutPoint` from the `Payment` struct.
    *   Creates a new output matching the `output` (`TxOut`) from the `Payment` struct, thereby sending the unwrapped BTC to the user.

6.  **Signing and Broadcasting**: The multisig members sign the fulfillment transaction and broadcast it to the Bitcoin network, completing the unwrap process.

This system ensures a high degree of transparency and security, as all pending unwraps are publicly recorded on-chain, and the funds to fulfill them are verifiably locked.