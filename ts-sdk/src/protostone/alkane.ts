// Alkane-specific types and encoding
// AlkaneId identifies an alkane contract, Cellpack encodes calldata for contract invocation

import { encodeVarInt } from './bytes';

export const CALLDATA_MAGIC = 1;

export class CalldataWrapper {
  serialize(): Buffer {
    throw new Error('Method not implemented');
  }

  serializeToCalldata(): Buffer {
    const magic = Buffer.alloc(1);
    magic[0] = CALLDATA_MAGIC;
    return Buffer.concat([magic, this.serialize(), magic]);
  }
}

export function lebEncodeU128(inputs: bigint[]): Buffer {
  return Buffer.concat(inputs.map((v) => encodeVarInt(v)));
}

export class AlkaneId {
  public block: bigint;
  public tx: bigint;

  constructor(block: bigint, tx: bigint) {
    this.block = block;
    this.tx = tx;
  }

  /** Serialize into LEB128 encoded buffer. First value is block, second is tx. */
  serialize(): Buffer {
    return lebEncodeU128([this.block, this.tx]);
  }
}

export class Cellpack extends CalldataWrapper {
  public target: AlkaneId;
  public inputs: Array<bigint>;

  constructor(block: bigint, tx: bigint, inputs: Array<bigint>) {
    super();
    this.target = new AlkaneId(block, tx);
    this.inputs = inputs;
  }

  /** Serialize target AlkaneId + inputs as LEB128 encoded buffer */
  serialize(): Buffer {
    return Buffer.concat([
      this.target.serialize(),
      lebEncodeU128(this.inputs),
    ]);
  }
}
