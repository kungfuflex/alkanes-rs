// Alkanes AssemblyScript Common Library
// Main exports

// u128 type (our own implementation for stub runtime compatibility)
export { u128, loadU128, storeU128, u128ToArrayBuffer } from "./u128";

// Utils
export * from "./utils/memcpy";
export * from "./utils/pointer";
export * from "./utils/box";

// Core serialization types
export * from "./parcel";
export * from "./storage-map";

// Alkanes runtime
export * from "./alkanes/runtime";
export * from "./alkanes/types";
export * from "./alkanes/responder";
