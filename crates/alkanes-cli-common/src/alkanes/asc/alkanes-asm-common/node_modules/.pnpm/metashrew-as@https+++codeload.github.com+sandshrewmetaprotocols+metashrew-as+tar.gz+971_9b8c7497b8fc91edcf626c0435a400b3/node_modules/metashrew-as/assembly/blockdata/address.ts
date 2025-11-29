import { console } from "../utils/logging";
import { bech32m, bech32, toWords } from "../utils/b32";
import { base58 } from "../utils/b58";
import { concat } from "../utils/utils";
import { sha256 } from "../utils/sha256";
import { Script } from "../utils/yabsp";
import { Box } from "../utils/box";
import { arrayBufferToArray } from "../indexer";
import { network } from "./network";

export function arrayToArrayBuffer(v: Array<u8>): ArrayBuffer {
  return new Box(v.dataStart, v.length).toArrayBuffer();
}

export class Address {
  /**
   * Returns the address from a given script
   * @param {Script} script - The script to extract the address from
   * @returns {ArrayBuffer | null} - The address or null if the script is not a valid address
   */
  static from(script: Script): ArrayBuffer | null {
    const _network: network.Network = network.getNetwork();
    if (script.is_p2pkh()) {
      let bytes: Box = script.data.sliceFrom(3).setLength(20);
      let prefix = new Uint8Array(1);
      prefix[0] = _network.p2pkhPrefix;
      return Address.toBase58Check(prefix.buffer, bytes.toArrayBuffer());
    } else if (script.is_p2sh()) {
      let bytes: Box = script.data.sliceFrom(2).setLength(20);
      let prefix = new Uint8Array(1);
      prefix[0] = _network.p2shPrefix;
      return Address.toBase58Check(prefix.buffer, bytes.toArrayBuffer());
    } else if (script.is_witness_program()) {
      let bytes = script.data.sliceFrom(2).toArrayBuffer();
      let version = load<u8>(script.witness_version) === 0x00 ? 0x00 : 0x01;
      return Address.toBech32(_network.bech32Prefix, bytes, <u8>version);
    } else {
      return null;
    }
  }

  /**
   * Returns the bech32 encoding of the given data
   * @param {ArrayBuffer} prefix - The prefix to use for the encoding
   * @param {ArrayBuffer} data - The data to encode
   * @param {u8} version - The version to use for the encoding
   * @returns {ArrayBuffer} - The bech32 encoding of the data
   */
  static toBech32(
    prefix: ArrayBuffer,
    data: ArrayBuffer,
    version: u8,
  ): ArrayBuffer {
    let words = arrayBufferToArray(toWords(data));
    words.unshift(version);

    if (version === 0) {
      return bech32(prefix, arrayToArrayBuffer(words));
    } else {
      return bech32m(prefix, arrayToArrayBuffer(words));
    }
  }

  /**
   * Returns the base58check encoding of the given data
   * @param {ArrayBuffer} prefix - The prefix to use for the encoding
   * @param {ArrayBuffer} data - The data to encode
   * @returns {ArrayBuffer} - The base58check encoding of the data
   */
  static toBase58Check(prefix: ArrayBuffer, data: ArrayBuffer): ArrayBuffer {
    let checksum: ArrayBuffer = sha256(sha256(concat([prefix, data]))).slice(
      0,
      4,
    );
    let hash: Uint8Array = Uint8Array.wrap(concat([prefix, data, checksum]));
    let encoded = base58(hash);
    return encoded;
  }
}
