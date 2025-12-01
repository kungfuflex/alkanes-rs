async function instantiate(module, imports = {}) {
  const adaptedImports = {
    env: Object.assign(Object.create(globalThis), imports.env || {}, {
      __staticcall(cellpack, incoming_alkanes, checkpoint, fuel) {
        // ../alkanes-asm-common/assembly/runtime/__staticcall(i32, i32, i32, u64) => i32
        fuel = BigInt.asUintN(64, fuel);
        return __staticcall(cellpack, incoming_alkanes, checkpoint, fuel);
      },
    }),
  };
  const { exports } = await WebAssembly.instantiate(module, adaptedImports);
  const memory = exports.memory || imports.env.memory;
  const adaptedExports = Object.setPrototypeOf({
    enrichAlkane(block, tx, response) {
      // assembly/index/enrichAlkane(../alkanes-asm-common/assembly/u128/u128, ../alkanes-asm-common/assembly/u128/u128, ../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse) => void
      block = __retain(__lowerInternref(block) || __notnull());
      tx = __retain(__lowerInternref(tx) || __notnull());
      response = __lowerInternref(response) || __notnull();
      try {
        exports.enrichAlkane(block, tx, response);
      } finally {
        __release(block);
        __release(tx);
      }
    },
  }, exports);
  class Internref extends Number {}
  function __lowerInternref(value) {
    if (value == null) return 0;
    if (value instanceof Internref) return value.valueOf();
    throw TypeError("internref expected");
  }
  const refcounts = new Map();
  function __retain(pointer) {
    if (pointer) {
      const refcount = refcounts.get(pointer);
      if (refcount) refcounts.set(pointer, refcount + 1);
      else refcounts.set(exports.__pin(pointer), 1);
    }
    return pointer;
  }
  function __release(pointer) {
    if (pointer) {
      const refcount = refcounts.get(pointer);
      if (refcount === 1) exports.__unpin(pointer), refcounts.delete(pointer);
      else if (refcount) refcounts.set(pointer, refcount - 1);
      else throw Error(`invalid refcount '${refcount}' for reference '${pointer}'`);
    }
  }
  function __notnull() {
    throw TypeError("value must not be null");
  }
  exports._start();
  return adaptedExports;
}
export const {
  memory,
  enrichAlkane,
  __execute,
} = await (async url => instantiate(
  await (async () => {
    const isNodeOrBun = typeof process != "undefined" && process.versions != null && (process.versions.node != null || process.versions.bun != null);
    if (isNodeOrBun) { return globalThis.WebAssembly.compile(await (await import("node:fs/promises")).readFile(url)); }
    else { return await globalThis.WebAssembly.compileStreaming(globalThis.fetch(url)); }
  })(), {
  }
))(new URL("release.wasm", import.meta.url));
