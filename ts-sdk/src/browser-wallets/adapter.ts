/**
 * Browser Wallet Adapters for Alkanes WASM Integration
 *
 * This module provides wallet adapter classes that bridge between browser wallet extensions
 * and the WASM BrowserWalletProvider. Each adapter implements the JsWalletAdapter interface
 * expected by the Rust code.
 *
 * @example
 * ```typescript
 * import { createWalletAdapter, WasmBrowserWalletProvider } from '@alkanes/ts-sdk';
 *
 * // Detect and connect to a wallet
 * const connector = new WalletConnector();
 * const wallets = await connector.detectWallets();
 * const connectedWallet = await connector.connect(wallets[0]);
 *
 * // Create an adapter for the WASM provider
 * const adapter = createWalletAdapter(connectedWallet);
 *
 * // Create the WASM provider with the adapter
 * const wasmProvider = await WasmBrowserWalletProvider.new(adapter, 'mainnet');
 * ```
 */

import { ConnectedWallet, BrowserWalletInfo, WalletAccount, PsbtSigningOptions } from './index';
import * as bitcoin from 'bitcoinjs-lib';

/**
 * Interface that wallet adapters must implement to work with WASM BrowserWalletProvider.
 * This matches the JsWalletAdapter type expected by Rust.
 */
export interface JsWalletAdapter {
  /** Get wallet metadata */
  getInfo(): WalletInfoForWasm;

  /** Connect to the wallet */
  connect(): Promise<WalletAccountForWasm>;

  /** Disconnect from the wallet */
  disconnect(): Promise<void>;

  /** Get all connected accounts */
  getAccounts(): Promise<WalletAccountForWasm[]>;

  /** Get current network */
  getNetwork(): Promise<string>;

  /** Switch to a different network */
  switchNetwork(network: string): Promise<void>;

  /** Sign a message */
  signMessage(message: string, address: string): Promise<string>;

  /** Sign a PSBT (hex encoded) */
  signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string>;

  /** Sign multiple PSBTs */
  signPsbts(psbtHexs: string[], options?: PsbtSigningOptionsForWasm): Promise<string[]>;

  /** Push a raw transaction */
  pushTx(txHex: string): Promise<string>;

  /** Push a signed PSBT */
  pushPsbt(psbtHex: string): Promise<string>;

  /** Get the wallet's public key */
  getPublicKey(): Promise<string>;

  /** Get balance (optional) */
  getBalance(): Promise<number | null>;

  /** Get inscriptions (optional) */
  getInscriptions(cursor?: number, size?: number): Promise<any>;
}

/**
 * Wallet info structure for WASM compatibility
 */
export interface WalletInfoForWasm {
  id: string;
  name: string;
  icon: string;
  website: string;
  injection_key: string;
  supports_psbt: boolean;
  supports_taproot: boolean;
  supports_ordinals: boolean;
  mobile_support: boolean;
  deep_link_scheme?: string;
}

/**
 * Account info structure for WASM compatibility
 */
export interface WalletAccountForWasm {
  address: string;
  public_key?: string;
  compressed_public_key?: string;
  address_type: string;
  /** Payment address for dual-address wallets */
  payment_address?: string;
  /** Payment public key for dual-address wallets */
  payment_public_key?: string;
}

/**
 * PSBT signing options for WASM compatibility
 */
export interface PsbtSigningOptionsForWasm {
  auto_finalized: boolean;
  to_sign_inputs?: Array<{
    index: number;
    address?: string;
    sighash_types?: number[];
    disable_tweaked_public_key?: boolean;
  }>;
}

/**
 * Base wallet adapter that wraps a ConnectedWallet
 */
export class BaseWalletAdapter implements JsWalletAdapter {
  protected wallet: ConnectedWallet;

  constructor(wallet: ConnectedWallet) {
    this.wallet = wallet;
  }

  getInfo(): WalletInfoForWasm {
    const info = this.wallet.info;
    return {
      id: info.id,
      name: info.name,
      icon: info.icon,
      website: info.website,
      injection_key: info.injectionKey,
      supports_psbt: info.supportsPsbt,
      supports_taproot: info.supportsTaproot,
      supports_ordinals: info.supportsOrdinals,
      mobile_support: info.mobileSupport,
      deep_link_scheme: info.deepLinkScheme,
    };
  }

  async connect(): Promise<WalletAccountForWasm> {
    // Already connected via ConnectedWallet
    return {
      address: this.wallet.address,
      public_key: this.wallet.publicKey,
      address_type: this.wallet.account.addressType || 'unknown',
      payment_address: this.wallet.paymentAddress,
      payment_public_key: this.wallet.paymentPublicKey,
    };
  }

  async disconnect(): Promise<void> {
    await this.wallet.disconnect();
  }

  async getAccounts(): Promise<WalletAccountForWasm[]> {
    // Most wallets only expose one account via the standard API
    return [
      {
        address: this.wallet.address,
        public_key: this.wallet.publicKey,
        address_type: this.wallet.account.addressType || 'unknown',
        payment_address: this.wallet.paymentAddress,
        payment_public_key: this.wallet.paymentPublicKey,
      },
    ];
  }

  async getNetwork(): Promise<string> {
    return this.wallet.getNetwork();
  }

  async switchNetwork(network: string): Promise<void> {
    // Most wallets don't support programmatic network switching
    throw new Error(`${this.wallet.info.name} does not support programmatic network switching`);
  }

  async signMessage(message: string, address: string): Promise<string> {
    return this.wallet.signMessage(message);
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    // Full PSBT patching pipeline:
    // 1. Patch tapInternalKey (dummy wallet key → user's real key)
    // 2. Patch input witnessUtxo scripts (dummy wallet scripts → user's real scripts)
    // 3. Inject redeemScripts for P2SH-P2WPKH wallets (Xverse)
    let patchedHex = this.patchTapInternalKey(psbtHex);
    patchedHex = this.patchInputWitnessScripts(patchedHex);
    patchedHex = this.injectRedeemScripts(patchedHex);

    const signingOptions: PsbtSigningOptions | undefined = options
      ? {
          autoFinalized: options.auto_finalized,
          toSignInputs: options.to_sign_inputs?.map((input) => ({
            index: input.index,
            address: input.address,
            sighashTypes: input.sighash_types,
            disableTweakedPublicKey: input.disable_tweaked_public_key,
          })),
        }
      : undefined;

    // 60-second timeout to detect wallet popup dismissed / extension crash
    const signPromise = this.wallet.signPsbt(patchedHex, signingOptions);
    const timeoutPromise = new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error(`${this.wallet.info.name} signing timed out after 60s`)), 60000)
    );
    return Promise.race([signPromise, timeoutPromise]);
  }

  async signPsbts(psbtHexs: string[], options?: PsbtSigningOptionsForWasm): Promise<string[]> {
    // Default implementation signs one at a time
    const results: string[] = [];
    for (const psbtHex of psbtHexs) {
      const signed = await this.signPsbt(psbtHex, options);
      results.push(signed);
    }
    return results;
  }

  async pushTx(txHex: string): Promise<string> {
    // Default: broadcast via external service
    // Most wallets don't have a direct pushTx method
    throw new Error(`${this.wallet.info.name} does not support direct transaction broadcasting`);
  }

  async pushPsbt(psbtHex: string): Promise<string> {
    // Finalize PSBT and extract transaction, then broadcast
    try {
      const psbt = bitcoin.Psbt.fromHex(psbtHex);
      psbt.finalizeAllInputs();
      const tx = psbt.extractTransaction();
      return this.pushTx(tx.toHex());
    } catch (e) {
      throw new Error(`Failed to push PSBT: ${e}`);
    }
  }

  async getPublicKey(): Promise<string> {
    if (!this.wallet.publicKey) {
      throw new Error('Public key not available');
    }
    return this.wallet.publicKey;
  }

  async getBalance(): Promise<number | null> {
    // Not directly available from most wallet APIs
    return null;
  }

  async getInscriptions(cursor?: number, size?: number): Promise<any> {
    // Not available from base wallet API
    return { list: [], total: 0 };
  }

  /**
   * Patch input witnessUtxo scripts from the dummy wallet's scriptPubKeys to the
   * connected wallet's real scriptPubKeys.
   *
   * The SDK's WASM builds PSBTs using a dummy wallet. The witnessUtxo.script fields
   * contain the dummy wallet's hashes. Browser wallets validate witnessUtxo consistency
   * for sighash computation:
   *   - UniSat: "Cannot read properties of undefined (reading 'scriptPk')"
   *   - All wallets: incorrect sighash if witnessUtxo.script doesn't match the real UTXO
   *
   * Matching is by SCRIPT TYPE PATTERN (opcode + length), not by exact bytes,
   * because the dummy wallet's hashes differ from the user's.
   */
  protected patchInputWitnessScripts(psbtHex: string): string {
    const taprootAddr = this.wallet.address;
    const segwitAddr = this.wallet.paymentAddress;

    if (!taprootAddr) return psbtHex;

    try {
      const psbt = bitcoin.Psbt.fromHex(psbtHex);
      const network = this.getBitcoinNetworkFromAddress(taprootAddr);

      const taprootScript = bitcoin.address.toOutputScript(taprootAddr, network);

      // Derive segwit script only if we have a distinct segwit address
      let segwitScript: Buffer | null = null;
      if (segwitAddr && segwitAddr !== taprootAddr) {
        try {
          const s = bitcoin.address.toOutputScript(segwitAddr, network);
          const buf = Buffer.from(s);
          // Only use for P2WPKH/P2SH patching
          if ((buf.length === 22 && buf[0] === 0x00 && buf[1] === 0x14) ||
              (buf.length === 23 && buf[0] === 0xa9)) {
            segwitScript = buf;
          }
        } catch { /* ignore invalid address */ }
      }

      let patched = 0;
      for (const input of psbt.data.inputs) {
        if (!input.witnessUtxo) continue;
        const script = Buffer.from(input.witnessUtxo.script);

        // P2TR (0x51, 0x20, 32-byte key)
        if (script.length === 34 && script[0] === 0x51 && script[1] === 0x20) {
          input.witnessUtxo = { ...input.witnessUtxo, script: taprootScript };
          patched++;
        }
        // P2WPKH (0x00, 0x14, 20-byte hash)
        else if (script.length === 22 && script[0] === 0x00 && script[1] === 0x14) {
          if (segwitScript) {
            input.witnessUtxo = { ...input.witnessUtxo, script: segwitScript };
            patched++;
          } else {
            // Single-address wallet — leave as-is (taproot wallet won't have P2WPKH UTXOs)
          }
        }
        // P2SH (0xa9, 0x14, ..., 0x87) — replace with segwit if user has native segwit
        else if (script.length === 23 && script[0] === 0xa9) {
          if (segwitScript && segwitScript.length === 22) {
            // User has native P2WPKH but dummy wallet used P2SH — patch to native
            input.witnessUtxo = { ...input.witnessUtxo, script: segwitScript };
            patched++;
          }
        }
      }

      return patched > 0 ? psbt.toHex() : psbtHex;
    } catch {
      return psbtHex;
    }
  }

  /**
   * Inject redeemScript for P2SH-P2WPKH inputs (needed for Xverse and similar wallets).
   *
   * Only applies when the wallet's payment address is a P2SH address (starts with '3' or '2').
   * For native segwit wallets, this is a no-op.
   */
  protected injectRedeemScripts(psbtHex: string): string {
    const paymentAddr = this.wallet.paymentAddress;
    const paymentPubKey = this.wallet.paymentPublicKey;

    if (!paymentAddr || !paymentPubKey) return psbtHex;

    // Only P2SH addresses need redeemScript injection
    if (!paymentAddr.startsWith('3') && !paymentAddr.startsWith('2')) return psbtHex;

    try {
      const psbt = bitcoin.Psbt.fromHex(psbtHex);
      const network = this.getBitcoinNetworkFromAddress(paymentAddr);
      const pubkey = Buffer.from(paymentPubKey, 'hex');
      const p2wpkh = bitcoin.payments.p2wpkh({ pubkey, network });
      const redeemScript = Buffer.from(p2wpkh.output!);
      const p2shScript = Buffer.from(bitcoin.address.toOutputScript(paymentAddr, network));

      let patched = 0;
      for (let i = 0; i < psbt.data.inputs.length; i++) {
        const input = psbt.data.inputs[i];
        if (input.redeemScript) continue; // Already has redeemScript

        if (!input.witnessUtxo) continue;
        const script = Buffer.from(input.witnessUtxo.script);

        // Match P2WPKH or P2SH scripts that need redeemScript
        if ((script.length === 22 && script[0] === 0x00 && script[1] === 0x14) ||
            script.equals(p2shScript)) {
          // Replace witnessUtxo script with the P2SH scriptPubKey
          input.witnessUtxo = { ...input.witnessUtxo, script: p2shScript };
          psbt.data.inputs[i].redeemScript = redeemScript;
          patched++;
        }
      }

      return patched > 0 ? psbt.toHex() : psbtHex;
    } catch {
      return psbtHex;
    }
  }

  /**
   * Detect Bitcoin network from address prefix
   */
  protected getBitcoinNetworkFromAddress(addr: string): bitcoin.Network {
    const lower = addr.toLowerCase();
    if (lower.startsWith('bc1') || lower.startsWith('1') || lower.startsWith('3')) {
      return bitcoin.networks.bitcoin;
    }
    if (lower.startsWith('tb1') || lower.startsWith('m') || lower.startsWith('n') || lower.startsWith('2')) {
      return bitcoin.networks.testnet;
    }
    return bitcoin.networks.regtest;
  }

  /**
   * Patch tapInternalKey on P2TR inputs to the connected wallet's actual x-only public key.
   *
   * When the SDK's WASM builds a PSBT, it sets tap_internal_key from the dummy wallet's
   * keypair (see execute.rs). Browser wallets use tapInternalKey to identify which inputs
   * belong to the connected account — a mismatch causes the wallet to skip all inputs:
   *   - UniSat: infinite loading spinner (silently skips unmatched inputs)
   *   - Xverse: "No taproot scripts signed" error
   *
   * This method replaces the dummy key with the real wallet key so the wallet can
   * recognize and sign its own inputs.
   */
  protected patchTapInternalKey(psbtHex: string): string {
    const pubKey = this.wallet.publicKey;
    if (!pubKey) return psbtHex;

    try {
      const psbt = bitcoin.Psbt.fromHex(psbtHex);
      // Derive x-only key (strip 02/03 prefix if compressed, or use as-is if already 32 bytes)
      const xOnlyHex = pubKey.length === 66 ? pubKey.slice(2) : pubKey;
      const xOnlyBuf = Buffer.from(xOnlyHex, 'hex');

      if (xOnlyBuf.length !== 32) return psbtHex; // Safety: not a valid x-only key

      let patched = 0;
      for (let i = 0; i < psbt.data.inputs.length; i++) {
        if (psbt.data.inputs[i].tapInternalKey) {
          psbt.data.inputs[i].tapInternalKey = xOnlyBuf;
          patched++;
        }
      }

      if (patched > 0) {
        return psbt.toHex();
      }
    } catch {
      // If PSBT parsing fails, return the original — let the wallet handle the error
    }

    return psbtHex;
  }
}

/**
 * Unisat-specific wallet adapter
 */
export class UnisatAdapter extends BaseWalletAdapter {
  private get unisat(): any {
    return (window as any).unisat;
  }

  async switchNetwork(network: string): Promise<void> {
    if (!this.unisat) throw new Error('Unisat not available');
    // Unisat uses different network names
    const unisatNetwork = network === 'mainnet' ? 'livenet' : network;
    await this.unisat.switchNetwork(unisatNetwork);
  }

  async pushTx(txHex: string): Promise<string> {
    if (!this.unisat) throw new Error('Unisat not available');
    return this.unisat.pushTx(txHex);
  }

  async pushPsbt(psbtHex: string): Promise<string> {
    if (!this.unisat) throw new Error('Unisat not available');
    return this.unisat.pushPsbt(psbtHex);
  }

  async getBalance(): Promise<number | null> {
    if (!this.unisat) return null;
    try {
      const balance = await this.unisat.getBalance();
      return balance?.total || balance?.confirmed || null;
    } catch {
      return null;
    }
  }

  async getInscriptions(cursor?: number, size?: number): Promise<any> {
    if (!this.unisat) return { list: [], total: 0 };
    try {
      return this.unisat.getInscriptions(cursor || 0, size || 20);
    } catch {
      return { list: [], total: 0 };
    }
  }

  async signPsbts(psbtHexs: string[], options?: PsbtSigningOptionsForWasm): Promise<string[]> {
    if (!this.unisat) throw new Error('Unisat not available');
    // Patch tapInternalKey on all PSBTs before batch signing
    const patchedHexs = psbtHexs.map((hex) => this.patchTapInternalKey(hex));
    return this.unisat.signPsbts(patchedHexs, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs: options?.to_sign_inputs,
    });
  }
}

/**
 * Xverse-specific wallet adapter
 *
 * Xverse is a dual-address wallet with separate ordinals (taproot) and payment (segwit) addresses.
 * Based on the lasereyes-mono implementation: https://github.com/omnisat/lasereyes-mono
 */
export class XverseAdapter extends BaseWalletAdapter {
  private get xverse(): any {
    return (window as any).XverseProviders?.BitcoinProvider;
  }

  /**
   * Get the Bitcoin network for address derivation
   */
  private getBitcoinNetwork(): bitcoin.Network {
    // Detect from address prefix
    const addr = this.wallet.address.toLowerCase();
    if (addr.startsWith('bc1') || addr.startsWith('1') || addr.startsWith('3')) {
      return bitcoin.networks.bitcoin;
    }
    if (addr.startsWith('tb1') || addr.startsWith('m') || addr.startsWith('n') || addr.startsWith('2')) {
      return bitcoin.networks.testnet;
    }
    // Default to mainnet
    return bitcoin.networks.bitcoin;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.xverse) throw new Error('Xverse not available');

    // Patch tapInternalKey before signing (dummy wallet key → user's real key)
    const patchedHex = this.patchTapInternalKey(psbtHex);

    // Xverse prefers base64 format
    const psbt = bitcoin.Psbt.fromHex(patchedHex);
    const psbtBase64 = psbt.toBase64();

    const signInputs = this.buildXverseSignInputs(psbt, options);

    const response = await this.xverse.request('signPsbt', {
      psbt: psbtBase64,
      signInputs,
      broadcast: false,
    });

    if (response.status === 'success') {
      // Convert back to hex
      const signedPsbt = bitcoin.Psbt.fromBase64(response.result.psbt);
      return signedPsbt.toHex();
    }

    throw new Error(response.error?.message || 'Xverse signing failed');
  }

  /**
   * Build signInputs mapping for Xverse PSBT signing.
   *
   * For dual-address wallets like Xverse, we need to:
   * 1. Get both the ordinals (taproot) and payment (segwit) addresses
   * 2. For each PSBT input, derive the address from witnessUtxo.script
   * 3. Map each input to the correct address
   *
   * Based on lasereyes-mono implementation.
   */
  private buildXverseSignInputs(
    psbt: bitcoin.Psbt,
    options?: PsbtSigningOptionsForWasm
  ): Record<string, number[]> {
    // If explicit inputs are provided, use them
    if (options?.to_sign_inputs) {
      return options.to_sign_inputs.reduce(
        (acc: Record<string, number[]>, input) => {
          const addr = input.address || this.wallet.address;
          acc[addr] = [...(acc[addr] || []), input.index];
          return acc;
        },
        {}
      );
    }

    // Get both addresses (ordinals = taproot, payment = segwit)
    const ordinalsAddress = this.wallet.address;
    const paymentAddress = this.wallet.paymentAddress;

    const inputs = psbt.data.inputs;
    const network = this.getBitcoinNetwork();

    // Initialize address data maps
    const ordinalAddressData: Record<string, number[]> = {
      [ordinalsAddress]: [],
    };
    const paymentsAddressData: Record<string, number[]> = paymentAddress
      ? { [paymentAddress]: [] }
      : {};

    // Analyze each input and determine which address it belongs to
    for (let i = 0; i < inputs.length; i++) {
      const input = inputs[i];

      // If no witnessUtxo, default to payment address (like lasereyes-mono)
      if (input.witnessUtxo === undefined) {
        if (paymentAddress) {
          paymentsAddressData[paymentAddress].push(i);
        } else {
          // Single-address mode: use the ordinals address
          ordinalAddressData[ordinalsAddress].push(i);
        }
        continue;
      }

      // Derive address from the witnessUtxo script
      const { script } = input.witnessUtxo;
      try {
        const addressFromScript = bitcoin.address.fromOutputScript(script, network);

        if (paymentAddress && addressFromScript === paymentAddress) {
          paymentsAddressData[paymentAddress].push(i);
        } else if (addressFromScript === ordinalsAddress) {
          ordinalAddressData[ordinalsAddress].push(i);
        } else {
          // Unknown address - try to match by address type
          const isSegwit = addressFromScript.toLowerCase().startsWith('bc1q') ||
                           addressFromScript.toLowerCase().startsWith('tb1q') ||
                           addressFromScript.startsWith('3') ||
                           addressFromScript.startsWith('2');

          if (isSegwit && paymentAddress) {
            paymentsAddressData[paymentAddress].push(i);
          } else {
            ordinalAddressData[ordinalsAddress].push(i);
          }
        }
      } catch (e) {
        // Failed to derive address - default to ordinals address
        ordinalAddressData[ordinalsAddress].push(i);
      }
    }

    // Build the final signInputs object
    const signInputs: Record<string, number[]> = {};

    if (ordinalAddressData[ordinalsAddress].length > 0) {
      signInputs[ordinalsAddress] = ordinalAddressData[ordinalsAddress];
    }

    if (paymentAddress && paymentsAddressData[paymentAddress]?.length > 0) {
      signInputs[paymentAddress] = paymentsAddressData[paymentAddress];
    }

    return signInputs;
  }

  async switchNetwork(network: string): Promise<void> {
    if (!this.xverse) throw new Error('Xverse not available');
    // Xverse has a wallet_changeNetwork method
    const xverseNetwork = network === 'mainnet' ? 'Mainnet' : 'Testnet';
    await this.xverse.request('wallet_changeNetwork', { name: xverseNetwork });
  }
}

/**
 * OKX-specific wallet adapter
 */
export class OkxAdapter extends BaseWalletAdapter {
  private get okx(): any {
    return (window as any).okxwallet?.bitcoin;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.okx) throw new Error('OKX wallet not available');
    const patchedHex = this.patchTapInternalKey(psbtHex);
    return this.okx.signPsbt(patchedHex, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs: options?.to_sign_inputs,
    });
  }

  async signPsbts(psbtHexs: string[], options?: PsbtSigningOptionsForWasm): Promise<string[]> {
    if (!this.okx) throw new Error('OKX wallet not available');
    const patchedHexs = psbtHexs.map((hex) => this.patchTapInternalKey(hex));
    return this.okx.signPsbts(patchedHexs, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs: options?.to_sign_inputs,
    });
  }

  async pushTx(txHex: string): Promise<string> {
    if (!this.okx) throw new Error('OKX wallet not available');
    return this.okx.pushTx(txHex);
  }

  async pushPsbt(psbtHex: string): Promise<string> {
    if (!this.okx) throw new Error('OKX wallet not available');
    return this.okx.pushPsbt(psbtHex);
  }

  async getBalance(): Promise<number | null> {
    if (!this.okx) return null;
    try {
      const balance = await this.okx.getBalance();
      return balance?.total || null;
    } catch {
      return null;
    }
  }

  async getInscriptions(cursor?: number, size?: number): Promise<any> {
    if (!this.okx) return { list: [], total: 0 };
    try {
      return this.okx.getInscriptions(cursor || 0, size || 20);
    } catch {
      return { list: [], total: 0 };
    }
  }
}

/**
 * Leather-specific wallet adapter
 *
 * Leather is a dual-address wallet with separate taproot and segwit addresses.
 */
export class LeatherAdapter extends BaseWalletAdapter {
  private get leather(): any {
    return (window as any).LeatherProvider;
  }

  /**
   * Get the Bitcoin network for address derivation
   */
  private getBitcoinNetwork(): bitcoin.Network {
    const addr = this.wallet.address.toLowerCase();
    if (addr.startsWith('bc1') || addr.startsWith('1') || addr.startsWith('3')) {
      return bitcoin.networks.bitcoin;
    }
    if (addr.startsWith('tb1') || addr.startsWith('m') || addr.startsWith('n') || addr.startsWith('2')) {
      return bitcoin.networks.testnet;
    }
    return bitcoin.networks.bitcoin;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.leather) throw new Error('Leather wallet not available');

    // Patch tapInternalKey before signing (dummy wallet key → user's real key)
    const patchedHex = this.patchTapInternalKey(psbtHex);

    // Leather uses signAtIndex - we need to determine which inputs to sign
    let signAtIndex: number[] | undefined;

    if (options?.to_sign_inputs) {
      signAtIndex = options.to_sign_inputs.map((i) => i.index);
    } else {
      // Auto-detect inputs to sign based on addresses
      const psbt = bitcoin.Psbt.fromHex(patchedHex);
      const inputs = psbt.data.inputs;
      const network = this.getBitcoinNetwork();
      const ordinalsAddress = this.wallet.address;
      const paymentAddress = this.wallet.paymentAddress;

      signAtIndex = [];
      for (let i = 0; i < inputs.length; i++) {
        const input = inputs[i];
        if (input.witnessUtxo) {
          try {
            const addressFromScript = bitcoin.address.fromOutputScript(
              input.witnessUtxo.script,
              network
            );
            // Include input if it matches either address
            if (addressFromScript === ordinalsAddress ||
                (paymentAddress && addressFromScript === paymentAddress)) {
              signAtIndex.push(i);
            }
          } catch {
            // If we can't determine, include by default
            signAtIndex.push(i);
          }
        } else {
          // No witnessUtxo - include by default
          signAtIndex.push(i);
        }
      }
    }

    const response = await this.leather.request('signPsbt', {
      hex: patchedHex,
      signAtIndex,
      broadcast: false,
    });

    return response.result.hex;
  }

  async signMessage(message: string, address: string): Promise<string> {
    if (!this.leather) throw new Error('Leather wallet not available');

    const response = await this.leather.request('signMessage', {
      message,
      paymentType: 'p2wpkh',
    });

    return response.result.signature;
  }
}

/**
 * Phantom-specific wallet adapter
 */
export class PhantomAdapter extends BaseWalletAdapter {
  private get phantom(): any {
    return (window as any).phantom?.bitcoin;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.phantom) throw new Error('Phantom Bitcoin not available');

    const patchedHex = this.patchTapInternalKey(psbtHex);
    const psbtBytes = hexToBytes(patchedHex);
    const { signedPsbt } = await this.phantom.signPSBT(psbtBytes, {
      inputsToSign: options?.to_sign_inputs?.map((i) => ({
        sigHash: i.sighash_types?.[0],
        address: i.address || this.wallet.address,
        signingIndexes: [i.index],
      })),
    });

    return bytesToHex(signedPsbt);
  }

  async signMessage(message: string, address: string): Promise<string> {
    if (!this.phantom) throw new Error('Phantom Bitcoin not available');

    const { signature } = await this.phantom.signMessage(
      address || this.wallet.address,
      new TextEncoder().encode(message)
    );

    return signature;
  }
}

/**
 * Magic Eden-specific wallet adapter
 *
 * Magic Eden is a dual-address wallet with separate ordinals and payment addresses.
 */
export class MagicEdenAdapter extends BaseWalletAdapter {
  private get magicEden(): any {
    return (window as any).magicEden?.bitcoin;
  }

  /**
   * Get the Bitcoin network for address derivation
   */
  private getBitcoinNetwork(): bitcoin.Network {
    const addr = this.wallet.address.toLowerCase();
    if (addr.startsWith('bc1') || addr.startsWith('1') || addr.startsWith('3')) {
      return bitcoin.networks.bitcoin;
    }
    if (addr.startsWith('tb1') || addr.startsWith('m') || addr.startsWith('n') || addr.startsWith('2')) {
      return bitcoin.networks.testnet;
    }
    return bitcoin.networks.bitcoin;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.magicEden) throw new Error('Magic Eden wallet not available');

    // Patch tapInternalKey before signing (dummy wallet key → user's real key)
    const patchedHex = this.patchTapInternalKey(psbtHex);

    // Build toSignInputs based on addresses if not explicitly provided
    let toSignInputs = options?.to_sign_inputs;

    if (!toSignInputs) {
      const psbt = bitcoin.Psbt.fromHex(patchedHex);
      const inputs = psbt.data.inputs;
      const network = this.getBitcoinNetwork();
      const ordinalsAddress = this.wallet.address;
      const paymentAddress = this.wallet.paymentAddress;

      toSignInputs = [];
      for (let i = 0; i < inputs.length; i++) {
        const input = inputs[i];
        if (input.witnessUtxo) {
          try {
            const addressFromScript = bitcoin.address.fromOutputScript(
              input.witnessUtxo.script,
              network
            );
            // Assign to the correct address
            if (addressFromScript === ordinalsAddress) {
              toSignInputs.push({ index: i, address: ordinalsAddress });
            } else if (paymentAddress && addressFromScript === paymentAddress) {
              toSignInputs.push({ index: i, address: paymentAddress });
            } else {
              // Default to ordinals address
              toSignInputs.push({ index: i, address: ordinalsAddress });
            }
          } catch {
            toSignInputs.push({ index: i, address: ordinalsAddress });
          }
        } else {
          // Default to payment address if available, otherwise ordinals
          toSignInputs.push({ index: i, address: paymentAddress || ordinalsAddress });
        }
      }
    }

    return this.magicEden.signPsbt(patchedHex, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs,
    });
  }

  async signMessage(message: string, address: string): Promise<string> {
    if (!this.magicEden) throw new Error('Magic Eden wallet not available');
    return this.magicEden.signMessage(message);
  }
}

/**
 * Wizz-specific wallet adapter
 */
export class WizzAdapter extends BaseWalletAdapter {
  private get wizz(): any {
    return (window as any).wizz;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.wizz) throw new Error('Wizz wallet not available');
    const patchedHex = this.patchTapInternalKey(psbtHex);
    return this.wizz.signPsbt(patchedHex, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs: options?.to_sign_inputs,
    });
  }

  async signPsbts(psbtHexs: string[], options?: PsbtSigningOptionsForWasm): Promise<string[]> {
    if (!this.wizz) throw new Error('Wizz wallet not available');
    const patchedHexs = psbtHexs.map((hex) => this.patchTapInternalKey(hex));
    return this.wizz.signPsbts(patchedHexs, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs: options?.to_sign_inputs,
    });
  }

  async pushTx(txHex: string): Promise<string> {
    if (!this.wizz) throw new Error('Wizz wallet not available');
    return this.wizz.pushTx(txHex);
  }

  async getBalance(): Promise<number | null> {
    if (!this.wizz) return null;
    try {
      const balance = await this.wizz.getBalance();
      return balance?.total || null;
    } catch {
      return null;
    }
  }
}

/**
 * Oyl-specific wallet adapter
 *
 * OYL's API uses object parameters for signPsbt ({psbt, finalize, broadcast})
 * rather than plain hex strings like Unisat-style wallets.
 */
export class OylAdapter extends BaseWalletAdapter {
  private get oyl(): any {
    return (window as any).oyl;
  }

  /**
   * OYL-specific signPsbt with reconnection on session expiry.
   *
   * OYL's sessions can expire mid-operation. When that happens, signing fails
   * with "Site origin must be connected first". We auto-reconnect via
   * getAddresses() and retry once.
   *
   * Also: OYL shows one confirmation popup PER INPUT — multiple popups are expected UX.
   */
  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.oyl) throw new Error("Oyl wallet not available");

    // Full patching pipeline (inherited from base)
    let patchedHex = this.patchTapInternalKey(psbtHex);
    patchedHex = this.patchInputWitnessScripts(patchedHex);
    patchedHex = this.injectRedeemScripts(patchedHex);

    const doSign = async () => {
      const result = await this.oyl.signPsbt({
        psbt: patchedHex,
        finalize: options?.auto_finalized,
        broadcast: false,
      });
      return result.psbt;
    };

    try {
      // 60-second timeout
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error('OYL signing timed out after 60s')), 60000)
      );
      return await Promise.race([doSign(), timeoutPromise]);
    } catch (err: any) {
      const msg = err?.message || String(err);
      // Auto-reconnect on session expiry
      if (msg.includes('connected first') || msg.includes('not connected')) {
        try {
          // Re-establish connection via getAddresses()
          if (typeof this.oyl.connect === 'function') {
            await this.oyl.connect();
          }
          await this.oyl.getAddresses();
        } catch { /* reconnection attempt failed, throw original */ }

        // Retry once after reconnection
        const timeoutPromise = new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('OYL signing timed out after 60s (retry)')), 60000)
        );
        return Promise.race([doSign(), timeoutPromise]);
      }
      throw err;
    }
  }

  async signPsbts(psbtHexs: string[], options?: PsbtSigningOptionsForWasm): Promise<string[]> {
    if (!this.oyl) throw new Error("Oyl wallet not available");
    const psbtsToSign = psbtHexs.map(hex => {
      let patched = this.patchTapInternalKey(hex);
      patched = this.patchInputWitnessScripts(patched);
      patched = this.injectRedeemScripts(patched);
      return {
        psbt: patched,
        finalize: options?.auto_finalized,
        broadcast: false,
      };
    });
    const results = await this.oyl.signPsbts(psbtsToSign);
    return results.map((r: { psbt: string }) => r.psbt);
  }

  async signMessage(message: string, address: string): Promise<string> {
    if (!this.oyl) throw new Error("Oyl wallet not available");
    const result = await this.oyl.signMessage({ address, message });
    return result.signature;
  }

  async pushPsbt(psbtHex: string): Promise<string> {
    if (!this.oyl) throw new Error("Oyl wallet not available");
    const result = await this.oyl.pushPsbt({ psbt: psbtHex });
    return result.txid;
  }

  async switchNetwork(network: string): Promise<void> {
    if (!this.oyl) throw new Error("Oyl wallet not available");
    await this.oyl.switchNetwork(network);
  }
}

/**
 * Tokeo-specific wallet adapter
 *
 * Tokeo injects at window.tokeo.bitcoin and follows the UniSat-like API pattern.
 */
export class TokeoAdapter extends BaseWalletAdapter {
  private get tokeo(): any {
    return (window as any).tokeo?.bitcoin;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.tokeo) throw new Error('Tokeo wallet not available');
    let patchedHex = this.patchTapInternalKey(psbtHex);
    patchedHex = this.patchInputWitnessScripts(patchedHex);
    patchedHex = this.injectRedeemScripts(patchedHex);
    return this.tokeo.signPsbt(patchedHex, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs: options?.to_sign_inputs,
    });
  }

  async signMessage(message: string, address: string): Promise<string> {
    if (!this.tokeo) throw new Error('Tokeo wallet not available');
    return this.tokeo.signMessage(message);
  }
}

/**
 * Orange-specific wallet adapter
 *
 * Orange injects at multiple possible window paths:
 *   window.OrangeBitcoinProvider
 *   window.OrangecryptoProviders?.BitcoinProvider
 *   window.OrangeWalletProviders?.OrangeBitcoinProvider
 */
export class OrangeAdapter extends BaseWalletAdapter {
  private get orange(): any {
    const win = window as any;
    return win.OrangeBitcoinProvider ||
           win.OrangecryptoProviders?.BitcoinProvider ||
           win.OrangeWalletProviders?.OrangeBitcoinProvider;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    if (!this.orange) throw new Error('Orange wallet not available');
    let patchedHex = this.patchTapInternalKey(psbtHex);
    patchedHex = this.patchInputWitnessScripts(patchedHex);
    patchedHex = this.injectRedeemScripts(patchedHex);
    return this.orange.signPsbt(patchedHex, {
      autoFinalized: options?.auto_finalized ?? true,
      toSignInputs: options?.to_sign_inputs,
    });
  }

  async signMessage(message: string, address: string): Promise<string> {
    if (!this.orange) throw new Error('Orange wallet not available');
    return this.orange.signMessage({ address, message });
  }
}

/**
 * Create a wallet adapter for a connected wallet
 *
 * @param wallet - The connected wallet instance
 * @returns A wallet adapter implementing JsWalletAdapter
 */
export function createWalletAdapter(wallet: ConnectedWallet): JsWalletAdapter {
  switch (wallet.info.id) {
    case 'unisat':
      return new UnisatAdapter(wallet);
    case 'xverse':
      return new XverseAdapter(wallet);
    case 'okx':
      return new OkxAdapter(wallet);
    case 'leather':
      return new LeatherAdapter(wallet);
    case 'phantom':
      return new PhantomAdapter(wallet);
    case 'magic-eden':
      return new MagicEdenAdapter(wallet);
    case 'wizz':
      return new WizzAdapter(wallet);
    case 'oyl':
      return new OylAdapter(wallet);
    case 'tokeo':
      return new TokeoAdapter(wallet);
    case 'orange':
      return new OrangeAdapter(wallet);
    default:
      // Use base adapter for unknown wallets
      return new BaseWalletAdapter(wallet);
  }
}

/**
 * Mock wallet adapter for testing
 *
 * This adapter can be used to test the WASM integration without a real wallet.
 */
export class MockWalletAdapter implements JsWalletAdapter {
  private mockAddress: string;
  private mockPublicKey: string;
  private mockNetwork: string;
  private signedPsbts: string[] = [];

  constructor(options?: {
    address?: string;
    publicKey?: string;
    network?: string;
  }) {
    this.mockAddress = options?.address || 'bc1qtest1234567890abcdef';
    this.mockPublicKey = options?.publicKey || '03' + '0'.repeat(64);
    this.mockNetwork = options?.network || 'mainnet';
  }

  getInfo(): WalletInfoForWasm {
    return {
      id: 'mock',
      name: 'Mock Wallet',
      icon: '/assets/wallets/mock.svg',
      website: 'https://mock.wallet',
      injection_key: 'mockWallet',
      supports_psbt: true,
      supports_taproot: true,
      supports_ordinals: true,
      mobile_support: false,
    };
  }

  async connect(): Promise<WalletAccountForWasm> {
    return {
      address: this.mockAddress,
      public_key: this.mockPublicKey,
      address_type: 'p2wpkh',
    };
  }

  async disconnect(): Promise<void> {}

  async getAccounts(): Promise<WalletAccountForWasm[]> {
    return [await this.connect()];
  }

  async getNetwork(): Promise<string> {
    return this.mockNetwork;
  }

  async switchNetwork(network: string): Promise<void> {
    this.mockNetwork = network;
  }

  async signMessage(message: string, address: string): Promise<string> {
    // Return a mock signature (base64 encoded)
    const mockSig = Buffer.from(`mock_sig_${message.substring(0, 10)}`).toString('base64');
    return mockSig;
  }

  async signPsbt(psbtHex: string, options?: PsbtSigningOptionsForWasm): Promise<string> {
    // For mock, just return the same PSBT (in reality, you'd want to sign it)
    this.signedPsbts.push(psbtHex);
    return psbtHex;
  }

  async signPsbts(psbtHexs: string[], options?: PsbtSigningOptionsForWasm): Promise<string[]> {
    return psbtHexs.map((psbt) => {
      this.signedPsbts.push(psbt);
      return psbt;
    });
  }

  async pushTx(txHex: string): Promise<string> {
    // Return a mock txid
    return '0'.repeat(64);
  }

  async pushPsbt(psbtHex: string): Promise<string> {
    return '0'.repeat(64);
  }

  async getPublicKey(): Promise<string> {
    return this.mockPublicKey;
  }

  async getBalance(): Promise<number | null> {
    return 100000000; // 1 BTC in satoshis
  }

  async getInscriptions(cursor?: number, size?: number): Promise<any> {
    return { list: [], total: 0 };
  }

  /** Get PSBTs that were signed (for testing) */
  getSignedPsbts(): string[] {
    return this.signedPsbts;
  }

  /** Clear signed PSBTs (for testing) */
  clearSignedPsbts(): void {
    this.signedPsbts = [];
  }
}

// Utility functions
function hexToBytes(hex: string): Uint8Array {
  const cleanHex = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = new Uint8Array(cleanHex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(cleanHex.substr(i * 2, 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}
