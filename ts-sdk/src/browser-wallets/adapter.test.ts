/**
 * Tests for browser wallet adapters
 *
 * These tests verify that the wallet adapters correctly implement
 * the JsWalletAdapter interface and can work with the WASM provider.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  MockWalletAdapter,
  BaseWalletAdapter,
  createWalletAdapter,
  JsWalletAdapter,
  WalletInfoForWasm,
} from './adapter';
import { ConnectedWallet, BrowserWalletInfo, WalletAccount } from './index';

// Mock ConnectedWallet for testing
function createMockConnectedWallet(walletId: string = 'unisat'): ConnectedWallet {
  const mockInfo: BrowserWalletInfo = {
    id: walletId,
    name: 'Test Wallet',
    icon: '/test.svg',
    website: 'https://test.com',
    injectionKey: 'testWallet',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: false,
  };

  const mockAccount: WalletAccount = {
    address: 'bc1qtest1234567890abcdef',
    publicKey: '03' + '0'.repeat(64),
    addressType: 'p2wpkh',
  };

  const mockProvider = {
    signMessage: vi.fn().mockResolvedValue('mock_signature'),
    signPsbt: vi.fn().mockResolvedValue('70736274ff...'),
    getNetwork: vi.fn().mockResolvedValue('mainnet'),
    disconnect: vi.fn().mockResolvedValue(undefined),
  };

  return new ConnectedWallet(mockInfo, mockProvider, mockAccount);
}

describe('MockWalletAdapter', () => {
  let adapter: MockWalletAdapter;

  beforeEach(() => {
    adapter = new MockWalletAdapter({
      address: 'bc1qmocktest123',
      publicKey: '03abcdef',
      network: 'mainnet',
    });
  });

  it('should return correct wallet info', () => {
    const info = adapter.getInfo();
    expect(info.id).toBe('mock');
    expect(info.name).toBe('Mock Wallet');
    expect(info.supports_psbt).toBe(true);
    expect(info.supports_taproot).toBe(true);
    expect(info.supports_ordinals).toBe(true);
  });

  it('should connect and return account info', async () => {
    const account = await adapter.connect();
    expect(account.address).toBe('bc1qmocktest123');
    expect(account.public_key).toBe('03abcdef');
    expect(account.address_type).toBe('p2wpkh');
  });

  it('should disconnect without error', async () => {
    await expect(adapter.disconnect()).resolves.toBeUndefined();
  });

  it('should return accounts', async () => {
    const accounts = await adapter.getAccounts();
    expect(accounts.length).toBe(1);
    expect(accounts[0].address).toBe('bc1qmocktest123');
  });

  it('should return network', async () => {
    const network = await adapter.getNetwork();
    expect(network).toBe('mainnet');
  });

  it('should switch network', async () => {
    await adapter.switchNetwork('testnet');
    const network = await adapter.getNetwork();
    expect(network).toBe('testnet');
  });

  it('should sign message', async () => {
    const signature = await adapter.signMessage('Hello', 'bc1qtest');
    // Signature is base64 encoded, decode to verify content
    const decoded = Buffer.from(signature, 'base64').toString();
    expect(decoded).toContain('mock_sig_');
  });

  it('should sign PSBT', async () => {
    const psbtHex = '70736274ff01003f0200000001...';
    const signedPsbt = await adapter.signPsbt(psbtHex);
    expect(signedPsbt).toBe(psbtHex);
    expect(adapter.getSignedPsbts()).toContain(psbtHex);
  });

  it('should sign multiple PSBTs', async () => {
    const psbtHexs = ['70736274ff01...', '70736274ff02...'];
    const signedPsbts = await adapter.signPsbts(psbtHexs);
    expect(signedPsbts.length).toBe(2);
    expect(adapter.getSignedPsbts().length).toBe(2);
  });

  it('should push transaction', async () => {
    const txid = await adapter.pushTx('0200000001...');
    expect(txid).toBe('0'.repeat(64));
  });

  it('should get public key', async () => {
    const publicKey = await adapter.getPublicKey();
    expect(publicKey).toBe('03abcdef');
  });

  it('should get balance', async () => {
    const balance = await adapter.getBalance();
    expect(balance).toBe(100000000);
  });

  it('should get inscriptions', async () => {
    const inscriptions = await adapter.getInscriptions(0, 20);
    expect(inscriptions.list).toEqual([]);
    expect(inscriptions.total).toBe(0);
  });

  it('should clear signed PSBTs', () => {
    adapter.signPsbt('test');
    expect(adapter.getSignedPsbts().length).toBe(1);
    adapter.clearSignedPsbts();
    expect(adapter.getSignedPsbts().length).toBe(0);
  });
});

describe('BaseWalletAdapter', () => {
  let adapter: BaseWalletAdapter;
  let mockWallet: ConnectedWallet;

  beforeEach(() => {
    mockWallet = createMockConnectedWallet('unisat');
    adapter = new BaseWalletAdapter(mockWallet);
  });

  it('should return correct wallet info', () => {
    const info = adapter.getInfo();
    expect(info.id).toBe('unisat');
    expect(info.name).toBe('Test Wallet');
    expect(info.injection_key).toBe('testWallet');
    expect(info.supports_psbt).toBe(true);
  });

  it('should connect and return account', async () => {
    const account = await adapter.connect();
    expect(account.address).toBe('bc1qtest1234567890abcdef');
    expect(account.address_type).toBe('p2wpkh');
  });

  it('should return accounts', async () => {
    const accounts = await adapter.getAccounts();
    expect(accounts.length).toBe(1);
    expect(accounts[0].address).toBe('bc1qtest1234567890abcdef');
  });

  it('should get network', async () => {
    const network = await adapter.getNetwork();
    expect(network).toBe('mainnet');
  });

  it('should get public key', async () => {
    const publicKey = await adapter.getPublicKey();
    expect(publicKey).toBe('03' + '0'.repeat(64));
  });

  it('should return null for balance', async () => {
    const balance = await adapter.getBalance();
    expect(balance).toBeNull();
  });

  it('should return empty inscriptions', async () => {
    const inscriptions = await adapter.getInscriptions();
    expect(inscriptions.list).toEqual([]);
    expect(inscriptions.total).toBe(0);
  });
});

describe('createWalletAdapter', () => {
  it('should create UnisatAdapter for unisat wallet', () => {
    const wallet = createMockConnectedWallet('unisat');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('unisat');
  });

  it('should create XverseAdapter for xverse wallet', () => {
    const wallet = createMockConnectedWallet('xverse');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('xverse');
  });

  it('should create OkxAdapter for okx wallet', () => {
    const wallet = createMockConnectedWallet('okx');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('okx');
  });

  it('should create LeatherAdapter for leather wallet', () => {
    const wallet = createMockConnectedWallet('leather');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('leather');
  });

  it('should create PhantomAdapter for phantom wallet', () => {
    const wallet = createMockConnectedWallet('phantom');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('phantom');
  });

  it('should create MagicEdenAdapter for magic-eden wallet', () => {
    const wallet = createMockConnectedWallet('magic-eden');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('magic-eden');
  });

  it('should create WizzAdapter for wizz wallet', () => {
    const wallet = createMockConnectedWallet('wizz');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('wizz');
  });

  it('should create BaseWalletAdapter for unknown wallet', () => {
    const wallet = createMockConnectedWallet('unknown-wallet');
    const adapter = createWalletAdapter(wallet);
    expect(adapter.getInfo().id).toBe('unknown-wallet');
  });
});

describe('JsWalletAdapter interface compliance', () => {
  it('should implement all required methods', () => {
    const adapter = new MockWalletAdapter();

    // Check that all required methods exist
    expect(typeof adapter.getInfo).toBe('function');
    expect(typeof adapter.connect).toBe('function');
    expect(typeof adapter.disconnect).toBe('function');
    expect(typeof adapter.getAccounts).toBe('function');
    expect(typeof adapter.getNetwork).toBe('function');
    expect(typeof adapter.switchNetwork).toBe('function');
    expect(typeof adapter.signMessage).toBe('function');
    expect(typeof adapter.signPsbt).toBe('function');
    expect(typeof adapter.signPsbts).toBe('function');
    expect(typeof adapter.pushTx).toBe('function');
    expect(typeof adapter.pushPsbt).toBe('function');
    expect(typeof adapter.getPublicKey).toBe('function');
    expect(typeof adapter.getBalance).toBe('function');
    expect(typeof adapter.getInscriptions).toBe('function');
  });

  it('should return correct types for all methods', async () => {
    const adapter = new MockWalletAdapter();

    // Verify return types
    const info = adapter.getInfo();
    expect(info).toHaveProperty('id');
    expect(info).toHaveProperty('name');
    expect(info).toHaveProperty('supports_psbt');

    const account = await adapter.connect();
    expect(account).toHaveProperty('address');
    expect(account).toHaveProperty('address_type');

    const accounts = await adapter.getAccounts();
    expect(Array.isArray(accounts)).toBe(true);

    const network = await adapter.getNetwork();
    expect(typeof network).toBe('string');

    const signature = await adapter.signMessage('test', 'bc1q');
    expect(typeof signature).toBe('string');

    const signedPsbt = await adapter.signPsbt('70736274ff');
    expect(typeof signedPsbt).toBe('string');

    const signedPsbts = await adapter.signPsbts(['psbt1', 'psbt2']);
    expect(Array.isArray(signedPsbts)).toBe(true);

    const txid = await adapter.pushTx('0200000001');
    expect(typeof txid).toBe('string');
    expect(txid.length).toBe(64);

    const publicKey = await adapter.getPublicKey();
    expect(typeof publicKey).toBe('string');

    const balance = await adapter.getBalance();
    expect(typeof balance === 'number' || balance === null).toBe(true);

    const inscriptions = await adapter.getInscriptions();
    expect(inscriptions).toHaveProperty('list');
    expect(inscriptions).toHaveProperty('total');
  });
});

describe('WalletInfoForWasm structure', () => {
  it('should have correct property names for WASM compatibility', () => {
    const adapter = new MockWalletAdapter();
    const info = adapter.getInfo();

    // These property names must match the Rust WalletInfo struct
    expect(info).toHaveProperty('id');
    expect(info).toHaveProperty('name');
    expect(info).toHaveProperty('icon');
    expect(info).toHaveProperty('website');
    expect(info).toHaveProperty('injection_key'); // snake_case for Rust
    expect(info).toHaveProperty('supports_psbt');
    expect(info).toHaveProperty('supports_taproot');
    expect(info).toHaveProperty('supports_ordinals');
    expect(info).toHaveProperty('mobile_support');
  });

  it('should use snake_case for Rust compatibility', () => {
    const adapter = new MockWalletAdapter();
    const info = adapter.getInfo();

    // Verify snake_case naming
    expect('injection_key' in info).toBe(true);
    expect('supports_psbt' in info).toBe(true);
    expect('supports_taproot' in info).toBe(true);
    expect('supports_ordinals' in info).toBe(true);
    expect('mobile_support' in info).toBe(true);

    // Verify camelCase is NOT used
    expect('injectionKey' in info).toBe(false);
    expect('supportsPsbt' in info).toBe(false);
  });
});
