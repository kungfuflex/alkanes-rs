/**
 * BIP-322 "simple" message signing & verification.
 *
 * Implements the interoperable, ecosystem-standard "simple" signature format
 * (base64 of the consensus-encoded `to_sign` witness stack, NO variant
 * prefix) — the same shape UniSat / Xverse / OKX / Leather / Sparrow produce
 * and verify. Covers the two address types the alkanes keystore derives:
 *
 *   - P2WPKH (BIP84, native segwit) — ECDSA, SIGHASH_ALL
 *   - P2TR   (BIP86, taproot key-path) — Schnorr, SIGHASH_DEFAULT
 *
 * P2SH-P2WPKH, P2WSH multisig, and BIP322 FULL format are intentionally out
 * of scope (throw). This is enough for wallet-auth "prove you control this
 * address" flows.
 *
 * Reference: https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki
 * Verified against the BIP-322 P2TR test vector (see bip322.test.ts).
 */

import * as bitcoin from 'bitcoinjs-lib';
import * as ecc from '@bitcoinerlab/secp256k1';
import { ECPairFactory, type ECPairInterface } from 'ecpair';

bitcoin.initEccLib(ecc);
const ECPair = ECPairFactory(ecc);

const BIP322_TAG = 'BIP0322-signed-message';

/** BIP-322 message hash: tagged hash "BIP0322-signed" of the raw message. */
export function bip322MessageHash(message: string | Uint8Array): Buffer {
  const tagHash = bitcoin.crypto.sha256(Buffer.from(BIP322_TAG, 'utf8'));
  const msg = typeof message === 'string' ? Buffer.from(message, 'utf8') : Buffer.from(message);
  return bitcoin.crypto.sha256(Buffer.concat([tagHash, tagHash, msg]));
}

/**
 * Build the BIP-322 `to_spend` virtual transaction for a given
 * scriptPubKey + message. Its txid is the prevout the `to_sign` tx spends.
 */
export function buildToSpend(scriptPubKey: Buffer, message: string | Uint8Array): bitcoin.Transaction {
  const messageHash = bip322MessageHash(message);
  const tx = new bitcoin.Transaction();
  tx.version = 0;
  tx.locktime = 0;
  // scriptSig = OP_0 PUSH32[messageHash]
  const scriptSig = bitcoin.script.compile([bitcoin.opcodes.OP_0, messageHash]);
  tx.addInput(Buffer.alloc(32, 0), 0xffffffff, 0, scriptSig);
  tx.addOutput(scriptPubKey, 0);
  return tx;
}

/**
 * Build the BIP-322 `to_sign` virtual transaction (unsigned). vin[0] spends
 * to_spend:0; vout[0] is a zero-value OP_RETURN.
 */
export function buildToSign(toSpendTxid: string, scriptPubKey: Buffer): bitcoin.Transaction {
  const tx = new bitcoin.Transaction();
  tx.version = 0;
  tx.locktime = 0;
  const hash = Buffer.from(toSpendTxid, 'hex').reverse();
  tx.addInput(hash, 0, 0);
  // Track the prevout so callers can compute the sighash.
  void scriptPubKey;
  tx.addOutput(bitcoin.script.compile([bitcoin.opcodes.OP_RETURN]), 0);
  return tx;
}

// --- Bitcoin CompactSize (varint) — hand-rolled to avoid a new dependency ---

function encodeCompactSize(n: number): Buffer {
  if (n < 0xfd) return Buffer.from([n]);
  if (n <= 0xffff) {
    const b = Buffer.alloc(3);
    b[0] = 0xfd;
    b.writeUInt16LE(n, 1);
    return b;
  }
  if (n <= 0xffffffff) {
    const b = Buffer.alloc(5);
    b[0] = 0xfe;
    b.writeUInt32LE(n, 1);
    return b;
  }
  const b = Buffer.alloc(9);
  b[0] = 0xff;
  b.writeBigUInt64LE(BigInt(n), 1);
  return b;
}

function readCompactSize(buf: Buffer, offset: number): { value: number; size: number } {
  const first = buf[offset];
  if (first < 0xfd) return { value: first, size: 1 };
  if (first === 0xfd) return { value: buf.readUInt16LE(offset + 1), size: 3 };
  if (first === 0xfe) return { value: buf.readUInt32LE(offset + 1), size: 5 };
  return { value: Number(buf.readBigUInt64LE(offset + 1)), size: 9 };
}

/** Consensus-encode a witness stack (vector of byte vectors). */
function encodeWitnessStack(items: Buffer[]): Buffer {
  const parts: Buffer[] = [encodeCompactSize(items.length)];
  for (const item of items) {
    parts.push(encodeCompactSize(item.length));
    parts.push(item);
  }
  return Buffer.concat(parts);
}

/** Decode a consensus-encoded witness stack. */
function decodeWitnessStack(buf: Buffer): Buffer[] {
  let offset = 0;
  const count = readCompactSize(buf, offset);
  offset += count.size;
  const items: Buffer[] = [];
  for (let i = 0; i < count.value; i++) {
    const len = readCompactSize(buf, offset);
    offset += len.size;
    items.push(buf.subarray(offset, offset + len.value));
    offset += len.value;
  }
  return items;
}

/** Address type this module can sign/verify. */
export type Bip322AddressType = 'p2wpkh' | 'p2tr';

function classifyScript(scriptPubKey: Buffer): Bip322AddressType {
  // P2WPKH: OP_0 <20-byte>  → 0x00 0x14 ...
  if (scriptPubKey.length === 22 && scriptPubKey[0] === 0x00 && scriptPubKey[1] === 0x14) {
    return 'p2wpkh';
  }
  // P2TR: OP_1 <32-byte> → 0x51 0x20 ...
  if (scriptPubKey.length === 34 && scriptPubKey[0] === 0x51 && scriptPubKey[1] === 0x20) {
    return 'p2tr';
  }
  throw new Error(
    'BIP-322: unsupported address type (only P2WPKH and P2TR key-path are supported)',
  );
}

function addressToScriptPubKey(address: string, network: bitcoin.Network): Buffer {
  return bitcoin.address.toOutputScript(address, network);
}

export interface SignMessageParams {
  message: string | Uint8Array;
  /** The address whose key signs. Determines the script type. */
  address: string;
  /** 33-byte compressed private key (hex or Buffer). */
  privateKey: Buffer | string;
  network: bitcoin.Network;
}

/**
 * Produce a BIP-322 simple signature (base64) for a P2WPKH or P2TR address.
 * The private key must correspond to `address` — this is asserted.
 */
export function signMessageSimple(params: SignMessageParams): string {
  const { message, address, network } = params;
  const privBuf = typeof params.privateKey === 'string'
    ? Buffer.from(params.privateKey, 'hex')
    : params.privateKey;
  const keyPair = ECPair.fromPrivateKey(privBuf, { network });

  const scriptPubKey = addressToScriptPubKey(address, network);
  const type = classifyScript(scriptPubKey);

  const toSpend = buildToSpend(scriptPubKey, message);
  const toSign = buildToSign(toSpend.getId(), scriptPubKey);

  let witness: Buffer[];
  if (type === 'p2wpkh') {
    // Assert the key matches the address.
    const derived = bitcoin.payments.p2wpkh({ pubkey: keyPair.publicKey, network }).address;
    if (derived !== address) {
      throw new Error('BIP-322: private key does not match the P2WPKH address');
    }
    // P2WPKH BIP143 sighash — scriptCode is the P2PKH script for the pubkey hash.
    const pubkeyHash = bitcoin.crypto.hash160(keyPair.publicKey);
    const scriptCode = bitcoin.script.compile([
      bitcoin.opcodes.OP_DUP,
      bitcoin.opcodes.OP_HASH160,
      pubkeyHash,
      bitcoin.opcodes.OP_EQUALVERIFY,
      bitcoin.opcodes.OP_CHECKSIG,
    ]);
    const sighash = toSign.hashForWitnessV0(
      0,
      scriptCode,
      0,
      bitcoin.Transaction.SIGHASH_ALL,
    );
    const sig = bitcoin.script.signature.encode(
      Buffer.from(keyPair.sign(sighash)),
      bitcoin.Transaction.SIGHASH_ALL,
    );
    witness = [sig, Buffer.from(keyPair.publicKey)];
  } else {
    // P2TR key-path — taproot-tweaked key, SIGHASH_DEFAULT, Schnorr.
    const internalPubkey = Buffer.from(keyPair.publicKey.subarray(1, 33)); // x-only
    const derived = bitcoin.payments.p2tr({ internalPubkey, network }).address;
    if (derived !== address) {
      throw new Error('BIP-322: private key does not match the P2TR address');
    }
    const tweaked = tweakSigner(keyPair, network);
    const sighash = toSign.hashForWitnessV1(
      0,
      [scriptPubKey],
      [0],
      bitcoin.Transaction.SIGHASH_DEFAULT,
    );
    const sig = Buffer.from(tweaked.signSchnorr(sighash));
    witness = [sig]; // SIGHASH_DEFAULT → no appended sighash byte
  }

  return encodeWitnessStack(witness).toString('base64');
}

export interface VerifyMessageParams {
  message: string | Uint8Array;
  address: string;
  /** Base64 BIP-322 simple signature. */
  signature: string;
  network: bitcoin.Network;
}

/**
 * Verify a BIP-322 simple signature for a P2WPKH or P2TR address.
 * Returns false (never throws) on any malformed input or signature mismatch.
 */
export function verifyMessageSimple(params: VerifyMessageParams): boolean {
  try {
    const { message, address, signature, network } = params;
    const scriptPubKey = addressToScriptPubKey(address, network);
    const type = classifyScript(scriptPubKey);

    const witness = decodeWitnessStack(Buffer.from(signature, 'base64'));
    const toSpend = buildToSpend(scriptPubKey, message);
    const toSign = buildToSign(toSpend.getId(), scriptPubKey);

    if (type === 'p2wpkh') {
      if (witness.length !== 2) return false;
      const [sig, pubkey] = witness;
      // The pubkey must hash to the address program.
      const program = scriptPubKey.subarray(2); // 20-byte hash
      if (!bitcoin.crypto.hash160(pubkey).equals(program)) return false;
      const decoded = bitcoin.script.signature.decode(sig);
      const scriptCode = bitcoin.script.compile([
        bitcoin.opcodes.OP_DUP,
        bitcoin.opcodes.OP_HASH160,
        bitcoin.crypto.hash160(pubkey),
        bitcoin.opcodes.OP_EQUALVERIFY,
        bitcoin.opcodes.OP_CHECKSIG,
      ]);
      const sighash = toSign.hashForWitnessV0(
        0,
        scriptCode,
        0,
        decoded.hashType,
      );
      return ecc.verify(sighash, pubkey, decoded.signature);
    }

    // P2TR key-path
    if (witness.length !== 1) return false;
    const sig = witness[0];
    // Signature is 64 bytes (SIGHASH_DEFAULT) or 65 (explicit sighash byte).
    let hashType = bitcoin.Transaction.SIGHASH_DEFAULT;
    let sig64 = sig;
    if (sig.length === 65) {
      hashType = sig[64];
      sig64 = sig.subarray(0, 64);
    } else if (sig.length !== 64) {
      return false;
    }
    const outputKey = scriptPubKey.subarray(2); // 32-byte x-only tweaked key
    const sighash = toSign.hashForWitnessV1(0, [scriptPubKey], [0], hashType);
    return ecc.verifySchnorr(sighash, outputKey, sig64);
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// Taproot key-path tweak (BIP341) — mirrors the SDK's PSBT taproot signer.
// ---------------------------------------------------------------------------

function tweakSigner(keyPair: ECPairInterface, network: bitcoin.Network): ECPairInterface {
  let privateKey = keyPair.privateKey;
  if (!privateKey) throw new Error('BIP-322: private key required for taproot signing');
  // BIP341: negate the private key when the internal pubkey has odd y-parity.
  if (keyPair.publicKey[0] === 0x03) {
    privateKey = Buffer.from(ecc.privateNegate(privateKey));
  }
  const xOnly = keyPair.publicKey.subarray(1, 33);
  const tweak = bitcoin.crypto.taggedHash('TapTweak', xOnly);
  const tweakedPriv = ecc.privateAdd(privateKey, tweak);
  if (!tweakedPriv) throw new Error('BIP-322: invalid tweaked private key');
  return ECPair.fromPrivateKey(Buffer.from(tweakedPriv), { network });
}
