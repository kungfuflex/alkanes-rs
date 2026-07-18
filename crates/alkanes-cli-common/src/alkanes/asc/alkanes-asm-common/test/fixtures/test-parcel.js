async function instantiate(module, imports = {}) {
  const { exports } = await WebAssembly.instantiate(module, imports);
  const memory = exports.memory || imports.env.memory;
  const adaptedExports = Object.setPrototypeOf({
    testEmpty() {
      // test/fixtures/test-parcel/testEmpty() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testEmpty() >>> 0);
    },
    testSingle() {
      // test/fixtures/test-parcel/testSingle() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testSingle() >>> 0);
    },
    testMultiple() {
      // test/fixtures/test-parcel/testMultiple() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testMultiple() >>> 0);
    },
    testRoundTrip() {
      // test/fixtures/test-parcel/testRoundTrip() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testRoundTrip() >>> 0);
    },
    testAlkaneId() {
      // test/fixtures/test-parcel/testAlkaneId() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testAlkaneId() >>> 0);
    },
    testAlkaneTransfer() {
      // test/fixtures/test-parcel/testAlkaneTransfer() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testAlkaneTransfer() >>> 0);
    },
  }, exports);
  function __liftBuffer(pointer) {
    if (!pointer) return null;
    return memory.buffer.slice(pointer, pointer + new Uint32Array(memory.buffer)[pointer - 4 >>> 2]);
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
  testEmpty,
  testSingle,
  testMultiple,
  testRoundTrip,
  testAlkaneId,
  testAlkaneTransfer,
} = await (async url => instantiate(
  await (async () => {
    const isNodeOrBun = typeof process != "undefined" && process.versions != null && (process.versions.node != null || process.versions.bun != null);
    if (isNodeOrBun) { return globalThis.WebAssembly.compile(await (await import("node:fs/promises")).readFile(url)); }
    else { return await globalThis.WebAssembly.compileStreaming(globalThis.fetch(url)); }
  })(), {
  }
))(new URL("test-parcel.wasm", import.meta.url));
