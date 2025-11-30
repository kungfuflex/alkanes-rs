async function instantiate(module, imports = {}) {
  const { exports } = await WebAssembly.instantiate(module, imports);
  const memory = exports.memory || imports.env.memory;
  const adaptedExports = Object.setPrototypeOf({
    testEmpty() {
      // test/fixtures/test-extended-response/testEmpty() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testEmpty() >>> 0);
    },
    testDataOnly() {
      // test/fixtures/test-extended-response/testDataOnly() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testDataOnly() >>> 0);
    },
    testWithAlkane() {
      // test/fixtures/test-extended-response/testWithAlkane() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testWithAlkane() >>> 0);
    },
    testWithStorage() {
      // test/fixtures/test-extended-response/testWithStorage() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testWithStorage() >>> 0);
    },
    testComplete() {
      // test/fixtures/test-extended-response/testComplete() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testComplete() >>> 0);
    },
    testMultiple() {
      // test/fixtures/test-extended-response/testMultiple() => ~lib/arraybuffer/ArrayBuffer
      return __liftBuffer(exports.testMultiple() >>> 0);
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
  testDataOnly,
  testWithAlkane,
  testWithStorage,
  testComplete,
  testMultiple,
} = await (async url => instantiate(
  await (async () => {
    const isNodeOrBun = typeof process != "undefined" && process.versions != null && (process.versions.node != null || process.versions.bun != null);
    if (isNodeOrBun) { return globalThis.WebAssembly.compile(await (await import("node:fs/promises")).readFile(url)); }
    else { return await globalThis.WebAssembly.compileStreaming(globalThis.fetch(url)); }
  })(), {
  }
))(new URL("test-extended-response.wasm", import.meta.url));
