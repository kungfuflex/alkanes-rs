/**
 * Represents the entire JSON keystore, compatible with wasm-bindgen.
 */
export class Keystore {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KeystoreFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_keystore_free(ptr, 0);
    }
    /**
     * @returns {string}
     */
    accountXpub() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.keystore_accountXpub(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} passphrase
     * @returns {Promise<any>}
     */
    decryptMnemonic(passphrase) {
        const ptr0 = passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.keystore_decryptMnemonic(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {any} val
     */
    constructor(val) {
        const ret = wasm.keystore_from_js(val);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        KeystoreFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {any}
     */
    hdPaths() {
        const ret = wasm.keystore_hdPaths(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    masterFingerprint() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.keystore_masterFingerprint(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {any}
     */
    to_js() {
        const ret = wasm.keystore_to_js(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
}
if (Symbol.dispose) Keystore.prototype[Symbol.dispose] = Keystore.prototype.free;

/**
 * Parameters for the PBKDF2/S2K key derivation function.
 */
export class PbkdfParams {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        PbkdfParamsFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_pbkdfparams_free(ptr, 0);
    }
    /**
     * @param {any} val
     */
    constructor(val) {
        const ret = wasm.pbkdfparams_from_js(val);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        PbkdfParamsFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {any}
     */
    to_js() {
        const ret = wasm.pbkdfparams_to_js(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
}
if (Symbol.dispose) PbkdfParams.prototype[Symbol.dispose] = PbkdfParams.prototype.free;

/**
 * WASM-exported BrowserWalletProvider that can be created from JavaScript
 */
export class WasmBrowserWalletProvider {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmBrowserWalletProvider.prototype);
        obj.__wbg_ptr = ptr;
        WasmBrowserWalletProviderFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmBrowserWalletProviderFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmbrowserwalletprovider_free(ptr, 0);
    }
    /**
     * Broadcast a transaction
     * @param {string} tx_hex
     * @returns {Promise<string>}
     */
    broadcastTransaction(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_broadcastTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Disconnect from the wallet
     * @returns {Promise<void>}
     */
    disconnect() {
        const ret = wasm.wasmbrowserwalletprovider_disconnect(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get the connected wallet address
     * @returns {string | undefined}
     */
    getAddress() {
        const ret = wasm.wasmbrowserwalletprovider_getAddress(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Get all balances (BTC + alkanes)
     * @returns {Promise<any>}
     */
    getAllBalances() {
        const ret = wasm.wasmbrowserwalletprovider_getAllBalances(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get balance
     * @returns {Promise<any>}
     */
    getBalance() {
        const ret = wasm.wasmbrowserwalletprovider_getBalance(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get connection status
     * @returns {string}
     */
    getConnectionStatus() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmbrowserwalletprovider_getConnectionStatus(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get enriched UTXOs with asset information
     * @returns {Promise<any>}
     */
    getEnrichedUtxos() {
        const ret = wasm.wasmbrowserwalletprovider_getEnrichedUtxos(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get current network
     * @returns {string}
     */
    getNetwork() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmbrowserwalletprovider_getNetwork(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the wallet public key
     * @returns {Promise<string>}
     */
    getPublicKey() {
        const ret = wasm.wasmbrowserwalletprovider_getPublicKey(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get UTXOs
     * @param {boolean} include_frozen
     * @returns {Promise<any>}
     */
    getUtxos(include_frozen) {
        const ret = wasm.wasmbrowserwalletprovider_getUtxos(this.__wbg_ptr, include_frozen);
        return ret;
    }
    /**
     * Get wallet info
     * @returns {any}
     */
    getWalletInfo() {
        const ret = wasm.wasmbrowserwalletprovider_getWalletInfo(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new BrowserWalletProvider from a JavaScript wallet adapter
     *
     * @param adapter - A JavaScript object implementing the JsWalletAdapter interface
     * @param network - Network string ("mainnet", "testnet", "signet", "regtest")
     * @returns Promise<WasmBrowserWalletProvider>
     * @param {JsWalletAdapter} adapter
     * @param {string} network
     */
    constructor(adapter, network) {
        const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_new(adapter, ptr0, len0);
        return ret;
    }
    /**
     * Sign a message
     * @param {string} message
     * @param {string | null} [address]
     * @returns {Promise<string>}
     */
    signMessage(message, address) {
        const ptr0 = passStringToWasm0(message, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_signMessage(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Sign a PSBT (hex encoded)
     * @param {string} psbt_hex
     * @param {any} options
     * @returns {Promise<string>}
     */
    signPsbt(psbt_hex, options) {
        const ptr0 = passStringToWasm0(psbt_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_signPsbt(this.__wbg_ptr, ptr0, len0, options);
        return ret;
    }
}
if (Symbol.dispose) WasmBrowserWalletProvider.prototype[Symbol.dispose] = WasmBrowserWalletProvider.prototype.free;

/**
 * Web-compatible provider implementation for browser environments
 *
 * The `WebProvider` is the main entry point for using deezel functionality in web browsers
 * and WASM environments. It implements all deezel-common traits using web-standard APIs,
 * providing complete Bitcoin wallet and Alkanes metaprotocol functionality.
 *
 * # Features
 *
 * - **Bitcoin Operations**: Full wallet functionality, transaction creation, and broadcasting
 * - **Alkanes Integration**: Smart contract execution, token operations, and AMM functionality
 * - **Web Standards**: Uses fetch API, localStorage, Web Crypto API, and console logging
 * - **Network Support**: Configurable for mainnet, testnet, signet, regtest, and custom networks
 * - **Privacy Features**: Rebar Labs Shield integration for private transaction broadcasting
 *
 * # Example
 *
 * ```rust,no_run
 * use deezel_web::WebProvider;
 * use alkanes_cli_common::*;
 *
 * async fn create_provider() -> Result<WebProvider> {
 *     let provider = WebProvider::new("mainnet".to_string()).await?;
 *
 *     provider.initialize().await?;
 *     Ok(provider)
 * }
 * ```
 */
export class WebProvider {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WebProviderFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_webprovider_free(ptr, 0);
    }
    /**
     * Get alkanes contract balance for an address
     * @param {string | null} [address]
     * @returns {Promise<any>}
     */
    alkanesBalance(address) {
        var ptr0 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesBalance(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @param {string | null} [block_tag]
     * @param {number | null} [protocol_tag]
     * @returns {Promise<any>}
     */
    alkanesByAddress(address, block_tag, protocol_tag) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesByAddress(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(protocol_tag), isLikeNone(protocol_tag) ? 0 : protocol_tag);
        return ret;
    }
    /**
     * @param {string} outpoint
     * @param {string | null} [block_tag]
     * @param {number | null} [protocol_tag]
     * @returns {Promise<any>}
     */
    alkanesByOutpoint(outpoint, block_tag, protocol_tag) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesByOutpoint(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(protocol_tag), isLikeNone(protocol_tag) ? 0 : protocol_tag);
        return ret;
    }
    /**
     * Get alkanes contract bytecode
     * @param {string} alkane_id
     * @param {string | null} [block_tag]
     * @returns {Promise<any>}
     */
    alkanesBytecode(alkane_id, block_tag) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesBytecode(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Execute an alkanes smart contract
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    alkanesExecute(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesExecute(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Execute an alkanes smart contract fully (handles complete flow internally)
     *
     * This method handles the complete execution flow:
     * - For deployments (with envelope): commit -> reveal -> mine -> trace
     * - For simple transactions: sign -> broadcast -> mine -> trace
     *
     * Returns the final EnhancedExecuteResult directly, avoiding serialization issues
     * with intermediate states.
     * @param {string} to_addresses_json
     * @param {string} input_requirements
     * @param {string} protostones
     * @param {number | null} [fee_rate]
     * @param {string | null} [envelope_hex]
     * @param {string | null} [options_json]
     * @returns {Promise<any>}
     */
    alkanesExecuteFull(to_addresses_json, input_requirements, protostones, fee_rate, envelope_hex, options_json) {
        const ptr0 = passStringToWasm0(to_addresses_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(input_requirements, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(protostones, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(envelope_hex) ? 0 : passStringToWasm0(envelope_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(options_json) ? 0 : passStringToWasm0(options_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len4 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesExecuteFull(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, isLikeNone(fee_rate) ? 0x100000001 : Math.fround(fee_rate), ptr3, len3, ptr4, len4);
        return ret;
    }
    /**
     * Execute an alkanes smart contract using CLI-style string parameters
     * This is the recommended method for executing alkanes contracts as it supports
     * the same parameter format as alkanes-cli.
     *
     * # Parameters
     * - `to_addresses`: JSON array of recipient addresses
     * - `input_requirements`: String format like "B:10000" or "2:0:1000" (alkane block:tx:amount)
     * - `protostones`: String format like "[32,0,77]:v0:v0" (cellpack:pointer:refund)
     * - `fee_rate`: Optional fee rate in sat/vB
     * - `envelope_hex`: Optional envelope data as hex string
     * - `options_json`: Optional JSON with additional options (trace_enabled, mine_enabled, auto_confirm, raw_output)
     * @param {string} to_addresses_json
     * @param {string} input_requirements
     * @param {string} protostones
     * @param {number | null} [fee_rate]
     * @param {string | null} [envelope_hex]
     * @param {string | null} [options_json]
     * @returns {Promise<any>}
     */
    alkanesExecuteWithStrings(to_addresses_json, input_requirements, protostones, fee_rate, envelope_hex, options_json) {
        const ptr0 = passStringToWasm0(to_addresses_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(input_requirements, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(protostones, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(envelope_hex) ? 0 : passStringToWasm0(envelope_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(options_json) ? 0 : passStringToWasm0(options_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len4 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesExecuteWithStrings(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, isLikeNone(fee_rate) ? 0x100000001 : Math.fround(fee_rate), ptr3, len3, ptr4, len4);
        return ret;
    }
    /**
     * Get all pools from a factory (lightweight, IDs only)
     * @param {string} factory_id
     * @returns {Promise<any>}
     */
    alkanesGetAllPools(factory_id) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesGetAllPools(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get all pools with details from an AMM factory (parallel optimized for browser)
     * @param {string} factory_id
     * @param {number | null} [chunk_size]
     * @param {number | null} [max_concurrent]
     * @returns {Promise<any>}
     */
    alkanesGetAllPoolsWithDetails(factory_id, chunk_size, max_concurrent) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesGetAllPoolsWithDetails(this.__wbg_ptr, ptr0, len0, !isLikeNone(chunk_size), isLikeNone(chunk_size) ? 0 : chunk_size, !isLikeNone(max_concurrent), isLikeNone(max_concurrent) ? 0 : max_concurrent);
        return ret;
    }
    /**
     * Initialize a new AMM liquidity pool
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    alkanesInitPool(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesInitPool(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} target
     * @param {any} config
     * @returns {Promise<any>}
     */
    alkanesInspect(target, config) {
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesInspect(this.__wbg_ptr, ptr0, len0, config);
        return ret;
    }
    /**
     * Inspect alkanes bytecode directly from WASM bytes (hex-encoded or raw bytes)
     * This allows inspection without fetching from RPC - useful for local/offline analysis
     * @param {string} bytecode_hex
     * @param {string} alkane_id
     * @param {any} config
     * @returns {Promise<any>}
     */
    alkanesInspectBytecode(bytecode_hex, alkane_id, config) {
        const ptr0 = passStringToWasm0(bytecode_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesInspectBytecode(this.__wbg_ptr, ptr0, len0, ptr1, len1, config);
        return ret;
    }
    /**
     * Get metadata (ABI) for an alkanes contract
     * @param {string} alkane_id
     * @param {string | null} [block_tag]
     * @returns {Promise<any>}
     */
    alkanesMeta(alkane_id, block_tag) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesMeta(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {string | null} [block_tag]
     * @returns {Promise<any>}
     */
    alkanesPendingUnwraps(block_tag) {
        var ptr0 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesPendingUnwraps(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get pool details for a specific pool
     * @param {string} pool_id
     * @returns {Promise<any>}
     */
    alkanesPoolDetails(pool_id) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesPoolDetails(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Reflect alkane token metadata by querying standard opcodes
     *
     * This method queries the alkane contract with standard opcodes to retrieve
     * token metadata like name, symbol, total supply, cap, minted, and value per mint.
     *
     * # Arguments
     * * `alkane_id` - The alkane ID in "block:tx" format (e.g., "2:1234")
     *
     * # Returns
     * An AlkaneReflection object with all available metadata
     * @param {string} alkane_id
     * @returns {Promise<any>}
     */
    alkanesReflect(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesReflect(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Reflect metadata for a range of alkanes
     * @param {number} block
     * @param {number} start_tx
     * @param {number} end_tx
     * @param {number | null} [concurrency]
     * @returns {Promise<any>}
     */
    alkanesReflectAlkaneRange(block, start_tx, end_tx, concurrency) {
        const ret = wasm.webprovider_alkanesReflectAlkaneRange(this.__wbg_ptr, block, start_tx, end_tx, !isLikeNone(concurrency), isLikeNone(concurrency) ? 0 : concurrency);
        return ret;
    }
    /**
     * Resume execution after commit transaction confirmation
     * @param {string} state_json
     * @returns {Promise<any>}
     */
    alkanesResumeCommitExecution(state_json) {
        const ptr0 = passStringToWasm0(state_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesResumeCommitExecution(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Resume execution after user confirmation (for simple transactions)
     * @param {string} state_json
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    alkanesResumeExecution(state_json, params_json) {
        const ptr0 = passStringToWasm0(state_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesResumeExecution(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Resume execution after reveal transaction confirmation
     * @param {string} state_json
     * @returns {Promise<any>}
     */
    alkanesResumeRevealExecution(state_json) {
        const ptr0 = passStringToWasm0(state_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesResumeRevealExecution(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string | null} [block_tag]
     * @returns {Promise<any>}
     */
    alkanesSequence(block_tag) {
        var ptr0 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSequence(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Simulate an alkanes contract call (read-only)
     * @param {string} contract_id
     * @param {string} context_json
     * @param {string | null} [block_tag]
     * @returns {Promise<any>}
     */
    alkanesSimulate(contract_id, context_json, block_tag) {
        const ptr0 = passStringToWasm0(contract_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(context_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSimulate(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    alkanesSpendables(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSpendables(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Execute an AMM swap
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    alkanesSwap(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSwap(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} outpoint
     * @returns {Promise<any>}
     */
    alkanesTrace(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesTrace(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Execute a tx-script with WASM bytecode
     * @param {string} wasm_hex
     * @param {string} inputs_json
     * @param {string | null} [block_tag]
     * @returns {Promise<any>}
     */
    alkanesTxScript(wasm_hex, inputs_json, block_tag) {
        const ptr0 = passStringToWasm0(wasm_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(inputs_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesTxScript(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * @param {string} contract_id
     * @param {string} view_fn
     * @param {Uint8Array | null} [params]
     * @param {string | null} [block_tag]
     * @returns {Promise<any>}
     */
    alkanesView(contract_id, view_fn, params, block_tag) {
        const ptr0 = passStringToWasm0(contract_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(view_fn, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(params) ? 0 : passArray8ToWasm0(params, wasm.__wbindgen_malloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesView(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret;
    }
    /**
     * Wrap BTC to frBTC
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    alkanesWrapBtc(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesWrapBtc(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get pool details including reserves using simulation
     * @param {string} pool_id
     * @returns {Promise<any>}
     */
    ammGetPoolDetails(pool_id) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ammGetPoolDetails(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {string}
     */
    bitcoin_rpc_url() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.webprovider_bitcoin_rpc_url(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} psbt
     * @returns {Promise<any>}
     */
    bitcoindDecodePsbt(psbt) {
        const ptr0 = passStringToWasm0(psbt, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindDecodePsbt(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hex
     * @returns {Promise<any>}
     */
    bitcoindDecodeRawTransaction(hex) {
        const ptr0 = passStringToWasm0(hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindDecodeRawTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} target
     * @returns {Promise<any>}
     */
    bitcoindEstimateSmartFee(target) {
        const ret = wasm.webprovider_bitcoindEstimateSmartFee(this.__wbg_ptr, target);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    bitcoindGenerateFuture(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGenerateFuture(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} nblocks
     * @param {string} address
     * @returns {Promise<any>}
     */
    bitcoindGenerateToAddress(nblocks, address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGenerateToAddress(this.__wbg_ptr, nblocks, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hash
     * @param {boolean} raw
     * @returns {Promise<any>}
     */
    bitcoindGetBlock(hash, raw) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetBlock(this.__wbg_ptr, ptr0, len0, raw);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    bitcoindGetBlockCount() {
        const ret = wasm.webprovider_bitcoindGetBlockCount(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} height
     * @returns {Promise<any>}
     */
    bitcoindGetBlockHash(height) {
        const ret = wasm.webprovider_bitcoindGetBlockHash(this.__wbg_ptr, height);
        return ret;
    }
    /**
     * @param {string} hash
     * @returns {Promise<any>}
     */
    bitcoindGetBlockHeader(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetBlockHeader(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hash
     * @returns {Promise<any>}
     */
    bitcoindGetBlockStats(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetBlockStats(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    bitcoindGetBlockchainInfo() {
        const ret = wasm.webprovider_bitcoindGetBlockchainInfo(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    bitcoindGetChainTips() {
        const ret = wasm.webprovider_bitcoindGetChainTips(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    bitcoindGetMempoolInfo() {
        const ret = wasm.webprovider_bitcoindGetMempoolInfo(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    bitcoindGetNetworkInfo() {
        const ret = wasm.webprovider_bitcoindGetNetworkInfo(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    bitcoindGetRawMempool() {
        const ret = wasm.webprovider_bitcoindGetRawMempool(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} txid
     * @param {string | null} [block_hash]
     * @returns {Promise<any>}
     */
    bitcoindGetRawTransaction(txid, block_hash) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_hash) ? 0 : passStringToWasm0(block_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetRawTransaction(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {string} txid
     * @param {number} vout
     * @param {boolean} include_mempool
     * @returns {Promise<any>}
     */
    bitcoindGetTxOut(txid, vout, include_mempool) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetTxOut(this.__wbg_ptr, ptr0, len0, vout, include_mempool);
        return ret;
    }
    /**
     * @param {string} tx_hex
     * @returns {Promise<any>}
     */
    bitcoindSendRawTransaction(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindSendRawTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {string}
     */
    brc20_prog_rpc_url() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.webprovider_brc20_prog_rpc_url(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Promise<any>}
     */
    brc20progBlockNumber() {
        const ret = wasm.webprovider_brc20progBlockNumber(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} to
     * @param {string} data
     * @param {string | null} [block]
     * @returns {Promise<any>}
     */
    brc20progCall(to, data, block) {
        const ptr0 = passStringToWasm0(to, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(data, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progCall(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    brc20progChainId() {
        const ret = wasm.webprovider_brc20progChainId(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} to
     * @param {string} data
     * @param {string | null} [block]
     * @returns {Promise<any>}
     */
    brc20progEstimateGas(to, data, block) {
        const ptr0 = passStringToWasm0(to, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(data, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progEstimateGas(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * @param {string} address
     * @param {string | null} [block]
     * @returns {Promise<any>}
     */
    brc20progGetBalance(address, block) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {string} block
     * @param {boolean} full_tx
     * @returns {Promise<any>}
     */
    brc20progGetBlockByNumber(block, full_tx) {
        const ptr0 = passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetBlockByNumber(this.__wbg_ptr, ptr0, len0, full_tx);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    brc20progGetCode(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetCode(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {any} filter
     * @returns {Promise<any>}
     */
    brc20progGetLogs(filter) {
        const ret = wasm.webprovider_brc20progGetLogs(this.__wbg_ptr, filter);
        return ret;
    }
    /**
     * @param {string} tx_hash
     * @returns {Promise<any>}
     */
    brc20progGetTransactionByHash(tx_hash) {
        const ptr0 = passStringToWasm0(tx_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetTransactionByHash(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @param {string | null} [block]
     * @returns {Promise<any>}
     */
    brc20progGetTransactionCount(address, block) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetTransactionCount(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {string} tx_hash
     * @returns {Promise<any>}
     */
    brc20progGetTransactionReceipt(tx_hash) {
        const ptr0 = passStringToWasm0(tx_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetTransactionReceipt(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    brc20progWeb3ClientVersion() {
        const ret = wasm.webprovider_brc20progWeb3ClientVersion(this.__wbg_ptr);
        return ret;
    }
    /**
     * Broadcast a raw transaction
     * @param {string} tx_hex
     * @returns {Promise<any>}
     */
    broadcastTransaction(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_broadcastTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} account
     * @returns {Promise<any>}
     */
    dataApiGetAccountBalance(account) {
        const ptr0 = passStringToWasm0(account, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAccountBalance(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} account
     * @returns {Promise<any>}
     */
    dataApiGetAccountUtxos(account) {
        const ptr0 = passStringToWasm0(account, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAccountUtxos(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    dataApiGetAddressBalance(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressBalance(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @param {boolean} include_outpoints
     * @returns {Promise<any>}
     */
    dataApiGetAddressBalances(address, include_outpoints) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressBalances(this.__wbg_ptr, ptr0, len0, include_outpoints);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    dataApiGetAddressOutpoints(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressOutpoints(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAddressPoolBurnHistory(address, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressPoolBurnHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAddressPoolCreationHistory(address, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressPoolCreationHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAddressPoolMintHistory(address, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressPoolMintHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @param {string} factory_id
     * @returns {Promise<any>}
     */
    dataApiGetAddressPositions(address, factory_id) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressPositions(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {string} address
     * @param {string} pool_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAddressSwapHistoryForPool(address, pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressSwapHistoryForPool(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @param {string} alkane_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAddressSwapHistoryForToken(address, alkane_id, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressSwapHistoryForToken(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAddressUnwrapHistory(address, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressUnwrapHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    dataApiGetAddressUtxos(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressUtxos(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAddressWrapHistory(address, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressWrapHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} alkane_id
     * @returns {Promise<any>}
     */
    dataApiGetAlkaneDetails(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAlkaneDetails(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} factory_id
     * @param {string} token_a_id
     * @param {string} token_b_id
     * @returns {Promise<any>}
     */
    dataApiGetAlkaneSwapPairDetails(factory_id, token_a_id, token_b_id) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(token_a_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(token_b_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAlkaneSwapPairDetails(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * @param {bigint | null} [page]
     * @param {bigint | null} [limit]
     * @returns {Promise<any>}
     */
    dataApiGetAlkanes(page, limit) {
        const ret = wasm.webprovider_dataApiGetAlkanes(this.__wbg_ptr, !isLikeNone(page), isLikeNone(page) ? BigInt(0) : page, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    dataApiGetAlkanesByAddress(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAlkanesByAddress(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    dataApiGetAlkanesUtxo(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAlkanesUtxo(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAllAddressAmmTxHistory(address, limit, offset) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAllAddressAmmTxHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAllAmmTxHistory(limit, offset) {
        const ret = wasm.webprovider_dataApiGetAllAmmTxHistory(this.__wbg_ptr, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} pool_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAllHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAllHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} factory_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @param {string | null} [sort_by]
     * @param {string | null} [order]
     * @returns {Promise<any>}
     */
    dataApiGetAllPoolsDetails(factory_id, limit, offset, sort_by, order) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(sort_by) ? 0 : passStringToWasm0(sort_by, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(order) ? 0 : passStringToWasm0(order, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAllPoolsDetails(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * @param {string} factory_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAllTokenPairs(factory_id, limit, offset) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAllTokenPairs(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAllUnwrapHistory(limit, offset) {
        const ret = wasm.webprovider_dataApiGetAllUnwrapHistory(this.__wbg_ptr, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetAllWrapHistory(limit, offset) {
        const ret = wasm.webprovider_dataApiGetAllWrapHistory(this.__wbg_ptr, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    dataApiGetAmmUtxos(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAmmUtxos(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} days
     * @returns {Promise<any>}
     */
    dataApiGetBitcoinMarketChart(days) {
        const ptr0 = passStringToWasm0(days, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetBitcoinMarketChart(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiGetBitcoinMarketWeekly() {
        const ret = wasm.webprovider_dataApiGetBitcoinMarketWeekly(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiGetBitcoinMarkets() {
        const ret = wasm.webprovider_dataApiGetBitcoinMarkets(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiGetBitcoinPrice() {
        const ret = wasm.webprovider_dataApiGetBitcoinPrice(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiGetBlockHash() {
        const ret = wasm.webprovider_dataApiGetBlockHash(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiGetBlockHeight() {
        const ret = wasm.webprovider_dataApiGetBlockHeight(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} pool_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetBurnHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetBurnHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} pool
     * @param {string} interval
     * @param {number | null} [start_time]
     * @param {number | null} [end_time]
     * @param {bigint | null} [limit]
     * @returns {Promise<any>}
     */
    dataApiGetCandles(pool, interval, start_time, end_time, limit) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetCandles(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(start_time), isLikeNone(start_time) ? 0 : start_time, !isLikeNone(end_time), isLikeNone(end_time) ? 0 : end_time, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit);
        return ret;
    }
    /**
     * @param {string} alkane
     * @param {bigint} page
     * @param {bigint} limit
     * @returns {Promise<any>}
     */
    dataApiGetHolders(alkane, page, limit) {
        const ptr0 = passStringToWasm0(alkane, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetHolders(this.__wbg_ptr, ptr0, len0, page, limit);
        return ret;
    }
    /**
     * @param {string} alkane
     * @returns {Promise<any>}
     */
    dataApiGetHoldersCount(alkane) {
        const ptr0 = passStringToWasm0(alkane, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetHoldersCount(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiGetIndexerPosition() {
        const ret = wasm.webprovider_dataApiGetIndexerPosition(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} address
     * @param {bigint | null} [total_txs]
     * @param {string | null} [last_seen_tx_id]
     * @returns {Promise<any>}
     */
    dataApiGetIntentHistory(address, total_txs, last_seen_tx_id) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(last_seen_tx_id) ? 0 : passStringToWasm0(last_seen_tx_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetIntentHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(total_txs), isLikeNone(total_txs) ? BigInt(0) : total_txs, ptr1, len1);
        return ret;
    }
    /**
     * @param {string} alkane
     * @param {string | null | undefined} prefix
     * @param {bigint} limit
     * @returns {Promise<any>}
     */
    dataApiGetKeys(alkane, prefix, limit) {
        const ptr0 = passStringToWasm0(alkane, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(prefix) ? 0 : passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetKeys(this.__wbg_ptr, ptr0, len0, ptr1, len1, limit);
        return ret;
    }
    /**
     * @param {string} pool_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetMintHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetMintHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} outpoint
     * @returns {Promise<any>}
     */
    dataApiGetOutpointBalances(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetOutpointBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string | null} [pool_id]
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetPoolBurnHistory(pool_id, limit, offset) {
        var ptr0 = isLikeNone(pool_id) ? 0 : passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolBurnHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} pool_id
     * @returns {Promise<any>}
     */
    dataApiGetPoolById(pool_id) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolById(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetPoolCreationHistory(limit, offset) {
        const ret = wasm.webprovider_dataApiGetPoolCreationHistory(this.__wbg_ptr, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} factory_id
     * @param {string} pool_id
     * @returns {Promise<any>}
     */
    dataApiGetPoolDetails(factory_id, pool_id) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolDetails(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {string} pool_id
     * @param {string | null} [category]
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetPoolHistory(pool_id, category, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(category) ? 0 : passStringToWasm0(category, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolHistory(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string | null} [pool_id]
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetPoolMintHistory(pool_id, limit, offset) {
        var ptr0 = isLikeNone(pool_id) ? 0 : passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolMintHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string | null} [pool_id]
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetPoolSwapHistory(pool_id, limit, offset) {
        var ptr0 = isLikeNone(pool_id) ? 0 : passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolSwapHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} factory_id
     * @returns {Promise<any>}
     */
    dataApiGetPools(factory_id) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPools(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} pool
     * @returns {Promise<any>}
     */
    dataApiGetReserves(pool) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetReserves(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} pool_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetSwapHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetSwapHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    dataApiGetTaprootBalance(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetTaprootBalance(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} taproot_address
     * @param {bigint} total_txs
     * @returns {Promise<any>}
     */
    dataApiGetTaprootHistory(taproot_address, total_txs) {
        const ptr0 = passStringToWasm0(taproot_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetTaprootHistory(this.__wbg_ptr, ptr0, len0, total_txs);
        return ret;
    }
    /**
     * @param {string} factory_id
     * @param {string | null} [alkane_id]
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetTokenPairs(factory_id, alkane_id, limit, offset) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(alkane_id) ? 0 : passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetTokenPairs(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @param {string} alkane_id
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGetTokenSwapHistory(alkane_id, limit, offset) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetTokenSwapHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiGetTotalUnwrapAmount() {
        const ret = wasm.webprovider_dataApiGetTotalUnwrapAmount(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} pool
     * @param {number | null} [start_time]
     * @param {number | null} [end_time]
     * @param {bigint | null} [limit]
     * @returns {Promise<any>}
     */
    dataApiGetTrades(pool, start_time, end_time, limit) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetTrades(this.__wbg_ptr, ptr0, len0, !isLikeNone(start_time), isLikeNone(start_time) ? 0 : start_time, !isLikeNone(end_time), isLikeNone(end_time) ? 0 : end_time, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit);
        return ret;
    }
    /**
     * @param {string} search_query
     * @param {bigint | null} [limit]
     * @param {bigint | null} [offset]
     * @returns {Promise<any>}
     */
    dataApiGlobalAlkanesSearch(search_query, limit, offset) {
        const ptr0 = passStringToWasm0(search_query, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGlobalAlkanesSearch(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    dataApiHealth() {
        const ret = wasm.webprovider_dataApiHealth(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} token_in
     * @param {string} token_out
     * @param {string} amount_in
     * @param {bigint | null} [max_hops]
     * @returns {Promise<any>}
     */
    dataApiPathfind(token_in, token_out, amount_in, max_hops) {
        const ptr0 = passStringToWasm0(token_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(token_out, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(amount_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiPathfind(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, !isLikeNone(max_hops), isLikeNone(max_hops) ? BigInt(0) : max_hops);
        return ret;
    }
    /**
     * @param {string} tx_hex
     * @returns {Promise<any>}
     */
    esploraBroadcastTx(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraBroadcastTx(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    esploraGetAddressInfo(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} prefix
     * @returns {Promise<any>}
     */
    esploraGetAddressPrefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressPrefix(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    esploraGetAddressTxs(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressTxs(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @param {string | null} [last_seen_txid]
     * @returns {Promise<any>}
     */
    esploraGetAddressTxsChain(address, last_seen_txid) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(last_seen_txid) ? 0 : passStringToWasm0(last_seen_txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressTxsChain(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    esploraGetAddressTxsMempool(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressTxsMempool(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    esploraGetAddressUtxo(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressUtxo(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hash
     * @returns {Promise<any>}
     */
    esploraGetBlock(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlock(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} height
     * @returns {Promise<any>}
     */
    esploraGetBlockByHeight(height) {
        const ret = wasm.webprovider_esploraGetBlockByHeight(this.__wbg_ptr, height);
        return ret;
    }
    /**
     * @param {string} hash
     * @returns {Promise<any>}
     */
    esploraGetBlockHeader(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockHeader(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hash
     * @returns {Promise<any>}
     */
    esploraGetBlockRaw(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockRaw(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hash
     * @returns {Promise<any>}
     */
    esploraGetBlockStatus(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockStatus(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hash
     * @param {number} index
     * @returns {Promise<any>}
     */
    esploraGetBlockTxid(hash, index) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockTxid(this.__wbg_ptr, ptr0, len0, index);
        return ret;
    }
    /**
     * @param {string} hash
     * @returns {Promise<any>}
     */
    esploraGetBlockTxids(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockTxids(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} hash
     * @param {number | null} [start_index]
     * @returns {Promise<any>}
     */
    esploraGetBlockTxs(hash, start_index) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockTxs(this.__wbg_ptr, ptr0, len0, !isLikeNone(start_index), isLikeNone(start_index) ? 0 : start_index);
        return ret;
    }
    /**
     * @param {number | null} [start_height]
     * @returns {Promise<any>}
     */
    esploraGetBlocks(start_height) {
        const ret = wasm.webprovider_esploraGetBlocks(this.__wbg_ptr, !isLikeNone(start_height), isLikeNone(start_height) ? 0 : start_height);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    esploraGetBlocksTipHash() {
        const ret = wasm.webprovider_esploraGetBlocksTipHash(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    esploraGetBlocksTipHeight() {
        const ret = wasm.webprovider_esploraGetBlocksTipHeight(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    esploraGetFeeEstimates() {
        const ret = wasm.webprovider_esploraGetFeeEstimates(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    esploraGetMempool() {
        const ret = wasm.webprovider_esploraGetMempool(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    esploraGetMempoolRecent() {
        const ret = wasm.webprovider_esploraGetMempoolRecent(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    esploraGetMempoolTxids() {
        const ret = wasm.webprovider_esploraGetMempoolTxids(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    esploraGetTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTx(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    esploraGetTxHex(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxHex(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    esploraGetTxMerkleProof(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxMerkleProof(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    esploraGetTxMerkleblockProof(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxMerkleblockProof(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @param {number} index
     * @returns {Promise<any>}
     */
    esploraGetTxOutspend(txid, index) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxOutspend(this.__wbg_ptr, ptr0, len0, index);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    esploraGetTxOutspends(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxOutspends(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    esploraGetTxRaw(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxRaw(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    esploraGetTxStatus(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxStatus(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} tx_hex
     * @returns {Promise<any>}
     */
    esploraPostTx(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraPostTx(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {string | undefined}
     */
    esplora_rpc_url() {
        const ret = wasm.webprovider_esplora_rpc_url(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Ping the ESPO AMM Data module
     * @returns {Promise<any>}
     */
    espoAmmdataPing() {
        const ret = wasm.webprovider_espoAmmdataPing(this.__wbg_ptr);
        return ret;
    }
    /**
     * Find the best swap path between two tokens using ESPO
     * @param {string} token_in
     * @param {string} token_out
     * @param {string | null} [mode]
     * @param {string | null} [amount_in]
     * @param {string | null} [amount_out]
     * @param {string | null} [amount_out_min]
     * @param {string | null} [amount_in_max]
     * @param {string | null} [available_in]
     * @param {number | null} [fee_bps]
     * @param {number | null} [max_hops]
     * @returns {Promise<any>}
     */
    espoFindBestSwapPath(token_in, token_out, mode, amount_in, amount_out, amount_out_min, amount_in_max, available_in, fee_bps, max_hops) {
        const ptr0 = passStringToWasm0(token_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(token_out, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(mode) ? 0 : passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(amount_in) ? 0 : passStringToWasm0(amount_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(amount_out) ? 0 : passStringToWasm0(amount_out, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len4 = WASM_VECTOR_LEN;
        var ptr5 = isLikeNone(amount_out_min) ? 0 : passStringToWasm0(amount_out_min, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len5 = WASM_VECTOR_LEN;
        var ptr6 = isLikeNone(amount_in_max) ? 0 : passStringToWasm0(amount_in_max, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len6 = WASM_VECTOR_LEN;
        var ptr7 = isLikeNone(available_in) ? 0 : passStringToWasm0(available_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len7 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoFindBestSwapPath(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6, ptr7, len7, !isLikeNone(fee_bps), isLikeNone(fee_bps) ? 0 : fee_bps, !isLikeNone(max_hops), isLikeNone(max_hops) ? 0 : max_hops);
        return ret;
    }
    /**
     * Get address activity from ESPO
     * @param {string} address
     * @returns {Promise<any>}
     */
    espoGetAddressActivity(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAddressActivity(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get alkanes balances for an address from ESPO
     * @param {string} address
     * @param {boolean} include_outpoints
     * @returns {Promise<any>}
     */
    espoGetAddressBalances(address, include_outpoints) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAddressBalances(this.__wbg_ptr, ptr0, len0, include_outpoints);
        return ret;
    }
    /**
     * Get outpoints containing alkanes for an address from ESPO
     * @param {string} address
     * @returns {Promise<any>}
     */
    espoGetAddressOutpoints(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAddressOutpoints(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get all transactions for an address from ESPO
     * @param {string} address
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @param {boolean | null} [only_alkane_txs]
     * @returns {Promise<any>}
     */
    espoGetAddressTransactions(address, page, limit, only_alkane_txs) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAddressTransactions(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, isLikeNone(only_alkane_txs) ? 0xFFFFFF : only_alkane_txs ? 1 : 0);
        return ret;
    }
    /**
     * Get alkane transactions for an address from ESPO
     * @param {string} address
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetAlkaneAddressTxs(address, page, limit) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneAddressTxs(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Get alkane balance via metashrew from ESPO
     * @param {string} owner
     * @param {string} target
     * @param {number | null} [height]
     * @returns {Promise<any>}
     */
    espoGetAlkaneBalanceMetashrew(owner, target, height) {
        const ptr0 = passStringToWasm0(owner, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneBalanceMetashrew(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(height), isLikeNone(height) ? 0 : height);
        return ret;
    }
    /**
     * Get alkane balance transactions from ESPO
     * @param {string} alkane_id
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetAlkaneBalanceTxs(alkane_id, page, limit) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneBalanceTxs(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Get alkane balance transactions by token from ESPO
     * @param {string} owner
     * @param {string} token
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetAlkaneBalanceTxsByToken(owner, token, page, limit) {
        const ptr0 = passStringToWasm0(owner, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(token, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneBalanceTxsByToken(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Get all balances for an alkane (all holders) from ESPO
     * @param {string} alkane_id
     * @returns {Promise<any>}
     */
    espoGetAlkaneBalances(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get alkane transactions in a block from ESPO
     * @param {number} height
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetAlkaneBlockTxs(height, page, limit) {
        const ret = wasm.webprovider_espoGetAlkaneBlockTxs(this.__wbg_ptr, height, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Get alkane ID from series ID (pizzafun namespace)
     * @param {string} series_id
     * @returns {Promise<any>}
     */
    espoGetAlkaneIdFromSeriesId(series_id) {
        const ptr0 = passStringToWasm0(series_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneIdFromSeriesId(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get alkane IDs from multiple series IDs (pizzafun namespace)
     * @param {string[]} series_ids
     * @returns {Promise<any>}
     */
    espoGetAlkaneIdsFromSeriesIds(series_ids) {
        const ptr0 = passArrayJsValueToWasm0(series_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneIdsFromSeriesIds(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get alkane info from ESPO
     * @param {string} alkane_id
     * @returns {Promise<any>}
     */
    espoGetAlkaneInfo(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get latest alkane traces from ESPO
     * @returns {Promise<any>}
     */
    espoGetAlkaneLatestTraces() {
        const ret = wasm.webprovider_espoGetAlkaneLatestTraces(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get alkane transaction summary from ESPO
     * @param {string} txid
     * @returns {Promise<any>}
     */
    espoGetAlkaneTxSummary(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAlkaneTxSummary(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get all alkanes from ESPO
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetAllAlkanes(page, limit) {
        const ret = wasm.webprovider_espoGetAllAlkanes(this.__wbg_ptr, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Get AMM factories from ESPO
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetAmmFactories(page, limit) {
        const ret = wasm.webprovider_espoGetAmmFactories(this.__wbg_ptr, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Find the best MEV swap opportunity for a token using ESPO
     * @param {string} token
     * @param {number | null} [fee_bps]
     * @param {number | null} [max_hops]
     * @returns {Promise<any>}
     */
    espoGetBestMevSwap(token, fee_bps, max_hops) {
        const ptr0 = passStringToWasm0(token, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetBestMevSwap(this.__wbg_ptr, ptr0, len0, !isLikeNone(fee_bps), isLikeNone(fee_bps) ? 0 : fee_bps, !isLikeNone(max_hops), isLikeNone(max_hops) ? 0 : max_hops);
        return ret;
    }
    /**
     * Get block summary from ESPO
     * @param {number} height
     * @returns {Promise<any>}
     */
    espoGetBlockSummary(height) {
        const ret = wasm.webprovider_espoGetBlockSummary(this.__wbg_ptr, height);
        return ret;
    }
    /**
     * Get block traces from ESPO
     * @param {number} height
     * @returns {Promise<any>}
     */
    espoGetBlockTraces(height) {
        const ret = wasm.webprovider_espoGetBlockTraces(this.__wbg_ptr, height);
        return ret;
    }
    /**
     * Get OHLCV candlestick data for a pool from ESPO
     * @param {string} pool
     * @param {string | null} [timeframe]
     * @param {string | null} [side]
     * @param {number | null} [limit]
     * @param {number | null} [page]
     * @returns {Promise<any>}
     */
    espoGetCandles(pool, timeframe, side, limit, page) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(timeframe) ? 0 : passStringToWasm0(timeframe, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(side) ? 0 : passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetCandles(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
    }
    /**
     * Get circulating supply of an alkane from ESPO
     * @param {string} alkane_id
     * @param {number | null} [height]
     * @returns {Promise<any>}
     */
    espoGetCirculatingSupply(alkane_id, height) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetCirculatingSupply(this.__wbg_ptr, ptr0, len0, !isLikeNone(height), isLikeNone(height) ? 0 : height);
        return ret;
    }
    /**
     * Get current ESPO indexer height
     * @returns {Promise<any>}
     */
    espoGetHeight() {
        const ret = wasm.webprovider_espoGetHeight(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get holders of an alkane token from ESPO
     * @param {string} alkane_id
     * @param {number} page
     * @param {number} limit
     * @returns {Promise<any>}
     */
    espoGetHolders(alkane_id, page, limit) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetHolders(this.__wbg_ptr, ptr0, len0, page, limit);
        return ret;
    }
    /**
     * Get holder count for an alkane from ESPO
     * @param {string} alkane_id
     * @returns {Promise<any>}
     */
    espoGetHoldersCount(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetHoldersCount(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get storage keys for an alkane contract from ESPO
     * @param {string} alkane_id
     * @param {number} page
     * @param {number} limit
     * @returns {Promise<any>}
     */
    espoGetKeys(alkane_id, page, limit) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetKeys(this.__wbg_ptr, ptr0, len0, page, limit);
        return ret;
    }
    /**
     * Get mempool traces from ESPO
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @param {string | null} [address]
     * @returns {Promise<any>}
     */
    espoGetMempoolTraces(page, limit, address) {
        var ptr0 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetMempoolTraces(this.__wbg_ptr, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, ptr0, len0);
        return ret;
    }
    /**
     * Get alkanes balances at a specific outpoint from ESPO
     * @param {string} outpoint
     * @returns {Promise<any>}
     */
    espoGetOutpointBalances(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetOutpointBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get all pools from ESPO
     * @param {number | null} [limit]
     * @param {number | null} [page]
     * @returns {Promise<any>}
     */
    espoGetPools(limit, page) {
        const ret = wasm.webprovider_espoGetPools(this.__wbg_ptr, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
    }
    /**
     * Get series ID from alkane ID (pizzafun namespace)
     * @param {string} alkane_id
     * @returns {Promise<any>}
     */
    espoGetSeriesIdFromAlkaneId(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetSeriesIdFromAlkaneId(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get series IDs from multiple alkane IDs (pizzafun namespace)
     * @param {string[]} alkane_ids
     * @returns {Promise<any>}
     */
    espoGetSeriesIdsFromAlkaneIds(alkane_ids) {
        const ptr0 = passArrayJsValueToWasm0(alkane_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetSeriesIdsFromAlkaneIds(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get total received for an alkane from ESPO
     * @param {string} alkane_id
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetTotalReceived(alkane_id, page, limit) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetTotalReceived(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Get trade history for a pool from ESPO
     * @param {string} pool
     * @param {number | null} [limit]
     * @param {number | null} [page]
     * @param {string | null} [side]
     * @param {string | null} [filter_side]
     * @param {string | null} [sort]
     * @param {string | null} [dir]
     * @returns {Promise<any>}
     */
    espoGetTrades(pool, limit, page, side, filter_side, sort, dir) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(side) ? 0 : passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(filter_side) ? 0 : passStringToWasm0(filter_side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(sort) ? 0 : passStringToWasm0(sort, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(dir) ? 0 : passStringToWasm0(dir, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len4 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetTrades(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, !isLikeNone(page), isLikeNone(page) ? 0 : page, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        return ret;
    }
    /**
     * Get transfer volume for an alkane from ESPO
     * @param {string} alkane_id
     * @param {number | null} [page]
     * @param {number | null} [limit]
     * @returns {Promise<any>}
     */
    espoGetTransferVolume(alkane_id, page, limit) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetTransferVolume(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit);
        return ret;
    }
    /**
     * Get all unwrap events from ESPO (subfrost namespace)
     * @param {number | null} [count]
     * @param {number | null} [offset]
     * @param {boolean | null} [successful]
     * @returns {Promise<any>}
     */
    espoGetUnwrapEvents(count, offset, successful) {
        const ret = wasm.webprovider_espoGetUnwrapEvents(this.__wbg_ptr, !isLikeNone(count), isLikeNone(count) ? 0 : count, !isLikeNone(offset), isLikeNone(offset) ? 0 : offset, isLikeNone(successful) ? 0xFFFFFF : successful ? 1 : 0);
        return ret;
    }
    /**
     * Get unwrap events for a specific address from ESPO (subfrost namespace)
     * @param {string} address
     * @param {number | null} [count]
     * @param {number | null} [offset]
     * @param {boolean | null} [successful]
     * @returns {Promise<any>}
     */
    espoGetUnwrapEventsByAddress(address, count, offset, successful) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetUnwrapEventsByAddress(this.__wbg_ptr, ptr0, len0, !isLikeNone(count), isLikeNone(count) ? 0 : count, !isLikeNone(offset), isLikeNone(offset) ? 0 : offset, isLikeNone(successful) ? 0xFFFFFF : successful ? 1 : 0);
        return ret;
    }
    /**
     * Get all wrap events from ESPO (subfrost namespace)
     * @param {number | null} [count]
     * @param {number | null} [offset]
     * @param {boolean | null} [successful]
     * @returns {Promise<any>}
     */
    espoGetWrapEvents(count, offset, successful) {
        const ret = wasm.webprovider_espoGetWrapEvents(this.__wbg_ptr, !isLikeNone(count), isLikeNone(count) ? 0 : count, !isLikeNone(offset), isLikeNone(offset) ? 0 : offset, isLikeNone(successful) ? 0xFFFFFF : successful ? 1 : 0);
        return ret;
    }
    /**
     * Get wrap events for a specific address from ESPO (subfrost namespace)
     * @param {string} address
     * @param {number | null} [count]
     * @param {number | null} [offset]
     * @param {boolean | null} [successful]
     * @returns {Promise<any>}
     */
    espoGetWrapEventsByAddress(address, count, offset, successful) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetWrapEventsByAddress(this.__wbg_ptr, ptr0, len0, !isLikeNone(count), isLikeNone(count) ? 0 : count, !isLikeNone(offset), isLikeNone(offset) ? 0 : offset, isLikeNone(successful) ? 0xFFFFFF : successful ? 1 : 0);
        return ret;
    }
    /**
     * Ping the ESPO essentials module
     * @returns {Promise<any>}
     */
    espoPing() {
        const ret = wasm.webprovider_espoPing(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get the FrBTC signer address for the current network
     * @returns {Promise<any>}
     */
    frbtcGetSignerAddress() {
        const ret = wasm.webprovider_frbtcGetSignerAddress(this.__wbg_ptr);
        return ret;
    }
    /**
     * Unwrap frBTC to BTC
     * params_json: { fee_rate?: number, from?: string[], change?: string }
     * @param {bigint} amount
     * @param {bigint} vout
     * @param {string} recipient_address
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    frbtcUnwrap(amount, vout, recipient_address, params_json) {
        const ptr0 = passStringToWasm0(recipient_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_frbtcUnwrap(this.__wbg_ptr, amount, vout, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Wrap BTC to frBTC
     * params_json: { fee_rate?: number, from?: string[], change?: string }
     * @param {bigint} amount
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    frbtcWrap(amount, params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_frbtcWrap(this.__wbg_ptr, amount, ptr0, len0);
        return ret;
    }
    /**
     * Wrap BTC and deploy+execute a script (wrapAndExecute)
     * params_json: { fee_rate?: number, from_addresses?: string[], change_address?: string, ... }
     * @param {bigint} amount
     * @param {string} script_bytecode
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    frbtcWrapAndExecute(amount, script_bytecode, params_json) {
        const ptr0 = passStringToWasm0(script_bytecode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_frbtcWrapAndExecute(this.__wbg_ptr, amount, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Wrap BTC and call an existing contract (wrapAndExecute2)
     * params_json: { fee_rate?: number, from_addresses?: string[], change_address?: string, ... }
     * @param {bigint} amount
     * @param {string} target_address
     * @param {string} signature
     * @param {string} calldata_args
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    frbtcWrapAndExecute2(amount, target_address, signature, calldata_args, params_json) {
        const ptr0 = passStringToWasm0(target_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(signature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(calldata_args, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len3 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_frbtcWrapAndExecute2(this.__wbg_ptr, amount, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret;
    }
    /**
     * Get all transactions for an address from Esplora
     * @param {string} address
     * @returns {Promise<any>}
     */
    getAddressTxs(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getAddressTxs(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get address transactions with complete runestone traces (CLI: esplora address-txs --runestone-trace)
     * @param {string} address
     * @param {boolean | null} [exclude_coinbase]
     * @returns {Promise<any>}
     */
    getAddressTxsWithTraces(address, exclude_coinbase) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getAddressTxsWithTraces(this.__wbg_ptr, ptr0, len0, isLikeNone(exclude_coinbase) ? 0xFFFFFF : exclude_coinbase ? 1 : 0);
        return ret;
    }
    /**
     * Get address UTXOs
     * @param {string} address
     * @returns {Promise<any>}
     */
    getAddressUtxos(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getAddressUtxos(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get enriched wallet balances using the balances.lua script
     *
     * This uses the built-in balances.lua script with automatic hash-based caching.
     * Returns comprehensive balance data including spendable UTXOs, asset UTXOs, and pending.
     * @param {string} address
     * @param {string | null} [protocol_tag]
     * @returns {Promise<any>}
     */
    getEnrichedBalances(address, protocol_tag) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(protocol_tag) ? 0 : passStringToWasm0(protocol_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getEnrichedBalances(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @param {bigint} block
     * @param {bigint} tx
     * @param {Uint8Array} path
     * @returns {Promise<any>}
     */
    getStorageAt(block, tx, path) {
        const ptr0 = passArray8ToWasm0(path, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getStorageAt(this.__wbg_ptr, block, tx, ptr0, len0);
        return ret;
    }
    /**
     * Get raw transaction hex
     * @param {string} txid
     * @returns {Promise<any>}
     */
    getTransactionHex(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getTransactionHex(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Execute a Lua script with arguments, using scripthash caching
     *
     * This method first tries to use the cached scripthash version (lua_evalsaved),
     * and falls back to the full script (lua_evalscript) if the hash isn't cached.
     * This is the recommended way to execute Lua scripts for better performance.
     *
     * # Arguments
     * * `script` - The Lua script content
     * * `args` - JSON-serialized array of arguments to pass to the script
     * @param {string} script
     * @param {any} args
     * @returns {Promise<any>}
     */
    luaEval(script, args) {
        const ptr0 = passStringToWasm0(script, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_luaEval(this.__wbg_ptr, ptr0, len0, args);
        return ret;
    }
    /**
     * @param {string} script
     * @returns {Promise<any>}
     */
    luaEvalScript(script) {
        const ptr0 = passStringToWasm0(script, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_luaEvalScript(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} height
     * @returns {Promise<any>}
     */
    metashrewGetBlockHash(height) {
        const ret = wasm.webprovider_metashrewGetBlockHash(this.__wbg_ptr, height);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    metashrewHeight() {
        const ret = wasm.webprovider_metashrewHeight(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number | null} [height]
     * @returns {Promise<any>}
     */
    metashrewStateRoot(height) {
        const ret = wasm.webprovider_metashrewStateRoot(this.__wbg_ptr, !isLikeNone(height), isLikeNone(height) ? 0 : height);
        return ret;
    }
    /**
     * Generic metashrew_view call
     *
     * Calls the metashrew_view RPC method with the given view function, payload, and block tag.
     * This is the low-level method for calling any metashrew view function.
     *
     * # Arguments
     * * `view_fn` - The view function name (e.g., "simulate", "protorunesbyaddress")
     * * `payload` - The hex-encoded payload (with or without 0x prefix)
     * * `block_tag` - The block tag ("latest" or a block height as string)
     *
     * # Returns
     * The hex-encoded response string from the view function
     * @param {string} view_fn
     * @param {string} payload
     * @param {string} block_tag
     * @returns {Promise<any>}
     */
    metashrewView(view_fn, payload, block_tag) {
        const ptr0 = passStringToWasm0(view_fn, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(payload, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_metashrewView(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * Create a new WebProvider from provider name and optional config overrides
     *
     * # Arguments
     * * `provider` - Network provider: "mainnet", "signet", "subfrost-regtest", "regtest"
     * * `config` - Optional JS object with RpcConfig fields to override defaults
     *
     * # Example (JavaScript)
     * ```js
     * // Simple - uses all defaults for signet
     * const provider = new WebProvider("signet");
     *
     * // With overrides
     * const provider = new WebProvider("signet", {
     *   bitcoin_rpc_url: "https://custom-rpc.example.com",
     *   esplora_url: "https://custom-esplora.example.com"
     * });
     * ```
     * @param {string} provider
     * @param {any | null} [config]
     */
    constructor(provider, config) {
        const ptr0 = passStringToWasm0(provider, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_new_js(ptr0, len0, isLikeNone(config) ? 0 : addToExternrefTable0(config));
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WebProviderFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get OPI activity on block
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiActivityOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiActivityOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI balance on block
     * @param {string} base_url
     * @param {number} block_height
     * @param {string} pkscript
     * @param {string} ticker
     * @returns {Promise<any>}
     */
    opiBalanceOnBlock(base_url, block_height, pkscript, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBalanceOnBlock(this.__wbg_ptr, ptr0, len0, block_height, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * Get OPI Bitcoin RPC results on block
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiBitcoinRpcResultsOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitcoinRpcResultsOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI Bitmap block height
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiBitmapBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI Bitmap hash of all activity
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiBitmapHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI Bitmap hash of all bitmaps
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiBitmapHashOfAllBitmaps(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapHashOfAllBitmaps(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI Bitmap inscription ID
     * @param {string} base_url
     * @param {string} bitmap
     * @returns {Promise<any>}
     */
    opiBitmapInscriptionId(base_url, bitmap) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(bitmap, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapInscriptionId(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI block height
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI current balance
     * @param {string} base_url
     * @param {string} ticker
     * @param {string | null} [address]
     * @param {string | null} [pkscript]
     * @returns {Promise<any>}
     */
    opiCurrentBalance(base_url, ticker, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiCurrentBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret;
    }
    /**
     * Get OPI database version
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiDbVersion(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiDbVersion(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI event
     * @param {string} base_url
     * @param {string} event_hash
     * @returns {Promise<any>}
     */
    opiEvent(base_url, event_hash) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(event_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiEvent(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI event hash version
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiEventHashVersion(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiEventHashVersion(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI extras block height
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiExtrasBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiExtrasBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI hash of all activity
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI hash of all current balances
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiHashOfAllCurrentBalances(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiHashOfAllCurrentBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI holders
     * @param {string} base_url
     * @param {string} ticker
     * @returns {Promise<any>}
     */
    opiHolders(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiHolders(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI IP address
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiIp(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiIp(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI POW20 activity on block
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiPow20ActivityOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20ActivityOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI POW20 balance on block
     * @param {string} base_url
     * @param {number} block_height
     * @param {string} pkscript
     * @param {string} ticker
     * @returns {Promise<any>}
     */
    opiPow20BalanceOnBlock(base_url, block_height, pkscript, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20BalanceOnBlock(this.__wbg_ptr, ptr0, len0, block_height, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * Get OPI POW20 block height
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiPow20BlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20BlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI POW20 current balance
     * @param {string} base_url
     * @param {string} ticker
     * @param {string | null} [address]
     * @param {string | null} [pkscript]
     * @returns {Promise<any>}
     */
    opiPow20CurrentBalance(base_url, ticker, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20CurrentBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret;
    }
    /**
     * Get OPI POW20 hash of all activity
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiPow20HashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20HashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI POW20 hash of all current balances
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiPow20HashOfAllCurrentBalances(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20HashOfAllCurrentBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI POW20 holders
     * @param {string} base_url
     * @param {string} ticker
     * @returns {Promise<any>}
     */
    opiPow20Holders(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20Holders(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI POW20 valid tx notes of ticker
     * @param {string} base_url
     * @param {string} ticker
     * @returns {Promise<any>}
     */
    opiPow20ValidTxNotesOfTicker(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20ValidTxNotesOfTicker(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI POW20 valid tx notes of wallet
     * @param {string} base_url
     * @param {string | null} [address]
     * @param {string | null} [pkscript]
     * @returns {Promise<any>}
     */
    opiPow20ValidTxNotesOfWallet(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20ValidTxNotesOfWallet(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * Get OPI raw endpoint
     * @param {string} base_url
     * @param {string} endpoint
     * @returns {Promise<any>}
     */
    opiRaw(base_url, endpoint) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(endpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRaw(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI Runes activity on block
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiRunesActivityOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesActivityOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI Runes balance on block
     * @param {string} base_url
     * @param {number} block_height
     * @param {string} pkscript
     * @param {string} rune_id
     * @returns {Promise<any>}
     */
    opiRunesBalanceOnBlock(base_url, block_height, pkscript, rune_id) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(rune_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesBalanceOnBlock(this.__wbg_ptr, ptr0, len0, block_height, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * Get OPI Runes block height
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiRunesBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI Runes current balance
     * @param {string} base_url
     * @param {string | null} [address]
     * @param {string | null} [pkscript]
     * @returns {Promise<any>}
     */
    opiRunesCurrentBalance(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesCurrentBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * Get OPI Runes event
     * @param {string} base_url
     * @param {string} txid
     * @returns {Promise<any>}
     */
    opiRunesEvent(base_url, txid) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesEvent(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI Runes hash of all activity
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiRunesHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI Runes holders
     * @param {string} base_url
     * @param {string} rune_id
     * @returns {Promise<any>}
     */
    opiRunesHolders(base_url, rune_id) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(rune_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesHolders(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI Runes unspent outpoints
     * @param {string} base_url
     * @param {string | null} [address]
     * @param {string | null} [pkscript]
     * @returns {Promise<any>}
     */
    opiRunesUnspentOutpoints(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesUnspentOutpoints(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * Get OPI SNS block height
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiSnsBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI SNS hash of all activity
     * @param {string} base_url
     * @param {number} block_height
     * @returns {Promise<any>}
     */
    opiSnsHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
    }
    /**
     * Get OPI SNS hash of all registered names
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiSnsHashOfAllRegisteredNames(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsHashOfAllRegisteredNames(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI SNS info
     * @param {string} base_url
     * @param {string} name
     * @returns {Promise<any>}
     */
    opiSnsInfo(base_url, name) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsInfo(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI SNS inscriptions of domain
     * @param {string} base_url
     * @param {string} domain
     * @returns {Promise<any>}
     */
    opiSnsInscriptionsOfDomain(base_url, domain) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(domain, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsInscriptionsOfDomain(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI SNS registered namespaces
     * @param {string} base_url
     * @returns {Promise<any>}
     */
    opiSnsRegisteredNamespaces(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsRegisteredNamespaces(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get OPI valid tx notes of ticker
     * @param {string} base_url
     * @param {string} ticker
     * @returns {Promise<any>}
     */
    opiValidTxNotesOfTicker(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiValidTxNotesOfTicker(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get OPI valid tx notes of wallet
     * @param {string} base_url
     * @param {string | null} [address]
     * @param {string | null} [pkscript]
     * @returns {Promise<any>}
     */
    opiValidTxNotesOfWallet(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiValidTxNotesOfWallet(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    ordAddressInfo(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordAddressInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    ordBlockCount() {
        const ret = wasm.webprovider_ordBlockCount(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} query
     * @returns {Promise<any>}
     */
    ordBlockInfo(query) {
        const ptr0 = passStringToWasm0(query, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordBlockInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    ordBlocks() {
        const ret = wasm.webprovider_ordBlocks(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} inscription_id
     * @param {number | null} [page]
     * @returns {Promise<any>}
     */
    ordChildren(inscription_id, page) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordChildren(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
    }
    /**
     * @param {string} inscription_id
     * @returns {Promise<any>}
     */
    ordContent(inscription_id) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordContent(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} sat
     * @returns {Promise<any>}
     */
    ordFind(sat) {
        const ret = wasm.webprovider_ordFind(this.__wbg_ptr, sat);
        return ret;
    }
    /**
     * @param {string} inscription_id
     * @returns {Promise<any>}
     */
    ordInscription(inscription_id) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordInscription(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {number | null} [page]
     * @returns {Promise<any>}
     */
    ordInscriptions(page) {
        const ret = wasm.webprovider_ordInscriptions(this.__wbg_ptr, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
    }
    /**
     * @param {string} outpoint
     * @returns {Promise<any>}
     */
    ordList(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordList(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} address
     * @returns {Promise<any>}
     */
    ordOutputs(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordOutputs(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} inscription_id
     * @param {number | null} [page]
     * @returns {Promise<any>}
     */
    ordParents(inscription_id, page) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordParents(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
    }
    /**
     * @param {string} rune
     * @returns {Promise<any>}
     */
    ordRune(rune) {
        const ptr0 = passStringToWasm0(rune, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordRune(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    ordTxInfo(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordTxInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    protorunesAnalyzeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_protorunesAnalyzeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    protorunesDecodeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_protorunesDecodeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    runestoneAnalyzeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_runestoneAnalyzeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    runestoneDecodeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_runestoneDecodeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {string}
     */
    sandshrew_rpc_url() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.webprovider_sandshrew_rpc_url(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Calculate minimum unwrap amount for subfrost frBTC unwrapping
     * @param {number | null} [fee_rate_override]
     * @param {number | null} [premium]
     * @param {number | null} [expected_inputs]
     * @param {number | null} [expected_outputs]
     * @param {boolean | null} [raw]
     * @returns {Promise<any>}
     */
    subfrostMinimumUnwrap(fee_rate_override, premium, expected_inputs, expected_outputs, raw) {
        const ret = wasm.webprovider_subfrostMinimumUnwrap(this.__wbg_ptr, !isLikeNone(fee_rate_override), isLikeNone(fee_rate_override) ? 0 : fee_rate_override, !isLikeNone(premium), isLikeNone(premium) ? 0 : premium, !isLikeNone(expected_inputs), isLikeNone(expected_inputs) ? 0 : expected_inputs, !isLikeNone(expected_outputs), isLikeNone(expected_outputs) ? 0 : expected_outputs, isLikeNone(raw) ? 0xFFFFFF : raw ? 1 : 0);
        return ret;
    }
    /**
     * @param {number} height
     * @returns {Promise<any>}
     */
    traceBlock(height) {
        const ret = wasm.webprovider_traceBlock(this.__wbg_ptr, height);
        return ret;
    }
    /**
     * Trace alkanes execution for a protostone outpoint
     * @param {string} outpoint
     * @returns {Promise<any>}
     */
    traceOutpoint(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_traceOutpoint(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {string} txid
     * @returns {Promise<any>}
     */
    traceProtostones(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_traceProtostones(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    waitForIndexer() {
        const ret = wasm.webprovider_waitForIndexer(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    walletBackup() {
        const ret = wasm.webprovider_walletBackup(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new wallet with an optional mnemonic phrase
     * If no mnemonic is provided, a new one will be generated
     * Returns wallet info including address and mnemonic
     *
     * Note: This sets the keystore on self synchronously so walletIsLoaded() returns true immediately
     * @param {string | null} [mnemonic]
     * @param {string | null} [passphrase]
     * @returns {any}
     */
    walletCreate(mnemonic, passphrase) {
        var ptr0 = isLikeNone(mnemonic) ? 0 : passStringToWasm0(mnemonic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(passphrase) ? 0 : passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletCreate(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    walletCreatePsbt(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletCreatePsbt(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {Promise<any>}
     */
    walletExport() {
        const ret = wasm.webprovider_walletExport(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get the wallet's primary address
     * @returns {Promise<any>}
     */
    walletGetAddress() {
        const ret = wasm.webprovider_walletGetAddress(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get addresses from the loaded wallet keystore
     * Uses the Keystore.get_addresses method from alkanes-cli-common
     *
     * # Arguments
     * * `address_type` - Address type: "p2tr", "p2wpkh", "p2sh-p2wpkh", "p2pkh"
     * * `start_index` - Starting index for address derivation
     * * `count` - Number of addresses to derive
     * * `chain` - Chain index (0 for external/receiving, 1 for internal/change)
     *
     * # Returns
     * Array of address info objects with: { derivation_path, address, script_type, index, used }
     * @param {string} address_type
     * @param {number} start_index
     * @param {number} count
     * @param {number | null} [chain]
     * @returns {any}
     */
    walletGetAddresses(address_type, start_index, count, chain) {
        const ptr0 = passStringToWasm0(address_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetAddresses(this.__wbg_ptr, ptr0, len0, start_index, count, isLikeNone(chain) ? 0x100000001 : (chain) >>> 0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Get the wallet's BTC balance
     * Returns { confirmed: number, pending: number }
     * @param {string[] | null} [addresses]
     * @returns {Promise<any>}
     */
    walletGetBalance(addresses) {
        var ptr0 = isLikeNone(addresses) ? 0 : passArrayJsValueToWasm0(addresses, wasm.__wbindgen_malloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetBalance(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get transaction history for an address
     * @param {string | null} [address]
     * @returns {Promise<any>}
     */
    walletGetHistory(address) {
        var ptr0 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetHistory(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get UTXOs for the wallet
     * @param {string[] | null} [addresses]
     * @returns {Promise<any>}
     */
    walletGetUtxos(addresses) {
        var ptr0 = isLikeNone(addresses) ? 0 : passArrayJsValueToWasm0(addresses, wasm.__wbindgen_malloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetUtxos(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Check if wallet is loaded (has keystore for signing)
     * @returns {boolean}
     */
    walletIsLoaded() {
        const ret = wasm.webprovider_walletIsLoaded(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Load an existing wallet from storage
     * @param {string | null} [passphrase]
     * @returns {Promise<any>}
     */
    walletLoad(passphrase) {
        var ptr0 = isLikeNone(passphrase) ? 0 : passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletLoad(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Load a wallet from mnemonic for signing transactions
     * This must be called before walletSend or other signing operations
     * @param {string} mnemonic_str
     * @param {string | null} [passphrase]
     */
    walletLoadMnemonic(mnemonic_str, passphrase) {
        const ptr0 = passStringToWasm0(mnemonic_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(passphrase) ? 0 : passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletLoadMnemonic(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Send BTC to an address
     * params: { address: string, amount: number (satoshis), fee_rate?: number }
     * Wallet must be loaded first via walletLoadMnemonic
     * @param {string} params_json
     * @returns {Promise<any>}
     */
    walletSend(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletSend(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
}
if (Symbol.dispose) WebProvider.prototype[Symbol.dispose] = WebProvider.prototype.free;

/**
 * @param {string} psbt_base64
 * @param {string} network_str
 * @returns {string}
 */
export function analyze_psbt(psbt_base64, network_str) {
    let deferred4_0;
    let deferred4_1;
    try {
        const ptr0 = passStringToWasm0(psbt_base64, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(network_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.analyze_psbt(ptr0, len0, ptr1, len1);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
            ptr3 = 0; len3 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
    } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
    }
}

/**
 * Analyze a transaction's runestone to extract Protostones
 *
 * This function takes a raw transaction hex string, decodes it, and extracts
 * all Protostones from the transaction's OP_RETURN output.
 *
 * # Arguments
 *
 * * `tx_hex` - Hexadecimal string of the raw transaction (with or without "0x" prefix)
 *
 * # Returns
 *
 * A JSON string containing:
 * - `protostone_count`: Number of Protostones found
 * - `protostones`: Array of Protostone objects with their details
 *
 * # Example
 *
 * ```javascript
 * const result = analyze_runestone(txHex);
 * const data = JSON.parse(result);
 * console.log(`Found ${data.protostone_count} Protostones`);
 * ```
 * @param {string} tx_hex
 * @returns {string}
 */
export function analyze_runestone(tx_hex) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.analyze_runestone(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Deploy a BRC20-prog contract from Foundry JSON
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `foundry_json` - Foundry build JSON as string containing contract bytecode
 * * `params_json` - JSON string with execution parameters:
 *   ```json
 *   {
 *     "from_addresses": ["address1", "address2"],  // optional
 *     "change_address": "address",                  // optional
 *     "fee_rate": 100.0,                            // optional, sat/vB
 *     "use_activation": false,                      // optional, use 3-tx pattern
 *     "use_slipstream": false,                      // optional
 *     "use_rebar": false,                           // optional
 *     "rebar_tier": 1,                              // optional (1 or 2)
 *     "resume_from_commit": "txid"                  // optional, auto-detects commit/reveal
 *   }
 *   ```
 *
 * # Returns
 *
 * A JSON string containing:
 * - `commit_txid`: Commit transaction ID
 * - `reveal_txid`: Reveal transaction ID
 * - `activation_txid`: Activation transaction ID (if use_activation=true)
 * - `commit_fee`: Commit fee in sats
 * - `reveal_fee`: Reveal fee in sats
 * - `activation_fee`: Activation fee in sats (if applicable)
 *
 * # Example
 *
 * ```javascript
 * const result = await brc20_prog_deploy_contract(
 *   "regtest",
 *   foundryJson,
 *   JSON.stringify({ fee_rate: 100, use_activation: false })
 * );
 * const data = JSON.parse(result);
 * console.log(`Deployed! Commit: ${data.commit_txid}, Reveal: ${data.reveal_txid}`);
 * ```
 * @param {string} network
 * @param {string} foundry_json
 * @param {string} params_json
 * @returns {Promise<any>}
 */
export function brc20_prog_deploy_contract(network, foundry_json, params_json) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(foundry_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.brc20_prog_deploy_contract(ptr0, len0, ptr1, len1, ptr2, len2);
    return ret;
}

/**
 * Call a BRC20-prog contract function (transact)
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `contract_address` - Contract address to call (0x-prefixed hex)
 * * `function_signature` - Function signature (e.g., "transfer(address,uint256)")
 * * `calldata` - Comma-separated calldata arguments
 * * `params_json` - JSON string with execution parameters (same as deploy_contract)
 *
 * # Returns
 *
 * A JSON string with transaction details (same format as deploy_contract)
 *
 * # Example
 *
 * ```javascript
 * const result = await brc20_prog_transact(
 *   "regtest",
 *   "0x1234567890abcdef1234567890abcdef12345678",
 *   "transfer(address,uint256)",
 *   "0xrecipient,1000",
 *   JSON.stringify({ fee_rate: 100 })
 * );
 * const data = JSON.parse(result);
 * console.log(`Transaction sent! Commit: ${data.commit_txid}`);
 * ```
 * @param {string} network
 * @param {string} contract_address
 * @param {string} function_signature
 * @param {string} calldata
 * @param {string} params_json
 * @returns {Promise<any>}
 */
export function brc20_prog_transact(network, contract_address, function_signature, calldata, params_json) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(contract_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(function_signature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len2 = WASM_VECTOR_LEN;
    const ptr3 = passStringToWasm0(calldata, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len3 = WASM_VECTOR_LEN;
    const ptr4 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len4 = WASM_VECTOR_LEN;
    const ret = wasm.brc20_prog_transact(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
    return ret;
}

/**
 * Wrap BTC into frBTC and execute a contract call in one transaction
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `target_contract` - Target contract address for wrapAndExecute2
 * * `function_signature` - Function signature for the target contract call
 * * `calldata` - Comma-separated calldata arguments for the target function
 * * `params_json` - JSON string with execution parameters:
 *   ```json
 *   {
 *     "from_addresses": ["address1", "address2"],  // optional
 *     "change_address": "address",                  // optional
 *     "fee_rate": 100.0                             // optional, sat/vB
 *   }
 *   ```
 *
 * # Returns
 *
 * A JSON string with transaction details
 *
 * # Example
 *
 * ```javascript
 * const result = await brc20_prog_wrap_btc(
 *   "regtest",
 *   100000,  // 100k sats
 *   "0xtargetContract",
 *   "someFunction(uint256)",
 *   "42",
 *   JSON.stringify({ fee_rate: 100 })
 * );
 * const data = JSON.parse(result);
 * console.log(`frBTC wrapped! Reveal: ${data.reveal_txid}`);
 * ```
 * @param {string} network
 * @param {bigint} amount
 * @param {string} target_contract
 * @param {string} function_signature
 * @param {string} calldata
 * @param {string} params_json
 * @returns {Promise<any>}
 */
export function brc20_prog_wrap_btc(network, amount, target_contract, function_signature, calldata, params_json) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(target_contract, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(function_signature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len2 = WASM_VECTOR_LEN;
    const ptr3 = passStringToWasm0(calldata, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len3 = WASM_VECTOR_LEN;
    const ptr4 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len4 = WASM_VECTOR_LEN;
    const ret = wasm.brc20_prog_wrap_btc(ptr0, len0, amount, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
    return ret;
}

/**
 * Decode a PSBT (Partially Signed Bitcoin Transaction) from base64
 *
 * This function decodes a PSBT from its base64 representation and returns
 * a JSON object containing detailed information about the transaction,
 * inputs, outputs, and PSBT-specific fields.
 *
 * # Arguments
 *
 * * `psbt_base64` - Base64 encoded PSBT string
 *
 * # Returns
 *
 * A JSON string containing the decoded PSBT information including:
 * - Transaction details (txid, version, locktime, inputs, outputs)
 * - Global PSBT data (xpubs)
 * - Per-input data (witness UTXOs, scripts, signatures, derivation paths)
 * - Per-output data (scripts, derivation paths)
 * - Fee information (if calculable)
 *
 * # Example
 *
 * ```javascript
 * const decodedPsbt = decode_psbt(psbtBase64);
 * const data = JSON.parse(decodedPsbt);
 * console.log(`TXID: ${data.tx.txid}`);
 * console.log(`Fee: ${data.fee} sats`);
 * ```
 * @param {string} psbt_base64
 * @returns {string}
 */
export function decode_psbt(psbt_base64) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(psbt_base64, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.decode_psbt(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Asynchronously encrypts data using the Web Crypto API.
 * @param {string} mnemonic
 * @param {string} passphrase
 * @returns {Promise<any>}
 */
export function encryptMnemonic(mnemonic, passphrase) {
    const ptr0 = passStringToWasm0(mnemonic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.encryptMnemonic(ptr0, len0, ptr1, len1);
    return ret;
}

/**
 * Get the FrBTC signer address for a network
 *
 * This calls getSignerAddress() on the FrBTC contract to get the p2tr address
 * where BTC should be sent for wrapping.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 *
 * # Returns
 *
 * A JSON string containing:
 * - `network`: The network name
 * - `frbtc_contract`: The FrBTC contract address
 * - `signer_address`: The Bitcoin p2tr address for the signer
 * @param {string} network
 * @returns {Promise<any>}
 */
export function frbtc_get_signer_address(network) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.frbtc_get_signer_address(ptr0, len0);
    return ret;
}

/**
 * Unwrap frBTC to BTC
 *
 * This calls unwrap2() on the FrBTC contract to burn frBTC and queue a BTC payment.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of frBTC to unwrap (in satoshis)
 * * `vout` - Vout index for the inscription output
 * * `recipient_address` - Bitcoin address to receive the unwrapped BTC
 * * `params_json` - JSON string with execution parameters
 *
 * # Returns
 *
 * A JSON string with transaction details
 * @param {string} network
 * @param {bigint} amount
 * @param {bigint} vout
 * @param {string} recipient_address
 * @param {string} params_json
 * @returns {Promise<any>}
 */
export function frbtc_unwrap(network, amount, vout, recipient_address, params_json) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(recipient_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.frbtc_unwrap(ptr0, len0, amount, vout, ptr1, len1, ptr2, len2);
    return ret;
}

/**
 * Simple wrap: convert BTC to frBTC without executing any contract
 *
 * This calls the wrap() function on the FrBTC contract.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `params_json` - JSON string with execution parameters:
 *   ```json
 *   {
 *     "from_addresses": ["address1", "address2"],  // optional
 *     "change_address": "address",                  // optional
 *     "fee_rate": 100.0                             // optional, sat/vB
 *   }
 *   ```
 *
 * # Returns
 *
 * A JSON string with transaction details
 * @param {string} network
 * @param {bigint} amount
 * @param {string} params_json
 * @returns {Promise<any>}
 */
export function frbtc_wrap(network, amount, params_json) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.frbtc_wrap(ptr0, len0, amount, ptr1, len1);
    return ret;
}

/**
 * Wrap BTC and deploy+execute a script (wrapAndExecute)
 *
 * This calls wrapAndExecute() on the FrBTC contract.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `script_bytecode` - Script bytecode to deploy and execute (hex-encoded)
 * * `params_json` - JSON string with execution parameters
 *
 * # Returns
 *
 * A JSON string with transaction details
 * @param {string} network
 * @param {bigint} amount
 * @param {string} script_bytecode
 * @param {string} params_json
 * @returns {Promise<any>}
 */
export function frbtc_wrap_and_execute(network, amount, script_bytecode, params_json) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(script_bytecode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.frbtc_wrap_and_execute(ptr0, len0, amount, ptr1, len1, ptr2, len2);
    return ret;
}

/**
 * Wrap BTC and call an existing contract (wrapAndExecute2)
 *
 * This calls wrapAndExecute2() on the FrBTC contract.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `target_address` - Target contract address
 * * `function_signature` - Function signature (e.g., "deposit()")
 * * `calldata_args` - Comma-separated calldata arguments
 * * `params_json` - JSON string with execution parameters
 *
 * # Returns
 *
 * A JSON string with transaction details
 * @param {string} network
 * @param {bigint} amount
 * @param {string} target_address
 * @param {string} function_signature
 * @param {string} calldata_args
 * @param {string} params_json
 * @returns {Promise<any>}
 */
export function frbtc_wrap_and_execute2(network, amount, target_address, function_signature, calldata_args, params_json) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(target_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(function_signature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len2 = WASM_VECTOR_LEN;
    const ptr3 = passStringToWasm0(calldata_args, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len3 = WASM_VECTOR_LEN;
    const ptr4 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len4 = WASM_VECTOR_LEN;
    const ret = wasm.frbtc_wrap_and_execute2(ptr0, len0, amount, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
    return ret;
}

/**
 * @param {string} network
 * @param {number} block
 * @param {number} tx
 * @param {string} block_tag
 * @returns {Promise<any>}
 */
export function get_alkane_bytecode(network, block, tx, block_tag) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.get_alkane_bytecode(ptr0, len0, block, tx, ptr1, len1);
    return ret;
}

/**
 * @param {string} network
 * @param {number} block
 * @param {number} tx
 * @param {string} block_tag
 * @returns {Promise<any>}
 */
export function get_alkane_meta(network, block, tx, block_tag) {
    const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.get_alkane_meta(ptr0, len0, block, tx, ptr1, len1);
    return ret;
}

/**
 * Initialize the panic hook for better error messages in WASM
 * This should be called early in your application
 */
export function init_panic_hook() {
    wasm.init_panic_hook();
}

/**
 * @param {string} alkane_id_str
 * @param {string} wasm_hex
 * @param {string} cellpack_hex
 * @returns {Promise<any>}
 */
export function simulate_alkane_call(alkane_id_str, wasm_hex, cellpack_hex) {
    const ptr0 = passStringToWasm0(alkane_id_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(wasm_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passStringToWasm0(cellpack_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.simulate_alkane_call(ptr0, len0, ptr1, len1, ptr2, len2);
    return ret;
}
export function __wbg_Error_83742b46f01ce22d(arg0, arg1) {
    const ret = Error(getStringFromWasm0(arg0, arg1));
    return ret;
}
export function __wbg_Number_a5a435bd7bbec835(arg0) {
    const ret = Number(arg0);
    return ret;
}
export function __wbg_String_8564e559799eccda(arg0, arg1) {
    const ret = String(arg1);
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg___wbindgen_bigint_get_as_i64_447a76b5c6ef7bda(arg0, arg1) {
    const v = arg1;
    const ret = typeof(v) === 'bigint' ? v : undefined;
    getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
}
export function __wbg___wbindgen_boolean_get_c0f3f60bac5a78d1(arg0) {
    const v = arg0;
    const ret = typeof(v) === 'boolean' ? v : undefined;
    return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
}
export function __wbg___wbindgen_debug_string_5398f5bb970e0daa(arg0, arg1) {
    const ret = debugString(arg1);
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg___wbindgen_in_41dbb8413020e076(arg0, arg1) {
    const ret = arg0 in arg1;
    return ret;
}
export function __wbg___wbindgen_is_bigint_e2141d4f045b7eda(arg0) {
    const ret = typeof(arg0) === 'bigint';
    return ret;
}
export function __wbg___wbindgen_is_falsy_30906e697739fcc2(arg0) {
    const ret = !arg0;
    return ret;
}
export function __wbg___wbindgen_is_function_3c846841762788c1(arg0) {
    const ret = typeof(arg0) === 'function';
    return ret;
}
export function __wbg___wbindgen_is_null_0b605fc6b167c56f(arg0) {
    const ret = arg0 === null;
    return ret;
}
export function __wbg___wbindgen_is_object_781bc9f159099513(arg0) {
    const val = arg0;
    const ret = typeof(val) === 'object' && val !== null;
    return ret;
}
export function __wbg___wbindgen_is_string_7ef6b97b02428fae(arg0) {
    const ret = typeof(arg0) === 'string';
    return ret;
}
export function __wbg___wbindgen_is_undefined_52709e72fb9f179c(arg0) {
    const ret = arg0 === undefined;
    return ret;
}
export function __wbg___wbindgen_jsval_eq_ee31bfad3e536463(arg0, arg1) {
    const ret = arg0 === arg1;
    return ret;
}
export function __wbg___wbindgen_jsval_loose_eq_5bcc3bed3c69e72b(arg0, arg1) {
    const ret = arg0 == arg1;
    return ret;
}
export function __wbg___wbindgen_number_get_34bb9d9dcfa21373(arg0, arg1) {
    const obj = arg1;
    const ret = typeof(obj) === 'number' ? obj : undefined;
    getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
}
export function __wbg___wbindgen_string_get_395e606bd0ee4427(arg0, arg1) {
    const obj = arg1;
    const ret = typeof(obj) === 'string' ? obj : undefined;
    var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg___wbindgen_throw_6ddd609b62940d55(arg0, arg1) {
    throw new Error(getStringFromWasm0(arg0, arg1));
}
export function __wbg__wbg_cb_unref_6b5b6b8576d35cb1(arg0) {
    arg0._wbg_cb_unref();
}
export function __wbg_abort_5ef96933660780b7(arg0) {
    arg0.abort();
}
export function __wbg_abort_6479c2d794ebf2ee(arg0, arg1) {
    arg0.abort(arg1);
}
export function __wbg_append_608dfb635ee8998f() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
    arg0.append(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
}, arguments); }
export function __wbg_arrayBuffer_eb8e9ca620af2a19() { return handleError(function (arg0) {
    const ret = arg0.arrayBuffer();
    return ret;
}, arguments); }
export function __wbg_call_2d781c1f4d5c0ef8() { return handleError(function (arg0, arg1, arg2) {
    const ret = arg0.call(arg1, arg2);
    return ret;
}, arguments); }
export function __wbg_call_dcc2662fa17a72cf() { return handleError(function (arg0, arg1, arg2, arg3) {
    const ret = arg0.call(arg1, arg2, arg3);
    return ret;
}, arguments); }
export function __wbg_call_e133b57c9155d22c() { return handleError(function (arg0, arg1) {
    const ret = arg0.call(arg1);
    return ret;
}, arguments); }
export function __wbg_clearTimeout_113b1cde814ec762(arg0) {
    const ret = clearTimeout(arg0);
    return ret;
}
export function __wbg_clearTimeout_5db739eb9091b37c(arg0) {
    const ret = clearTimeout(arg0);
    return ret;
}
export function __wbg_connect_7e0e52a995d66bc1(arg0) {
    const ret = arg0.connect();
    return ret;
}
export function __wbg_crypto_48300657fced39f9(arg0) {
    const ret = arg0.crypto;
    return ret;
}
export function __wbg_decrypt_1b72b2941d2dfb44() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
    const ret = arg0.decrypt(arg1, arg2, getArrayU8FromWasm0(arg3, arg4));
    return ret;
}, arguments); }
export function __wbg_deriveBits_5738cc9a13a38aec() { return handleError(function (arg0, arg1, arg2, arg3) {
    const ret = arg0.deriveBits(arg1, arg2, arg3 >>> 0);
    return ret;
}, arguments); }
export function __wbg_disconnect_28d5361403e91b2e(arg0) {
    const ret = arg0.disconnect();
    return ret;
}
export function __wbg_done_08ce71ee07e3bd17(arg0) {
    const ret = arg0.done;
    return ret;
}
export function __wbg_encrypt_f249dadef2c0f0ba() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
    const ret = arg0.encrypt(arg1, arg2, getArrayU8FromWasm0(arg3, arg4));
    return ret;
}, arguments); }
export function __wbg_entries_e8a20ff8c9757101(arg0) {
    const ret = Object.entries(arg0);
    return ret;
}
export function __wbg_error_a6fa202b58aa1cd3(arg0, arg1) {
    let deferred0_0;
    let deferred0_1;
    try {
        deferred0_0 = arg0;
        deferred0_1 = arg1;
        console.error(getStringFromWasm0(arg0, arg1));
    } finally {
        wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
    }
}
export function __wbg_fetch_3d08fa8b3caa5cf7(arg0) {
    const ret = fetch(arg0);
    return ret;
}
export function __wbg_fetch_5550a88cf343aaa9(arg0, arg1) {
    const ret = arg0.fetch(arg1);
    return ret;
}
export function __wbg_fromCodePoint_4a02c6dcced1d233() { return handleError(function (arg0) {
    const ret = String.fromCodePoint(arg0 >>> 0);
    return ret;
}, arguments); }
export function __wbg_from_4bdf88943703fd48(arg0) {
    const ret = Array.from(arg0);
    return ret;
}
export function __wbg_getAccounts_ae121c0a1e74deef(arg0) {
    const ret = arg0.getAccounts();
    return ret;
}
export function __wbg_getBalance_d750c654508b9e26(arg0) {
    const ret = arg0.getBalance();
    return ret;
}
export function __wbg_getInfo_2f5f96913db24745(arg0) {
    const ret = arg0.getInfo();
    return ret;
}
export function __wbg_getInscriptions_33763691ba0be59e(arg0, arg1, arg2) {
    const ret = arg0.getInscriptions(arg1, arg2);
    return ret;
}
export function __wbg_getItem_a7cc1d4f154f2e6f() { return handleError(function (arg0, arg1, arg2, arg3) {
    const ret = arg1.getItem(getStringFromWasm0(arg2, arg3));
    var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}, arguments); }
export function __wbg_getNetwork_90fad8bc0d0c14c2(arg0) {
    const ret = arg0.getNetwork();
    return ret;
}
export function __wbg_getPublicKey_1db490c01333436a(arg0) {
    const ret = arg0.getPublicKey();
    return ret;
}
export function __wbg_getRandomValues_227324ee0d4080c2() { return handleError(function (arg0, arg1, arg2) {
    const ret = arg0.getRandomValues(getArrayU8FromWasm0(arg1, arg2));
    return ret;
}, arguments); }
export function __wbg_getRandomValues_263d0aa5464054ee() { return handleError(function (arg0, arg1) {
    arg0.getRandomValues(arg1);
}, arguments); }
export function __wbg_get_326e41e095fb2575() { return handleError(function (arg0, arg1) {
    const ret = Reflect.get(arg0, arg1);
    return ret;
}, arguments); }
export function __wbg_get_3ef1eba1850ade27() { return handleError(function (arg0, arg1) {
    const ret = Reflect.get(arg0, arg1);
    return ret;
}, arguments); }
export function __wbg_get_a8ee5c45dabc1b3b(arg0, arg1) {
    const ret = arg0[arg1 >>> 0];
    return ret;
}
export function __wbg_get_unchecked_329cfe50afab7352(arg0, arg1) {
    const ret = arg0[arg1 >>> 0];
    return ret;
}
export function __wbg_get_with_ref_key_6412cf3094599694(arg0, arg1) {
    const ret = arg0[arg1];
    return ret;
}
export function __wbg_has_926ef2ff40b308cf() { return handleError(function (arg0, arg1) {
    const ret = Reflect.has(arg0, arg1);
    return ret;
}, arguments); }
export function __wbg_headers_eb2234545f9ff993(arg0) {
    const ret = arg0.headers;
    return ret;
}
export function __wbg_importKey_397406e38b080c57() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
    const ret = arg0.importKey(getStringFromWasm0(arg1, arg2), arg3, arg4, arg5 !== 0, arg6);
    return ret;
}, arguments); }
export function __wbg_instanceof_ArrayBuffer_101e2bf31071a9f6(arg0) {
    let result;
    try {
        result = arg0 instanceof ArrayBuffer;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_CryptoKey_8aec6f1abbdff9a3(arg0) {
    let result;
    try {
        result = arg0 instanceof CryptoKey;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Crypto_5c2a4c15287b0332(arg0) {
    let result;
    try {
        result = arg0 instanceof Crypto;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Map_f194b366846aca0c(arg0) {
    let result;
    try {
        result = arg0 instanceof Map;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Object_be1962063fcc0c9f(arg0) {
    let result;
    try {
        result = arg0 instanceof Object;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Response_9b4d9fd451e051b1(arg0) {
    let result;
    try {
        result = arg0 instanceof Response;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Uint8Array_740438561a5b956d(arg0) {
    let result;
    try {
        result = arg0 instanceof Uint8Array;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Window_23e677d2c6843922(arg0) {
    let result;
    try {
        result = arg0 instanceof Window;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_isArray_33b91feb269ff46e(arg0) {
    const ret = Array.isArray(arg0);
    return ret;
}
export function __wbg_isSafeInteger_ecd6a7f9c3e053cd(arg0) {
    const ret = Number.isSafeInteger(arg0);
    return ret;
}
export function __wbg_iterator_d8f549ec8fb061b1() {
    const ret = Symbol.iterator;
    return ret;
}
export function __wbg_key_84733a6ee7e4d63e() { return handleError(function (arg0, arg1, arg2) {
    const ret = arg1.key(arg2 >>> 0);
    var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}, arguments); }
export function __wbg_length_8632bd8b5ab30449() { return handleError(function (arg0) {
    const ret = arg0.length;
    return ret;
}, arguments); }
export function __wbg_length_b3416cf66a5452c8(arg0) {
    const ret = arg0.length;
    return ret;
}
export function __wbg_length_ea16607d7b61445b(arg0) {
    const ret = arg0.length;
    return ret;
}
export function __wbg_localStorage_51c38b3b222e1ed2() { return handleError(function (arg0) {
    const ret = arg0.localStorage;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
}, arguments); }
export function __wbg_msCrypto_8c6d45a75ef1d3da(arg0) {
    const ret = arg0.msCrypto;
    return ret;
}
export function __wbg_new_0837727332ac86ba() { return handleError(function () {
    const ret = new Headers();
    return ret;
}, arguments); }
export function __wbg_new_0_1dcafdf5e786e876() {
    const ret = new Date();
    return ret;
}
export function __wbg_new_227d7c05414eb861() {
    const ret = new Error();
    return ret;
}
export function __wbg_new_49d5571bd3f0c4d4() {
    const ret = new Map();
    return ret;
}
export function __wbg_new_5f486cdf45a04d78(arg0) {
    const ret = new Uint8Array(arg0);
    return ret;
}
export function __wbg_new_a70fbab9066b301f() {
    const ret = new Array();
    return ret;
}
export function __wbg_new_ab79df5bd7c26067() {
    const ret = new Object();
    return ret;
}
export function __wbg_new_c518c60af666645b() { return handleError(function () {
    const ret = new AbortController();
    return ret;
}, arguments); }
export function __wbg_new_d098e265629cd10f(arg0, arg1) {
    try {
        var state0 = {a: arg0, b: arg1};
        var cb0 = (arg0, arg1) => {
            const a = state0.a;
            state0.a = 0;
            try {
                return wasm_bindgen__convert__closures_____invoke__h8504f2f9dcdf50c7(a, state0.b, arg0, arg1);
            } finally {
                state0.a = a;
            }
        };
        const ret = new Promise(cb0);
        return ret;
    } finally {
        state0.a = state0.b = 0;
    }
}
export function __wbg_new_from_slice_22da9388ac046e50(arg0, arg1) {
    const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
    return ret;
}
export function __wbg_new_typed_aaaeaf29cf802876(arg0, arg1) {
    try {
        var state0 = {a: arg0, b: arg1};
        var cb0 = (arg0, arg1) => {
            const a = state0.a;
            state0.a = 0;
            try {
                return wasm_bindgen__convert__closures_____invoke__h8504f2f9dcdf50c7(a, state0.b, arg0, arg1);
            } finally {
                state0.a = a;
            }
        };
        const ret = new Promise(cb0);
        return ret;
    } finally {
        state0.a = state0.b = 0;
    }
}
export function __wbg_new_with_length_825018a1616e9e55(arg0) {
    const ret = new Uint8Array(arg0 >>> 0);
    return ret;
}
export function __wbg_new_with_str_and_init_b4b54d1a819bc724() { return handleError(function (arg0, arg1, arg2) {
    const ret = new Request(getStringFromWasm0(arg0, arg1), arg2);
    return ret;
}, arguments); }
export function __wbg_next_11b99ee6237339e3() { return handleError(function (arg0) {
    const ret = arg0.next();
    return ret;
}, arguments); }
export function __wbg_next_e01a967809d1aa68(arg0) {
    const ret = arg0.next;
    return ret;
}
export function __wbg_node_95beb7570492fd97(arg0) {
    const ret = arg0.node;
    return ret;
}
export function __wbg_now_16f0c993d5dd6c27() {
    const ret = Date.now();
    return ret;
}
export function __wbg_of_8bf7ed3eca00ea43(arg0) {
    const ret = Array.of(arg0);
    return ret;
}
export function __wbg_parse_e9eddd2a82c706eb() { return handleError(function (arg0, arg1) {
    const ret = JSON.parse(getStringFromWasm0(arg0, arg1));
    return ret;
}, arguments); }
export function __wbg_process_b2fea42461d03994(arg0) {
    const ret = arg0.process;
    return ret;
}
export function __wbg_prototypesetcall_d62e5099504357e6(arg0, arg1, arg2) {
    Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
}
export function __wbg_pushPsbt_57e9bb97befe68ba(arg0, arg1, arg2) {
    const ret = arg0.pushPsbt(getStringFromWasm0(arg1, arg2));
    return ret;
}
export function __wbg_pushTx_16e80c6176095f04(arg0, arg1, arg2) {
    const ret = arg0.pushTx(getStringFromWasm0(arg1, arg2));
    return ret;
}
export function __wbg_push_e87b0e732085a946(arg0, arg1) {
    const ret = arg0.push(arg1);
    return ret;
}
export function __wbg_queueMicrotask_0c399741342fb10f(arg0) {
    const ret = arg0.queueMicrotask;
    return ret;
}
export function __wbg_queueMicrotask_a082d78ce798393e(arg0) {
    queueMicrotask(arg0);
}
export function __wbg_randomFillSync_ca9f178fb14c88cb() { return handleError(function (arg0, arg1) {
    arg0.randomFillSync(arg1);
}, arguments); }
export function __wbg_removeItem_95c258b9afdd7580() { return handleError(function (arg0, arg1, arg2) {
    arg0.removeItem(getStringFromWasm0(arg1, arg2));
}, arguments); }
export function __wbg_require_7a9419e39d796c95() { return handleError(function () {
    const ret = module.require;
    return ret;
}, arguments); }
export function __wbg_resolve_ae8d83246e5bcc12(arg0) {
    const ret = Promise.resolve(arg0);
    return ret;
}
export function __wbg_setItem_5f84aeef0d7f3c17() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
    arg0.setItem(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
}, arguments); }
export function __wbg_setTimeout_ef24d2fc3ad97385() { return handleError(function (arg0, arg1) {
    const ret = setTimeout(arg0, arg1);
    return ret;
}, arguments); }
export function __wbg_setTimeout_f863349add4de61f(arg0, arg1) {
    const ret = setTimeout(arg0, arg1);
    return ret;
}
export function __wbg_set_282384002438957f(arg0, arg1, arg2) {
    arg0[arg1 >>> 0] = arg2;
}
export function __wbg_set_6be42768c690e380(arg0, arg1, arg2) {
    arg0[arg1] = arg2;
}
export function __wbg_set_7eaa4f96924fd6b3() { return handleError(function (arg0, arg1, arg2) {
    const ret = Reflect.set(arg0, arg1, arg2);
    return ret;
}, arguments); }
export function __wbg_set_8c0b3ffcf05d61c2(arg0, arg1, arg2) {
    arg0.set(getArrayU8FromWasm0(arg1, arg2));
}
export function __wbg_set_bf7251625df30a02(arg0, arg1, arg2) {
    const ret = arg0.set(arg1, arg2);
    return ret;
}
export function __wbg_set_body_a3d856b097dfda04(arg0, arg1) {
    arg0.body = arg1;
}
export function __wbg_set_cache_ec7e430c6056ebda(arg0, arg1) {
    arg0.cache = __wbindgen_enum_RequestCache[arg1];
}
export function __wbg_set_credentials_ed63183445882c65(arg0, arg1) {
    arg0.credentials = __wbindgen_enum_RequestCredentials[arg1];
}
export function __wbg_set_headers_3c8fecc693b75327(arg0, arg1) {
    arg0.headers = arg1;
}
export function __wbg_set_method_8c015e8bcafd7be1(arg0, arg1, arg2) {
    arg0.method = getStringFromWasm0(arg1, arg2);
}
export function __wbg_set_mode_5a87f2c809cf37c2(arg0, arg1) {
    arg0.mode = __wbindgen_enum_RequestMode[arg1];
}
export function __wbg_set_signal_0cebecb698f25d21(arg0, arg1) {
    arg0.signal = arg1;
}
export function __wbg_signMessage_c82d87f3c9b483fc(arg0, arg1, arg2, arg3, arg4) {
    const ret = arg0.signMessage(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
    return ret;
}
export function __wbg_signPsbt_f6807c996d7476d6(arg0, arg1, arg2, arg3) {
    const ret = arg0.signPsbt(getStringFromWasm0(arg1, arg2), arg3);
    return ret;
}
export function __wbg_signPsbts_43afe2e25d24a995(arg0, arg1, arg2) {
    const ret = arg0.signPsbts(arg1, arg2);
    return ret;
}
export function __wbg_signal_166e1da31adcac18(arg0) {
    const ret = arg0.signal;
    return ret;
}
export function __wbg_stack_3b0d974bbf31e44f(arg0, arg1) {
    const ret = arg1.stack;
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg_static_accessor_GLOBAL_8adb955bd33fac2f() {
    const ret = typeof global === 'undefined' ? null : global;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
}
export function __wbg_static_accessor_GLOBAL_THIS_ad356e0db91c7913() {
    const ret = typeof globalThis === 'undefined' ? null : globalThis;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
}
export function __wbg_static_accessor_SELF_f207c857566db248() {
    const ret = typeof self === 'undefined' ? null : self;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
}
export function __wbg_static_accessor_WINDOW_bb9f1ba69d61b386() {
    const ret = typeof window === 'undefined' ? null : window;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
}
export function __wbg_status_318629ab93a22955(arg0) {
    const ret = arg0.status;
    return ret;
}
export function __wbg_stringify_5ae93966a84901ac() { return handleError(function (arg0) {
    const ret = JSON.stringify(arg0);
    return ret;
}, arguments); }
export function __wbg_subarray_a068d24e39478a8a(arg0, arg1, arg2) {
    const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
    return ret;
}
export function __wbg_subtle_c060fa3eb5c6248d(arg0) {
    const ret = arg0.subtle;
    return ret;
}
export function __wbg_switchNetwork_06dfbb04ab0de615(arg0, arg1, arg2) {
    const ret = arg0.switchNetwork(getStringFromWasm0(arg1, arg2));
    return ret;
}
export function __wbg_text_372f5b91442c50f9() { return handleError(function (arg0) {
    const ret = arg0.text();
    return ret;
}, arguments); }
export function __wbg_then_098abe61755d12f6(arg0, arg1) {
    const ret = arg0.then(arg1);
    return ret;
}
export function __wbg_then_9e335f6dd892bc11(arg0, arg1, arg2) {
    const ret = arg0.then(arg1, arg2);
    return ret;
}
export function __wbg_toISOString_87e7eaab337f7dcc(arg0) {
    const ret = arg0.toISOString();
    return ret;
}
export function __wbg_url_7fefc1820fba4e0c(arg0, arg1) {
    const ret = arg1.url;
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg_value_21fc78aab0322612(arg0) {
    const ret = arg0.value;
    return ret;
}
export function __wbg_versions_215a3ab1c9d5745a(arg0) {
    const ret = arg0.versions;
    return ret;
}
export function __wbg_warn_69424c2d92a2fa73(arg0) {
    console.warn(arg0);
}
export function __wbg_wasmbrowserwalletprovider_new(arg0) {
    const ret = WasmBrowserWalletProvider.__wrap(arg0);
    return ret;
}
export function __wbindgen_cast_0000000000000001(arg0, arg1) {
    // Cast intrinsic for `Closure(Closure { dtor_idx: 3165, function: Function { arguments: [], shim_idx: 3166, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
    const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h2734ba064662ab68, wasm_bindgen__convert__closures_____invoke__h455513963c9e1b53);
    return ret;
}
export function __wbindgen_cast_0000000000000002(arg0, arg1) {
    // Cast intrinsic for `Closure(Closure { dtor_idx: 3850, function: Function { arguments: [], shim_idx: 3851, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
    const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h1d0669ed74bc8e06, wasm_bindgen__convert__closures_____invoke__h59e39e4e43f2f72d);
    return ret;
}
export function __wbindgen_cast_0000000000000003(arg0, arg1) {
    // Cast intrinsic for `Closure(Closure { dtor_idx: 4592, function: Function { arguments: [Externref], shim_idx: 4593, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
    const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h07405935f6e373f9, wasm_bindgen__convert__closures_____invoke__h2270663853d2b7ca);
    return ret;
}
export function __wbindgen_cast_0000000000000004(arg0) {
    // Cast intrinsic for `F64 -> Externref`.
    const ret = arg0;
    return ret;
}
export function __wbindgen_cast_0000000000000005(arg0) {
    // Cast intrinsic for `I64 -> Externref`.
    const ret = arg0;
    return ret;
}
export function __wbindgen_cast_0000000000000006(arg0, arg1) {
    // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
    const ret = getArrayU8FromWasm0(arg0, arg1);
    return ret;
}
export function __wbindgen_cast_0000000000000007(arg0, arg1) {
    // Cast intrinsic for `Ref(String) -> Externref`.
    const ret = getStringFromWasm0(arg0, arg1);
    return ret;
}
export function __wbindgen_cast_0000000000000008(arg0, arg1) {
    // Cast intrinsic for `U128 -> Externref`.
    const ret = (BigInt.asUintN(64, arg0) | (BigInt.asUintN(64, arg1) << BigInt(64)));
    return ret;
}
export function __wbindgen_cast_0000000000000009(arg0) {
    // Cast intrinsic for `U64 -> Externref`.
    const ret = BigInt.asUintN(64, arg0);
    return ret;
}
export function __wbindgen_init_externref_table() {
    const table = wasm.__wbindgen_externrefs;
    const offset = table.grow(4);
    table.set(0, undefined);
    table.set(offset + 0, undefined);
    table.set(offset + 1, null);
    table.set(offset + 2, true);
    table.set(offset + 3, false);
}
function wasm_bindgen__convert__closures_____invoke__h455513963c9e1b53(arg0, arg1) {
    wasm.wasm_bindgen__convert__closures_____invoke__h455513963c9e1b53(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__h59e39e4e43f2f72d(arg0, arg1) {
    wasm.wasm_bindgen__convert__closures_____invoke__h59e39e4e43f2f72d(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__h2270663853d2b7ca(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen__convert__closures_____invoke__h2270663853d2b7ca(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen__convert__closures_____invoke__h8504f2f9dcdf50c7(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen__convert__closures_____invoke__h8504f2f9dcdf50c7(arg0, arg1, arg2, arg3);
}


const __wbindgen_enum_RequestCache = ["default", "no-store", "reload", "no-cache", "force-cache", "only-if-cached"];


const __wbindgen_enum_RequestCredentials = ["omit", "same-origin", "include"];


const __wbindgen_enum_RequestMode = ["same-origin", "no-cors", "cors", "navigate"];
const KeystoreFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_keystore_free(ptr >>> 0, 1));
const PbkdfParamsFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_pbkdfparams_free(ptr >>> 0, 1));
const WasmBrowserWalletProviderFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmbrowserwalletprovider_free(ptr >>> 0, 1));
const WebProviderFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_webprovider_free(ptr >>> 0, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => state.dtor(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            state.dtor(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;


let wasm;
export function __wbg_set_wasm(val) {
    wasm = val;
}
