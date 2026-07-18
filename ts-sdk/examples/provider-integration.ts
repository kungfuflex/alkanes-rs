/**
 * Provider integration example
 * 
 * Demonstrates using the Alkanes provider with @oyl/sdk compatibility.
 */

import {
  createProvider,
  createWalletFromMnemonic,
  satoshisToBTC,
} from '../src';
import * as bitcoin from 'bitcoinjs-lib';

async function main() {
  console.log('🚀 Alkanes SDK - Provider Integration Example\n');

  // 1. Create provider (regtest for testing)
  console.log('1️⃣  Creating provider...');
  const provider = createProvider({
    url: 'http://localhost:18443', // Local regtest node
    network: bitcoin.networks.regtest,
    networkType: 'regtest',
  });
  console.log('✅ Provider created!\n');

  // 2. Create a wallet
  console.log('2️⃣  Creating test wallet...');
  const wallet = createWalletFromMnemonic(
    'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
    'regtest'
  );
  const address = wallet.getReceivingAddress(0);
  console.log('📬 Wallet address:', address);
  console.log();

  // 3. Get blockchain info
  console.log('3️⃣  Fetching blockchain info...');
  try {
    const blockCount = await provider.bitcoin.getBlockCount();
    console.log('📊 Block height:', blockCount);
    
    if (blockCount > 0) {
      const blockHash = await provider.bitcoin.getBlockHash(blockCount);
      console.log('🔗 Latest block hash:', blockHash);
      
      const blockInfo = await provider.getBlockInfo(blockCount);
      console.log('⏰ Block timestamp:', new Date(blockInfo.timestamp * 1000).toISOString());
      console.log('📦 Transactions:', blockInfo.txCount);
    }
  } catch (error) {
    console.log('⚠️  Could not connect to Bitcoin node:', (error as Error).message);
    console.log('   Make sure you have a regtest node running on localhost:18443');
  }
  console.log();

  // 4. Check address balance (via Esplora)
  console.log('4️⃣  Checking address balance...');
  try {
    const balance = await provider.getBalance(address);
    console.log('💰 Confirmed balance:', satoshisToBTC(balance.confirmed), 'BTC');
    console.log('⏳ Unconfirmed balance:', satoshisToBTC(balance.unconfirmed), 'BTC');
    console.log('📝 UTXOs:', balance.utxos.length);
    
    if (balance.utxos.length > 0) {
      console.log('   First UTXO:');
      console.log('     TX:', balance.utxos[0].txid);
      console.log('     Value:', satoshisToBTC(balance.utxos[0].value), 'BTC');
    }
  } catch (error) {
    console.log('⚠️  Could not fetch balance:', (error as Error).message);
    console.log('   Esplora API might not be available');
  }
  console.log();

  // 5. Example: Create and broadcast PSBT (commented out for safety)
  console.log('5️⃣  PSBT example (not executed)...');
  console.log('   To create and broadcast a transaction:');
  console.log('   ```typescript');
  console.log('   const psbt = await wallet.createPsbt({');
  console.log('     inputs: [{ txid, vout, value, address }],');
  console.log('     outputs: [{ address: recipientAddr, value: 10000 }],');
  console.log('     feeRate: 10,');
  console.log('   });');
  console.log('   ');
  console.log('   const result = await provider.pushPsbt({ psbtBase64: psbt });');
  console.log('   console.log("TX ID:", result.txId);');
  console.log('   ```');
  console.log();

  console.log('✅ Provider integration example completed!');
}

main().catch(console.error);
