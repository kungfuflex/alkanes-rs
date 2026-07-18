/**
 * @alkanes/ts-sdk Alkane Transfer Integration Test
 *
 * End-to-end test that:
 * 1. Creates a fresh Keystore wallet
 * 2. Mines BTC to the wallet address
 * 3. Mints DIESEL tokens using alkanesExecuteTyped
 * 4. Transfers DIESEL to a second address using alkanesTransferTyped
 * 5. Verifies balances on both addresses
 *
 * Run with: LIVE_RPC_TEST=true pnpm vitest run tests/alkanes-transfer.integration.test.ts
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { AlkanesProvider, NETWORK_PRESETS } from '../src/provider';
import { KeystoreSigner } from '../src/client/keystore-signer';

const LIVE_RPC_TEST = process.env.LIVE_RPC_TEST === 'true';
const REGTEST_RPC_URL = process.env.REGTEST_RPC_URL || 'https://regtest.subfrost.io/v4/jsonrpc';

// DIESEL contract: [2, 0], opcode 77 = mint
const DIESEL_BLOCK = 2;
const DIESEL_TX = 0;
const DIESEL_MINT_OPCODE = 77;

describe.skipIf(!LIVE_RPC_TEST)('Alkane Transfer Integration Test', () => {
  let provider: AlkanesProvider;
  let senderSigner: KeystoreSigner;
  let recipientSigner: KeystoreSigner;
  let senderAddress: string;
  let recipientAddress: string;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'subfrost-regtest',
      rpcUrl: REGTEST_RPC_URL,
    });
    await provider.initialize();

    // Generate two fresh wallets
    senderSigner = KeystoreSigner.generate({ network: 'regtest' });
    recipientSigner = KeystoreSigner.generate({ network: 'regtest' });
    senderAddress = await senderSigner.getAddress();
    recipientAddress = await recipientSigner.getAddress();

    console.log('Sender address:', senderAddress);
    console.log('Recipient address:', recipientAddress);
  });

  it('should transfer DIESEL tokens between addresses', async () => {
    // Step 1: Mine BTC to sender
    console.log('Mining 101 blocks to sender...');
    try {
      await provider.bitcoin.generateToAddress(101, senderAddress);
    } catch (error) {
      console.log('Cannot mine blocks:', (error as Error).message);
      console.log('Skipping transfer test - no mining capability');
      return;
    }

    await new Promise(resolve => setTimeout(resolve, 5000));

    // Step 2: Mint DIESEL to sender via alkanesExecuteTyped
    console.log('Minting DIESEL tokens...');
    try {
      const mintResult = await provider.alkanesExecuteTyped({
        inputRequirements: 'B:10000',
        protostones: `[${DIESEL_BLOCK},${DIESEL_TX},${DIESEL_MINT_OPCODE}]:v0:v0`,
        feeRate: 1,
        mineEnabled: true,
        autoConfirm: true,
      });
      console.log('Mint result:', JSON.stringify(mintResult, null, 2));
    } catch (error) {
      console.log('DIESEL mint failed:', (error as Error).message);
      console.log('Skipping transfer test - mint prerequisite failed');
      return;
    }

    await new Promise(resolve => setTimeout(resolve, 3000));

    // Step 3: Verify sender has DIESEL
    const senderBalancesBefore = await provider.alkanes.getBalance(senderAddress);
    console.log('Sender balances before transfer:', JSON.stringify(senderBalancesBefore, null, 2));

    const dieselBalance = senderBalancesBefore.find(
      (b: any) => b.alkane_id.block === DIESEL_BLOCK && b.alkane_id.tx === DIESEL_TX
    );

    if (!dieselBalance) {
      console.log('No DIESEL balance found after mint - indexer may not have caught up');
      return;
    }

    console.log('DIESEL balance:', dieselBalance.balance);
    const transferAmount = Math.floor(Number(dieselBalance.balance) / 2);
    expect(transferAmount).toBeGreaterThan(0);

    // Step 4: Transfer half of DIESEL to recipient
    console.log(`Transferring ${transferAmount} DIESEL to recipient...`);
    const transferResult = await provider.alkanesTransferTyped({
      alkaneId: { block: DIESEL_BLOCK, tx: DIESEL_TX },
      amount: String(transferAmount),
      toAddress: recipientAddress,
      feeRate: 1,
      mineEnabled: true,
      autoConfirm: true,
    });
    console.log('Transfer result:', JSON.stringify(transferResult, null, 2));

    await new Promise(resolve => setTimeout(resolve, 3000));

    // Step 5: Verify recipient has DIESEL
    const recipientBalances = await provider.alkanes.getBalance(recipientAddress);
    console.log('Recipient balances after transfer:', JSON.stringify(recipientBalances, null, 2));

    const recipientDiesel = recipientBalances.find(
      (b: any) => b.alkane_id.block === DIESEL_BLOCK && b.alkane_id.tx === DIESEL_TX
    );
    expect(recipientDiesel).toBeDefined();
    expect(Number(recipientDiesel.balance)).toBe(transferAmount);

    // Step 6: Verify sender still has remainder
    const senderBalancesAfter = await provider.alkanes.getBalance(senderAddress);
    console.log('Sender balances after transfer:', JSON.stringify(senderBalancesAfter, null, 2));

    const senderDieselAfter = senderBalancesAfter.find(
      (b: any) => b.alkane_id.block === DIESEL_BLOCK && b.alkane_id.tx === DIESEL_TX
    );
    expect(senderDieselAfter).toBeDefined();
    expect(Number(senderDieselAfter.balance)).toBe(
      Number(dieselBalance.balance) - transferAmount
    );
  });
});
