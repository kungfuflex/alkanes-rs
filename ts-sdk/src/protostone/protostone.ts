// ProtoStone - Encode protostone payloads for burn, message, and edict operations
// Protostones are protocol extensions embedded within Runestones

import { u128, u32 } from './integer';
import { Tag } from './tag';
import { Some, Option } from './monads';
import { ProtoruneRuneId } from './protoruneruneid';
import { ProtoruneEdict } from './protoruneedict';
import { unpack } from './bytes';

export type ProtoBurn = {
  pointer: Option<u32>;
  from?: Array<u32>;
};

export type ProtoMessage = {
  calldata: Buffer;
  pointer: Option<u32>;
  refundPointer: Option<u32>;
};

export class ProtoStone {
  burn?: ProtoBurn;
  message?: ProtoMessage;
  protocolTag: u128;
  edicts?: ProtoruneEdict[];

  constructor({
    burn,
    message,
    protocolTag,
    edicts,
  }: {
    protocolTag: bigint;
    burn?: {
      pointer: number;
      from?: Array<u32>;
    };
    message?: {
      calldata: Buffer;
      pointer: number;
      refundPointer: number;
    };
    edicts?: ProtoruneEdict[];
  }) {
    this.protocolTag = u128(protocolTag);
    this.edicts = edicts;
    if (burn) {
      this.burn = {
        pointer: Some<u32>(u32(burn.pointer)),
        from: burn.from,
      };
    }
    if (message) {
      this.message = {
        calldata: message.calldata,
        pointer: Some<u32>(u32(message.pointer)),
        refundPointer: Some<u32>(u32(message.refundPointer)),
      };
    }
  }

  encipher_payloads(): bigint[] {
    let payloads: bigint[] = [];
    if (this.burn) {
      payloads.push(u128(Tag.POINTER));
      payloads.push(this.burn.pointer.map(u128).unwrap());
      if (this.burn.from) {
        payloads.push(u128(Tag.FROM));
        payloads.push(this.burn.from.map(u128)[0]);
      }
    } else if (this.message) {
      if (this.message.pointer.isSome()) {
        payloads.push(u128(Tag.POINTER));
        payloads.push(u128(this.message.pointer.map(u128).unwrap()));
      }
      if (this.message.refundPointer.isSome()) {
        payloads.push(u128(Tag.REFUND));
        payloads.push(u128(this.message.refundPointer.map(u128).unwrap()));
      }
      if (this.message.calldata.length) {
        unpack(this.message.calldata).forEach((v) => {
          payloads.push(u128(Tag.MESSAGE));
          payloads.push(u128(v));
        });
      }
    }
    if (this.edicts && this.edicts.length) {
      payloads.push(u128(Tag.BODY));

      const edicts = [...this.edicts].sort((x, y) =>
        Number(x.id.block - y.id.block || x.id.tx - y.id.tx),
      );

      let previous = new ProtoruneRuneId(u128(0), u128(0));
      for (const edict of edicts) {
        const [block, tx] = previous.delta(edict.id).unwrap();
        payloads.push(block);
        payloads.push(tx);
        payloads.push(edict.amount);
        payloads.push(u128(edict.output));
        previous = edict.id;
      }
    }

    // protocol_id and length first, per spec
    const length_payload = payloads.length;
    payloads.unshift(u128(length_payload));
    payloads.unshift(u128(this.protocolTag));
    return payloads;
  }

  static burn({
    protocolTag,
    edicts,
    ...burn
  }: {
    protocolTag: bigint;
    pointer: number;
    from?: Array<u32>;
    edicts?: ProtoruneEdict[];
  }): ProtoStone {
    return new ProtoStone({ burn, protocolTag, edicts });
  }

  static message({
    protocolTag,
    edicts,
    ...message
  }: {
    calldata: Buffer;
    protocolTag: bigint;
    pointer: number;
    refundPointer: number;
    edicts?: ProtoruneEdict[];
  }): ProtoStone {
    return new ProtoStone({ message, protocolTag, edicts });
  }

  static edicts({
    protocolTag,
    edicts,
  }: {
    edicts?: ProtoruneEdict[];
    protocolTag: bigint;
  }): ProtoStone {
    return new ProtoStone({ edicts, protocolTag });
  }
}
