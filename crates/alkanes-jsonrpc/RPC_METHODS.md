# Complete RPC Method List for Lua _RPC Table

Based on handler.rs, ord API documentation, electrs API, and our routing system.

## Esplora (Electrs) Methods

### Address Methods
- `_RPC.esplora_address(address)` - GET /address/{address}
- `_RPC.esplora_address_txs(address)` - GET /address/{address}/txs
- `_RPC.esplora_address_txs_chain(address)` - GET /address/{address}/txs/chain
- `_RPC.esplora_address_txs_mempool(address)` - GET /address/{address}/txs/mempool  
- `_RPC.esplora_address_utxo(address)` - GET /address/{address}/utxo

### Transaction Methods
- `_RPC.esplora_tx(txid)` - GET /tx/{txid}
- `_RPC.esplora_tx_status(txid)` - GET /tx/{txid}/status
- `_RPC.esplora_tx_hex(txid)` - GET /tx/{txid}/hex
- `_RPC.esplora_tx_raw(txid)` - GET /tx/{txid}/raw
- `_RPC.esplora_tx_merkleblock_proof(txid)` - GET /tx/{txid}/merkleblock-proof
- `_RPC.esplora_tx_merkle_proof(txid)` - GET /tx/{txid}/merkle-proof
- `_RPC.esplora_tx_outspend(txid, vout)` - GET /tx/{txid}/outspend/{vout}
- `_RPC.esplora_tx_outspends(txid)` - GET /tx/{txid}/outspends

### Block Methods
- `_RPC.esplora_block(hash)` - GET /block/{hash}
- `_RPC.esplora_block_status(hash)` - GET /block/{hash}/status
- `_RPC.esplora_block_txs(hash)` - GET /block/{hash}/txs
- `_RPC.esplora_block_txs_at_index(hash, index)` - GET /block/{hash}/txs/{index}
- `_RPC.esplora_block_txids(hash)` - GET /block/{hash}/txids
- `_RPC.esplora_block_txid_at_index(hash, index)` - GET /block/{hash}/txid/{index}
- `_RPC.esplora_block_raw(hash)` - GET /block/{hash}/raw
- `_RPC.esplora_block_height(height)` - GET /block-height/{height}

### Mempool Methods
- `_RPC.esplora_mempool()` - GET /mempool
- `_RPC.esplora_mempool_txids()` - GET /mempool/txids
- `_RPC.esplora_mempool_recent()` - GET /mempool/recent

### Fee Methods
- `_RPC.esplora_fee_estimates()` - GET /fee-estimates

## Ord Methods

### Block Methods
- `_RPC.ord_block(hash_or_height)` - GET /block/{hash_or_height}
- `_RPC.ord_blockcount()` - GET /blockcount
- `_RPC.ord_blockhash()` - GET /blockhash
- `_RPC.ord_blockhash_at_height(height)` - GET /blockhash/{height}
- `_RPC.ord_blockheight()` - GET /blockheight
- `_RPC.ord_blocks()` - GET /blocks
- `_RPC.ord_blocktime()` - GET /blocktime

### Inscription Methods
- `_RPC.ord_inscription(id)` - GET /inscription/{id}
- `_RPC.ord_inscriptions()` - GET /inscriptions
- `_RPC.ord_inscriptions_page(page)` - GET /inscriptions/{page}
- `_RPC.ord_inscriptions_block(height)` - GET /inscriptions/block/{height}
- `_RPC.ord_inscriptions_block_page(height, page)` - GET /inscriptions/block/{height}/{page}
- `_RPC.ord_content(inscription_id)` - GET /content/{inscription_id}
- `_RPC.ord_decode(txid)` - GET /decode/{txid}

### Output Methods
- `_RPC.ord_output(outpoint)` - GET /output/{outpoint}
- `_RPC.ord_outputs(address)` - GET /outputs/{address}

### Rune Methods
- `_RPC.ord_rune(rune)` - GET /rune/{rune}
- `_RPC.ord_runes()` - GET /runes
- `_RPC.ord_runes_page(page)` - GET /runes/{page}

### Sat Methods
- `_RPC.ord_sat(sat)` - GET /sat/{sat}
- `_RPC.ord_sat_at_index(sat, index)` - GET /sat/{sat}/at/{index}
- `_RPC.ord_sat_inscriptions(sat)` - GET /sat/{sat}/inscriptions
- `_RPC.ord_sat_inscriptions_page(sat, page)` - GET /sat/{sat}/inscriptions/{page}

### Child/Parent Methods  
- `_RPC.ord_children(id)` - GET /children/{id}
- `_RPC.ord_children_page(id, page)` - GET /children/{id}/{page}
- `_RPC.ord_children_inscriptions(id)` - GET /children/{id}/inscriptions
- `_RPC.ord_children_inscriptions_page(id, page)` - GET /children/{id}/inscriptions/{page}
- `_RPC.ord_parents(id)` - GET /parents/{id}
- `_RPC.ord_parents_page(id, page)` - GET /parents/{id}/{page}

### Collection Methods
- `_RPC.ord_collections()` - GET /collections
- `_RPC.ord_collections_page(page)` - GET /collections/{page}

## Bitcoin Core RPC Methods

### Blockchain
- `_RPC.btc_getbestblockhash()` - getbestblockhash
- `_RPC.btc_getblock(blockhash, verbosity)` - getblock
- `_RPC.btc_getblockchaininfo()` - getblockchaininfo
- `_RPC.btc_getblockcount()` - getblockcount
- `_RPC.btc_getblockhash(height)` - getblockhash
- `_RPC.btc_getblockheader(blockhash, verbose)` - getblockheader
- `_RPC.btc_getblockstats(hash_or_height, stats)` - getblockstats
- `_RPC.btc_getchaintips()` - getchaintips
- `_RPC.btc_getchaintxstats(nblocks, blockhash)` - getchaintxstats
- `_RPC.btc_getdifficulty()` - getdifficulty
- `_RPC.btc_getmempoolancestors(txid, verbose)` - getmempoolancestors
- `_RPC.btc_getmempooldescendants(txid, verbose)` - getmempooldescendants
- `_RPC.btc_getmempoolentry(txid)` - getmempoolentry
- `_RPC.btc_getmempoolinfo()` - getmempoolinfo
- `_RPC.btc_getrawmempool(verbose)` - getrawmempool
- `_RPC.btc_gettxout(txid, n, include_mempool)` - gettxout
- `_RPC.btc_gettxoutproof(txids, blockhash)` - gettxoutproof
- `_RPC.btc_gettxoutsetinfo()` - gettxoutsetinfo
- `_RPC.btc_verifytxoutproof(proof)` - verifytxoutproof

### Mining
- `_RPC.btc_getmininginfo()` - getmininginfo
- `_RPC.btc_getnetworkhashps(nblocks, height)` - getnetworkhashps

### Network
- `_RPC.btc_getnetworkinfo()` - getnetworkinfo
- `_RPC.btc_getnettotals()` - getnettotals
- `_RPC.btc_getpeerinfo()` - getpeerinfo
- `_RPC.btc_ping()` - ping

### Raw Transactions
- `_RPC.btc_getrawtransaction(txid, verbose, blockhash)` - getrawtransaction
- `_RPC.btc_sendrawtransaction(hexstring, maxfeerate)` - sendrawtransaction
- `_RPC.btc_testmempoolaccept(rawtxs, maxfeerate)` - testmempoolaccept
- `_RPC.btc_decoderawtransaction(hexstring, iswitness)` - decoderawtransaction
- `_RPC.btc_decodescript(hexstring)` - decodescript

## Metashrew Methods

### View Methods
- `_RPC.metashrew_view(method, input, block_tag)` - metashrew_view
- `_RPC.metashrew_height()` - metashrew_height

## Alkanes Methods (via metashrew_view)

All alkanes methods are forwarded to metashrew_view internally:
- `_RPC.alkanes_getbytecode({block, tx}, block_tag)` - Get contract bytecode
- `_RPC.alkanes_protorunesbyaddress({address, protocolTag}, block_tag)` - Get protorunes by address
- `_RPC.alkanes_<custom_method>(input, block_tag)` - Any custom alkanes contract method

## Sandshrew Methods

### Utility Methods
- `_RPC.sandshrew_multicall(calls)` - Execute multiple RPC calls
- `_RPC.sandshrew_balances(request)` - Get comprehensive address balance info

## Method Naming Convention

The flat `_RPC` table uses the following naming convention:

1. **Namespace prefix**: `<namespace>_` (e.g., `esplora_`, `ord_`, `btc_`)
2. **Method path**: Underscores replace slashes and colons (e.g., `/address/{addr}/txs` → `address_txs`)
3. **Parameters**: Passed as function arguments in order

### Examples:
- `esplora_address::txs` → `_RPC.esplora_address_txs(address)`
- `ord_inscription::child` → `_RPC.ord_inscription_child(id, child_id)`
- `btc_getblock` → `_RPC.btc_getblock(hash, verbosity)`

## Dynamic Methods

Some methods accept dynamic parameters that become part of the path:
- Methods with `::` in handler.rs indicate dynamic segments
- These are passed as arguments: `_RPC.esplora_tx(txid)` → `/tx/{txid}`
