# Protorune Spendable Outpoint Tracking

This document details how the `protorune` crate indexes and queries spendable outpoints (UTXOs) within the Subfrost protocol.

## Data Structures

The `protorune` crate uses three key-value tables, implemented as `IndexPointer`s, to manage spendable outpoints:

| Table | Indexer Path | Description |
| :--- | :--- | :--- |
| `OUTPOINTS_FOR_ADDRESS` | `/outpoint/byaddress/` | Maps a Bitcoin address to a list of all `OutPoint`s ever sent to that address, providing a complete historical record. |
| `OUTPOINT_SPENDABLE_BY` | `/outpoint/spendableby/` | Maps a specific `OutPoint` to the address that can spend it. This table is the source of truth for the current spendability of any given UTXO. |
| `OUTPOINT_SPENDABLE_BY_ADDRESS` | `/outpoint/spendablebyaddress/` | An optimized index that maps an address directly to a list of its *currently spendable* `OutPoint`s. |

## Indexing Process

Spendable outpoints are indexed by the `index_spendables` function in `crates/protorune/src/lib.rs`. For each new block:

-   **Outputs**: For each transaction output, the function derives the recipient's address and updates the tables:
    1.  Appends the new `OutPoint` to the address's list in `OUTPOINTS_FOR_ADDRESS`.
    2.  Creates an entry in `OUTPOINT_SPENDABLE_BY`, mapping the `OutPoint` to the recipient's address.
-   **Inputs**: For each transaction input, the function removes the spent `OutPoint` from the `OUTPOINT_SPENDABLE_BY` table by calling `nullify()`.

## Querying Spendable Outpoints

### By Address

There are two methods to retrieve spendable outpoints for an address:

1.  **Standard (`runes_by_address`)**:
    *   Retrieves all historical `OutPoint`s for the address from `OUTPOINTS_FOR_ADDRESS`.
    *   Filters this list by checking for the existence of each `OutPoint` in `OUTPOINT_SPENDABLE_BY`.
    *   Returns the filtered list of spendable outpoints.

2.  **Optimized (`protorunes_by_address2`)**:
    *   Directly queries the `OUTPOINT_SPENDABLE_BY_ADDRESS` table, which contains a pre-filtered list of spendable `OutPoint`s for the address.
    *   This method is significantly more performant as it avoids the filtering step.

### By Single Outpoint

To check if a single `OutPoint` is spendable:

1.  **Serialize the `OutPoint`**: The `OutPoint` is serialized into a byte vector.
2.  **Query `OUTPOINT_SPENDABLE_BY`**: The serialized `OutPoint` is used as a key to query the `/outpoint/spendableby/` table.
3.  **Check the Result**:
    *   If the query returns a non-empty value (the spending address), the `OutPoint` is **spendable**.
    *   If the query returns an empty or null value, the `OutPoint` has been spent and is **not spendable**.