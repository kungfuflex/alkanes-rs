/**
 * Provider integration for Alkanes SDK
 * 
 * Compatible with @oyl/sdk Provider interface.
 * Integrates with alkanes-web-sys WASM backend for alkanes-specific functionality.
 */

import * as bitcoin from 'bitcoinjs-lib';
import {
  ProviderConfig,
  NetworkType,
  TransactionResult,
  BlockInfo,
  UTXO,
  AddressBalance,
  AlkaneBalance,
  AlkaneCallParams,
  AlkaneId,
} from '../types';

// Import WASM module types
import type * as AlkanesWasm from '../../build/wasm/alkanes_web_sys';

/**
 * RPC client for Bitcoin Core / Sandshrew
 */
export class BitcoinRpcClient {
  constructor(private url: string) {}

  async call(method: string, params: any[] = []): Promise<any> {
    const response = await fetch(this.url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: Date.now(),
        method,
        params,
      }),
    });

    const json = await response.json();
    if (json.error) {
      throw new Error(`RPC error: ${json.error.message}`);
    }
    return json.result;
  }

  async getBlockCount(): Promise<number> {
    return this.call('getblockcount');
  }

  async getBlockHash(height: number): Promise<string> {
    return this.call('getblockhash', [height]);
  }

  async getBlock(hash: string): Promise<any> {
    return this.call('getblock', [hash, 2]); // Verbosity 2 for full tx data
  }

  async sendRawTransaction(hex: string): Promise<string> {
    return this.call('sendrawtransaction', [hex]);
  }

  async getTransaction(txid: string): Promise<any> {
    return this.call('getrawtransaction', [txid, true]);
  }

  async testMempoolAccept(txHex: string[]): Promise<any[]> {
    return this.call('testmempoolaccept', [txHex]);
  }

  async getMempoolEntry(txid: string): Promise<any> {
    return this.call('getmempoolentry', [txid]);
  }
}

/**
 * Esplora API client
 */
export class EsploraClient {
  constructor(private baseUrl: string) {}

  async getAddressInfo(address: string): Promise<any> {
    const response = await fetch(`${this.baseUrl}/address/${address}`);
    return response.json();
  }

  async getAddressUtxos(address: string): Promise<UTXO[]> {
    const response = await fetch(`${this.baseUrl}/address/${address}/utxo`);
    return response.json();
  }

  async getAddressBalance(address: string): Promise<AddressBalance> {
    const [info, utxos] = await Promise.all([
      this.getAddressInfo(address),
      this.getAddressUtxos(address),
    ]);

    return {
      address,
      confirmed: info.chain_stats.funded_txo_sum - info.chain_stats.spent_txo_sum,
      unconfirmed: info.mempool_stats.funded_txo_sum - info.mempool_stats.spent_txo_sum,
      utxos,
    };
  }

  async getTxInfo(txid: string): Promise<any> {
    const response = await fetch(`${this.baseUrl}/tx/${txid}`);
    return response.json();
  }

  async broadcastTx(txHex: string): Promise<string> {
    const response = await fetch(`${this.baseUrl}/tx`, {
      method: 'POST',
      body: txHex,
    });
    return response.text();
  }
}

/**
 * Alkanes RPC client (integrates with WASM)
 */
export class AlkanesRpcClient {
  private wasm?: typeof AlkanesWasm;

  constructor(
    private metashrewUrl: string,
    private sandshrewUrl?: string,
    wasmModule?: typeof AlkanesWasm
  ) {
    this.wasm = wasmModule;
  }

  async getAlkaneBalance(address: string, alkaneId: AlkaneId): Promise<AlkaneBalance> {
    // Use WASM backend if available
    if (this.wasm) {
      const provider = new this.wasm.WebProvider(this.metashrewUrl, this.sandshrewUrl || '');
      const balance = await provider.getAlkaneBalance(
        address,
        JSON.stringify(alkaneId)
      );
      return JSON.parse(balance);
    }

    // Fallback to direct HTTP call
    const response = await fetch(`${this.metashrewUrl}/alkanes/balance`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ address, alkaneId }),
    });

    return response.json();
  }

  async getAlkaneBytecode(alkaneId: AlkaneId, blockTag?: string): Promise<string> {
    if (this.wasm) {
      return this.wasm.get_alkane_bytecode(
        'mainnet',
        alkaneId.block,
        alkaneId.tx,
        blockTag || ''
      ) as any; // Cast needed as Promise return
    }

    const response = await fetch(`${this.metashrewUrl}/alkanes/bytecode`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ alkaneId, blockTag }),
    });

    const data = await response.json();
    return data.bytecode;
  }

  async simulateAlkaneCall(params: AlkaneCallParams): Promise<any> {
    if (this.wasm) {
      // Get bytecode first
      const bytecode = await this.getAlkaneBytecode(params.alkaneId);
      
      // Simulate using WASM
      const result = await this.wasm.simulate_alkane_call(
        JSON.stringify(params.alkaneId),
        bytecode,
        '0x' // Empty cellpack for now
      );
      
      return JSON.parse(result as any);
    }

    // Fallback
    const response = await fetch(`${this.metashrewUrl}/alkanes/simulate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(params),
    });

    return response.json();
  }
}

/**
 * Main Alkanes Provider (compatible with @oyl/sdk)
 */
export class AlkanesProvider {
  public bitcoin: BitcoinRpcClient;
  public esplora: EsploraClient;
  public alkanes: AlkanesRpcClient;
  public network: bitcoin.networks.Network;
  public networkType: NetworkType;
  public url: string;

  constructor(config: ProviderConfig, wasmModule?: typeof AlkanesWasm) {
    this.network = config.network;
    this.networkType = config.networkType;
    this.url = config.url;

    const masterUrl = config.projectId ? 
      `${config.url}/${config.version || 'v1'}/${config.projectId}` :
      config.url;

    this.bitcoin = new BitcoinRpcClient(masterUrl);
    this.esplora = new EsploraClient(masterUrl);
    this.alkanes = new AlkanesRpcClient(
      masterUrl,
      undefined,
      wasmModule
    );
  }

  /**
   * Push a PSBT to the network (compatible with @oyl/sdk)
   */
  async pushPsbt({ psbtHex, psbtBase64 }: { 
    psbtHex?: string;
    psbtBase64?: string;
  }): Promise<TransactionResult> {
    if (!psbtHex && !psbtBase64) {
      throw new Error('Please supply psbt in either base64 or hex format');
    }

    if (psbtHex && psbtBase64) {
      throw new Error('Please select one format of psbt to broadcast');
    }

    let psbt: bitcoin.Psbt;
    if (psbtHex) {
      psbt = bitcoin.Psbt.fromHex(psbtHex, { network: this.network });
    } else {
      psbt = bitcoin.Psbt.fromBase64(psbtBase64!, { network: this.network });
    }

    let extractedTx: bitcoin.Transaction;
    try {
      extractedTx = psbt.extractTransaction();
    } catch (error) {
      throw new Error(`Transaction could not be extracted due to invalid Psbt. ${error}`);
    }

    const txId = extractedTx.getId();
    const rawTx = extractedTx.toHex();

    // Test mempool acceptance
    const [result] = await this.bitcoin.testMempoolAccept([rawTx]);
    if (!result.allowed) {
      throw new Error(`Mempool rejected tx due to ${result['reject-reason']}`);
    }

    // Broadcast
    await this.bitcoin.sendRawTransaction(rawTx);

    // Get transaction info
    try {
      const mempoolEntry = await this.bitcoin.getMempoolEntry(txId);
      const fee = mempoolEntry.fees['base'] * 10 ** 8;

      return {
        txId,
        rawTx,
        size: mempoolEntry.vsize,
        weight: mempoolEntry.weight,
        fee,
        satsPerVByte: (fee / (mempoolEntry.weight / 4)).toFixed(2),
      };
    } catch (error) {
      // Fallback to esplora
      await new Promise(resolve => setTimeout(resolve, 1000));
      const tx = await this.esplora.getTxInfo(txId);
      
      return {
        txId,
        rawTx,
        size: tx.size,
        weight: tx.weight,
        fee: tx.fee,
        satsPerVByte: (tx.fee / (tx.weight / 4)).toFixed(2),
      };
    }
  }

  /**
   * Get block information
   */
  async getBlockInfo(hashOrHeight: string | number): Promise<BlockInfo> {
    const hash = typeof hashOrHeight === 'number' ?
      await this.bitcoin.getBlockHash(hashOrHeight) :
      hashOrHeight;

    const block = await this.bitcoin.getBlock(hash);

    return {
      hash: block.hash,
      height: block.height,
      timestamp: block.time,
      txCount: block.tx.length,
    };
  }

  /**
   * Get address balance
   */
  async getBalance(address: string): Promise<AddressBalance> {
    return this.esplora.getAddressBalance(address);
  }

  /**
   * Get alkane balance for address
   */
  async getAlkaneBalance(address: string, alkaneId: AlkaneId): Promise<AlkaneBalance> {
    return this.alkanes.getAlkaneBalance(address, alkaneId);
  }

  /**
   * Simulate alkane contract call
   */
  async simulateAlkaneCall(params: AlkaneCallParams): Promise<any> {
    return this.alkanes.simulateAlkaneCall(params);
  }
}

/**
 * Create an Alkanes provider instance
 * 
 * @param config - Provider configuration
 * @param wasmModule - Optional WASM module for alkanes functionality
 * @returns AlkanesProvider instance compatible with @oyl/sdk
 */
export function createProvider(
  config: ProviderConfig,
  wasmModule?: typeof AlkanesWasm
): AlkanesProvider {
  return new AlkanesProvider(config, wasmModule);
}
