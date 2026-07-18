/** Exported memory */
export declare const memory: WebAssembly.Memory;
// Exported runtime interface
export declare function __new(size: number, id: number): number;
export declare function __pin(ptr: number): number;
export declare function __unpin(ptr: number): void;
export declare function __collect(): void;
export declare const __rtti_base: number;
/**
 * test/fixtures/test-parcel/testEmpty
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testEmpty(): ArrayBuffer;
/**
 * test/fixtures/test-parcel/testSingle
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testSingle(): ArrayBuffer;
/**
 * test/fixtures/test-parcel/testMultiple
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testMultiple(): ArrayBuffer;
/**
 * test/fixtures/test-parcel/testRoundTrip
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testRoundTrip(): ArrayBuffer;
/**
 * test/fixtures/test-parcel/testAlkaneId
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testAlkaneId(): ArrayBuffer;
/**
 * test/fixtures/test-parcel/testAlkaneTransfer
 * @returns `~lib/arraybuffer/ArrayBuffer`
 */
export declare function testAlkaneTransfer(): ArrayBuffer;
