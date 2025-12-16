#!/usr/bin/env node
"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));

// src/cli/index.ts
var import_commander = require("commander");

// src/cli/utils/formatting.ts
var import_chalk = __toESM(require("chalk"));
var import_cli_table3 = __toESM(require("cli-table3"));
function formatOutput(data, options = {}) {
  const { raw = false, color = true } = options;
  if (raw) {
    return JSON.stringify(data, null, 2);
  }
  if (typeof data === "string" || typeof data === "number" || typeof data === "boolean") {
    return String(data);
  }
  return JSON.stringify(data, null, 2);
}
function success(message) {
  console.log(import_chalk.default.green("\u2713 ") + message);
}
function error(message) {
  console.error(import_chalk.default.red("\u2717 ") + message);
}
function info(message) {
  console.log(import_chalk.default.blue("\u2139 ") + message);
}
function createTable(headers) {
  return new import_cli_table3.default({
    head: headers.map((h) => import_chalk.default.cyan(h)),
    style: {
      head: [],
      // Don't apply default styling to headers
      border: []
    }
  });
}
function formatAddress(address, maxLength = 20) {
  if (address.length <= maxLength) {
    return address;
  }
  const start = Math.floor((maxLength - 3) / 2);
  const end = Math.ceil((maxLength - 3) / 2);
  return `${address.slice(0, start)}...${address.slice(-end)}`;
}
function formatBTC(satoshis) {
  const btc = Number(satoshis) / 1e8;
  return `${btc.toFixed(8)} BTC`;
}

// src/cli/commands/wallet.ts
var import_chalk2 = __toESM(require("chalk"));

// src/cli/utils/provider.ts
var path2 = __toESM(require("path"));

// src/cli/utils/config.ts
var fs = __toESM(require("fs"));
var path = __toESM(require("path"));
var os = __toESM(require("os"));
var DEFAULT_CONFIG_PATH = path.join(os.homedir(), ".alkanes", "config.json");
async function loadConfigFile(configPath) {
  const filePath = configPath || DEFAULT_CONFIG_PATH;
  try {
    if (fs.existsSync(filePath)) {
      const content = fs.readFileSync(filePath, "utf-8");
      return JSON.parse(content);
    }
  } catch (error2) {
    console.warn(`Warning: Could not load config from ${filePath}`);
  }
  return {};
}
function loadConfigFromEnv() {
  return {
    network: process.env.ALKANES_NETWORK,
    jsonrpcUrl: process.env.JSONRPC_URL || process.env.BITCOIN_RPC_URL,
    esploraUrl: process.env.ESPLORA_URL,
    metashrewUrl: process.env.METASHREW_URL || process.env.SANDSHREW_URL,
    walletFile: process.env.WALLET_FILE,
    subfrostApiKey: process.env.SUBFROST_API_KEY
  };
}
async function getConfig(configPath) {
  const fileConfig = await loadConfigFile(configPath);
  const envConfig = loadConfigFromEnv();
  const merged = {
    ...fileConfig,
    ...Object.fromEntries(
      Object.entries(envConfig).filter(([_, v]) => v !== void 0)
    )
  };
  return merged;
}
function expandPath(filePath) {
  if (filePath.startsWith("~")) {
    return path.join(os.homedir(), filePath.slice(1));
  }
  return filePath;
}

// src/cli/utils/provider.ts
var WebProvider = null;
async function loadWasmModule() {
  if (WebProvider) return WebProvider;
  const wasmPath = path2.join(process.cwd(), "node_modules", "@alkanes", "ts-sdk", "wasm", "alkanes_web_sys.js");
  try {
    const wasmModule = await import(wasmPath);
    WebProvider = wasmModule.WebProvider;
    return WebProvider;
  } catch {
    const relativePath = path2.join(__dirname, "..", "..", "..", "wasm", "alkanes_web_sys.js");
    const wasmModule = await import(relativePath);
    WebProvider = wasmModule.WebProvider;
    return WebProvider;
  }
}
async function createProvider(options) {
  const Provider = await loadWasmModule();
  const config = await getConfig();
  const network = options.network || config.network || "mainnet";
  const jsonrpcUrl = options.jsonrpcUrl || config.jsonrpcUrl;
  const esploraUrl = options.esploraUrl || config.esploraUrl;
  const metashrewUrl = options.metashrewUrl || config.metashrewUrl;
  const providerConfig = {
    jsonrpc_url: jsonrpcUrl,
    esplora_url: esploraUrl,
    metashrew_url: metashrewUrl
  };
  const provider = new Provider(network, JSON.stringify(providerConfig));
  return provider;
}

// src/cli/utils/wallet.ts
var fs2 = __toESM(require("fs"));
function walletExists(walletPath) {
  const expandedPath = expandPath(walletPath);
  return fs2.existsSync(expandedPath);
}
function isValidMnemonic(mnemonic) {
  const words = mnemonic.trim().split(/\s+/);
  return [12, 15, 18, 21, 24].includes(words.length);
}

// src/cli/utils/prompts.ts
var import_inquirer = __toESM(require("inquirer"));
async function confirm(message, defaultValue = false) {
  const { confirmed } = await import_inquirer.default.prompt([
    {
      type: "confirm",
      name: "confirmed",
      message,
      default: defaultValue
    }
  ]);
  return confirmed;
}
async function password(message) {
  const { value } = await import_inquirer.default.prompt([
    {
      type: "password",
      name: "value",
      message,
      mask: "*"
    }
  ]);
  return value;
}

// src/cli/commands/wallet.ts
var import_ora = __toESM(require("ora"));
function registerWalletCommands(program2) {
  const wallet = program2.command("wallet").description("Wallet management operations");
  wallet.command("create").description("Create a new wallet").option("--mnemonic <phrase>", "Restore from mnemonic phrase (12-24 words)").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (walletExists(walletPath)) {
        error(`Wallet already exists at ${walletPath}`);
        const overwrite = await confirm("Do you want to overwrite it?", false);
        if (!overwrite) {
          info("Wallet creation cancelled");
          return;
        }
      }
      const passphrase = globalOpts.passphrase || await password("Enter passphrase to encrypt wallet:");
      const passphraseConfirm = globalOpts.passphrase || await password("Confirm passphrase:");
      if (passphrase !== passphraseConfirm) {
        error("Passphrases do not match");
        return;
      }
      const spinner = (0, import_ora.default)("Creating wallet...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider || "mainnet",
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        let mnemonic = options.mnemonic;
        if (mnemonic) {
          if (!isValidMnemonic(mnemonic)) {
            spinner.fail();
            error("Invalid mnemonic phrase. Must be 12, 15, 18, 21, or 24 words");
            return;
          }
        }
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        const result = provider.wallet_create_js(
          JSON.stringify(walletConfig),
          mnemonic || void 0,
          passphrase
        );
        const walletInfo = await result;
        spinner.succeed("Wallet created successfully!");
        console.log();
        success(`Wallet saved to: ${walletPath}`);
        info(`Network: ${walletInfo.network}`);
        info(`First address (p2tr:0): ${walletInfo.address}`);
        if (walletInfo.mnemonic && !options.mnemonic) {
          console.log();
          console.log(import_chalk2.default.yellow.bold("\u26A0 IMPORTANT: Write down your recovery phrase!"));
          console.log();
          console.log(import_chalk2.default.cyan(walletInfo.mnemonic));
          console.log();
          console.log(import_chalk2.default.yellow("Keep this phrase safe. It's the only way to recover your wallet."));
        }
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to create wallet: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("addresses <spec>").description("Get addresses from wallet (e.g., p2tr:0-10, p2wpkh:0)").action(async (spec, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        info("Create a wallet first with: alkanes-cli wallet create");
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Loading wallet...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        spinner.succeed("Wallet loaded");
        const [addressType, range] = spec.split(":");
        if (!addressType || !range) {
          error("Invalid address spec format. Use: <type>:<range> (e.g., p2tr:0-10 or p2wpkh:5)");
          return;
        }
        let indices = [];
        if (range.includes("-")) {
          const [start, end] = range.split("-").map(Number);
          for (let i = start; i <= end; i++) {
            indices.push(i);
          }
        } else {
          indices.push(Number(range));
        }
        console.log();
        const table = createTable(["Index", "Address Type", "Address"]);
        for (const index of indices) {
          const addr = await provider.get_address(addressType, index);
          table.push([String(index), addressType, addr]);
        }
        console.log(table.toString());
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to get addresses: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("utxos <spec>").description("Get UTXOs for addresses (e.g., p2tr:0-5)").action(async (spec, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Loading wallet and fetching UTXOs...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          esploraUrl: globalOpts.esploraUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const utxos_result = await provider.get_utxos_by_spec_js([spec]);
        const utxos = JSON.parse(utxos_result);
        spinner.succeed(`Found ${utxos.length} UTXOs`);
        if (utxos.length === 0) {
          info("No UTXOs found for the specified addresses");
          return;
        }
        console.log();
        const table = createTable(["Outpoint", "Amount (BTC)", "Address"]);
        let totalAmount = 0;
        for (const utxo of utxos) {
          table.push([
            `${utxo.txid.slice(0, 8)}...${utxo.txid.slice(-8)}:${utxo.vout}`,
            formatBTC(utxo.amount),
            formatAddress(utxo.address, 30)
          ]);
          totalAmount += utxo.amount;
        }
        console.log(table.toString());
        console.log();
        success(`Total: ${formatBTC(totalAmount)}`);
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to get UTXOs: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("balance").description("Get wallet balance").option("--address <spec>", "Get balance for specific addresses (e.g., p2tr:0-5)").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Calculating balance...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          esploraUrl: globalOpts.esploraUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const balance_result = await provider.wallet_get_balance_js(options.address);
        const balance = JSON.parse(balance_result);
        spinner.succeed("Balance calculated");
        console.log();
        success(`Total Balance: ${formatBTC(balance.total || 0)}`);
        if (balance.confirmed !== void 0) {
          info(`Confirmed: ${formatBTC(balance.confirmed)}`);
        }
        if (balance.unconfirmed !== void 0 && balance.unconfirmed > 0) {
          info(`Unconfirmed: ${formatBTC(balance.unconfirmed)}`);
        }
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to get balance: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("send <address> <amount>").description("Send BTC to an address").option("--fee-rate <sats/vB>", "Fee rate in satoshis per virtual byte", "1").option("--from <spec>", "Source addresses (e.g., p2tr:0-5)").action(async (address, amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      if (!globalOpts.autoConfirm) {
        console.log();
        info(`Sending ${amount} BTC to ${address}`);
        info(`Fee rate: ${options.feeRate} sats/vB`);
        const confirmed = await confirm("Proceed with transaction?", false);
        if (!confirmed) {
          info("Transaction cancelled");
          return;
        }
      }
      const spinner = (0, import_ora.default)("Creating and broadcasting transaction...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const sendParams = {
          to_address: address,
          amount: parseFloat(amount) * 1e8,
          // Convert BTC to satoshis
          fee_rate: parseFloat(options.feeRate),
          from: options.from
        };
        const txid_result = await provider.wallet_send_js(JSON.stringify(sendParams));
        const txid = JSON.parse(txid_result);
        spinner.succeed("Transaction broadcast successfully!");
        console.log();
        success(`Transaction ID: ${txid}`);
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to send transaction: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("history").description("Get transaction history").option("--count <n>", "Number of transactions to fetch", "10").option("--address <spec>", "Filter by address spec").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Fetching transaction history...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const history_result = await provider.wallet_get_history_js(
          options.address,
          parseInt(options.count)
        );
        const history = JSON.parse(history_result);
        spinner.succeed("Transaction history fetched");
        if (history.length === 0) {
          info("No transactions found");
          return;
        }
        console.log();
        const table = createTable(["TXID", "Height", "Confirmations", "Amount"]);
        for (const tx of history) {
          table.push([
            formatAddress(tx.txid, 20),
            tx.block_height || "unconfirmed",
            tx.confirmations || 0,
            formatBTC(tx.amount || 0)
          ]);
        }
        console.log(table.toString());
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to get history: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("sign <psbt>").description("Sign a PSBT").action(async (psbt, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Signing PSBT...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const signed_result = await provider.wallet_sign_psbt_js(psbt);
        const signed = JSON.parse(signed_result);
        spinner.succeed("PSBT signed");
        console.log();
        success("Signed PSBT:");
        console.log(formatOutput(signed, globalOpts));
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to sign PSBT: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("freeze <outpoint>").description("Freeze a UTXO").option("--reason <text>", "Reason for freezing").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Freezing UTXO...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        await provider.wallet_freeze_utxo_js(outpoint, options.reason || "");
        spinner.succeed(`UTXO ${outpoint} frozen`);
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to freeze UTXO: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("unfreeze <outpoint>").description("Unfreeze a UTXO").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Unfreezing UTXO...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        await provider.wallet_unfreeze_utxo_js(outpoint);
        spinner.succeed(`UTXO ${outpoint} unfrozen`);
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to unfreeze UTXO: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("create-tx").description("Create a transaction").requiredOption("--to <address>", "Recipient address").requiredOption("--amount <satoshis>", "Amount in satoshis").option("--fee-rate <sats/vB>", "Fee rate", "1").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Creating transaction...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const tx_result = await provider.wallet_create_tx_js(
          options.to,
          parseInt(options.amount),
          parseFloat(options.feeRate)
        );
        const tx = JSON.parse(tx_result);
        spinner.succeed("Transaction created");
        console.log();
        console.log(formatOutput(tx, globalOpts));
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to create transaction: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("sign-tx <tx-hex>").description("Sign a transaction").action(async (txHex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Signing transaction...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const signed_result = await provider.wallet_sign_tx_js(txHex);
        const signed = JSON.parse(signed_result);
        spinner.succeed("Transaction signed");
        console.log();
        success("Signed transaction:");
        console.log(formatOutput(signed, globalOpts));
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to sign transaction: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("decode-tx <tx-hex>").description("Decode a transaction").action(async (txHex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora.default)("Decoding transaction...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const decoded_result = await provider.bitcoin_decoderawtransaction_js(txHex);
        const decoded = JSON.parse(decoded_result);
        spinner.succeed("Transaction decoded");
        console.log();
        console.log(formatOutput(decoded, globalOpts));
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to decode transaction: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("broadcast-tx <tx-hex>").description("Broadcast a transaction").action(async (txHex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora.default)("Broadcasting transaction...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const txid_result = await provider.bitcoin_sendrawtransaction_js(txHex);
        const txid = JSON.parse(txid_result);
        spinner.succeed("Transaction broadcast");
        console.log();
        success(`TXID: ${txid}`);
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to broadcast transaction: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("estimate-fee").description("Estimate transaction fee").requiredOption("--to <address>", "Recipient address").requiredOption("--amount <satoshis>", "Amount in satoshis").option("--fee-rate <sats/vB>", "Fee rate", "1").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Estimating fee...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const fee_result = await provider.wallet_estimate_fee_js(
          options.to,
          parseInt(options.amount),
          parseFloat(options.feeRate)
        );
        const fee = JSON.parse(fee_result);
        spinner.succeed("Fee estimated");
        console.log();
        info(`Estimated fee: ${formatBTC(fee.fee || 0)}`);
        info(`Total: ${formatBTC(parseInt(options.amount) + (fee.fee || 0))}`);
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to estimate fee: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("fee-rates").description("Get current fee rates").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora.default)("Getting fee rates...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          esploraUrl: globalOpts.esploraUrl
        });
        const rates_result = await provider.esplora_get_fee_estimates_js();
        const rates = JSON.parse(rates_result);
        spinner.succeed("Fee rates fetched");
        console.log();
        console.log(formatOutput(rates, globalOpts));
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to get fee rates: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("sync").description("Sync wallet with blockchain").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Syncing wallet...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        await provider.wallet_sync_js();
        spinner.succeed("Wallet synced");
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to sync wallet: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("backup <output-path>").description("Backup wallet").action(async (outputPath, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const spinner = (0, import_ora.default)("Backing up wallet...").start();
      try {
        const fs3 = await import("fs");
        const expandedOutput = expandPath(outputPath);
        fs3.copyFileSync(walletPath, expandedOutput);
        spinner.succeed(`Wallet backed up to ${expandedOutput}`);
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to backup wallet: ${err.message}`);
      process.exit(1);
    }
  });
  wallet.command("mnemonic").description("Get wallet mnemonic").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Getting mnemonic...").start();
      try {
        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const walletConfig = {
          wallet_path: walletPath,
          network: globalOpts.provider || "mainnet"
        };
        await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
        const mnemonic_result = await provider.wallet_get_mnemonic_js();
        const mnemonic = JSON.parse(mnemonic_result);
        spinner.succeed("Mnemonic retrieved");
        console.log();
        console.log(import_chalk2.default.yellow.bold("\u26A0 WARNING: Keep this mnemonic safe and private!"));
        console.log();
        console.log(import_chalk2.default.cyan(mnemonic));
        console.log();
      } catch (err) {
        spinner.fail();
        throw err;
      }
    } catch (err) {
      error(`Failed to get mnemonic: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/bitcoind.ts
var import_ora2 = __toESM(require("ora"));
function registerBitcoindCommands(program2) {
  const bitcoind = program2.command("bitcoind").description("Bitcoin Core RPC commands");
  bitcoind.command("getblockcount").description("Get current block count").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block count...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_getblockcount_js();
      const blockCount = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(blockCount, globalOpts));
    } catch (err) {
      error(`Failed to get block count: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("generatetoaddress <nblocks> <address>").description("Generate blocks to an address (regtest only)").action(async (nblocks, address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)(`Generating ${nblocks} blocks...`).start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_generatetoaddress_js(parseInt(nblocks), address);
      const hashes = JSON.parse(result);
      spinner.succeed(`Generated ${nblocks} blocks`);
      console.log(formatOutput(hashes, globalOpts));
    } catch (err) {
      error(`Failed to generate blocks: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockchaininfo").description("Get blockchain information").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting blockchain info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_getblockchaininfo_js();
      const info2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(info2, globalOpts));
    } catch (err) {
      error(`Failed to get blockchain info: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getrawtransaction <txid>").description("Get raw transaction").option("--verbose", "Return decoded transaction").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_getrawtransaction_js(txid, options.verbose || false);
      const tx = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(tx, globalOpts));
    } catch (err) {
      error(`Failed to get transaction: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblock <hash>").description("Get block by hash").option("--verbosity <level>", "Verbosity level (0-2)", "1").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_getblock_js(hash, parseInt(options.verbosity));
      const block = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(block, globalOpts));
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockhash <height>").description("Get block hash by height").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block hash...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_getblockhash_js(parseInt(height));
      const hash = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(hash, globalOpts));
    } catch (err) {
      error(`Failed to get block hash: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("sendrawtransaction <hex>").description("Broadcast a raw transaction").action(async (hex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Broadcasting transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_sendrawtransaction_js(hex);
      const txid = JSON.parse(result);
      spinner.succeed("Transaction broadcast");
      success(`TXID: ${txid}`);
    } catch (err) {
      error(`Failed to broadcast transaction: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getnetworkinfo").description("Get network information").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting network info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_getnetworkinfo_js();
      const info2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(info2, globalOpts));
    } catch (err) {
      error(`Failed to get network info: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getmempoolinfo").description("Get mempool information").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting mempool info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoin_getmempoolinfo_js();
      const info2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(info2, globalOpts));
    } catch (err) {
      error(`Failed to get mempool info: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("generatefuture <address>").description("Generate a future block (regtest only)").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Generating future block...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_generate_future_js(address);
      const hash = JSON.parse(result);
      spinner.succeed("Future block generated");
      console.log(formatOutput(hash, globalOpts));
    } catch (err) {
      error(`Failed to generate future block: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockheader <hash>").description("Get block header by hash").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block header...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_get_block_header_js(hash);
      const header = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(header, globalOpts));
    } catch (err) {
      error(`Failed to get block header: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockstats <hash>").description("Get block statistics by hash").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block stats...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_get_block_stats_js(hash);
      const stats = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(stats, globalOpts));
    } catch (err) {
      error(`Failed to get block stats: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("estimatesmartfee <blocks>").description("Estimate smart fee for confirmation in N blocks").action(async (blocks, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Estimating fee...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_estimate_smart_fee_js(parseInt(blocks));
      const estimate = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(estimate, globalOpts));
    } catch (err) {
      error(`Failed to estimate fee: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getchaintips").description("Get chain tips information").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting chain tips...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_get_chain_tips_js();
      const tips = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(tips, globalOpts));
    } catch (err) {
      error(`Failed to get chain tips: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("decoderawtransaction <hex>").description("Decode a raw transaction hex").action(async (hex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Decoding transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_decode_raw_transaction_js(hex);
      const decoded = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(decoded, globalOpts));
    } catch (err) {
      error(`Failed to decode transaction: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("decodepsbt <psbt>").description("Decode a PSBT (base64)").action(async (psbt, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Decoding PSBT...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_decode_psbt_js(psbt);
      const decoded = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(decoded, globalOpts));
    } catch (err) {
      error(`Failed to decode PSBT: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getrawmempool").description("Get raw mempool transactions").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting mempool transactions...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_get_raw_mempool_js();
      const mempool = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(mempool, globalOpts));
    } catch (err) {
      error(`Failed to get mempool: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("gettxout <txid> <vout>").description("Get transaction output details").option("--include-mempool", "Include mempool transactions", false).action(async (txid, vout, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting transaction output...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.bitcoind_get_tx_out_js(txid, parseInt(vout), options.includeMempool);
      const txout = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txout, globalOpts));
    } catch (err) {
      error(`Failed to get tx out: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/index.ts
var program = new import_commander.Command();
program.name("alkanes-bindgen-cli").version("0.1.0").description("Alkanes Bindgen CLI - Bitcoin smart contracts (WASM/TypeScript version)").option("-p, --provider <network>", "Network: mainnet/testnet/signet/regtest", "mainnet").option("--wallet-file <path>", "Wallet file path", "~/.alkanes/wallet.json").option("--passphrase <password>", "Wallet passphrase").option("--jsonrpc-url <url>", "JSON-RPC URL").option("--esplora-url <url>", "Esplora API URL").option("--metashrew-url <url>", "Metashrew RPC URL").option("--raw", "Output raw JSON").option("-y, --auto-confirm", "Skip confirmation prompts");
registerWalletCommands(program);
registerBitcoindCommands(program);
process.on("unhandledRejection", (reason, promise) => {
  error(`Unhandled rejection at: ${promise}, reason: ${reason}`);
  process.exit(1);
});
process.on("uncaughtException", (error2) => {
  error(`Uncaught exception: ${error2.message}`);
  if (error2.stack) {
    console.error(error2.stack);
  }
  process.exit(1);
});
program.parse(process.argv);
if (!process.argv.slice(2).length) {
  program.outputHelp();
}
