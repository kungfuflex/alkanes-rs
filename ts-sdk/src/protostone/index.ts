// Protostone encoding module
// Pure TypeScript encoding for Protostones, Runestones, and Alkane calldata

// Monads (Rust-style Option type)
export { Option, Some, None, isSome, isNone, OptionType } from './monads';

// Integer types
export { u128, u64, u32, u8 } from './integer';
export type { FixedArray } from './integer';

// Binary encoding utilities
export {
  encodeVarInt,
  decodeVarInt,
  tryDecodeVarInt,
  encipher,
  decipher,
  pack,
  unpack,
  decipherPacked,
  toBuffer,
  fromBuffer,
  leftPad15,
  leftPadByte,
  rightPadByte,
  leftPad16,
} from './bytes';
export type { AlkaneId as AlkaneIdType, AlkaneTransfer } from './bytes';

// SeekBuffer
export { SeekBuffer } from './seekbuffer';

// Protocol tags
export { Tag } from './tag';

// Rune IDs and edicts
export { ProtoruneRuneId } from './protoruneruneid';
export { ProtoruneEdict } from './protoruneedict';

// Protostone encoding
export { ProtoStone } from './protostone';
export type { ProtoBurn, ProtoMessage } from './protostone';

// Runestone encoding with protostone extensions
export {
  RunestoneProtostoneUpgrade,
  encodeRunestoneProtostone,
} from './runestone';
export type {
  RunestoneProtostoneSpec,
  RuneEtchingSpec,
  EtchingTermsSpec,
} from './runestone';

// Alkane types and calldata encoding
export {
  AlkaneId,
  Cellpack,
  CalldataWrapper,
  lebEncodeU128,
  CALLDATA_MAGIC,
} from './alkane';
