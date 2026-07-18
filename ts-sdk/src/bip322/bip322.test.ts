import { describe, it, expect } from 'vitest';
import * as bitcoin from 'bitcoinjs-lib';
import * as ecc from '@bitcoinerlab/secp256k1';
import { ECPairFactory } from 'ecpair';
import {
  bip322MessageHash,
  signMessageSimple,
  verifyMessageSimple,
} from './index';

bitcoin.initEccLib(ecc);
const ECPair = ECPairFactory(ecc);
const mainnet = bitcoin.networks.bitcoin;

describe('BIP-322 message hash', () => {
  // Message hashes from the BIP-322 test vectors (tx_hashes section).
  it('matches the spec message hash for the empty string', () => {
    expect(bip322MessageHash('').toString('hex')).toBe(
      'c90c269c4f8fcbe6880f72a721ddfbf1914268a794cbb21cfafee13770ae19f1',
    );
  });
  it('matches the spec message hash for "Hello World"', () => {
    expect(bip322MessageHash('Hello World').toString('hex')).toBe(
      'f0eb03b1a75ac6d9847f55c624a99169b5dccba2a31f5b23bea77ba270de0a7a',
    );
  });
});

describe('BIP-322 P2TR (official test vector)', () => {
  // BIP-322 basic-test-vectors.json → simple[3] (P2TR, classic no-prefix format).
  const wif = 'KyrSGCFPhqZMjCe5fNTYddiLMp4tMj4gLKuJ26TsB2rvr1VJGPbt';
  const address = 'bc1pss0zhytly75awhm6x2hhvd5lnzv3vssgrf9axfheq8ldyzn88ges79fler';
  const message = 'No prefix fallback';
  const expectedSig =
    'AUCJYOwOjxYAvatTAGYaVlNXBVyFuc4MwNQkOuK2tl8xhfKDONd0NjfYyNSYcRqeCp8hsAnCEPHAVEkO9h6vbQ/R';

  const keyPair = ECPair.fromWIF(wif, mainnet);
  const privHex = Buffer.from(keyPair.privateKey!).toString('hex');

  it('verifies the exact spec signature', () => {
    expect(verifyMessageSimple({ message, address, signature: expectedSig, network: mainnet })).toBe(true);
  });

  it('produces a signature that verifies (Schnorr aux is random, so bytes vary)', () => {
    const sig = signMessageSimple({ message, address, privateKey: privHex, network: mainnet });
    expect(verifyMessageSimple({ message, address, signature: sig, network: mainnet })).toBe(true);
  });

  it('rejects a tampered message', () => {
    expect(verifyMessageSimple({ message: 'wrong', address, signature: expectedSig, network: mainnet })).toBe(false);
  });
});

describe('BIP-322 P2WPKH round-trip', () => {
  // The current spec vectors use the newer "smp"-prefixed encoding for
  // P2WPKH; the ecosystem (UniSat/Xverse/etc.) uses the classic format we
  // emit, so we assert an internal sign→verify round-trip here.
  const wif = 'L3VFeEujGtevx9w18HD1fhRbCH67Az2dpCymeRE1SoPK6XQtaN2k';
  const keyPair = ECPair.fromWIF(wif, mainnet);
  const address = bitcoin.payments.p2wpkh({ pubkey: Buffer.from(keyPair.publicKey), network: mainnet }).address!;
  const privHex = Buffer.from(keyPair.privateKey!).toString('hex');

  it('signs and verifies "Hello World"', () => {
    const sig = signMessageSimple({ message: 'Hello World', address, privateKey: privHex, network: mainnet });
    expect(verifyMessageSimple({ message: 'Hello World', address, signature: sig, network: mainnet })).toBe(true);
  });

  it('rejects a different address', () => {
    const other = ECPair.makeRandom({ network: mainnet });
    const otherAddr = bitcoin.payments.p2wpkh({ pubkey: Buffer.from(other.publicKey), network: mainnet }).address!;
    const sig = signMessageSimple({ message: 'auth', address, privateKey: privHex, network: mainnet });
    expect(verifyMessageSimple({ message: 'auth', address: otherAddr, signature: sig, network: mainnet })).toBe(false);
  });
});

describe('BIP-322 unsupported types', () => {
  it('throws for a P2PKH (legacy) address', () => {
    const keyPair = ECPair.makeRandom({ network: mainnet });
    const address = bitcoin.payments.p2pkh({ pubkey: Buffer.from(keyPair.publicKey), network: mainnet }).address!;
    const privHex = Buffer.from(keyPair.privateKey!).toString('hex');
    expect(() => signMessageSimple({ message: 'x', address, privateKey: privHex, network: mainnet })).toThrow(/unsupported/i);
  });
});
