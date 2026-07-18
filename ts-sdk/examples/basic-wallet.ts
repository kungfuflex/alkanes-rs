/**
 * Basic wallet example
 * 
 * Demonstrates creating a wallet, generating addresses, and managing keystores.
 */

import {
  createKeystore,
  unlockKeystore,
  createWallet,
  AddressType,
} from '../src';

async function main() {
  console.log('🚀 Alkanes SDK - Basic Wallet Example\n');

  // 1. Create a new encrypted keystore
  console.log('1️⃣  Creating new keystore...');
  const { keystore, mnemonic } = await createKeystore('MySecurePassword123!', {
    network: 'regtest',
  }, 12);

  console.log('✅ Keystore created!');
  console.log('📝 Mnemonic (SAVE THIS SECURELY):', mnemonic);
  console.log('💾 Keystore JSON (first 100 chars):', keystore.substring(0, 100) + '...\n');

  // 2. Unlock the keystore
  console.log('2️⃣  Unlocking keystore...');
  const unlockedKeystore = await unlockKeystore(keystore, 'MySecurePassword123!');
  console.log('✅ Keystore unlocked!');
  console.log('🔑 Master Fingerprint:', unlockedKeystore.masterFingerprint);
  console.log('📊 Account xPub:', unlockedKeystore.accountXpub.substring(0, 20) + '...\n');

  // 3. Create wallet
  console.log('3️⃣  Creating wallet...');
  const wallet = createWallet(unlockedKeystore);
  console.log('✅ Wallet created!\n');

  // 4. Generate addresses
  console.log('4️⃣  Generating addresses...');
  
  // SegWit (P2WPKH) - Most common
  const p2wpkh = wallet.deriveAddress(AddressType.P2WPKH, 0);
  console.log('📬 SegWit Address (P2WPKH):', p2wpkh.address);
  console.log('   Path:', p2wpkh.path);
  
  // Taproot (P2TR) - Latest standard
  const p2tr = wallet.deriveAddress(AddressType.P2TR, 0);
  console.log('📬 Taproot Address (P2TR):', p2tr.address);
  console.log('   Path:', p2tr.path);
  
  // Legacy (P2PKH) - For compatibility
  const p2pkh = wallet.deriveAddress(AddressType.P2PKH, 0);
  console.log('📬 Legacy Address (P2PKH):', p2pkh.address);
  console.log('   Path:', p2pkh.path);
  console.log();

  // 5. Get multiple addresses
  console.log('5️⃣  Generating first 5 receiving addresses...');
  const addresses = wallet.getAddresses(0, 5, AddressType.P2WPKH);
  addresses.forEach((addr, i) => {
    console.log(`   [${i}] ${addr.address}`);
  });
  console.log();

  // 6. Sign a message
  console.log('6️⃣  Signing a message...');
  const message = 'Hello Alkanes!';
  const signature = wallet.signMessage(message, 0);
  console.log(`   Message: "${message}"`);
  console.log(`   Signature: ${signature.substring(0, 40)}...`);
  console.log();

  console.log('✅ All operations completed successfully!');
}

main().catch(console.error);
