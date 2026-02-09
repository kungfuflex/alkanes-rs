// Protostone protocol tags
// These are used to encode structured data within Protostone payloads

import { u128, FixedArray } from './integer';
import { Option, None, Some } from './monads';

export enum Tag {
  BODY = 0,
  PROTOCOL = 2 ** 14 - 1,

  // Protostone-specific tags
  MESSAGE = 81,
  BURN = 83,
  SPLIT = 85,
  POINTER = 91,
  REFUND = 93,
  FROM = 95,
  CENOTAPH = 126,
  NOP = 127,
}

export namespace Tag {
  export function take<N extends number, T extends {}>(
    tag: Tag,
    fields: Map<u128, u128[]>,
    n: N,
    withFn: (values: FixedArray<u128, N>) => Option<T>,
  ): Option<T> {
    const field = fields.get(u128(tag));
    if (field === undefined) {
      return None;
    }

    const values: u128[] = [];
    for (const i of [...Array(n).keys()]) {
      if (field[i] === undefined) {
        return None;
      }
      values[i] = field[i];
    }

    const optionValue = withFn(values as FixedArray<u128, N>);
    if (optionValue.isNone()) {
      return None;
    }

    field.splice(0, n);

    if (field.length === 0) {
      fields.delete(u128(tag));
    }

    return Some(optionValue.unwrap());
  }

  export function encode(tag: Tag, values: u128[]): Buffer {
    return Buffer.concat(
      values
        .map((value) => [
          u128.encodeVarInt(u128(tag)),
          u128.encodeVarInt(value),
        ])
        .flat(),
    );
  }

  export function encodeOptionInt(tag: Tag, value: Option<number | bigint>) {
    return value
      .map((value) => Tag.encode(tag, [u128(value)]))
      .unwrapOr(Buffer.alloc(0));
  }
}
