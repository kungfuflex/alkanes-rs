/** Exported memory */
export declare const memory: WebAssembly.Memory;
// Exported runtime interface
export declare function __new(size: number, id: number): number;
export declare function __pin(ptr: number): number;
export declare function __unpin(ptr: number): void;
export declare function __collect(): void;
export declare const __rtti_base: number;
/**
 * test/fixtures/test-storage-map/testEmpty
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testEmpty(): ArrayBuffer;
/**
 * test/fixtures/test-storage-map/testSingle
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testSingle(): ArrayBuffer;
/**
 * test/fixtures/test-storage-map/testMultiple
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testMultiple(): ArrayBuffer;
/**
 * test/fixtures/test-storage-map/testRoundTrip
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testRoundTrip(): ArrayBuffer;
