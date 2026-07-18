/**
 * Node.js-compatible WASM loader for alkanes-web-sys
 *
 * This module provides a way to load the WASM module in Node.js environments
 * where ES module WASM imports are not supported.
 */

const fs = require('fs');
const path = require('path');

let initialized = false;
let jsBindings = null;

/**
 * Initialize the WASM module for Node.js
 * @returns {Promise<object>} The WASM module exports including WebProvider
 */
async function init() {
  if (initialized) {
    return jsBindings;
  }

  // Read the WASM binary
  const wasmPath = path.join(__dirname, 'alkanes_web_sys_bg.wasm');
  const wasmBuffer = fs.readFileSync(wasmPath);

  // Import the JS bindings using dynamic import (since it's an ES module)
  const bindings = await import('./alkanes_web_sys_bg.js');
  jsBindings = bindings;

  // Create import object for WASM instantiation
  // The WASM module expects imports from './alkanes_web_sys_bg.js' (bundler target)
  const imports = {
    './alkanes_web_sys_bg.js': bindings,
  };

  // Instantiate the WASM module
  const wasmModule = await WebAssembly.instantiate(wasmBuffer, imports);
  const wasmInstance = wasmModule.instance;

  // Set the WASM instance in the JS bindings
  bindings.__wbg_set_wasm(wasmInstance.exports);

  // Call the start function if it exists
  if (wasmInstance.exports.__wbindgen_start) {
    wasmInstance.exports.__wbindgen_start();
  }

  initialized = true;

  return jsBindings;
}

/**
 * Get the WebProvider class (initializes WASM if needed)
 * @returns {Promise<typeof WebProvider>}
 */
async function getWebProvider() {
  const exports = await init();
  return exports.WebProvider;
}

module.exports = {
  init,
  getWebProvider,
  get WebProvider() {
    if (!initialized) {
      throw new Error('WASM not initialized. Call init() first or use getWebProvider().');
    }
    return jsBindings.WebProvider;
  }
};
