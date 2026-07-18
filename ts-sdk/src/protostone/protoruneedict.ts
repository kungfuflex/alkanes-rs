// ProtoruneEdict - Instructions for routing protorunes between outputs

import { None, Option, Some } from './monads';
import { u32, u128 } from './integer';
import { ProtoruneRuneId } from './protoruneruneid';

export type ProtoruneEdict = {
  id: ProtoruneRuneId;
  amount: u128;
  output: u32;
};

export namespace ProtoruneEdict {
  export function fromIntegers(
    numOutputs: number,
    id: ProtoruneRuneId,
    amount: u128,
    output: u128,
  ): Option<ProtoruneEdict> {
    if (id.block === 0n && id.tx > 0n) {
      return None;
    }

    const optionOutputU32 = u128.tryIntoU32(output);
    if (optionOutputU32.isNone()) {
      return None;
    }
    const outputU32 = optionOutputU32.unwrap();

    if (outputU32 > numOutputs) {
      return None;
    }

    return Some({ id, amount, output: outputU32 });
  }
}
