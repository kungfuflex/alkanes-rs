#!/usr/bin/env node

/**
 * Alkanes Deployment Script for Regtest (using @alkanes/ts-sdk bindings)
 *
 * This script deploys all alkanes to a local regtest environment
 * Pattern follows deploy-regtest.sh but uses the TypeScript SDK
 *
 * Usage:
 *   node deploy-regtest-bindgen.js
 *   node deploy-regtest-bindgen.js --skip-wasms  # Skip WASM deployments, only setup wallet
 */

import { readFileSync, existsSync, readdirSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { homedir } from 'os';

// Get __dirname equivalent in ESM
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ============================================================================
// Configuration
// ============================================================================

const CONFIG = {
  WASM_DIR: join(__dirname, '../prod_wasms'),
  RPC_URL: process.env.RPC_URL || 'http://127.0.0.1:18888',
  NETWORK: 'regtest',
  WALLET_DIR: join(homedir(), '.alkanes'),
  WALLET_FILE: join(homedir(), '.alkanes', 'wallet.json'),

  // OYL AMM Constants (matching oyl-sdk deployment pattern)
  AUTH_TOKEN_FACTORY_ID: 65517,      // 0xffed
  POOL_BEACON_PROXY_TX: 780993,
  AMM_FACTORY_LOGIC_IMPL_TX: 65524,  // 0xfff4
  POOL_LOGIC_TX: 65520,              // 0xfff0
  AMM_FACTORY_PROXY_TX: 65522,       // 0xfff2 (upgradeable proxy)
  POOL_UPGRADEABLE_BEACON_TX: 65523, // 0xfff3

  // Reserved range addresses
  DX_BTC_ID: 0x1f00,
  YV_FR_BTC_VAULT_ID: 0x1f01,
  LBTC_YIELD_SPLITTER_ID: 0x1f10,
  PLBTC_ID: 0x1f11,
  YXLBTC_ID: 0x1f12,
  FROST_TOKEN_ID: 0x1f13,
  VX_FROST_GAUGE_ID: 0x1f14,
  SYNTH_POOL_ID: 0x1f15,
  LBTC_ORACLE_ID: 0x1f16,
  LBTC_ID: 0x1f17,
  UNIT_TEMPLATE_ID: 0x1f20,
  VE_TOKEN_VAULT_TEMPLATE_ID: 0x1f21,
  YVE_TOKEN_NFT_TEMPLATE_ID: 0x1f22,
  VX_TOKEN_GAUGE_TEMPLATE_ID: 0x1f23,

  // Test pool configuration
  DIESEL_ID: '2:0',
  FRBTC_ID: '32:0',
  DIESEL_AMOUNT: '300000000',  // 300M DIESEL
  FRBTC_AMOUNT: '50000',       // 0.0005 BTC in sats
};

// ============================================================================
// Logging Helpers
// ============================================================================

const COLORS = {
  reset: '\x1b[0m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
};

function logInfo(msg) {
  console.log(`${COLORS.blue}[INFO]${COLORS.reset} ${msg}`);
}

function logSuccess(msg) {
  console.log(`${COLORS.green}[SUCCESS]${COLORS.reset} ${msg}`);
}

function logError(msg) {
  console.log(`${COLORS.red}[ERROR]${COLORS.reset} ${msg}`);
}

function logWarn(msg) {
  console.log(`${COLORS.yellow}[WARN]${COLORS.reset} ${msg}`);
}

function logSection(title) {
  console.log('');
  logInfo('==========================================');
  logInfo(title);
  logInfo('==========================================');
  console.log('');
}

// ============================================================================
// Utility Functions
// ============================================================================

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function readWasmAsHex(wasmPath) {
  if (!existsSync(wasmPath)) {
    throw new Error(`WASM file not found: ${wasmPath}`);
  }
  const buffer = readFileSync(wasmPath);
  return buffer.toString('hex');
}

// ============================================================================
// SDK Setup
// ============================================================================

async function setupSDK() {
  // Dynamic import of the SDK
  const { AlkanesProvider, NETWORK_PRESETS } = await import('../ts-sdk/dist/index.mjs');

  // Create provider with regtest configuration
  const provider = new AlkanesProvider({
    ...NETWORK_PRESETS.regtest,
    jsonRpcUrl: CONFIG.RPC_URL,
  });

  await provider.initialize();

  return { provider };
}

// ============================================================================
// Pre-deployment Checks
// ============================================================================

async function checkRegtest(provider) {
  logInfo('Checking if regtest node is running...');
  try {
    const height = await provider.bitcoind.getBlockCount();
    logSuccess(`Regtest node is running at ${CONFIG.RPC_URL} (height: ${height})`);
    return true;
  } catch (err) {
    logError(`Cannot connect to regtest node at ${CONFIG.RPC_URL}`);
    logInfo('Please start the regtest node first:');
    logInfo('  cd alkanes-rs && docker-compose up -d');
    return false;
  }
}

function checkWasms() {
  logInfo('Checking if WASM files exist in prod_wasms...');

  if (!existsSync(CONFIG.WASM_DIR)) {
    logError(`WASM directory not found: ${CONFIG.WASM_DIR}`);
    return false;
  }

  const wasmFiles = readdirSync(CONFIG.WASM_DIR).filter(f => f.endsWith('.wasm'));
  if (wasmFiles.length === 0) {
    logError(`No WASM files found in ${CONFIG.WASM_DIR}`);
    return false;
  }

  logSuccess(`Found ${wasmFiles.length} WASM files in ${CONFIG.WASM_DIR}`);
  return true;
}

// ============================================================================
// Wallet Setup
// ============================================================================

function loadOrCreateWallet(provider) {
  logInfo('Setting up wallet...');

  // Create wallet directory if it doesn't exist
  if (!existsSync(CONFIG.WALLET_DIR)) {
    mkdirSync(CONFIG.WALLET_DIR, { recursive: true });
  }

  // Check if wallet file exists
  if (existsSync(CONFIG.WALLET_FILE)) {
    logInfo('Loading existing wallet...');
    try {
      const walletData = JSON.parse(readFileSync(CONFIG.WALLET_FILE, 'utf8'));
      provider.walletLoadMnemonic(walletData.mnemonic);
      const addresses = provider.walletGetAddresses('p2tr', 0, 1);
      logSuccess(`Loaded wallet with address: ${addresses[0].address}`);
      return addresses[0].address;
    } catch (err) {
      logWarn(`Failed to load wallet: ${err.message}`);
      logInfo('Creating new wallet...');
    }
  }

  // Create new wallet using SDK binding
  logInfo('Creating new wallet via SDK...');
  const walletInfo = provider.walletCreate();

  // Save wallet to file
  const walletData = {
    mnemonic: walletInfo.mnemonic,
    network: CONFIG.NETWORK,
    created_at: new Date().toISOString(),
  };
  writeFileSync(CONFIG.WALLET_FILE, JSON.stringify(walletData, null, 2));

  logSuccess(`Wallet created and saved to: ${CONFIG.WALLET_FILE}`);
  logInfo(`Address (p2tr:0): ${walletInfo.address}`);
  console.log('');
  console.log(`${COLORS.yellow}IMPORTANT: Save your mnemonic!${COLORS.reset}`);
  console.log(`${COLORS.cyan}${walletInfo.mnemonic}${COLORS.reset}`);
  console.log('');

  return walletInfo.address;
}

async function fundWallet(provider, walletAddress) {
  logInfo('Checking if wallet needs funding...');

  // Check UTXO count
  const utxos = await provider.esplora.getAddressUtxos(walletAddress);

  if (utxos.length > 0) {
    logSuccess(`Wallet already funded with ${utxos.length} UTXOs`);
    return;
  }

  logInfo('No UTXOs found, mining blocks to fund wallet...');
  logInfo('Mining 400 blocks to wallet address...');

  await provider.bitcoind.call('generatetoaddress', [400, walletAddress]);

  logInfo('Waiting for indexer to sync blocks (15 seconds)...');
  await sleep(15000);

  logSuccess('Wallet funded! Ready for deployments');
}

async function ensureCoinbaseMaturity(provider, walletAddress) {
  logInfo('Ensuring coinbase maturity (mining 101 blocks)...');

  await provider.bitcoind.call('generatetoaddress', [101, walletAddress]);

  logInfo('Waiting for indexer to sync (10 seconds)...');
  await sleep(10000);

  logSuccess('Coinbase outputs matured');
}

// ============================================================================
// Contract Deployment
// ============================================================================

async function deployContract(provider, walletAddress, contractName, wasmFile, targetTx, initArgs = '') {
  logInfo(`Deploying ${contractName} using [3, ${targetTx}] -> will create at [4, ${targetTx}]...`);

  const wasmPath = join(CONFIG.WASM_DIR, wasmFile);
  if (!existsSync(wasmPath)) {
    logError(`WASM file not found: ${wasmPath}`);
    return false;
  }

  // Read WASM as hex
  const envelopeHex = readWasmAsHex(wasmPath);

  // Build protostone: [3,tx,init_args...]:v0:v0 for deployment
  const protostone = initArgs
    ? `[3,${targetTx},${initArgs}]:v0:v0`
    : `[3,${targetTx}]:v0:v0`;

  logInfo(`  Protostone: ${protostone}`);
  logInfo(`  Envelope size: ${envelopeHex.length / 2} bytes`);

  try {
    // Execute deployment using typed method
    const result = await provider.alkanesExecuteTyped({
      toAddresses: [walletAddress],
      inputRequirements: '',  // No input requirements for deployment
      protostones: protostone,
      feeRate: 1,
      envelopeHex: envelopeHex,
      autoConfirm: true,
      mineEnabled: true,  // Auto-mine on regtest
    });

    logSuccess(`${contractName} deployed to [4, ${targetTx}]`);
    logInfo(`  Reveal TXID: ${result.reveal_txid || result.txid || 'N/A'}`);

    // Wait for metashrew to index
    logInfo('Waiting for metashrew to index (5 seconds)...');
    await sleep(5000);

    // Verify deployment
    await verifyDeployment(provider, contractName, targetTx);

    return true;
  } catch (err) {
    logError(`Failed to deploy ${contractName}: ${err.message}`);
    return false;
  }
}

async function verifyDeployment(provider, contractName, targetTx) {
  logInfo(`Verifying ${contractName} deployment at [4, ${targetTx}]...`);

  for (let i = 0; i < 3; i++) {
    try {
      const bytecode = await provider.alkanes.getBytecode(`4:${targetTx}`);
      if (bytecode && bytecode !== 'null' && bytecode !== '""' && bytecode.length > 0) {
        const bytecodeSize = typeof bytecode === 'string' ? bytecode.length / 2 : 0;
        logSuccess(`Bytecode verified at [4, ${targetTx}] (${bytecodeSize} bytes)`);
        return true;
      }
    } catch (err) {
      // Ignore errors, retry
    }

    if (i < 2) {
      logInfo('Bytecode not found yet, waiting 2 seconds...');
      await sleep(2000);
    }
  }

  logError(`Bytecode verification failed for ${contractName} at [4, ${targetTx}]`);
  return false;
}

// ============================================================================
// Token Operations
// ============================================================================

async function mineDiesel(provider, walletAddress) {
  logInfo('Mining DIESEL tokens...');

  try {
    const result = await provider.alkanesExecuteTyped({
      toAddresses: [walletAddress],
      inputRequirements: '',
      protostones: '[2,0,77]:v0:v0',  // Call DIESEL mint (opcode 77)
      feeRate: 1,
      autoConfirm: true,
      mineEnabled: true,
    });

    logSuccess('DIESEL mined');
    return true;
  } catch (err) {
    logError(`Failed to mine DIESEL: ${err.message}`);
    return false;
  }
}

async function wrapBtc(provider, walletAddress, amount) {
  logInfo(`Wrapping ${amount} sats to frBTC...`);

  try {
    const result = await provider.frbtcWrapTyped({
      amount: BigInt(amount),
      toAddress: walletAddress,
      fromAddress: walletAddress,
      feeRate: 1,
      autoConfirm: true,
    });

    logSuccess('frBTC wrapped');
    return true;
  } catch (err) {
    logError(`Failed to wrap frBTC: ${err.message}`);
    return false;
  }
}

// ============================================================================
// AMM Pool Operations
// ============================================================================

async function initializeFactory(provider, walletAddress) {
  logInfo('Initializing OYL Factory with InitFactory opcode...');
  logInfo('This requires spending auth token [2:1] to authenticate the call...');

  const protostone = `[4,${CONFIG.AMM_FACTORY_PROXY_TX},0,${CONFIG.POOL_BEACON_PROXY_TX},4,${CONFIG.POOL_UPGRADEABLE_BEACON_TX}]:v0:v0`;
  logInfo(`  Protostone: ${protostone}`);
  logInfo('  Opcode 0 = InitFactory(pool_beacon_proxy_id, pool_beacon_id)');

  try {
    const result = await provider.alkanesExecuteTyped({
      toAddresses: [walletAddress],
      inputRequirements: '2:1:1',  // Input: 1 auth token from 2:1
      protostones: protostone,
      feeRate: 1,
      autoConfirm: true,
      mineEnabled: true,
      traceEnabled: true,
    });

    logSuccess('OYL Factory initialized successfully!');

    logInfo('Waiting for metashrew to index (5 seconds)...');
    await sleep(5000);

    return true;
  } catch (err) {
    logError(`Failed to initialize OYL Factory: ${err.message}`);
    return false;
  }
}

async function createTestPool(provider, walletAddress) {
  logSection('Creating Test Pool (DIESEL/frBTC)');

  // Mine DIESEL
  if (!await mineDiesel(provider, walletAddress)) {
    return false;
  }

  // Wrap BTC for frBTC
  if (!await wrapBtc(provider, walletAddress, 100000000)) {
    return false;
  }

  // Mine a block to confirm
  logInfo('Mining a block to confirm transactions...');
  await provider.bitcoind.call('generatetoaddress', [1, walletAddress]);

  logInfo('Waiting for metashrew to index (15 seconds)...');
  await sleep(15000);

  // Create the pool using typed method
  logInfo('Creating DIESEL/frBTC pool...');

  try {
    const txid = await provider.alkanesInitPoolTyped({
      factoryId: { block: 4, tx: CONFIG.AMM_FACTORY_PROXY_TX },
      token0: { block: 2, tx: 0 },   // DIESEL
      token1: { block: 32, tx: 0 },  // frBTC
      amount0: CONFIG.DIESEL_AMOUNT,
      amount1: CONFIG.FRBTC_AMOUNT,
      toAddress: walletAddress,
      fromAddress: walletAddress,
      feeRate: 1,
      trace: true,
      autoConfirm: true,
    });

    logSuccess('Pool created successfully!');
    logInfo(`  TXID: ${txid}`);

    logInfo('Waiting for metashrew to index (5 seconds)...');
    await sleep(5000);

    return true;
  } catch (err) {
    logError(`Failed to create pool: ${err.message}`);
    return false;
  }
}

// ============================================================================
// Main Deployment Process
// ============================================================================

async function main() {
  const skipWasms = process.argv.includes('--skip-wasms');

  logSection('Alkanes Regtest Deployment (ts-sdk)');

  // Setup SDK
  logInfo('Initializing @alkanes/ts-sdk...');
  let sdk;
  try {
    sdk = await setupSDK();
  } catch (err) {
    logError(`Failed to initialize SDK: ${err.message}`);
    process.exit(1);
  }

  const { provider } = sdk;

  // Pre-deployment checks
  if (!await checkRegtest(provider)) {
    process.exit(1);
  }

  if (!skipWasms && !checkWasms()) {
    process.exit(1);
  }

  // Wallet setup using SDK bindings
  const walletAddress = loadOrCreateWallet(provider);
  await fundWallet(provider, walletAddress);
  await ensureCoinbaseMaturity(provider, walletAddress);

  if (skipWasms) {
    logSuccess('Wallet setup complete (skipping WASM deployments)');
    process.exit(0);
  }

  // Deploy contracts
  logSection('Starting Contract Deployments');

  // Genesis info
  logSection('Genesis Contracts (auto-deployed by alkanes-rs)');
  logInfo('  - Genesis Alkane at [1, 0]');
  logInfo('  - DIESEL at [2, 0]');
  logInfo('  - frBTC at [32, 0]');
  logInfo('  - frSIGIL at [32, 1]');
  logInfo('  - ftrBTC Master at [31, 0]');

  // Phase 1: Core Infrastructure
  logSection('Phase 1: Core Infrastructure');

  await deployContract(provider, walletAddress, 'dxBTC',
    'dx_btc.wasm', CONFIG.DX_BTC_ID,
    `0,32,0,4,${CONFIG.YV_FR_BTC_VAULT_ID},4,${CONFIG.YVE_TOKEN_NFT_TEMPLATE_ID},4,${CONFIG.VX_FROST_GAUGE_ID}`
  );

  await deployContract(provider, walletAddress, 'yv-fr-btc Vault',
    'yv_fr_btc_vault.wasm', CONFIG.YV_FR_BTC_VAULT_ID,
    `0,4,${CONFIG.YV_FR_BTC_VAULT_ID},2,1,2,2,2,3`
  );

  // Phase 2: LBTC Yield System
  logSection('Phase 2: LBTC Yield System');

  await deployContract(provider, walletAddress, 'LBTC Yield Splitter',
    'lbtc_yield_splitter.wasm', CONFIG.LBTC_YIELD_SPLITTER_ID,
    `0,4,${CONFIG.LBTC_ID},4,${CONFIG.PLBTC_ID},4,${CONFIG.YXLBTC_ID},1000000`
  );

  await deployContract(provider, walletAddress, 'pLBTC (Principal LBTC)',
    'p_lbtc.wasm', CONFIG.PLBTC_ID,
    `0,4,${CONFIG.LBTC_YIELD_SPLITTER_ID}`
  );

  await deployContract(provider, walletAddress, 'yxLBTC (Yield LBTC)',
    'yx_lbtc.wasm', CONFIG.YXLBTC_ID,
    `0,4,${CONFIG.LBTC_YIELD_SPLITTER_ID}`
  );

  await deployContract(provider, walletAddress, 'FROST Token',
    'frost_token.wasm', CONFIG.FROST_TOKEN_ID,
    `0,1000000000000000000,4,${CONFIG.DX_BTC_ID}`
  );

  await deployContract(provider, walletAddress, 'vxFROST Gauge',
    'vx_frost_gauge.wasm', CONFIG.VX_FROST_GAUGE_ID,
    `0,4,${CONFIG.FROST_TOKEN_ID}`
  );

  await deployContract(provider, walletAddress, 'Synth Pool (pLBTC/frBTC)',
    'synth_pool.wasm', CONFIG.SYNTH_POOL_ID,
    '0'
  );

  // Phase 3: LBTC Oracle System
  logSection('Phase 3: LBTC Oracle System');

  await deployContract(provider, walletAddress, 'LBTC Oracle',
    'unit.wasm', CONFIG.LBTC_ORACLE_ID,
    '0,1000000000000'
  );

  await deployContract(provider, walletAddress, 'LBTC Token',
    'lbtc.wasm', CONFIG.LBTC_ID,
    `0,4,${CONFIG.LBTC_ORACLE_ID}`
  );

  // Phase 4: Template Contracts
  logSection('Phase 4: Template Contracts');

  await deployContract(provider, walletAddress, 'Unit Template',
    'unit.wasm', CONFIG.UNIT_TEMPLATE_ID,
    '0,0'
  );

  await deployContract(provider, walletAddress, 'VE Token Vault Template',
    've_token_vault_template.wasm', CONFIG.VE_TOKEN_VAULT_TEMPLATE_ID,
    '0'
  );

  await deployContract(provider, walletAddress, 'YVE Token NFT Template',
    'yve_token_nft_template.wasm', CONFIG.YVE_TOKEN_NFT_TEMPLATE_ID,
    '0'
  );

  await deployContract(provider, walletAddress, 'VX Token Gauge Template',
    'vx_token_gauge_template.wasm', CONFIG.VX_TOKEN_GAUGE_TEMPLATE_ID,
    '0'
  );

  // Phase 6: OYL AMM System
  logSection('Phase 6: OYL AMM System');

  await deployContract(provider, walletAddress, 'OYL Auth Token Factory',
    'alkanes_std_auth_token.wasm', CONFIG.AUTH_TOKEN_FACTORY_ID,
    '100'
  );

  await deployContract(provider, walletAddress, 'OYL Beacon Proxy',
    'alkanes_std_beacon_proxy.wasm', CONFIG.POOL_BEACON_PROXY_TX,
    '36863'
  );

  await deployContract(provider, walletAddress, 'OYL Factory Logic',
    'factory.wasm', CONFIG.AMM_FACTORY_LOGIC_IMPL_TX,
    '50'
  );

  await deployContract(provider, walletAddress, 'OYL Pool Logic',
    'pool.wasm', CONFIG.POOL_LOGIC_TX,
    '50'
  );

  await deployContract(provider, walletAddress, 'OYL Factory Proxy (Upgradeable)',
    'alkanes_std_upgradeable.wasm', CONFIG.AMM_FACTORY_PROXY_TX,
    `${0x7fff},4,${CONFIG.AMM_FACTORY_LOGIC_IMPL_TX},5`
  );

  await deployContract(provider, walletAddress, 'OYL Upgradeable Beacon',
    'alkanes_std_upgradeable_beacon.wasm', CONFIG.POOL_UPGRADEABLE_BEACON_TX,
    `${0x7fff},4,${CONFIG.POOL_LOGIC_TX},5`
  );

  // Initialize Factory
  await initializeFactory(provider, walletAddress);

  // Create test pool
  await createTestPool(provider, walletAddress);

  // Summary
  logSection('Deployment Summary');

  logSuccess('All contracts deployed successfully!');
  console.log('');
  console.log('Deployed Alkanes:');
  console.log('');
  console.log('Genesis (Auto-deployed):');
  console.log('  - DIESEL:                 [2, 0]');
  console.log('  - frBTC:                  [32, 0]');
  console.log('');
  console.log('Core Contracts:');
  console.log(`  - dxBTC:                  [4, ${CONFIG.DX_BTC_ID}]`);
  console.log(`  - yv-fr-btc Vault:        [4, ${CONFIG.YV_FR_BTC_VAULT_ID}]`);
  console.log('');
  console.log('LBTC System:');
  console.log(`  - LBTC Yield Splitter:    [4, ${CONFIG.LBTC_YIELD_SPLITTER_ID}]`);
  console.log(`  - pLBTC:                  [4, ${CONFIG.PLBTC_ID}]`);
  console.log(`  - yxLBTC:                 [4, ${CONFIG.YXLBTC_ID}]`);
  console.log(`  - FROST Token:            [4, ${CONFIG.FROST_TOKEN_ID}]`);
  console.log(`  - vxFROST Gauge:          [4, ${CONFIG.VX_FROST_GAUGE_ID}]`);
  console.log(`  - Synth Pool:             [4, ${CONFIG.SYNTH_POOL_ID}]`);
  console.log(`  - LBTC Oracle:            [4, ${CONFIG.LBTC_ORACLE_ID}]`);
  console.log(`  - LBTC Token:             [4, ${CONFIG.LBTC_ID}]`);
  console.log('');
  console.log('Templates:');
  console.log(`  - Unit Template:          [4, ${CONFIG.UNIT_TEMPLATE_ID}]`);
  console.log(`  - VE Token Vault:         [4, ${CONFIG.VE_TOKEN_VAULT_TEMPLATE_ID}]`);
  console.log(`  - YVE Token NFT:          [4, ${CONFIG.YVE_TOKEN_NFT_TEMPLATE_ID}]`);
  console.log(`  - VX Token Gauge:         [4, ${CONFIG.VX_TOKEN_GAUGE_TEMPLATE_ID}]`);
  console.log('');
  console.log('OYL AMM System:');
  console.log(`  - Auth Token Factory:     [4, ${CONFIG.AUTH_TOKEN_FACTORY_ID}]`);
  console.log(`  - Beacon Proxy:           [4, ${CONFIG.POOL_BEACON_PROXY_TX}]`);
  console.log(`  - Factory Logic:          [4, ${CONFIG.AMM_FACTORY_LOGIC_IMPL_TX}]`);
  console.log(`  - Pool Logic:             [4, ${CONFIG.POOL_LOGIC_TX}]`);
  console.log(`  - Factory Proxy:          [4, ${CONFIG.AMM_FACTORY_PROXY_TX}]`);
  console.log(`  - Upgradeable Beacon:     [4, ${CONFIG.POOL_UPGRADEABLE_BEACON_TX}]`);
  console.log('');
  console.log('Test Pool:');
  console.log(`  - DIESEL/frBTC Pool:      Created with ${CONFIG.DIESEL_AMOUNT} DIESEL / ${CONFIG.FRBTC_AMOUNT} frBTC`);
  console.log('');

  logSuccess('Deployment complete! Your regtest environment is ready.');
}

// Run
main().catch(err => {
  logError(`Deployment failed: ${err.message}`);
  console.error(err);
  process.exit(1);
});
