/* @ts-self-types="./alkanes_web_sys.d.ts" */

import * as wasm from "./alkanes_web_sys_bg.wasm";
import { __wbg_set_wasm } from "./alkanes_web_sys_bg.js";
__wbg_set_wasm(wasm);
wasm.__wbindgen_start();
export {
    Keystore, PbkdfParams, WasmBrowserWalletProvider, WebProvider, analyze_psbt, analyze_runestone, brc20_prog_deploy_contract, brc20_prog_transact, brc20_prog_wrap_btc, decode_psbt, encryptMnemonic, frbtc_get_signer_address, frbtc_unwrap, frbtc_wrap, frbtc_wrap_and_execute, frbtc_wrap_and_execute2, get_alkane_bytecode, get_alkane_meta, init_panic_hook, simulate_alkane_call
} from "./alkanes_web_sys_bg.js";
