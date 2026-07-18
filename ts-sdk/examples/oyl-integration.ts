/**
 * Example: Integrating @alkanes/ts-sdk with @oyl/sdk
 * 
 * This example demonstrates how to use alkanes-rs ts-sdk as a keystore
 * backend for @oyl/sdk, with full regtest support.
 */

import { 
  createKeystore, 
  unlockKeystore, 
  createWallet,
  createProvider,
  type AlkanesProvider,
} from '../src/index';
import * as bitcoin from 'bitcoinjs-lib';
// import { Wallet as OylWallet } from '@oyl/sdk';  // Uncomment when @oyl/sdk is installed
import init, * as wasm from '../build/wasm/alkanes_web_sys';

/**
 * Step 1: Initialize WASM module
 * Call this once at application startup
 */
async function initializeWasm() {
  console.log('🔧 Initializing WASM module...');
  await init();
  console.log('✅ WASM initialized');
}

/**
 * Step 2: Create or restore wallet keystore
 */
async function setupKeystore(password: string, existingKeystore?: string) {
  if (existingKeystore) {
    // Restore existing keystore
    console.log('🔓 Unlocking existing keystore...');
    const keystore = await unlockKeystore(existingKeystore, password);
    console.log('✅ Keystore unlocked');
    return { keystore, mnemonic: null };
  } else {
    // Create new keystore
    console.log('🔑 Creating new keystore...');
    const { keystore, mnemonic } = await createKeystore(password, {
      network: 'regtest',
    }, 12);
    
    console.log('✅ Keystore created');
    console.log('⚠️  IMPORTANT: Save this mnemonic securely!');
    console.log('Mnemonic:', mnemonic);
    console.log('\nEncrypted Keystore (can be stored):');
    console.log(keystore);
    
    return { keystore: JSON.parse(keystore), mnemonic };
  }
}

/**
 * Step 3: Create Alkanes wallet from keystore
 */
function createAlkanesWallet(keystoreData: any) {
  console.log('\n💼 Creating Alkanes wallet...');
  const wallet = createWallet(keystoreData);
  
  // Generate addresses
  const p2wpkh = wallet.getReceivingAddress(0);
  const p2tr = wallet.deriveAddress('p2tr' as any, 0, 0);
  
  console.log('✅ Wallet created');
  console.log('P2WPKH Address:', p2wpkh);
  console.log('P2TR Address:', p2tr.address);
  
  return wallet;
}

/**
 * Step 4: Create Alkanes provider for @oyl/sdk
 */
function createAlkanesProvider(): AlkanesProvider {
  console.log('\n🌐 Creating Alkanes provider...');
  
  const provider = createProvider({
    url: 'http://localhost:18443',  // Bitcoin Core regtest RPC
    network: bitcoin.networks.regtest,
    networkType: 'regtest',
  }, wasm);  // Pass WASM module for alkanes features
  
  console.log('✅ Provider created');
  return provider;
}

/**
 * Step 5: Test provider functionality
 */
async function testProvider(provider: AlkanesProvider, address: string) {
  console.log('\n🧪 Testing provider...');
  
  try {
    // Test block info
    const blockCount = await provider.bitcoin.getBlockCount();
    console.log('Current block height:', blockCount);
    
    // Test balance
    const balance = await provider.getBalance(address);
    console.log('Address balance:', {
      confirmed: balance.confirmed,
      unconfirmed: balance.unconfirmed,
      utxos: balance.utxos?.length || 0,
    });
    
    console.log('✅ Provider tests passed');
  } catch (error) {
    console.error('❌ Provider test failed:', error);
  }
}

/**
 * Step 6: Integrate with @oyl/sdk (example)
 * 
 * Uncomment when @oyl/sdk is installed
 */
/*
async function createOylWallet(
  alkanesWallet: any,
  provider: AlkanesProvider,
  address: string
) {
  console.log('\n🔗 Creating @oyl/sdk wallet...');
  
  const oylWallet = new OylWallet({
    provider: provider as any,  // Alkanes provider is compatible
    address,
    signer: async (psbtBase64: string) => {
      // Use Alkanes wallet for signing
      console.log('📝 Signing PSBT with Alkanes wallet...');
      return alkanesWallet.signPsbt(psbtBase64);
    },
  });
  
  console.log('✅ @oyl/sdk wallet created');
  
  // Sync wallet
  await oylWallet.sync();
  console.log('✅ Wallet synced');
  
  return oylWallet;
}

async function sendTransaction(oylWallet: any, toAddress: string, amount: number) {
  console.log('\n💸 Sending transaction...');
  console.log('To:', toAddress);
  console.log('Amount:', amount, 'sats');
  
  try {
    const result = await oylWallet.send({
      to: toAddress,
      amount,
    });
    
    console.log('✅ Transaction sent!');
    console.log('TX ID:', result.txId);
    console.log('Fee:', result.fee, 'sats');
    
    return result;
  } catch (error) {
    console.error('❌ Transaction failed:', error);
    throw error;
  }
}
*/

/**
 * Step 7: Alkanes-specific features
 */
async function testAlkanesFeatures(provider: AlkanesProvider, address: string) {
  console.log('\n🧬 Testing Alkanes features...');
  
  try {
    // Example alkane ID (block:tx)
    const alkaneId = { block: 840000, tx: 1 };
    
    // Get alkane balance
    const alkaneBalance = await provider.getAlkaneBalance(address, alkaneId);
    console.log('Alkane balance:', alkaneBalance);
    
    // Get alkane bytecode
    const bytecode = await provider.alkanes.getAlkaneBytecode(alkaneId);
    console.log('Alkane bytecode length:', bytecode.length);
    
    console.log('✅ Alkanes features working');
  } catch (error) {
    console.warn('⚠️  Alkanes features test skipped (requires alkanes node):', error.message);
  }
}

/**
 * Main integration example
 */
async function main() {
  console.log('🚀 Alkanes + @oyl/sdk Integration Example\n');
  console.log('=====================================\n');
  
  // Configuration
  const PASSWORD = 'secure-password-123';
  const EXISTING_KEYSTORE = process.env.KEYSTORE_JSON;  // Optional: restore from env
  
  try {
    // 1. Initialize WASM
    await initializeWasm();
    
    // 2. Setup keystore
    const { keystore, mnemonic } = await setupKeystore(PASSWORD, EXISTING_KEYSTORE);
    
    // 3. Create Alkanes wallet
    const alkanesWallet = createAlkanesWallet(keystore);
    const address = alkanesWallet.getReceivingAddress(0);
    
    // 4. Create provider
    const provider = createAlkanesProvider();
    
    // 5. Test provider
    await testProvider(provider, address);
    
    // 6. Test alkanes features
    await testAlkanesFeatures(provider, address);
    
    // 7. Integrate with @oyl/sdk (uncomment when installed)
    /*
    const oylWallet = await createOylWallet(alkanesWallet, provider, address);
    
    // Example: Send transaction
    const recipientAddress = 'bcrt1q...';  // Replace with actual address
    const amount = 10000;  // 10k sats
    await sendTransaction(oylWallet, recipientAddress, amount);
    */
    
    console.log('\n✅ Integration example completed!');
    console.log('\n📝 Next steps:');
    console.log('1. Install @oyl/sdk: npm install @oyl/sdk');
    console.log('2. Uncomment @oyl/sdk integration code');
    console.log('3. Start Bitcoin Core in regtest mode');
    console.log('4. Generate test blocks and fund addresses');
    console.log('5. Test sending transactions');
    
  } catch (error) {
    console.error('\n❌ Error:', error);
    process.exit(1);
  }
}

/**
 * Helper: Setup regtest environment
 */
export function printRegtestSetupInstructions() {
  console.log('\n📋 Regtest Setup Instructions:\n');
  console.log('1. Start Bitcoin Core in regtest mode:');
  console.log('   bitcoind -regtest -daemon -rpcuser=user -rpcpassword=pass -rpcport=18443\n');
  
  console.log('2. Create a wallet:');
  console.log('   bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass createwallet "test"\n');
  
  console.log('3. Generate blocks to get funds:');
  console.log('   bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass generatetoaddress 101 $(bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass getnewaddress)\n');
  
  console.log('4. Send test funds to your wallet address:');
  console.log('   bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass sendtoaddress <your_address> 1.0\n');
  
  console.log('5. Mine a block to confirm:');
  console.log('   bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass generatetoaddress 1 $(bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass getnewaddress)\n');
}

// Run the example
if (require.main === module) {
  printRegtestSetupInstructions();
  main().catch(console.error);
}

export {
  initializeWasm,
  setupKeystore,
  createAlkanesWallet,
  createAlkanesProvider,
  testProvider,
  testAlkanesFeatures,
};
