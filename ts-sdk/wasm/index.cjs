// CommonJS export for wasm module
const wasm_module = require('./alkanes_web_sys.js');
const wasm = require('./alkanes_web_sys_bg.wasm');

module.exports = {
  ...wasm_module,
  wasm
};
