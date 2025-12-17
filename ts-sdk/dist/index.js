#!/usr/bin/env node
"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __esm = (fn, res) => function __init() {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
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

// src/cli/utils/formatting.ts
var formatting_exports = {};
__export(formatting_exports, {
  createTable: () => createTable,
  error: () => error,
  formatAddress: () => formatAddress,
  formatBTC: () => formatBTC,
  formatDate: () => formatDate,
  formatNumber: () => formatNumber,
  formatOutput: () => formatOutput,
  formatTxid: () => formatTxid,
  info: () => info,
  printHeader: () => printHeader,
  printRule: () => printRule,
  success: () => success,
  warn: () => warn
});
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
function warn(message) {
  console.warn(import_chalk.default.yellow("\u26A0 ") + message);
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
function formatTxid(txid, maxLength = 20) {
  return formatAddress(txid, maxLength);
}
function formatBTC(satoshis) {
  const btc = Number(satoshis) / 1e8;
  return `${btc.toFixed(8)} BTC`;
}
function formatNumber(num) {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}
function formatDate(timestamp) {
  return new Date(timestamp * 1e3).toLocaleString();
}
function printRule() {
  console.log(import_chalk.default.gray("\u2500".repeat(80)));
}
function printHeader(text) {
  console.log();
  console.log(import_chalk.default.bold.cyan(text));
  printRule();
}
var import_chalk, import_cli_table3;
var init_formatting = __esm({
  "src/cli/utils/formatting.ts"() {
    "use strict";
    import_chalk = __toESM(require("chalk"));
    import_cli_table3 = __toESM(require("cli-table3"));
  }
});

// src/cli/index.ts
var import_commander = require("commander");
init_formatting();

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

// src/cli/commands/wallet.ts
init_formatting();

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
init_formatting();
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

// src/cli/commands/alkanes.ts
init_formatting();
var import_ora3 = __toESM(require("ora"));
function registerAlkanesCommands(program2) {
  const alkanes = program2.command("alkanes").description("Alkanes smart contract operations");
  alkanes.command("getbytecode <alkane-id>").description("Get bytecode for an alkanes contract").option("--block-tag <tag>", 'Block tag (e.g., "latest" or height)').action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting bytecode...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes_bytecode_js(alkaneId, options.blockTag || null);
      const bytecode = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(bytecode, globalOpts));
    } catch (err) {
      error(`Failed to get bytecode: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("balance").description("Get alkanes balance for an address").option("--address <address>", "Address to check (defaults to wallet)").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting balance...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes_balance_js(options.address || null);
      const balance = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(balance, globalOpts));
    } catch (err) {
      error(`Failed to get balance: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("trace <outpoint>").description("Trace an alkanes transaction").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Tracing transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes_trace_js(outpoint);
      const trace = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(trace, globalOpts));
    } catch (err) {
      error(`Failed to trace: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("inspect <target>").description("Inspect alkanes bytecode").option("--disasm", "Enable disassembly to WAT format", false).option("--fuzz", "Enable fuzzing analysis", false).option("--fuzz-ranges <ranges>", "Opcode ranges for fuzzing").option("--meta", "Extract and display metadata", false).option("--codehash", "Compute and display codehash", false).action(async (target, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Inspecting bytecode...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const config = {
        disasm: options.disasm,
        fuzz: options.fuzz,
        fuzz_ranges: options.fuzzRanges || null,
        meta: options.meta,
        codehash: options.codehash
      };
      const result = await provider.alkanes_inspect_js(target, config);
      const inspection = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(inspection, globalOpts));
    } catch (err) {
      error(`Failed to inspect: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("simulate <contract-id>").description("Simulate alkanes execution").option("--params <params>", "Calldata params (format: [block,tx,inputs...]:[block:tx:value])").option("--block-hex <hex>", "Block hex").option("--transaction-hex <hex>", "Transaction hex").option("--block-tag <tag>", "Block tag").action(async (contractId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Simulating execution...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const context = {
        params: options.params || null,
        block_hex: options.blockHex || null,
        transaction_hex: options.transactionHex || null
      };
      const result = await provider.alkanes_simulate_js(
        contractId,
        JSON.stringify(context),
        options.blockTag || null
      );
      const simulation = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(simulation, globalOpts));
    } catch (err) {
      error(`Failed to simulate: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("unwrap").description("Get pending unwraps").option("--block-tag <tag>", "Block tag").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting pending unwraps...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes_pending_unwraps_js(options.blockTag || null);
      const unwraps = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(unwraps, globalOpts));
    } catch (err) {
      error(`Failed to get unwraps: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("get-all-pools <factory-id>").description("Get all pools from an AMM factory").action(async (factoryId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting all pools...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes_get_all_pools_js(factoryId);
      const pools = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(pools, globalOpts));
    } catch (err) {
      error(`Failed to get pools: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("all-pools-details <factory-id>").description("Get all pools with detailed information").action(async (factoryId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting pool details...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes_get_all_pools_with_details_js(
        factoryId,
        null
        // protocol_tag
      );
      const details = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(details, globalOpts));
    } catch (err) {
      error(`Failed to get pool details: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("reflect <alkane-id>").description("Reflect alkane metadata").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Reflecting alkane...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes_reflect_js(alkaneId);
      const metadata = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(metadata, globalOpts));
    } catch (err) {
      error(`Failed to reflect alkane: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("by-address <address>").description("Get alkanes by address").option("--block-tag <tag>", "Block tag").option("--protocol-tag <tag>", "Protocol tag", "0").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting alkanes by address...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const protocolTag = options.protocolTag ? parseFloat(options.protocolTag) : null;
      const result = await provider.alkanes_by_address_js(
        address,
        options.blockTag || null,
        protocolTag
      );
      const alkanes2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(alkanes2, globalOpts));
    } catch (err) {
      error(`Failed to get alkanes by address: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("by-outpoint <outpoint>").description("Get alkanes by outpoint").option("--block-tag <tag>", "Block tag").option("--protocol-tag <tag>", "Protocol tag", "0").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting alkanes by outpoint...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const protocolTag = options.protocolTag ? parseFloat(options.protocolTag) : null;
      const result = await provider.alkanes_by_outpoint_js(
        outpoint,
        options.blockTag || null,
        protocolTag
      );
      const alkanes2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(alkanes2, globalOpts));
    } catch (err) {
      error(`Failed to get alkanes by outpoint: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("traceblock <height>").description("Trace all alkanes transactions in a block").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Tracing block...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.traceBlock(parseFloat(height));
      const trace = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(trace, globalOpts));
    } catch (err) {
      error(`Failed to trace block: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("sequence").description("Get sequence for the current block").option("--block-tag <tag>", 'Block tag (e.g., "latest" or block height)').action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting sequence...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanesSequence(options.blockTag || null);
      const sequence = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(sequence, globalOpts));
    } catch (err) {
      error(`Failed to get sequence: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("spendables <address>").description("Get spendable outpoints for an address").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting spendables...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanesSpendables(address);
      const spendables = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(spendables, globalOpts));
    } catch (err) {
      error(`Failed to get spendables: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/esplora.ts
init_formatting();
var import_ora4 = __toESM(require("ora"));
function registerEsploraCommands(program2) {
  const esplora = program2.command("esplora").description("Esplora REST API operations");
  esplora.command("tx <txid>").description("Get transaction by txid").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_tx_js(txid);
      const tx = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(tx, globalOpts));
    } catch (err) {
      error(`Failed to get transaction: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-status <txid>").description("Get transaction status").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transaction status...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_tx_status_js(txid);
      const status = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(status, globalOpts));
    } catch (err) {
      error(`Failed to get transaction status: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address <address>").description("Get address information").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting address info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_address_info_js(address);
      const info2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(info2, globalOpts));
    } catch (err) {
      error(`Failed to get address info: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-utxos <address>").description("Get UTXOs for an address").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting UTXOs...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_address_utxo_js(address);
      const utxos = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(utxos, globalOpts));
    } catch (err) {
      error(`Failed to get UTXOs: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs <address>").description("Get transactions for an address").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transactions...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_address_txs_js(address);
      const txs = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txs, globalOpts));
    } catch (err) {
      error(`Failed to get transactions: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs-chain <address>").description("Get paginated transactions for an address").option("--last-seen <txid>", "Last seen txid for pagination").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transactions...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_address_txs_chain_js(
        address,
        options.lastSeen || null
      );
      const txs = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txs, globalOpts));
    } catch (err) {
      error(`Failed to get transactions: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("blocks-tip-height").description("Get current block tip height").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting tip height...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_blocks_tip_height_js();
      const height = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(height, globalOpts));
    } catch (err) {
      error(`Failed to get tip height: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("blocks-tip-hash").description("Get current block tip hash").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting tip hash...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_blocks_tip_hash_js();
      const hash = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(hash, globalOpts));
    } catch (err) {
      error(`Failed to get tip hash: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("fee-estimates").description("Get fee estimates").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting fee estimates...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_fee_estimates_js();
      const estimates = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(estimates, globalOpts));
    } catch (err) {
      error(`Failed to get fee estimates: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("broadcast-tx <hex>").description("Broadcast a transaction").action(async (hex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Broadcasting transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_broadcast_tx_js(hex);
      const txid = JSON.parse(result);
      spinner.succeed("Transaction broadcast");
      success(`TXID: ${txid}`);
    } catch (err) {
      error(`Failed to broadcast transaction: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-hex <txid>").description("Get raw transaction hex").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transaction hex...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esplora_get_tx_hex_js(txid);
      const hex = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(hex, globalOpts));
    } catch (err) {
      error(`Failed to get transaction hex: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("blocks [start-height]").description("Get blocks starting from height").action(async (startHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting blocks...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetBlocks(startHeight ? parseFloat(startHeight) : null);
      const blocks = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(blocks, globalOpts));
    } catch (err) {
      error(`Failed to get blocks: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-height <height>").description("Get block hash by height").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const hash = await provider.esploraGetBlockByHeight(parseFloat(height));
      spinner.succeed();
      console.log(formatOutput(hash, globalOpts));
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block <hash>").description("Get block by hash").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetBlock(hash);
      const block = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(block, globalOpts));
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-status <hash>").description("Get block status").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block status...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetBlockStatus(hash);
      const status = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(status, globalOpts));
    } catch (err) {
      error(`Failed to get block status: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-txids <hash>").description("Get transaction IDs in block").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block txids...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetBlockTxids(hash);
      const txids = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txids, globalOpts));
    } catch (err) {
      error(`Failed to get block txids: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-header <hash>").description("Get block header").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block header...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const header = await provider.esploraGetBlockHeader(hash);
      spinner.succeed();
      console.log(formatOutput(header, globalOpts));
    } catch (err) {
      error(`Failed to get block header: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-raw <hash>").description("Get raw block data").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting raw block...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const raw = await provider.esploraGetBlockRaw(hash);
      spinner.succeed();
      console.log(formatOutput(raw, globalOpts));
    } catch (err) {
      error(`Failed to get raw block: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-txid <hash> <index>").description("Get transaction ID by block hash and index").action(async (hash, index, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block txid...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txid = await provider.esploraGetBlockTxid(hash, parseFloat(index));
      spinner.succeed();
      console.log(formatOutput(txid, globalOpts));
    } catch (err) {
      error(`Failed to get block txid: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-txs <hash> [start-index]").description("Get block transactions").action(async (hash, startIndex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block txs...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetBlockTxs(hash, startIndex ? parseFloat(startIndex) : null);
      const txs = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txs, globalOpts));
    } catch (err) {
      error(`Failed to get block txs: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs-mempool <address>").description("Get mempool transactions for address").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting mempool transactions...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetAddressTxsMempool(address);
      const txs = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txs, globalOpts));
    } catch (err) {
      error(`Failed to get mempool transactions: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-prefix <prefix>").description("Search addresses by prefix").action(async (prefix, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Searching addresses...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetAddressPrefix(prefix);
      const addresses = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(addresses, globalOpts));
    } catch (err) {
      error(`Failed to search addresses: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-raw <txid>").description("Get raw transaction").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting raw transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const raw = await provider.esploraGetTxRaw(txid);
      spinner.succeed();
      console.log(formatOutput(raw, globalOpts));
    } catch (err) {
      error(`Failed to get raw transaction: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-merkle-proof <txid>").description("Get merkle proof for transaction").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting merkle proof...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetTxMerkleProof(txid);
      const proof = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(proof, globalOpts));
    } catch (err) {
      error(`Failed to get merkle proof: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-merkleblock-proof <txid>").description("Get merkle block proof").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting merkleblock proof...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const proof = await provider.esploraGetTxMerkleblockProof(txid);
      spinner.succeed();
      console.log(formatOutput(proof, globalOpts));
    } catch (err) {
      error(`Failed to get merkleblock proof: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-outspend <txid> <index>").description("Get outspend for transaction output").action(async (txid, index, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting outspend...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetTxOutspend(txid, parseFloat(index));
      const outspend = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(outspend, globalOpts));
    } catch (err) {
      error(`Failed to get outspend: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-outspends <txid>").description("Get all outspends for transaction").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting outspends...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetTxOutspends(txid);
      const outspends = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(outspends, globalOpts));
    } catch (err) {
      error(`Failed to get outspends: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("mempool").description("Get mempool info").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting mempool info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetMempool();
      const mempool = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(mempool, globalOpts));
    } catch (err) {
      error(`Failed to get mempool info: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("mempool-txids").description("Get mempool transaction IDs").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting mempool txids...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetMempoolTxids();
      const txids = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txids, globalOpts));
    } catch (err) {
      error(`Failed to get mempool txids: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("mempool-recent").description("Get recent mempool transactions").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting recent mempool txs...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.esploraGetMempoolRecent();
      const txs = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txs, globalOpts));
    } catch (err) {
      error(`Failed to get recent mempool txs: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("post-tx <tx-hex>").description("Post transaction (alternative to broadcast)").action(async (txHex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Posting transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txid = await provider.esploraPostTx(txHex);
      spinner.succeed("Transaction posted");
      success(`TXID: ${txid}`);
    } catch (err) {
      error(`Failed to post transaction: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/ord.ts
init_formatting();
var import_ora5 = __toESM(require("ora"));
function registerOrdCommands(program2) {
  const ord = program2.command("ord").description("Ordinals and Inscriptions operations");
  ord.command("inscription <id>").description("Get inscription by ID").action(async (id, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting inscription...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord_inscription_js(id);
      const inscription = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(inscription, globalOpts));
    } catch (err) {
      error(`Failed to get inscription: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("inscriptions").description("List inscriptions").option("--page <number>", "Page number", "0").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting inscriptions...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const page = options.page ? parseFloat(options.page) : null;
      const result = await provider.ord_inscriptions_js(page);
      const inscriptions = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(inscriptions, globalOpts));
    } catch (err) {
      error(`Failed to get inscriptions: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("outputs <address>").description("Get ordinal outputs for an address").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting outputs...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord_outputs_js(address);
      const outputs = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(outputs, globalOpts));
    } catch (err) {
      error(`Failed to get outputs: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("rune <name>").description("Get rune information").action(async (name, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting rune...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord_rune_js(name);
      const rune = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(rune, globalOpts));
    } catch (err) {
      error(`Failed to get rune: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("list <outpoint>").description("List ordinals in an output").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Listing ordinals...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord_list_js(outpoint);
      const list = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(list, globalOpts));
    } catch (err) {
      error(`Failed to list ordinals: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("find <sat>").description("Find ordinal by satoshi number").action(async (sat, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Finding ordinal...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord_find_js(parseFloat(sat));
      const location = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(location, globalOpts));
    } catch (err) {
      error(`Failed to find ordinal: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("address-info <address>").description("Get address information").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting address info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ordAddressInfo(address);
      const addressInfo = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(addressInfo, globalOpts));
    } catch (err) {
      error(`Failed to get address info: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("block-info <query>").description("Get block information (height or hash)").action(async (query, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting block info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ordBlockInfo(query);
      const blockInfo = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(blockInfo, globalOpts));
    } catch (err) {
      error(`Failed to get block info: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("block-count").description("Get latest block count").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting block count...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ordBlockCount();
      const blockCount = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(blockCount, globalOpts));
    } catch (err) {
      error(`Failed to get block count: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("blocks").description("Get latest blocks").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting blocks...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ordBlocks();
      const blocks = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(blocks, globalOpts));
    } catch (err) {
      error(`Failed to get blocks: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("children <inscription-id>").description("Get children of an inscription").option("--page <number>", "Page number", "0").action(async (inscriptionId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting children...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const page = options.page ? parseFloat(options.page) : null;
      const result = await provider.ordChildren(inscriptionId, page);
      const children = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(children, globalOpts));
    } catch (err) {
      error(`Failed to get children: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("content <inscription-id>").description("Get inscription content").action(async (inscriptionId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting content...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ordContent(inscriptionId);
      const content = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(content, globalOpts));
    } catch (err) {
      error(`Failed to get content: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("parents <inscription-id>").description("Get parents of an inscription").option("--page <number>", "Page number", "0").action(async (inscriptionId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting parents...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const page = options.page ? parseFloat(options.page) : null;
      const result = await provider.ordParents(inscriptionId, page);
      const parents = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(parents, globalOpts));
    } catch (err) {
      error(`Failed to get parents: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("tx-info <txid>").description("Get transaction information").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting transaction info...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ordTxInfo(txid);
      const txInfo = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(txInfo, globalOpts));
    } catch (err) {
      error(`Failed to get transaction info: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/runestone.ts
init_formatting();
var import_ora6 = __toESM(require("ora"));
function registerRunestoneCommands(program2) {
  const runestone = program2.command("runestone").description("Runestone protocol operations");
  runestone.command("decode <txid>").description("Decode runestone from transaction").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora6.default)("Decoding runestone...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.runestone_decode_tx_js(txid);
      const decoded = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(decoded, globalOpts));
    } catch (err) {
      error(`Failed to decode runestone: ${err.message}`);
      process.exit(1);
    }
  });
  runestone.command("analyze <txid>").description("Analyze runestone transaction").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora6.default)("Analyzing runestone...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.runestone_analyze_tx_js(txid);
      const analysis = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(analysis, globalOpts));
    } catch (err) {
      error(`Failed to analyze runestone: ${err.message}`);
      process.exit(1);
    }
  });
  runestone.command("trace <txid>").description("Trace all protostones in a runestone transaction").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora6.default)("Tracing runestone...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.traceProtostones(txid);
      const trace = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(trace, globalOpts));
    } catch (err) {
      error(`Failed to trace runestone: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/protorunes.ts
init_formatting();
var import_ora7 = __toESM(require("ora"));
function registerProtorunesCommands(program2) {
  const protorunes = program2.command("protorunes").description("Protorunes protocol operations");
  protorunes.command("by-address <address>").description("Get protorunes by address").option("--block-tag <tag>", 'Block tag (e.g., "latest" or height)').action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora7.default)("Getting protorunes...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanesByAddress(
        address,
        options.blockTag || null,
        1
      );
      const protorunes2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(protorunes2, globalOpts));
    } catch (err) {
      error(`Failed to get protorunes: ${err.message}`);
      process.exit(1);
    }
  });
  protorunes.command("by-outpoint <outpoint>").description("Get protorunes by outpoint").option("--block-tag <tag>", 'Block tag (e.g., "latest" or height)').action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora7.default)("Getting protorunes...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanesByOutpoint(
        outpoint,
        options.blockTag || null,
        1
      );
      const protorunes2 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(protorunes2, globalOpts));
    } catch (err) {
      error(`Failed to get protorunes: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/metashrew.ts
init_formatting();
var import_ora8 = __toESM(require("ora"));
function registerMetashrewCommands(program2) {
  const metashrew = program2.command("metashrew").description("Metashrew RPC operations");
  metashrew.command("height").description("Get current metashrew height").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Getting height...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.metashrew_height_js();
      const height = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(height, globalOpts));
    } catch (err) {
      error(`Failed to get height: ${err.message}`);
      process.exit(1);
    }
  });
  metashrew.command("state-root").description("Get state root at height").option("--height <number>", "Block height").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Getting state root...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const height = options.height ? parseFloat(options.height) : null;
      const result = await provider.metashrew_state_root_js(height);
      const stateRoot = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(stateRoot, globalOpts));
    } catch (err) {
      error(`Failed to get state root: ${err.message}`);
      process.exit(1);
    }
  });
  metashrew.command("getblockhash <height>").description("Get block hash at height").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Getting block hash...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.metashrew_get_block_hash_js(parseFloat(height));
      const hash = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(hash, globalOpts));
    } catch (err) {
      error(`Failed to get block hash: ${err.message}`);
      process.exit(1);
    }
  });
  metashrew.command("view <function> <payload> <block-tag>").description("Call metashrew view function").action(async (fn, payload, blockTag, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Calling view function...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.metashrew_view_js(fn, payload, blockTag);
      const view = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(view, globalOpts));
    } catch (err) {
      error(`Failed to call view function: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/lua.ts
init_formatting();
var import_ora9 = __toESM(require("ora"));
function registerLuaCommands(program2) {
  const lua = program2.command("lua").description("Lua script execution");
  lua.command("evalscript <script>").description("Evaluate a Lua script").action(async (script, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora9.default)("Evaluating script...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.lua_eval_script_js(script);
      const output = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(output, globalOpts));
    } catch (err) {
      error(`Failed to evaluate script: ${err.message}`);
      process.exit(1);
    }
  });
  lua.command("eval <script>").description("Evaluate Lua with arguments").option("--args <json>", "Arguments as JSON", "{}").action(async (script, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora9.default)("Evaluating script...").start();
      const provider = await createProvider({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const args = JSON.parse(options.args);
      const result = await provider.lua_eval_js(script, args);
      const output = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(output, globalOpts));
    } catch (err) {
      error(`Failed to evaluate script: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/dataapi.ts
init_formatting();
var import_ora10 = __toESM(require("ora"));
function registerDataapiCommands(program2) {
  const dataapi = program2.command("dataapi").description("Analytics and data API operations");
  dataapi.command("pools <factory-id>").description("Get pools for factory").action(async (factoryId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting pools...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_pools_js(factoryId);
      const pools = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(pools, globalOpts));
    } catch (err) {
      error(`Failed to get pools: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("pool-history <pool-id>").description("Get pool history").option("--category <category>", "History category").option("--limit <limit>", "Limit results", "100").option("--offset <offset>", "Offset for pagination", "0").action(async (poolId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting pool history...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_pool_history_js(
        poolId,
        options.category || null,
        options.limit ? parseInt(options.limit) : null,
        options.offset ? parseInt(options.offset) : null
      );
      const history = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(history, globalOpts));
    } catch (err) {
      error(`Failed to get pool history: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("trades <pool>").description("Get trade history for pool").option("--start-time <timestamp>", "Start time").option("--end-time <timestamp>", "End time").option("--limit <limit>", "Limit results", "100").action(async (pool, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting trades...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_trades_js(
        pool,
        options.startTime ? parseFloat(options.startTime) : null,
        options.endTime ? parseFloat(options.endTime) : null,
        options.limit ? parseInt(options.limit) : null
      );
      const trades = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(trades, globalOpts));
    } catch (err) {
      error(`Failed to get trades: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("candles <pool>").description("Get candle data for pool").requiredOption("--interval <interval>", "Interval (1m, 5m, 1h, 1d)").option("--start-time <timestamp>", "Start time").option("--end-time <timestamp>", "End time").option("--limit <limit>", "Limit results", "100").action(async (pool, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting candles...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_candles_js(
        pool,
        options.interval,
        options.startTime ? parseFloat(options.startTime) : null,
        options.endTime ? parseFloat(options.endTime) : null,
        options.limit ? parseInt(options.limit) : null
      );
      const candles = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(candles, globalOpts));
    } catch (err) {
      error(`Failed to get candles: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("reserves <pool>").description("Get pool reserves").action(async (pool, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting reserves...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_reserves_js(pool);
      const reserves = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(reserves, globalOpts));
    } catch (err) {
      error(`Failed to get reserves: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("holders <alkane>").description("Get alkane holders").option("--page <page>", "Page number", "0").option("--limit <limit>", "Limit results", "100").action(async (alkane, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting holders...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_holders_js(
        alkane,
        parseInt(options.page),
        parseInt(options.limit)
      );
      const holders = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(holders, globalOpts));
    } catch (err) {
      error(`Failed to get holders: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("holders-count <alkane>").description("Get count of alkane holders").action(async (alkane, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting holders count...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_holders_count_js(alkane);
      const count = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(count, globalOpts));
    } catch (err) {
      error(`Failed to get holders count: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("bitcoin-price").description("Get current Bitcoin price").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting Bitcoin price...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_bitcoin_price_js();
      const price = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(price, globalOpts));
    } catch (err) {
      error(`Failed to get Bitcoin price: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("bitcoin-market-chart <days>").description("Get Bitcoin market chart").action(async (days, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting market chart...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_bitcoin_market_chart_js(days);
      const chart = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(chart, globalOpts));
    } catch (err) {
      error(`Failed to get market chart: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("address-balances <address>").description("Get alkanes balances for address").option("--include-outpoints", "Include outpoint details", false).action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting balances...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_address_balances_js(
        address,
        options.includeOutpoints
      );
      const balances = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(balances, globalOpts));
    } catch (err) {
      error(`Failed to get balances: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("alkanes-by-address <address>").description("Get alkanes owned by address").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting alkanes...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.data_api_get_alkanes_by_address_js(address);
      const alkanes = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(alkanes, globalOpts));
    } catch (err) {
      error(`Failed to get alkanes: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("health").description("Check data API health").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Checking health...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      await provider.dataApiHealth();
      spinner.succeed("Data API is healthy");
    } catch (err) {
      error(`Health check failed: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("get-alkanes").description("Get all alkanes").option("--page <number>", "Page number", "0").option("--limit <number>", "Results per page", "100").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting alkanes...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const page = options.page ? parseInt(options.page) : null;
      const limit = options.limit ? parseInt(options.limit) : null;
      const result = await provider.dataApiGetAlkanes(page, limit);
      const alkanes = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(alkanes, globalOpts));
    } catch (err) {
      error(`Failed to get alkanes: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("get-alkane-details <alkane-id>").description("Get alkane details by ID (format: block:tx)").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting alkane details...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.dataApiGetAlkaneDetails(alkaneId);
      const details = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(details, globalOpts));
    } catch (err) {
      error(`Failed to get alkane details: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("get-pool-by-id <pool-id>").description("Get pool details by ID (format: block:tx)").action(async (poolId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting pool details...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.dataApiGetPoolById(poolId);
      const pool = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(pool, globalOpts));
    } catch (err) {
      error(`Failed to get pool: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("get-outpoint-balances <outpoint>").description("Get balances for an outpoint (format: txid:vout)").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting outpoint balances...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.dataApiGetOutpointBalances(outpoint);
      const balances = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(balances, globalOpts));
    } catch (err) {
      error(`Failed to get outpoint balances: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("get-block-height").description("Get latest indexed block height").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting block height...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.dataApiGetBlockHeight();
      const height = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(height, globalOpts));
    } catch (err) {
      error(`Failed to get block height: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("get-block-hash").description("Get latest indexed block hash").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting block hash...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.dataApiGetBlockHash();
      const hash = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(hash, globalOpts));
    } catch (err) {
      error(`Failed to get block hash: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("get-indexer-position").description("Get indexer position (height and hash)").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting indexer position...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.dataApiGetIndexerPosition();
      const position = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(position, globalOpts));
    } catch (err) {
      error(`Failed to get indexer position: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/espo.ts
init_formatting();
var import_ora11 = __toESM(require("ora"));
function registerEspoCommands(program2) {
  const espo = program2.command("espo").description("ESPO balance indexer operations");
  espo.command("height").description("Get current ESPO height").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting height...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_get_height_js();
      const height = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(height, globalOpts));
    } catch (err) {
      error(`Failed to get height: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("ping").description("Ping ESPO service").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Pinging...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_ping_js();
      const pong = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(pong, globalOpts));
    } catch (err) {
      error(`Failed to ping: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("address-balances <address>").description("Get balances for an address").option("--include-outpoints", "Include outpoint details", false).action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting balances...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_get_address_balances_js(
        address,
        options.includeOutpoints
      );
      const balances = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(balances, globalOpts));
    } catch (err) {
      error(`Failed to get balances: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("address-outpoints <address>").description("Get outpoints for an address").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting outpoints...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_get_address_outpoints_js(address);
      const outpoints = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(outpoints, globalOpts));
    } catch (err) {
      error(`Failed to get outpoints: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("outpoint-balances <outpoint>").description("Get balances for an outpoint").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting balances...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_get_outpoint_balances_js(outpoint);
      const balances = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(balances, globalOpts));
    } catch (err) {
      error(`Failed to get balances: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("holders <alkane-id>").description("Get holders for an alkane").option("--page <page>", "Page number", "0").option("--limit <limit>", "Limit results", "100").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting holders...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_get_holders_js(
        alkaneId,
        parseFloat(options.page),
        parseFloat(options.limit)
      );
      const holders = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(holders, globalOpts));
    } catch (err) {
      error(`Failed to get holders: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("holders-count <alkane-id>").description("Get holder count for an alkane").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting holder count...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_get_holders_count_js(alkaneId);
      const count = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(count, globalOpts));
    } catch (err) {
      error(`Failed to get holder count: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("keys <alkane-id>").description("Get storage keys for an alkane").option("--page <page>", "Page number", "0").option("--limit <limit>", "Limit results", "100").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting keys...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_get_keys_js(
        alkaneId,
        parseFloat(options.page),
        parseFloat(options.limit)
      );
      const keys = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(keys, globalOpts));
    } catch (err) {
      error(`Failed to get keys: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("ammdata-ping").description("Ping ESPO AMM data service").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Pinging AMM data service...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espo_ammdata_ping_js();
      const pong = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(pong, globalOpts));
    } catch (err) {
      error(`Failed to ping: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("candles <pool>").description("Get OHLCV candlestick data for a pool").option("--timeframe <timeframe>", 'Timeframe (e.g., "1m", "5m", "1h", "1d")').option("--side <side>", 'Side ("buy" or "sell")').option("--limit <limit>", "Limit results").option("--page <page>", "Page number").action(async (pool, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting candles...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espoGetCandles(
        pool,
        options.timeframe || null,
        options.side || null,
        options.limit ? parseFloat(options.limit) : null,
        options.page ? parseFloat(options.page) : null
      );
      const candles = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(candles, globalOpts));
    } catch (err) {
      error(`Failed to get candles: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("trades <pool>").description("Get trade history for a pool").option("--limit <limit>", "Limit results").option("--page <page>", "Page number").option("--side <side>", "Side filter").option("--filter-side <side>", "Filter by side").option("--sort <field>", "Sort field").option("--dir <direction>", "Sort direction").action(async (pool, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting trades...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espoGetTrades(
        pool,
        options.limit ? parseFloat(options.limit) : null,
        options.page ? parseFloat(options.page) : null,
        options.side || null,
        options.filterSide || null,
        options.sort || null,
        options.dir || null
      );
      const trades = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(trades, globalOpts));
    } catch (err) {
      error(`Failed to get trades: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("pools").description("Get all pools with pagination").option("--limit <limit>", "Limit results").option("--page <page>", "Page number").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting pools...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espoGetPools(
        options.limit ? parseFloat(options.limit) : null,
        options.page ? parseFloat(options.page) : null
      );
      const pools = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(pools, globalOpts));
    } catch (err) {
      error(`Failed to get pools: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("find-best-swap-path <token-in> <token-out>").description("Find the best swap path between two tokens").option("--mode <mode>", "Mode").option("--amount-in <amount>", "Amount in").option("--amount-out <amount>", "Amount out").option("--amount-out-min <amount>", "Minimum amount out").option("--amount-in-max <amount>", "Maximum amount in").option("--available-in <amount>", "Available amount in").option("--fee-bps <bps>", "Fee in basis points").option("--max-hops <hops>", "Maximum number of hops").action(async (tokenIn, tokenOut, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Finding best swap path...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espoFindBestSwapPath(
        tokenIn,
        tokenOut,
        options.mode || null,
        options.amountIn || null,
        options.amountOut || null,
        options.amountOutMin || null,
        options.amountInMax || null,
        options.availableIn || null,
        options.feeBps ? parseFloat(options.feeBps) : null,
        options.maxHops ? parseFloat(options.maxHops) : null
      );
      const path3 = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(path3, globalOpts));
    } catch (err) {
      error(`Failed to find swap path: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("get-best-mev-swap <token>").description("Find the best MEV swap opportunity for a token").option("--fee-bps <bps>", "Fee in basis points").option("--max-hops <hops>", "Maximum number of hops").action(async (token, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Finding best MEV swap...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.espoGetBestMevSwap(
        token,
        options.feeBps ? parseFloat(options.feeBps) : null,
        options.maxHops ? parseFloat(options.maxHops) : null
      );
      const mevSwap = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(mevSwap, globalOpts));
    } catch (err) {
      error(`Failed to find MEV swap: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/brc20prog.ts
init_formatting();
var import_ora12 = __toESM(require("ora"));
function registerBrc20ProgCommands(program2) {
  const brc20prog = program2.command("brc20-prog").description("Programmable BRC-20 operations");
  brc20prog.command("balance <address>").description("Get balance for address").option("--block <tag>", "Block tag").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting balance...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_get_balance_js(
        address,
        options.block || null
      );
      const balance = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(balance, globalOpts));
    } catch (err) {
      error(`Failed to get balance: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("code <address>").description("Get contract code").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting code...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_get_code_js(address);
      const code = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(code, globalOpts));
    } catch (err) {
      error(`Failed to get code: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("block-number").description("Get current block number").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting block number...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_block_number_js();
      const blockNumber = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(blockNumber, globalOpts));
    } catch (err) {
      error(`Failed to get block number: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("chain-id").description("Get chain ID").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting chain ID...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_chain_id_js();
      const chainId = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(chainId, globalOpts));
    } catch (err) {
      error(`Failed to get chain ID: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("tx-receipt <hash>").description("Get transaction receipt").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting transaction receipt...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_get_transaction_receipt_js(hash);
      const receipt = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(receipt, globalOpts));
    } catch (err) {
      error(`Failed to get transaction receipt: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("tx <hash>").description("Get transaction by hash").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting transaction...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_get_transaction_by_hash_js(hash);
      const tx = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(tx, globalOpts));
    } catch (err) {
      error(`Failed to get transaction: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("block <number>").description("Get block by number").option("--full", "Include full transactions", false).action(async (number, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting block...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_get_block_by_number_js(
        number,
        options.full
      );
      const block = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(block, globalOpts));
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("call <to> <data>").description("Call contract function").option("--block <tag>", "Block tag").action(async (to, data, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Calling contract...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_call_js(
        to,
        data,
        options.block || null
      );
      const output = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(output, globalOpts));
    } catch (err) {
      error(`Failed to call contract: ${err.message}`);
      process.exit(1);
    }
  });
  brc20prog.command("estimate-gas <to> <data>").description("Estimate gas for transaction").option("--block <tag>", "Block tag").action(async (to, data, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Estimating gas...").start();
      const provider = await createProvider({
        network: globalOpts.provider
      });
      const result = await provider.brc20prog_estimate_gas_js(
        to,
        data,
        options.block || null
      );
      const gas = JSON.parse(result);
      spinner.succeed();
      console.log(formatOutput(gas, globalOpts));
    } catch (err) {
      error(`Failed to estimate gas: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/opi.ts
init_formatting();
function registerOpiCommands(program2) {
  const opi = program2.command("opi").description("Open Protocol Indexer operations");
  const notImplemented = () => {
    error("OPI commands require direct HTTP endpoint access.");
    error("These will be implemented in a future version.");
    error("For now, please use the Rust alkanes-cli for OPI operations.");
    process.exit(1);
  };
  opi.command("block-height").description("Get current indexed block height (BRC-20)").action(notImplemented);
  opi.command("extras-block-height").description("Get extras indexed block height (BRC-20)").action(notImplemented);
  opi.command("db-version").description("Get database version (BRC-20)").action(notImplemented);
  opi.command("current-balance <wallet>").description("Get current balance (BRC-20)").action(notImplemented);
  opi.command("holders <ticker>").description("Get holders of a BRC-20 ticker").action(notImplemented);
  const runes = opi.command("runes").description("Runes indexer subcommands");
  runes.command("placeholder").description("Runes commands placeholder").action(notImplemented);
  const bitmap = opi.command("bitmap").description("Bitmap indexer subcommands");
  bitmap.command("placeholder").description("Bitmap commands placeholder").action(notImplemented);
  const pow20 = opi.command("pow20").description("POW20 indexer subcommands");
  pow20.command("placeholder").description("POW20 commands placeholder").action(notImplemented);
  const sns = opi.command("sns").description("SNS (Sats Names Service) indexer subcommands");
  sns.command("placeholder").description("SNS commands placeholder").action(notImplemented);
}

// src/cli/commands/subfrost.ts
init_formatting();
function registerSubfrostCommands(program2) {
  const subfrost = program2.command("subfrost").description("Subfrost operations (frBTC unwrap utilities)");
  subfrost.command("minimum-unwrap").description("Calculate minimum unwrap amount based on current fee rates").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      error("Subfrost minimum-unwrap command is not yet implemented.");
      error("This requires WASM bindings for subfrost minimum unwrap calculation.");
      error("For now, please use the Rust alkanes-cli for Subfrost operations.");
      process.exit(1);
    } catch (err) {
      error(`Failed to calculate minimum unwrap: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/index.ts
var program = new import_commander.Command();
program.name("alkanes-bindgen-cli").version("0.1.0").description("Alkanes Bindgen CLI - Bitcoin smart contracts (WASM/TypeScript version)").option("-p, --provider <network>", "Network: mainnet/testnet/signet/regtest", "mainnet").option("--wallet-file <path>", "Wallet file path", "~/.alkanes/wallet.json").option("--passphrase <password>", "Wallet passphrase").option("--jsonrpc-url <url>", "JSON-RPC URL").option("--esplora-url <url>", "Esplora API URL").option("--metashrew-url <url>", "Metashrew RPC URL").option("--raw", "Output raw JSON").option("-y, --auto-confirm", "Skip confirmation prompts");
registerWalletCommands(program);
registerBitcoindCommands(program);
registerAlkanesCommands(program);
registerEsploraCommands(program);
registerOrdCommands(program);
registerRunestoneCommands(program);
registerProtorunesCommands(program);
registerMetashrewCommands(program);
registerLuaCommands(program);
registerDataapiCommands(program);
registerEspoCommands(program);
registerBrc20ProgCommands(program);
registerOpiCommands(program);
registerSubfrostCommands(program);
program.command("decodepsbt <psbt>").description("Decode a PSBT (Partially Signed Bitcoin Transaction) without calling bitcoind").action(async (psbt, options, command) => {
  try {
    const { decode_psbt } = await import("../wasm/alkanes_web_sys.js");
    const globalOpts = command.parent?.opts() || {};
    const result = decode_psbt(psbt);
    const decoded = JSON.parse(result);
    const { formatOutput: formatOutput3 } = await Promise.resolve().then(() => (init_formatting(), formatting_exports));
    console.log(formatOutput3(decoded, globalOpts));
  } catch (err) {
    const { error: error2 } = await Promise.resolve().then(() => (init_formatting(), formatting_exports));
    error2(`Failed to decode PSBT: ${err.message}`);
    process.exit(1);
  }
});
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
