async function instantiate(module, imports = {}) {
  const adaptedImports = {
    env: Object.assign(Object.create(globalThis), imports.env || {}, {
      __call(cellpack, incoming_alkanes, checkpoint, start_fuel) {
        // assembly/alkanes/runtime/__call(i32, i32, i32, u64) => i32
        start_fuel = BigInt.asUintN(64, start_fuel);
        return __call(cellpack, incoming_alkanes, checkpoint, start_fuel);
      },
      __staticcall(cellpack, incoming_alkanes, checkpoint, start_fuel) {
        // assembly/alkanes/runtime/__staticcall(i32, i32, i32, u64) => i32
        start_fuel = BigInt.asUintN(64, start_fuel);
        return __staticcall(cellpack, incoming_alkanes, checkpoint, start_fuel);
      },
      __delegatecall(cellpack, incoming_alkanes, checkpoint, start_fuel) {
        // assembly/alkanes/runtime/__delegatecall(i32, i32, i32, u64) => i32
        start_fuel = BigInt.asUintN(64, start_fuel);
        return __delegatecall(cellpack, incoming_alkanes, checkpoint, start_fuel);
      },
    }),
  };
  const { exports } = await WebAssembly.instantiate(module, adaptedImports);
  const memory = exports.memory || imports.env.memory;
  const adaptedExports = Object.setPrototypeOf({
    memcpy(dest, src, len) {
      // assembly/utils/memcpy/memcpy(usize, usize, usize) => usize
      return exports.memcpy(dest, src, len) >>> 0;
    },
    toPointer(v) {
      // assembly/utils/pointer/toPointer(usize) => assembly/utils/pointer/Pointer
      return __liftInternref(exports.toPointer(v) >>> 0);
    },
    __call(cellpack, incoming_alkanes, checkpoint, start_fuel) {
      // assembly/alkanes/runtime/__call(i32, i32, i32, u64) => i32
      start_fuel = start_fuel || 0n;
      return exports.__call(cellpack, incoming_alkanes, checkpoint, start_fuel);
    },
    __staticcall(cellpack, incoming_alkanes, checkpoint, start_fuel) {
      // assembly/alkanes/runtime/__staticcall(i32, i32, i32, u64) => i32
      start_fuel = start_fuel || 0n;
      return exports.__staticcall(cellpack, incoming_alkanes, checkpoint, start_fuel);
    },
    __delegatecall(cellpack, incoming_alkanes, checkpoint, start_fuel) {
      // assembly/alkanes/runtime/__delegatecall(i32, i32, i32, u64) => i32
      start_fuel = start_fuel || 0n;
      return exports.__delegatecall(cellpack, incoming_alkanes, checkpoint, start_fuel);
    },
    ExtcallType: (values => (
      // assembly/alkanes/responder/ExtcallType
      values[values.CALL = exports["ExtcallType.CALL"].valueOf()] = "CALL",
      values[values.STATICCALL = exports["ExtcallType.STATICCALL"].valueOf()] = "STATICCALL",
      values[values.DELEGATECALL = exports["ExtcallType.DELEGATECALL"].valueOf()] = "DELEGATECALL",
      values
    ))({}),
  }, exports);
  class Internref extends Number {}
  const registry = new FinalizationRegistry(__release);
  function __liftInternref(pointer) {
    if (!pointer) return null;
    const sentinel = new Internref(__retain(pointer));
    registry.register(sentinel, pointer);
    return sentinel;
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
  return adaptedExports;
}
export const {
  memory,
  __new,
  __pin,
  __unpin,
  __collect,
  __rtti_base,
  memcpy,
  toPointer,
  abort,
  __load_storage,
  __request_storage,
  __log,
  __balance,
  __request_context,
  __load_context,
  __sequence,
  __fuel,
  __height,
  __returndatacopy,
  __request_transaction,
  __load_transaction,
  __request_block,
  __load_block,
  __call,
  __staticcall,
  __delegatecall,
  ExtcallType,
} = await (async url => instantiate(
  await (async () => {
    const isNodeOrBun = typeof process != "undefined" && process.versions != null && (process.versions.node != null || process.versions.bun != null);
    if (isNodeOrBun) { return globalThis.WebAssembly.compile(await (await import("node:fs/promises")).readFile(url)); }
    else { return await globalThis.WebAssembly.compileStreaming(globalThis.fetch(url)); }
  })(), {
  }
))(new URL("release.wasm", import.meta.url));
