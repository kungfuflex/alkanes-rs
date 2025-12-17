/**
 * BRC20-prog Usage Example
 *
 * This example shows how to use the @alkanes/ts-sdk to interact with BRC20-prog contracts
 * using the clean, object-based TypeScript API (no JSON strings!).
 *
 * The SDK follows the ethers.js pattern with separate Provider and Signer concepts:
 * - Provider: Read-only blockchain access (fetching data)
 * - Signer: Transaction signing capability (sending transactions)
 * - AlkanesClient: Combines both for full functionality
 */

import {
  AlkanesClient,
  connectWallet,
  Brc20ProgDeployParams,
  Brc20ProgTransactParams,
  Brc20ProgWrapBtcParams,
  Brc20ProgExecuteResult,
} from '@alkanes/ts-sdk';

// ============================================================================
// 1. CONNECTING TO A WALLET
// ============================================================================

/**
 * Connect to a browser wallet (Unisat, Xverse, OKX, etc.)
 */
async function connectToBrowserWallet() {
  // Simple connection
  const client = await connectWallet('unisat');

  // Or connect to any available wallet
  // const client = await connectAnyWallet();

  // Or create manually for more control
  // const client = await AlkanesClient.withBrowserWallet('xverse', 'mainnet');

  const address = await client.getAddress();
  console.log('Connected address:', address);

  return client;
}

/**
 * Connect with a keystore (in-memory HD wallet)
 */
async function connectWithKeystore(keystoreJson: string, password: string) {
  const client = await AlkanesClient.withKeystore(
    keystoreJson,
    password,
    'regtest'  // or 'mainnet', 'testnet', 'signet'
  );

  const address = await client.getAddress();
  console.log('Keystore address:', address);

  return client;
}

// ============================================================================
// 2. DEPLOYING A BRC20-PROG CONTRACT
// ============================================================================

/**
 * Deploy a contract from Foundry build output
 *
 * The SDK accepts TypeScript objects - no JSON string manipulation needed!
 */
async function deployContract(client: AlkanesClient) {
  // Read your Foundry build output (out/MyContract.sol/MyContract.json)
  const foundryBuild = {
    bytecode: {
      object: "0x608060405234801561001057600080fd5b50..."
    }
  };

  // Deploy with clean TypeScript API
  const params: Brc20ProgDeployParams = {
    foundry_json: foundryBuild,  // Can be object or string
    fee_rate: 100,               // sat/vB
    use_activation: false,       // Use 2-tx or 3-tx pattern
  };

  const result: Brc20ProgExecuteResult = await client.deployBrc20ProgContract(params);

  console.log('Contract deployed!');
  console.log('Commit TXID:', result.commit_txid);
  console.log('Reveal TXID:', result.reveal_txid);
  console.log('Fees:', result.commit_fee + result.reveal_fee, 'sats');

  return result;
}

/**
 * Deploy with advanced options (Rebar Shield, Slipstream)
 */
async function deployWithAdvancedOptions(client: AlkanesClient, foundryOutput: any) {
  const params: Brc20ProgDeployParams = {
    foundry_json: foundryOutput,
    fee_rate: 200,
    use_activation: false,
    // Use Rebar Shield for private relay (prevents frontrunning)
    use_rebar: true,
    rebar_tier: 2,  // Tier 2 = ~16% hashrate coverage
  };

  const result = await client.deployBrc20ProgContract(params);
  return result;
}

/**
 * Resume a failed deployment
 *
 * The smart resume feature auto-detects if you pass a commit or reveal txid!
 */
async function resumeDeployment(
  client: AlkanesClient,
  txid: string,
  foundryBuild: any
) {
  const params: Brc20ProgDeployParams = {
    foundry_json: foundryBuild,
    fee_rate: 100,
    resume_from_commit: txid,  // Can be EITHER commit OR reveal txid!
  };

  // The system will auto-detect and resume from the correct point
  const result = await client.deployBrc20ProgContract(params);

  console.log('Deployment resumed and completed!');
  return result;
}

// ============================================================================
// 3. CALLING CONTRACT FUNCTIONS (TRANSACT)
// ============================================================================

/**
 * Call a contract function with clean TypeScript API
 */
async function callContractFunction(client: AlkanesClient, contractAddress: string) {
  const params: Brc20ProgTransactParams = {
    contract_address: contractAddress,
    function_signature: "transfer(address,uint256)",
    // Calldata can be an array (recommended) or comma-separated string
    calldata: [
      "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb0",  // recipient
      "1000"                                          // amount
    ],
    fee_rate: 100,
  };

  const result: Brc20ProgExecuteResult = await client.transactBrc20Prog(params);

  console.log('Transaction sent!');
  console.log('Activation TXID:', result.activation_txid);
  return result;
}

/**
 * Call a more complex function
 */
async function callComplexFunction(client: AlkanesClient, contractAddress: string) {
  const params: Brc20ProgTransactParams = {
    contract_address: contractAddress,
    function_signature: "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)",
    calldata: [
      "1000000",           // amountIn
      "950000",            // amountOutMin
      "[0xToken1,0xToken2]",  // path
      "0xRecipient",       // to
      "1735689600"         // deadline
    ],
    fee_rate: 150,
    use_rebar: true,
    rebar_tier: 2,
  };

  const result = await client.transactBrc20Prog(params);
  return result;
}

// ============================================================================
// 4. WRAPPING BTC INTO FRBTC
// ============================================================================

/**
 * Wrap BTC into frBTC and execute a contract call atomically
 */
async function wrapAndExecute(client: AlkanesClient) {
  const params: Brc20ProgWrapBtcParams = {
    amount: 100000,  // 100k sats
    target_contract: "0xYourDeFiContract",
    function_signature: "deposit(uint256)",
    calldata: ["100000"],
    fee_rate: 100,
  };

  const result: Brc20ProgExecuteResult = await client.wrapBtc(params);

  console.log('BTC wrapped and contract executed!');
  console.log('Reveal TXID:', result.reveal_txid);

  return result;
}

export {
  connectToBrowserWallet,
  connectWithKeystore,
  deployContract,
  deployWithAdvancedOptions,
  resumeDeployment,
  callContractFunction,
  callComplexFunction,
  wrapAndExecute,
};
