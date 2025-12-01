async function instantiate(module, imports = {}) {
  const adaptedImports = {
    env: Object.assign(Object.create(globalThis), imports.env || {}, {
      __staticcall(cellpack, incoming_alkanes, checkpoint, start_fuel) {
        // ../alkanes-asm-common/assembly/alkanes/runtime/__staticcall(i32, i32, i32, u64) => i32
        start_fuel = BigInt.asUintN(64, start_fuel);
        return __staticcall(cellpack, incoming_alkanes, checkpoint, start_fuel);
      },
      __call(cellpack, incoming_alkanes, checkpoint, start_fuel) {
        // ../alkanes-asm-common/assembly/alkanes/runtime/__call(i32, i32, i32, u64) => i32
        start_fuel = BigInt.asUintN(64, start_fuel);
        return __call(cellpack, incoming_alkanes, checkpoint, start_fuel);
      },
      __delegatecall(cellpack, incoming_alkanes, checkpoint, start_fuel) {
        // ../alkanes-asm-common/assembly/alkanes/runtime/__delegatecall(i32, i32, i32, u64) => i32
        start_fuel = BigInt.asUintN(64, start_fuel);
        return __delegatecall(cellpack, incoming_alkanes, checkpoint, start_fuel);
      },
    }),
  };
  const { exports } = await WebAssembly.instantiate(module, adaptedImports);
  exports._start();
  return exports;
}
export const {
  memory,
  __execute,
} = await (async url => instantiate(
  await (async () => {
    const isNodeOrBun = typeof process != "undefined" && process.versions != null && (process.versions.node != null || process.versions.bun != null);
    if (isNodeOrBun) { return globalThis.WebAssembly.compile(await (await import("node:fs/promises")).readFile(url)); }
    else { return await globalThis.WebAssembly.compileStreaming(globalThis.fetch(url)); }
  })(), {
  }
))(new URL("release.wasm", import.meta.url));
