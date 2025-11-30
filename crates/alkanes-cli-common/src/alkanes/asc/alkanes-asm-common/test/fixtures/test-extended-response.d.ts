/** Exported memory */
export declare const memory: WebAssembly.Memory;
// Exported runtime interface
export declare function __new(size: number, id: number): number;
export declare function __pin(ptr: number): number;
export declare function __unpin(ptr: number): void;
export declare function __collect(): void;
export declare const __rtti_base: number;
/**
 * test/fixtures/test-extended-response/testEmpty
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testEmpty(): ArrayBuffer;
/**
 * test/fixtures/test-extended-response/testDataOnly
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testDataOnly(): ArrayBuffer;
/**
 * test/fixtures/test-extended-response/testWithAlkane
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testWithAlkane(): ArrayBuffer;
/**
 * test/fixtures/test-extended-response/testWithStorage
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testWithStorage(): ArrayBuffer;
/**
 * test/fixtures/test-extended-response/testComplete
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testComplete(): ArrayBuffer;
/**
 * test/fixtures/test-extended-response/testMultiple
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testMultiple(): ArrayBuffer;
