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
var __commonJS = (cb, mod) => function __require() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
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
  TreeNode: () => TreeNode,
  createTable: () => createTable,
  error: () => error,
  formatAddress: () => formatAddress,
  formatAlkaneBalances: () => formatAlkaneBalances,
  formatBTC: () => formatBTC,
  formatBlockInfo: () => formatBlockInfo,
  formatBlockchainInfo: () => formatBlockchainInfo,
  formatBytes: () => formatBytes,
  formatDate: () => formatDate,
  formatFeeEstimates: () => formatFeeEstimates,
  formatInscriptions: () => formatInscriptions,
  formatNumber: () => formatNumber,
  formatOutput: () => formatOutput,
  formatReflectMetadata: () => formatReflectMetadata,
  formatTxid: () => formatTxid,
  getVerbosity: () => getVerbosity,
  info: () => info,
  printHeader: () => printHeader,
  printRule: () => printRule,
  setVerbosity: () => setVerbosity,
  success: () => success,
  tree: () => tree,
  verbose: () => verbose,
  warn: () => warn
});
function setVerbosity(level) {
  globalVerbosity = level;
}
function getVerbosity() {
  return globalVerbosity;
}
function verbose(level, message) {
  if (globalVerbosity >= level) {
    const prefix = level === 3 ? import_chalk.default.dim("[DEBUG]") : level === 2 ? import_chalk.default.blue("[INFO]") : import_chalk.default.cyan("[VERBOSE]");
    console.error(`${prefix} ${message}`);
  }
}
function tree(label) {
  return new TreeNode(label);
}
function jsonReplacer(key, value) {
  if (typeof value === "bigint") {
    return value.toString();
  }
  return value;
}
function formatOutput(data, options = {}) {
  const { raw = false } = options;
  if (raw) {
    return JSON.stringify(data, jsonReplacer, 2);
  }
  if (typeof data === "string") {
    return data;
  }
  if (typeof data === "number" || typeof data === "bigint") {
    return import_chalk.default.yellow(String(data));
  }
  if (typeof data === "boolean") {
    return data ? import_chalk.default.green("true") : import_chalk.default.red("false");
  }
  if (data === null || data === void 0) {
    return import_chalk.default.dim("(none)");
  }
  if (Array.isArray(data)) {
    if (data.length === 0) {
      return import_chalk.default.dim("(empty)");
    }
    if (data.every((item) => typeof item === "string" || typeof item === "number")) {
      return data.join("\n");
    }
    return formatArrayAsTree(data);
  }
  return formatObjectAsTree(data);
}
function formatObjectAsTree(obj, rootLabel) {
  const root = tree(rootLabel || "");
  for (const [key, value] of Object.entries(obj)) {
    const formattedKey = import_chalk.default.bold(formatKey(key));
    if (value === null || value === void 0) {
      root.push(`${formattedKey}: ${import_chalk.default.dim("(none)")}`);
    } else if (typeof value === "boolean") {
      root.push(`${formattedKey}: ${value ? import_chalk.default.green("yes") : import_chalk.default.red("no")}`);
    } else if (typeof value === "number" || typeof value === "bigint") {
      root.push(`${formattedKey}: ${import_chalk.default.yellow(String(value))}`);
    } else if (typeof value === "string") {
      root.push(`${formattedKey}: ${formatStringValue(value)}`);
    } else if (Array.isArray(value)) {
      if (value.length === 0) {
        root.push(`${formattedKey}: ${import_chalk.default.dim("[]")}`);
      } else if (value.length <= 3 && value.every((v) => typeof v === "string" || typeof v === "number")) {
        root.push(`${formattedKey}: ${value.join(", ")}`);
      } else {
        const arrayNode = tree(`${formattedKey}: ${import_chalk.default.dim(`[${value.length} items]`)}`);
        for (const item of value.slice(0, 5)) {
          if (typeof item === "object" && item !== null) {
            arrayNode.push(formatNestedObject(item));
          } else {
            arrayNode.push(String(item));
          }
        }
        if (value.length > 5) {
          arrayNode.push(import_chalk.default.dim(`... and ${value.length - 5} more`));
        }
        root.push(arrayNode);
      }
    } else if (typeof value === "object") {
      const objNode = tree(formattedKey);
      for (const [k, v] of Object.entries(value)) {
        objNode.push(`${import_chalk.default.bold(formatKey(k))}: ${formatSimpleValue(v)}`);
      }
      root.push(objNode);
    }
  }
  if (!rootLabel) {
    return root.children.map((c) => c.toString("", true, true)).join("\n");
  }
  return root.toString();
}
function formatNestedObject(obj) {
  const firstKey = Object.keys(obj)[0];
  const label = firstKey ? `${import_chalk.default.bold(formatKey(firstKey))}: ${formatSimpleValue(obj[firstKey])}` : "{}";
  const node = tree(label);
  let first = true;
  for (const [k, v] of Object.entries(obj)) {
    if (first) {
      first = false;
      continue;
    }
    node.push(`${import_chalk.default.bold(formatKey(k))}: ${formatSimpleValue(v)}`);
  }
  return node;
}
function formatArrayAsTree(arr) {
  const lines = [];
  for (const item of arr) {
    if (typeof item === "object" && item !== null) {
      lines.push(formatObjectAsTree(item));
    } else {
      lines.push(String(item));
    }
  }
  return lines.join("\n\n");
}
function formatKey(key) {
  return key.replace(/([A-Z])/g, " $1").replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase()).trim();
}
function formatStringValue(value) {
  if (value.length > 40 && /^[0-9a-fA-F]+$/.test(value)) {
    return import_chalk.default.cyan(value);
  }
  if (value.startsWith("0x")) {
    return import_chalk.default.cyan(value);
  }
  return value;
}
function formatSimpleValue(value) {
  if (value === null || value === void 0) {
    return import_chalk.default.dim("(none)");
  }
  if (typeof value === "boolean") {
    return value ? import_chalk.default.green("yes") : import_chalk.default.red("no");
  }
  if (typeof value === "number" || typeof value === "bigint") {
    return import_chalk.default.yellow(String(value));
  }
  if (typeof value === "string") {
    return formatStringValue(value);
  }
  if (Array.isArray(value)) {
    if (value.length === 0) return import_chalk.default.dim("[]");
    if (value.length <= 3 && value.every((v) => typeof v === "string" || typeof v === "number")) {
      return value.join(", ");
    }
    return import_chalk.default.dim(`[${value.length} items]`);
  }
  if (typeof value === "object") {
    const keys = Object.keys(value);
    if (keys.length === 0) return import_chalk.default.dim("{}");
    return import_chalk.default.dim(`{${keys.length} fields}`);
  }
  return String(value);
}
function formatBlockchainInfo(info2) {
  const root = tree(`${import_chalk.default.bold("\u26D3\uFE0F  Blockchain Info")}`);
  root.push(`${import_chalk.default.bold("Chain:")} ${info2.chain}`);
  root.push(`${import_chalk.default.bold("Blocks:")} ${import_chalk.default.yellow(info2.blocks)}`);
  root.push(`${import_chalk.default.bold("Headers:")} ${import_chalk.default.yellow(info2.headers)}`);
  root.push(`${import_chalk.default.bold("Best Block Hash:")} ${import_chalk.default.cyan(info2.bestblockhash)}`);
  root.push(`${import_chalk.default.bold("Difficulty:")} ${import_chalk.default.yellow(info2.difficulty)}`);
  if (info2.mediantime) {
    root.push(`${import_chalk.default.bold("Median Time:")} ${formatDate(info2.mediantime)}`);
  }
  if (info2.verificationprogress !== void 0) {
    const progress = (info2.verificationprogress * 100).toFixed(2);
    root.push(`${import_chalk.default.bold("Verification:")} ${import_chalk.default.yellow(progress + "%")}`);
  }
  if (info2.initialblockdownload !== void 0) {
    root.push(`${import_chalk.default.bold("Initial Download:")} ${info2.initialblockdownload ? import_chalk.default.yellow("yes") : import_chalk.default.green("no")}`);
  }
  if (info2.pruned !== void 0) {
    root.push(`${import_chalk.default.bold("Pruned:")} ${info2.pruned ? import_chalk.default.yellow("yes") : import_chalk.default.green("no")}`);
  }
  if (info2.size_on_disk) {
    root.push(`${import_chalk.default.bold("Size on Disk:")} ${formatBytes(info2.size_on_disk)}`);
  }
  if (info2.warnings) {
    root.push(`${import_chalk.default.bold("\u26A0\uFE0F  Warnings:")} ${import_chalk.default.yellow(info2.warnings)}`);
  }
  return root.toString();
}
function formatBlockInfo(block) {
  const root = tree(`${import_chalk.default.bold("\u{1F4E6} Block")}`);
  if (block.hash) root.push(`${import_chalk.default.bold("Hash:")} ${import_chalk.default.cyan(block.hash)}`);
  if (block.height !== void 0) root.push(`${import_chalk.default.bold("Height:")} ${import_chalk.default.yellow(block.height)}`);
  if (block.number !== void 0) {
    const num = typeof block.number === "string" ? parseInt(block.number, 16) : block.number;
    root.push(`${import_chalk.default.bold("Number:")} ${import_chalk.default.yellow(num)}`);
  }
  if (block.timestamp) {
    const ts = typeof block.timestamp === "string" ? parseInt(block.timestamp, 16) : block.timestamp;
    root.push(`${import_chalk.default.bold("Timestamp:")} ${formatDate(ts)}`);
  }
  if (block.difficulty) root.push(`${import_chalk.default.bold("Difficulty:")} ${block.difficulty}`);
  if (block.nonce) root.push(`${import_chalk.default.bold("Nonce:")} ${block.nonce}`);
  if (block.size) root.push(`${import_chalk.default.bold("Size:")} ${formatBytes(parseInt(block.size, 16) || block.size)}`);
  if (block.transactions) {
    root.push(`${import_chalk.default.bold("Transactions:")} ${import_chalk.default.yellow(Array.isArray(block.transactions) ? block.transactions.length : 0)}`);
  }
  if (block.parentHash) root.push(`${import_chalk.default.bold("Parent:")} ${import_chalk.default.cyan(block.parentHash)}`);
  return root.toString();
}
function formatAlkaneBalances(balances) {
  if (!balances || balances.length === 0) {
    return import_chalk.default.dim("No alkane balances found");
  }
  const root = tree(`${import_chalk.default.bold("\u{1FA99} Alkane Balances")}`);
  for (const balance of balances) {
    const id = balance.alkane_id ? `${balance.alkane_id.block}:${balance.alkane_id.tx}` : `${balance.block}:${balance.tx}`;
    const balanceNode = tree(`${import_chalk.default.bold("ID:")} ${import_chalk.default.cyan(id)}`);
    if (balance.name) balanceNode.push(`${import_chalk.default.bold("Name:")} ${balance.name}`);
    if (balance.symbol) balanceNode.push(`${import_chalk.default.bold("Symbol:")} ${balance.symbol}`);
    balanceNode.push(`${import_chalk.default.bold("Balance:")} ${import_chalk.default.yellow(balance.balance || balance.value || "0")}`);
    root.push(balanceNode);
  }
  return root.toString();
}
function formatInscriptions(inscriptions) {
  const ids = inscriptions.ids || inscriptions;
  if (!ids || ids.length === 0) {
    return import_chalk.default.dim("No inscriptions found");
  }
  const root = tree(`${import_chalk.default.bold("\u{1F4DC} Inscriptions")} ${import_chalk.default.dim(`(${ids.length} total)`)}`);
  for (const id of ids.slice(0, 10)) {
    root.push(import_chalk.default.cyan(id));
  }
  if (ids.length > 10) {
    root.push(import_chalk.default.dim(`... and ${ids.length - 10} more`));
  }
  if (inscriptions.more) {
    root.push(import_chalk.default.dim("(more available)"));
  }
  return root.toString();
}
function formatFeeEstimates(estimates) {
  const root = tree(`${import_chalk.default.bold("\u{1F4B0} Fee Estimates")} ${import_chalk.default.dim("(sat/vB)")}`);
  const blocks = Object.keys(estimates).map(Number).sort((a, b) => a - b);
  for (const block of blocks) {
    const fee = estimates[block];
    const label = block === 1 ? "Next block" : block <= 6 ? `~${block * 10} min` : block <= 144 ? `~${Math.round(block / 6)} hours` : `~${Math.round(block / 144)} days`;
    root.push(`${import_chalk.default.bold(block.toString().padStart(4))} blocks (${label}): ${import_chalk.default.yellow(fee.toFixed(3))}`);
  }
  return root.toString();
}
function formatReflectMetadata(metadata) {
  const root = tree(`${import_chalk.default.bold("\u{1F50D} Alkane Metadata")}`);
  if (metadata.id) root.push(`${import_chalk.default.bold("ID:")} ${import_chalk.default.cyan(metadata.id)}`);
  if (metadata.name) root.push(`${import_chalk.default.bold("Name:")} ${metadata.name}`);
  if (metadata.symbol) root.push(`${import_chalk.default.bold("Symbol:")} ${metadata.symbol}`);
  if (metadata.total_supply !== void 0) root.push(`${import_chalk.default.bold("Total Supply:")} ${import_chalk.default.yellow(metadata.total_supply)}`);
  if (metadata.cap !== void 0) root.push(`${import_chalk.default.bold("Cap:")} ${import_chalk.default.yellow(metadata.cap)}`);
  if (metadata.minted !== void 0) root.push(`${import_chalk.default.bold("Minted:")} ${import_chalk.default.yellow(metadata.minted)}`);
  if (metadata.value_per_mint !== void 0) root.push(`${import_chalk.default.bold("Value Per Mint:")} ${import_chalk.default.yellow(metadata.value_per_mint)}`);
  if (metadata.premine !== void 0) root.push(`${import_chalk.default.bold("Premine:")} ${import_chalk.default.yellow(metadata.premine)}`);
  if (metadata.decimals !== void 0) root.push(`${import_chalk.default.bold("Decimals:")} ${import_chalk.default.yellow(metadata.decimals)}`);
  if (metadata.data) root.push(`${import_chalk.default.bold("Data:")} ${metadata.data}`);
  return root.toString();
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
function formatBytes(bytes) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let unitIndex = 0;
  let size = bytes;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(2)} ${units[unitIndex]}`;
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
var import_chalk, import_cli_table3, globalVerbosity, TreeNode;
var init_formatting = __esm({
  "src/cli/utils/formatting.ts"() {
    "use strict";
    import_chalk = __toESM(require("chalk"));
    import_cli_table3 = __toESM(require("cli-table3"));
    globalVerbosity = 0;
    TreeNode = class _TreeNode {
      constructor(label) {
        this.children = [];
        this.label = label;
      }
      /**
       * Add a child node
       */
      push(child) {
        if (typeof child === "string") {
          this.children.push(new _TreeNode(child));
        } else {
          this.children.push(child);
        }
        return this;
      }
      /**
       * Add multiple children
       */
      withLeaves(children) {
        for (const child of children) {
          this.push(child);
        }
        return this;
      }
      /**
       * Render the tree as a string
       */
      toString(prefix = "", isLast = true, isRoot = true) {
        const lines = [];
        if (isRoot) {
          lines.push(this.label);
        } else {
          const connector = isLast ? "\u2514\u2500\u2500 " : "\u251C\u2500\u2500 ";
          lines.push(prefix + connector + this.label);
        }
        const childPrefix = isRoot ? "" : prefix + (isLast ? "    " : "\u2502   ");
        for (let i = 0; i < this.children.length; i++) {
          const child = this.children[i];
          const childIsLast = i === this.children.length - 1;
          lines.push(child.toString(childPrefix, childIsLast, false));
        }
        return lines.join("\n");
      }
    };
  }
});

// src/provider/index.ts
var provider_exports = {};
__export(provider_exports, {
  AlkanesProvider: () => AlkanesProvider,
  AlkanesRpcClient: () => AlkanesRpcClient,
  BitcoinRpcClient: () => BitcoinRpcClient,
  Brc20ProgClient: () => Brc20ProgClient,
  DataApiClient: () => DataApiClient,
  EsploraClient: () => EsploraClient,
  EspoClient: () => EspoClient,
  LuaClient: () => LuaClient,
  MetashrewClient: () => MetashrewClient,
  NETWORK_PRESETS: () => NETWORK_PRESETS,
  OrdClient: () => OrdClient,
  createProvider: () => createProvider
});
function mapToObject(value) {
  if (value instanceof Map) {
    const obj = {};
    value.forEach((v, k) => {
      obj[k] = mapToObject(v);
    });
    return obj;
  }
  if (Array.isArray(value)) {
    return value.map(mapToObject);
  }
  return value;
}
function getLogLevelFromEnv() {
  if (typeof process !== "undefined" && process.env) {
    const alkLog = process.env.ALKANES_LOG_LEVEL;
    const rustLog = process.env.RUST_LOG;
    const level = alkLog || rustLog;
    if (level) {
      const normalized = level.toLowerCase();
      if (["off", "error", "warn", "info", "debug", "trace"].includes(normalized)) {
        return normalized;
      }
    }
  }
  return void 0;
}
function createProvider(config) {
  return new AlkanesProvider(config);
}
var bitcoin, NETWORK_PRESETS, BitcoinRpcClient, EsploraClient, AlkanesRpcClient, MetashrewClient, OrdClient, Brc20ProgClient, LuaClient, DataApiClient, EspoClient, Logger, logger, AlkanesProvider;
var init_provider = __esm({
  "src/provider/index.ts"() {
    "use strict";
    bitcoin = __toESM(require("bitcoinjs-lib"));
    NETWORK_PRESETS = {
      "mainnet": {
        rpcUrl: "https://mainnet.subfrost.io/v4/subfrost",
        dataApiUrl: "https://mainnet.subfrost.io/v4/subfrost",
        networkType: "mainnet"
      },
      "testnet": {
        rpcUrl: "https://testnet.subfrost.io/v4/subfrost",
        dataApiUrl: "https://testnet.subfrost.io/v4/subfrost",
        networkType: "testnet"
      },
      "signet": {
        rpcUrl: "https://signet.subfrost.io/v4/subfrost",
        dataApiUrl: "https://signet.subfrost.io/v4/subfrost",
        networkType: "signet"
      },
      "subfrost-regtest": {
        rpcUrl: "https://regtest.subfrost.io/v4/subfrost",
        dataApiUrl: "https://regtest.subfrost.io/v4/subfrost",
        networkType: "regtest"
      },
      "regtest": {
        rpcUrl: "http://localhost:18888",
        dataApiUrl: "http://localhost:18888",
        networkType: "regtest"
      },
      "local": {
        rpcUrl: "http://localhost:18888",
        dataApiUrl: "http://localhost:18888",
        networkType: "regtest"
      }
    };
    BitcoinRpcClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      async getBlockCount() {
        return this.provider.bitcoindGetBlockCount();
      }
      async getBlockHash(height) {
        return this.provider.bitcoindGetBlockHash(height);
      }
      async getBlock(hash, raw = false) {
        const result = await this.provider.bitcoindGetBlock(hash, raw);
        return mapToObject(result);
      }
      async sendRawTransaction(hex) {
        return this.provider.bitcoindSendRawTransaction(hex);
      }
      async getTransaction(txid, blockHash) {
        const result = await this.provider.bitcoindGetRawTransaction(txid, blockHash);
        return mapToObject(result);
      }
      async getBlockchainInfo() {
        const result = await this.provider.bitcoindGetBlockchainInfo();
        return mapToObject(result);
      }
      async getNetworkInfo() {
        const result = await this.provider.bitcoindGetNetworkInfo();
        return mapToObject(result);
      }
      async getMempoolInfo() {
        const result = await this.provider.bitcoindGetMempoolInfo();
        return mapToObject(result);
      }
      async estimateSmartFee(target) {
        const result = await this.provider.bitcoindEstimateSmartFee(target);
        return mapToObject(result);
      }
      async generateToAddress(nblocks, address) {
        const result = await this.provider.bitcoindGenerateToAddress(nblocks, address);
        return mapToObject(result);
      }
      async generateFuture(address) {
        const result = await this.provider.bitcoindGenerateFuture(address);
        return mapToObject(result);
      }
      async getBlockHeader(hash) {
        const result = await this.provider.bitcoindGetBlockHeader(hash);
        return mapToObject(result);
      }
      async getBlockStats(hash) {
        const result = await this.provider.bitcoindGetBlockStats(hash);
        return mapToObject(result);
      }
      async getChainTips() {
        const result = await this.provider.bitcoindGetChainTips();
        return mapToObject(result);
      }
      async getRawMempool() {
        const result = await this.provider.bitcoindGetRawMempool();
        return mapToObject(result);
      }
      async getTxOut(txid, vout, includeMempool) {
        const result = await this.provider.bitcoindGetTxOut(txid, vout, includeMempool);
        return mapToObject(result);
      }
      async decodeRawTransaction(hex) {
        const result = await this.provider.bitcoindDecodeRawTransaction(hex);
        return mapToObject(result);
      }
      async decodePsbt(psbt) {
        const result = await this.provider.bitcoindDecodePsbt(psbt);
        return mapToObject(result);
      }
    };
    EsploraClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      async getAddressInfo(address) {
        const result = await this.provider.esploraGetAddressInfo(address);
        return mapToObject(result);
      }
      async getAddressUtxos(address) {
        const result = await this.provider.esploraGetAddressUtxo(address);
        return mapToObject(result);
      }
      async getAddressTxs(address) {
        const result = await this.provider.esploraGetAddressTxs(address);
        return mapToObject(result);
      }
      async getTx(txid) {
        const result = await this.provider.esploraGetTx(txid);
        return mapToObject(result);
      }
      async getTxStatus(txid) {
        const result = await this.provider.esploraGetTxStatus(txid);
        return mapToObject(result);
      }
      async getTxHex(txid) {
        return this.provider.esploraGetTxHex(txid);
      }
      async getBlocksTipHeight() {
        return this.provider.esploraGetBlocksTipHeight();
      }
      async getBlocksTipHash() {
        return this.provider.esploraGetBlocksTipHash();
      }
      async broadcastTx(txHex) {
        return this.provider.esploraBroadcastTx(txHex);
      }
      async getFeeEstimates() {
        const result = await this.provider.esploraGetFeeEstimates();
        return mapToObject(result);
      }
      async getBlocks(startHeight) {
        const result = await this.provider.esploraGetBlocks(startHeight);
        return mapToObject(result);
      }
      async getBlockByHeight(height) {
        const result = await this.provider.esploraGetBlockByHeight(height);
        return mapToObject(result);
      }
      async getBlock(hash) {
        const result = await this.provider.esploraGetBlock(hash);
        return mapToObject(result);
      }
      async getBlockStatus(hash) {
        const result = await this.provider.esploraGetBlockStatus(hash);
        return mapToObject(result);
      }
      async getBlockTxids(hash) {
        return this.provider.esploraGetBlockTxids(hash);
      }
      async getBlockHeader(hash) {
        const result = await this.provider.esploraGetBlockHeader(hash);
        return mapToObject(result);
      }
      async getBlockRaw(hash) {
        return this.provider.esploraGetBlockRaw(hash);
      }
      async getBlockTxid(hash, index) {
        return this.provider.esploraGetBlockTxid(hash, index);
      }
      async getBlockTxs(hash, startIndex) {
        const result = await this.provider.esploraGetBlockTxs(hash, startIndex);
        return mapToObject(result);
      }
      async getAddressTxsChain(address, lastSeenTxid) {
        const result = await this.provider.esploraGetAddressTxsChain(address, lastSeenTxid);
        return mapToObject(result);
      }
      async getAddressTxsMempool(address) {
        const result = await this.provider.esploraGetAddressTxsMempool(address);
        return mapToObject(result);
      }
      async getAddressPrefix(prefix) {
        const result = await this.provider.esploraGetAddressPrefix(prefix);
        return mapToObject(result);
      }
      async getTxRaw(txid) {
        return this.provider.esploraGetTxRaw(txid);
      }
      async getTxMerkleProof(txid) {
        const result = await this.provider.esploraGetTxMerkleProof(txid);
        return mapToObject(result);
      }
      async getTxMerkleblockProof(txid) {
        const result = await this.provider.esploraGetTxMerkleblockProof(txid);
        return mapToObject(result);
      }
      async getTxOutspend(txid, index) {
        const result = await this.provider.esploraGetTxOutspend(txid, index);
        return mapToObject(result);
      }
      async getTxOutspends(txid) {
        const result = await this.provider.esploraGetTxOutspends(txid);
        return mapToObject(result);
      }
      async getMempool() {
        const result = await this.provider.esploraGetMempool();
        return mapToObject(result);
      }
      async getMempoolTxids() {
        return this.provider.esploraGetMempoolTxids();
      }
      async getMempoolRecent() {
        const result = await this.provider.esploraGetMempoolRecent();
        return mapToObject(result);
      }
    };
    AlkanesRpcClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      async getBalance(address) {
        const result = await this.provider.alkanesBalance(address);
        return mapToObject(result);
      }
      async getByAddress(address, blockTag, protocolTag) {
        const result = await this.provider.alkanesByAddress(address, blockTag, protocolTag);
        return mapToObject(result);
      }
      async getByOutpoint(outpoint, blockTag, protocolTag) {
        const result = await this.provider.alkanesByOutpoint(outpoint, blockTag, protocolTag);
        return mapToObject(result);
      }
      async getBytecode(alkaneId, blockTag) {
        return this.provider.alkanesBytecode(alkaneId, blockTag);
      }
      /**
       * Simulate an Alkanes contract call (read-only)
       *
       * @param contractId - Contract ID in "block:tx" format (e.g., "2:0")
       * @param context - Simulation context (object or JSON string for backward compatibility)
       * @param blockTag - Optional block tag for historical simulation
       * @returns Simulation result
       *
       * @example
       * ```typescript
       * const result = await provider.alkanes.simulate('2:0', {
       *   alkanes: [],
       *   transaction: [],
       *   block: [],
       *   height: 800000,
       *   vout: 0,
       *   txindex: 0,
       *   calldata: [77],  // opcode 77 = mint
       *   pointer: 0,
       *   refund_pointer: 0,
       * });
       * ```
       */
      async simulate(contractId, context, blockTag) {
        const contextJson = typeof context === "string" ? context : JSON.stringify(context);
        const result = await this.provider.alkanesSimulate(contractId, contextJson, blockTag);
        return mapToObject(result);
      }
      /**
       * Execute an Alkanes contract call
       *
       * @param params - Execute parameters (object or JSON string for backward compatibility)
       * @returns Execution result with txid
       *
       * @example
       * ```typescript
       * const result = await provider.alkanes.execute({
       *   target: '2:0',
       *   calldata: [77],  // opcode 77 = mint
       *   fee_rate: 10,
       * });
       * console.log('TXID:', result.txid);
       * ```
       */
      async execute(params) {
        const paramsJson = typeof params === "string" ? params : JSON.stringify(params);
        const result = await this.provider.alkanesExecute(paramsJson);
        return mapToObject(result);
      }
      async trace(outpoint) {
        const result = await this.provider.alkanesTrace(outpoint);
        return mapToObject(result);
      }
      async traceBlock(height) {
        const result = await this.provider.traceBlock(height);
        return mapToObject(result);
      }
      async view(contractId, viewFn, params, blockTag) {
        const result = await this.provider.alkanesView(contractId, viewFn, params, blockTag);
        return mapToObject(result);
      }
      async getAllPools(factoryId) {
        const result = await this.provider.alkanesGetAllPools(factoryId);
        return mapToObject(result);
      }
      async getAllPoolsWithDetails(factoryId, chunkSize, maxConcurrent) {
        const result = await this.provider.alkanesGetAllPoolsWithDetails(factoryId, chunkSize, maxConcurrent);
        return mapToObject(result);
      }
      async getPendingUnwraps(blockTag) {
        const result = await this.provider.alkanesPendingUnwraps(blockTag);
        return mapToObject(result);
      }
      async reflect(alkaneId) {
        const result = await this.provider.alkanesReflect(alkaneId);
        return mapToObject(result);
      }
      async getSequence(blockTag) {
        const result = await this.provider.alkanesSequence(blockTag);
        return mapToObject(result);
      }
      async getSpendables(address) {
        const result = await this.provider.alkanesSpendables(address);
        return mapToObject(result);
      }
      async getPoolDetails(poolId) {
        const result = await this.provider.alkanesPoolDetails(poolId);
        return mapToObject(result);
      }
      async reflectAlkaneRange(block, startTx, endTx) {
        const result = await this.provider.alkanesReflectAlkaneRange(block, startTx, endTx);
        return mapToObject(result);
      }
      async inspect(target, config) {
        const result = await this.provider.alkanesInspect(target, config);
        return mapToObject(result);
      }
      /**
       * Inspect alkanes bytecode directly from WASM bytes.
       * This allows inspection without fetching from RPC - useful for local/offline analysis.
       *
       * @param bytecodeHex - The WASM bytecode as hex string (with or without 0x prefix)
       * @param alkaneId - The alkane ID in format "block:tx"
       * @param config - Inspection configuration
       * @returns Inspection result with codehash, disassembly, metadata, and fuzzing results
       */
      async inspectBytecode(bytecodeHex, alkaneId, config) {
        const result = await this.provider.alkanesInspectBytecode(bytecodeHex, alkaneId, config);
        return mapToObject(result);
      }
    };
    MetashrewClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      /**
       * Get current blockchain height
       */
      async getHeight() {
        return this.provider.metashrewHeight();
      }
      /**
       * Get state root at a specific height
       */
      async getStateRoot(height) {
        return this.provider.metashrewStateRoot(height);
      }
      /**
       * Get block hash at a specific height
       */
      async getBlockHash(height) {
        return this.provider.metashrewGetBlockHash(height);
      }
      /**
       * Call a metashrew view function
       *
       * This is the generic low-level method for calling any metashrew_view function.
       *
       * @param viewFn - The view function name (e.g., "simulate", "protorunesbyaddress")
       * @param payload - The hex-encoded payload (with or without 0x prefix)
       * @param blockTag - The block tag ("latest" or a block height as string)
       * @returns The hex-encoded response string
       */
      async view(viewFn, payload, blockTag = "latest") {
        return this.provider.metashrewView(viewFn, payload, blockTag);
      }
    };
    OrdClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      async getInscription(id) {
        const result = await this.provider.ordInscription(id);
        return mapToObject(result);
      }
      async getInscriptions(page) {
        const result = await this.provider.ordInscriptions(page);
        return mapToObject(result);
      }
      async getOutputs(address) {
        const result = await this.provider.ordOutputs(address);
        return mapToObject(result);
      }
      async getRune(name) {
        const result = await this.provider.ordRune(name);
        return mapToObject(result);
      }
      async list(outpoint) {
        const result = await this.provider.ordList(outpoint);
        return mapToObject(result);
      }
      async find(sat) {
        const result = await this.provider.ordFind(sat);
        return mapToObject(result);
      }
      async getAddressInfo(address) {
        const result = await this.provider.ordAddressInfo(address);
        return mapToObject(result);
      }
      async getBlockInfo(query) {
        const result = await this.provider.ordBlockInfo(query);
        return mapToObject(result);
      }
      async getBlockCount() {
        return this.provider.ordBlockCount();
      }
      async getBlocks() {
        const result = await this.provider.ordBlocks();
        return mapToObject(result);
      }
      async getChildren(inscriptionId, page) {
        const result = await this.provider.ordChildren(inscriptionId, page);
        return mapToObject(result);
      }
      async getContent(inscriptionId) {
        const result = await this.provider.ordContent(inscriptionId);
        return mapToObject(result);
      }
      async getParents(inscriptionId, page) {
        const result = await this.provider.ordParents(inscriptionId, page);
        return mapToObject(result);
      }
      async getTxInfo(txid) {
        const result = await this.provider.ordTxInfo(txid);
        return mapToObject(result);
      }
    };
    Brc20ProgClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      async getBalance(address) {
        const result = await this.provider.brc20progGetBalance(address);
        return mapToObject(result);
      }
      async getCode(address) {
        const result = await this.provider.brc20progGetCode(address);
        return mapToObject(result);
      }
      async getBlockNumber() {
        return this.provider.brc20progBlockNumber();
      }
      async getChainId() {
        return this.provider.brc20progChainId();
      }
      async getTxReceipt(hash) {
        const result = await this.provider.brc20progGetTransactionReceipt(hash);
        return mapToObject(result);
      }
      async getTx(hash) {
        const result = await this.provider.brc20progGetTransactionByHash(hash);
        return mapToObject(result);
      }
      async getBlock(number, includeTxs) {
        const result = await this.provider.brc20progGetBlockByNumber(String(number), includeTxs);
        return mapToObject(result);
      }
      async call(to, data, from, blockTag) {
        const result = await this.provider.brc20progCall(to, data, from, blockTag);
        return mapToObject(result);
      }
      async estimateGas(to, data, from) {
        const result = await this.provider.brc20progEstimateGas(to, data, from);
        return mapToObject(result);
      }
    };
    LuaClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      /**
       * Execute a Lua script with automatic scripthash caching
       *
       * This is the recommended way to execute Lua scripts. It:
       * 1. Computes the SHA256 hash of the script
       * 2. Tries to execute using the cached hash (lua_evalsaved)
       * 3. Falls back to full script execution (lua_evalscript) if not cached
       *
       * @param script - The Lua script content
       * @param args - Arguments to pass to the script
       * @returns The script execution result
       */
      async eval(script, args = []) {
        return this.provider.luaEval(script, args);
      }
      /**
       * Execute a Lua script directly (no caching)
       *
       * Use this only when you need to bypass the scripthash cache.
       * For most use cases, prefer the eval() method.
       *
       * @param script - The Lua script content
       * @returns The script execution result
       */
      async evalScript(script) {
        return this.provider.luaEvalScript(script);
      }
    };
    DataApiClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      // Pool operations
      async getPools(factoryId) {
        return this.provider.dataApiGetPools(factoryId);
      }
      async getPoolHistory(poolId, category, limit, offset) {
        return this.provider.dataApiGetPoolHistory(poolId, category, limit ? BigInt(limit) : void 0, offset ? BigInt(offset) : void 0);
      }
      async getAllHistory(poolId, limit, offset) {
        return this.provider.dataApiGetAllHistory(poolId, limit ? BigInt(limit) : void 0, offset ? BigInt(offset) : void 0);
      }
      async getSwapHistory(poolId, limit, offset) {
        return this.provider.dataApiGetSwapHistory(poolId, limit ? BigInt(limit) : void 0, offset ? BigInt(offset) : void 0);
      }
      async getMintHistory(poolId, limit, offset) {
        return this.provider.dataApiGetMintHistory(poolId, limit ? BigInt(limit) : void 0, offset ? BigInt(offset) : void 0);
      }
      async getBurnHistory(poolId, limit, offset) {
        return this.provider.dataApiGetBurnHistory(poolId, limit ? BigInt(limit) : void 0, offset ? BigInt(offset) : void 0);
      }
      // Trading data
      async getTrades(pool, startTime, endTime, limit) {
        return this.provider.dataApiGetTrades(pool, startTime, endTime, limit ? BigInt(limit) : void 0);
      }
      async getCandles(pool, interval, startTime, endTime, limit) {
        return this.provider.dataApiGetCandles(pool, interval, startTime, endTime, limit ? BigInt(limit) : void 0);
      }
      async getReserves(pool) {
        return this.provider.dataApiGetReserves(pool);
      }
      // Balance operations
      async getAlkanesByAddress(address) {
        return this.provider.dataApiGetAlkanesByAddress(address);
      }
      async getAddressBalances(address, includeOutpoints = false) {
        return this.provider.dataApiGetAddressBalances(address, includeOutpoints);
      }
      // Token operations
      async getHolders(alkane, page = 0, limit = 100) {
        return this.provider.dataApiGetHolders(alkane, BigInt(page), BigInt(limit));
      }
      async getHoldersCount(alkane) {
        return this.provider.dataApiGetHoldersCount(alkane);
      }
      async getKeys(alkane, prefix, limit = 100) {
        return this.provider.dataApiGetKeys(alkane, prefix, BigInt(limit));
      }
      // Market data
      async getBitcoinPrice() {
        return this.provider.dataApiGetBitcoinPrice();
      }
      async getBitcoinMarketChart(days) {
        return this.provider.dataApiGetBitcoinMarketChart(days);
      }
    };
    EspoClient = class {
      constructor(provider) {
        this.provider = provider;
      }
      // ============================================================================
      // ESSENTIALS MODULE
      // ============================================================================
      /**
       * Get current Espo indexer height
       */
      async getHeight() {
        return this.provider.espoGetHeight();
      }
      /**
       * Ping the Espo server
       */
      async ping() {
        return this.provider.espoPing();
      }
      /**
       * Get alkanes balances for an address
       * @param address - Bitcoin address
       * @param includeOutpoints - Include detailed outpoint information
       */
      async getAddressBalances(address, includeOutpoints = false) {
        const result = await this.provider.espoGetAddressBalances(address, includeOutpoints);
        return mapToObject(result);
      }
      /**
       * Get outpoints containing alkanes for an address
       * @param address - Bitcoin address
       */
      async getAddressOutpoints(address) {
        const result = await this.provider.espoGetAddressOutpoints(address);
        return mapToObject(result);
      }
      /**
       * Get alkanes balances at a specific outpoint
       * @param outpoint - Outpoint in format "txid:vout"
       */
      async getOutpointBalances(outpoint) {
        const result = await this.provider.espoGetOutpointBalances(outpoint);
        return mapToObject(result);
      }
      /**
       * Get holders of an alkane token with pagination
       * @param alkaneId - Alkane ID in format "block:tx"
       * @param page - Page number (default: 0)
       * @param limit - Items per page (default: 100)
       */
      async getHolders(alkaneId, page = 0, limit = 100) {
        const result = await this.provider.espoGetHolders(alkaneId, page, limit);
        return mapToObject(result);
      }
      /**
       * Get total holder count for an alkane
       * @param alkaneId - Alkane ID in format "block:tx"
       */
      async getHoldersCount(alkaneId) {
        const result = await this.provider.espoGetHoldersCount(alkaneId);
        return result;
      }
      /**
       * Get storage keys for an alkane contract with pagination
       * @param alkaneId - Alkane ID in format "block:tx"
       * @param page - Page number (default: 0)
       * @param limit - Items per page (default: 100)
       */
      async getKeys(alkaneId, page = 0, limit = 100) {
        const result = await this.provider.espoGetKeys(alkaneId, page, limit);
        return mapToObject(result);
      }
      // ============================================================================
      // AMM DATA MODULE
      // ============================================================================
      /**
       * Ping the AMM Data module
       */
      async ammdataPing() {
        return this.provider.espoAmmdataPing();
      }
      /**
       * Get OHLCV candlestick data for a pool
       * @param pool - Pool ID in format "block:tx"
       * @param timeframe - Candle timeframe: "10m" | "1h" | "1d" | "1w" | "1M"
       * @param side - Price side: "base" | "quote"
       * @param limit - Number of candles (default: 100)
       * @param page - Page number (default: 0)
       */
      async getCandles(pool, timeframe, side, limit, page) {
        const result = await this.provider.espoGetCandles(
          pool,
          timeframe,
          side,
          limit,
          page
        );
        return mapToObject(result);
      }
      /**
       * Get trade history for a pool
       * @param pool - Pool ID in format "block:tx"
       * @param limit - Number of trades (default: 100)
       * @param page - Page number (default: 0)
       * @param side - Price side: "base" | "quote"
       * @param filterSide - Filter by trade side: "buy" | "sell" | "all"
       * @param sort - Sort field
       * @param dir - Sort direction: "asc" | "desc"
       */
      async getTrades(pool, limit, page, side, filterSide, sort, dir) {
        const result = await this.provider.espoGetTrades(
          pool,
          limit,
          page,
          side,
          filterSide,
          sort,
          dir
        );
        return mapToObject(result);
      }
      /**
       * Get all pools with pagination
       * @param limit - Number of pools (default: 100)
       * @param page - Page number (default: 0)
       */
      async getPools(limit, page) {
        const result = await this.provider.espoGetPools(limit, page);
        return mapToObject(result);
      }
      /**
       * Find the best swap path between two tokens
       * @param tokenIn - Input token ID
       * @param tokenOut - Output token ID
       * @param mode - Swap mode: "exact_in" | "exact_out" | "implicit"
       * @param amountIn - Input amount
       * @param amountOut - Output amount
       * @param amountOutMin - Minimum output amount
       * @param amountInMax - Maximum input amount
       * @param availableIn - Available input amount
       * @param feeBps - Fee in basis points
       * @param maxHops - Maximum swap hops
       */
      async findBestSwapPath(tokenIn, tokenOut, mode, amountIn, amountOut, amountOutMin, amountInMax, availableIn, feeBps, maxHops) {
        const result = await this.provider.espoFindBestSwapPath(
          tokenIn,
          tokenOut,
          mode,
          amountIn,
          amountOut,
          amountOutMin,
          amountInMax,
          availableIn,
          feeBps,
          maxHops
        );
        return mapToObject(result);
      }
      /**
       * Find the best MEV swap opportunity for a token
       * @param token - Token ID
       * @param feeBps - Fee in basis points
       * @param maxHops - Maximum swap hops
       */
      async getBestMevSwap(token, feeBps, maxHops) {
        const result = await this.provider.espoGetBestMevSwap(
          token,
          feeBps,
          maxHops
        );
        return mapToObject(result);
      }
    };
    Logger = class {
      constructor(level = "off") {
        this.levels = {
          off: 0,
          error: 1,
          warn: 2,
          info: 3,
          debug: 4,
          trace: 5
        };
        this.level = level;
      }
      setLevel(level) {
        this.level = level;
      }
      shouldLog(msgLevel) {
        return this.levels[msgLevel] <= this.levels[this.level];
      }
      error(...args) {
        if (this.shouldLog("error")) console.error("[SDK Error]", ...args);
      }
      warn(...args) {
        if (this.shouldLog("warn")) console.warn("[SDK Warn]", ...args);
      }
      info(...args) {
        if (this.shouldLog("info")) console.info("[SDK Info]", ...args);
      }
      debug(...args) {
        if (this.shouldLog("debug")) console.log("[SDK Debug]", ...args);
      }
      trace(...args) {
        if (this.shouldLog("trace")) console.log("[SDK Trace]", ...args);
      }
    };
    logger = new Logger();
    AlkanesProvider = class {
      constructor(config) {
        this._provider = null;
        this._bitcoin = null;
        this._esplora = null;
        this._alkanes = null;
        this._dataApi = null;
        this._espo = null;
        this._lua = null;
        this._metashrew = null;
        this._ord = null;
        this._brc20prog = null;
        const preset = NETWORK_PRESETS[config.network] || NETWORK_PRESETS["mainnet"];
        this.networkPreset = config.network;
        this.networkType = preset.networkType;
        this.rpcUrl = config.rpcUrl || preset.rpcUrl;
        this.dataApiUrl = config.dataApiUrl || config.rpcUrl || preset.dataApiUrl;
        this.logLevel = config.logLevel || getLogLevelFromEnv() || "off";
        logger.setLevel(this.logLevel);
        if (config.bitcoinNetwork) {
          this.network = config.bitcoinNetwork;
        } else {
          switch (this.networkType) {
            case "mainnet":
              this.network = bitcoin.networks.bitcoin;
              break;
            case "testnet":
            case "signet":
              this.network = bitcoin.networks.testnet;
              break;
            case "regtest":
            default:
              this.network = bitcoin.networks.regtest;
          }
        }
        logger.debug(`Provider configured for ${this.networkType} (${this.rpcUrl})`);
      }
      /**
       * Initialize the provider (loads WASM if needed)
       *
       * This method handles cross-platform WASM loading for both Node.js and browser environments.
       */
      async initialize() {
        if (this._provider) return;
        let WebProviderClass;
        const isNode = typeof process !== "undefined" && process.versions != null && process.versions.node != null;
        if (isNode) {
          const loaderPath = "@alkanes/ts-sdk/wasm/node-loader.cjs";
          const nodeLoaderModule = await import(
            /* @vite-ignore */
            loaderPath
          );
          const nodeLoader = nodeLoaderModule.default || nodeLoaderModule;
          await nodeLoader.init();
          WebProviderClass = nodeLoader.WebProvider;
        } else {
          const wasmPath = "@alkanes/ts-sdk/wasm";
          const wasm = await import(
            /* @vite-ignore */
            wasmPath
          );
          WebProviderClass = wasm.WebProvider;
        }
        const providerName = this.networkPreset === "local" ? "regtest" : this.networkPreset;
        const configOverride = {
          jsonrpc_url: this.rpcUrl
        };
        this._provider = new WebProviderClass(
          providerName,
          configOverride
        );
      }
      /**
       * Get the underlying WASM provider (initializes if needed)
       */
      async getProvider() {
        if (!this._provider) {
          await this.initialize();
        }
        return this._provider;
      }
      /**
       * Get the raw WASM WebProvider for direct access to low-level methods.
       *
       * This is useful for CLI tools that need access to wallet methods
       * like wallet_create_js, wallet_load_js, etc. that are not wrapped
       * by the higher-level API.
       *
       * @throws Error if provider is not initialized
       */
      get rawProvider() {
        if (!this._provider) {
          throw new Error("Provider not initialized. Call initialize() first.");
        }
        return this._provider;
      }
      /**
       * Bitcoin RPC client
       */
      get bitcoin() {
        if (!this._bitcoin) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._bitcoin = new BitcoinRpcClient(this._provider);
        }
        return this._bitcoin;
      }
      /**
       * Esplora API client
       */
      get esplora() {
        if (!this._esplora) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._esplora = new EsploraClient(this._provider);
        }
        return this._esplora;
      }
      /**
       * Alkanes RPC client
       */
      get alkanes() {
        if (!this._alkanes) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._alkanes = new AlkanesRpcClient(this._provider);
        }
        return this._alkanes;
      }
      /**
       * Data API client
       */
      get dataApi() {
        if (!this._dataApi) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._dataApi = new DataApiClient(this._provider);
        }
        return this._dataApi;
      }
      /**
       * Espo client
       *
       * Provides access to Espo indexer for alkanes data and AMM analytics.
       * - Essentials module: balances, holders, storage keys
       * - AMM Data module: candles, trades, pools, swap routing
       */
      get espo() {
        if (!this._espo) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._espo = new EspoClient(this._provider);
        }
        return this._espo;
      }
      /**
       * Lua script execution client
       *
       * Provides Lua script execution with automatic scripthash caching.
       * This is the recommended way to execute Lua scripts for optimal performance.
       */
      get lua() {
        if (!this._lua) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._lua = new LuaClient(this._provider);
        }
        return this._lua;
      }
      /**
       * Metashrew RPC client
       *
       * Provides low-level access to metashrew_view RPC calls.
       * For most use cases, prefer the higher-level methods on alkanes or the convenience methods.
       */
      get metashrew() {
        if (!this._metashrew) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._metashrew = new MetashrewClient(this._provider);
        }
        return this._metashrew;
      }
      /**
       * Ord (Ordinals) RPC client
       */
      get ord() {
        if (!this._ord) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._ord = new OrdClient(this._provider);
        }
        return this._ord;
      }
      /**
       * BRC-20 Prog (Programmable BRC-20) RPC client
       */
      get brc20prog() {
        if (!this._brc20prog) {
          if (!this._provider) {
            throw new Error("Provider not initialized. Call initialize() first.");
          }
          this._brc20prog = new Brc20ProgClient(this._provider);
        }
        return this._brc20prog;
      }
      // ============================================================================
      // CONVENIENCE METHODS
      // ============================================================================
      /**
       * Get BTC balance for an address
       */
      async getBalance(address) {
        const provider = await this.getProvider();
        const info2 = await provider.esploraGetAddressInfo(address);
        const utxos = await provider.esploraGetAddressUtxo(address);
        return {
          address,
          confirmed: info2.chain_stats?.funded_txo_sum - info2.chain_stats?.spent_txo_sum || 0,
          unconfirmed: info2.mempool_stats?.funded_txo_sum - info2.mempool_stats?.spent_txo_sum || 0,
          utxos
        };
      }
      /**
       * Get enriched balances (BTC + alkanes) for an address
       */
      async getEnrichedBalances(address, protocolTag) {
        const provider = await this.getProvider();
        return provider.getEnrichedBalances(address, protocolTag);
      }
      /**
       * Get alkane token balance for an address
       */
      async getAlkaneBalance(address, alkaneId) {
        const provider = await this.getProvider();
        const balances = await provider.alkanesBalance(address);
        if (alkaneId) {
          return balances.filter(
            (b) => b.id?.block === alkaneId.block && b.id?.tx === alkaneId.tx
          );
        }
        return balances;
      }
      /**
       * Get alkane token details
       */
      async getAlkaneTokenDetails(params) {
        const provider = await this.getProvider();
        const id = `${params.alkaneId.block}:${params.alkaneId.tx}`;
        const nameResult = await provider.alkanesView(id, "name", void 0, void 0);
        const symbolResult = await provider.alkanesView(id, "symbol", void 0, void 0);
        const decimalsResult = await provider.alkanesView(id, "decimals", void 0, void 0);
        const totalSupplyResult = await provider.alkanesView(id, "totalSupply", void 0, void 0);
        return {
          id: params.alkaneId,
          name: nameResult?.data || "",
          symbol: symbolResult?.data || "",
          decimals: decimalsResult?.data || 8,
          totalSupply: totalSupplyResult?.data || "0"
        };
      }
      /**
       * Get transaction history for an address (first page, max 25 transactions)
       */
      async getAddressHistory(address) {
        const provider = await this.getProvider();
        return provider.getAddressTxs(address);
      }
      /**
       * Get transaction history for an address from Esplora (first page, max 25 transactions)
       */
      async getAddressTxs(address) {
        const provider = await this.getProvider();
        return provider.esploraGetAddressTxs(address);
      }
      /**
       * Get next page of transaction history for an address
       * @param address The address to fetch transactions for
       * @param lastSeenTxid The last transaction ID from the previous page (undefined for first page)
       */
      async getAddressTxsChain(address, lastSeenTxid) {
        const provider = await this.getProvider();
        return provider.esploraGetAddressTxsChain(address, lastSeenTxid);
      }
      /**
       * Get storage value at a specific path for an alkane
       * @param block - Block number of the alkane
       * @param tx - Transaction number of the alkane
       * @param path - Storage path as bytes (use TextEncoder to convert string to bytes)
       * @returns Hex string (0x-prefixed) of the storage value
       */
      async getStorageAt(block, tx, path3) {
        const provider = await this.getProvider();
        return provider.getStorageAt(BigInt(block), BigInt(tx), Array.from(path3));
      }
      /**
       * Get address history with alkane traces
       */
      async getAddressHistoryWithTraces(address, excludeCoinbase) {
        const provider = await this.getProvider();
        return provider.getAddressTxsWithTraces(address, excludeCoinbase);
      }
      /**
       * Get current block height
       */
      async getBlockHeight() {
        const provider = await this.getProvider();
        return provider.metashrewHeight();
      }
      /**
       * Broadcast a transaction
       */
      async broadcastTransaction(txHex) {
        const provider = await this.getProvider();
        return provider.broadcastTransaction(txHex);
      }
      /**
       * Get all AMM pools from a factory
       */
      async getAllPools(factoryId) {
        const provider = await this.getProvider();
        return provider.alkanesGetAllPoolsWithDetails(factoryId, void 0, void 0);
      }
      /**
       * Get pool reserves
       */
      async getPoolReserves(poolId) {
        const provider = await this.getProvider();
        return provider.dataApiGetReserves(poolId);
      }
      /**
       * Get recent trades for a pool
       */
      async getPoolTrades(poolId, limit) {
        const provider = await this.getProvider();
        return provider.dataApiGetTrades(poolId, void 0, void 0, limit ? BigInt(limit) : void 0);
      }
      /**
       * Get candle data for a pool
       */
      async getPoolCandles(poolId, interval = "1h", limit) {
        const provider = await this.getProvider();
        return provider.dataApiGetCandles(poolId, interval, void 0, void 0, limit ? BigInt(limit) : void 0);
      }
      /**
       * Get Bitcoin price in USD
       */
      async getBitcoinPrice() {
        const provider = await this.getProvider();
        const result = await provider.dataApiGetBitcoinPrice();
        return result?.price || 0;
      }
      /**
       * Execute an alkanes contract call
       */
      async executeAlkanes(params) {
        const provider = await this.getProvider();
        const paramsJson = JSON.stringify({
          target: params.contractId,
          calldata: params.calldata,
          fee_rate: params.feeRate,
          inputs: params.inputs
        });
        return provider.alkanesExecute(paramsJson);
      }
      /**
       * Simulate an alkanes contract call (read-only)
       */
      async simulateAlkanes(contractId, calldata, blockTag) {
        const provider = await this.getProvider();
        const context = {
          alkanes: [],
          transaction: [],
          block: [],
          height: 0,
          vout: 0,
          txindex: 0,
          calldata,
          pointer: 0,
          refund_pointer: 0
        };
        return provider.alkanesSimulate(contractId, JSON.stringify(context), blockTag);
      }
      // ============================================================================
      // WALLET OPERATIONS
      // ============================================================================
      /**
       * Create a new wallet
       *
       * @param options - Wallet creation options
       * @returns Wallet info including address and mnemonic
       *
       * @example
       * ```typescript
       * const wallet = await provider.walletCreate();
       * console.log('Address:', wallet.address);
       * console.log('Mnemonic:', wallet.mnemonic); // Save this!
       * ```
       */
      walletCreate(options) {
        if (!this._provider) {
          throw new Error("Provider not initialized. Call initialize() first.");
        }
        return this._provider.walletCreate(
          options?.mnemonic ?? void 0,
          options?.passphrase ?? void 0
        );
      }
      /**
       * Load an existing wallet from storage
       *
       * @param passphrase - Optional passphrase for BIP39
       */
      async walletLoad(passphrase) {
        const provider = await this.getProvider();
        return provider.walletLoad(passphrase ?? void 0);
      }
      /**
       * Load a wallet from mnemonic for signing transactions
       *
       * @param mnemonic - The mnemonic phrase
       * @param passphrase - Optional BIP39 passphrase
       */
      walletLoadMnemonic(mnemonic, passphrase) {
        if (!this._provider) {
          throw new Error("Provider not initialized. Call initialize() first.");
        }
        this._provider.walletLoadMnemonic(mnemonic, passphrase ?? void 0);
      }
      /**
       * Check if wallet is loaded (has keystore for signing)
       */
      walletIsLoaded() {
        if (!this._provider) {
          return false;
        }
        return this._provider.walletIsLoaded();
      }
      /**
       * Get the wallet's primary address
       */
      async walletGetAddress() {
        const provider = await this.getProvider();
        const result = await provider.walletGetAddress();
        return result?.address || result;
      }
      /**
       * Get addresses from the loaded wallet
       *
       * @param addressType - Address type: 'p2tr', 'p2wpkh', 'p2sh-p2wpkh', 'p2pkh'
       * @param startIndex - Starting derivation index
       * @param count - Number of addresses to generate
       * @param chain - Optional chain (0 = external/receiving, 1 = internal/change)
       * @returns Array of address info objects
       *
       * @example
       * ```typescript
       * const addresses = provider.walletGetAddresses('p2tr', 0, 5);
       * console.log(addresses[0].address);
       * ```
       */
      walletGetAddresses(addressType, startIndex, count, chain) {
        if (!this._provider) {
          throw new Error("Provider not initialized. Call initialize() first.");
        }
        return this._provider.walletGetAddresses(addressType, startIndex, count, chain ?? void 0);
      }
      /**
       * Get wallet BTC balance
       *
       * @param addresses - Optional specific addresses to check
       */
      async walletGetBalance(addresses) {
        const provider = await this.getProvider();
        return provider.walletGetBalance(addresses ?? void 0);
      }
      /**
       * Get wallet UTXOs
       *
       * @param addresses - Optional specific addresses to check
       */
      async walletGetUtxos(addresses) {
        const provider = await this.getProvider();
        return provider.walletGetUtxos(addresses ?? void 0);
      }
      // ============================================================================
      // TYPED ALKANES EXECUTE OPERATIONS
      // ============================================================================
      /**
       * Execute an Alkanes contract call with full control (typed parameters)
       *
       * This method accepts TypeScript objects and handles JSON serialization internally.
       *
       * @param params - Execute parameters
       * @returns Execution result
       *
       * @example
       * ```typescript
       * // Mint DIESEL tokens
       * const result = await provider.alkanesExecuteTyped({
       *   toAddresses: [myAddress],
       *   inputRequirements: 'B:10000',
       *   protostones: '[2,0,77]:v0:v0',
       *   feeRate: 100,
       * });
       * ```
       */
      async alkanesExecuteTyped(params) {
        const provider = await this.getProvider();
        const options = {};
        if (params.fromAddresses !== void 0) options.from_addresses = params.fromAddresses;
        if (params.changeAddress !== void 0) options.change_address = params.changeAddress;
        if (params.alkanesChangeAddress !== void 0) options.alkanes_change_address = params.alkanesChangeAddress;
        if (params.traceEnabled !== void 0) options.trace_enabled = params.traceEnabled;
        if (params.mineEnabled !== void 0) options.mine_enabled = params.mineEnabled;
        if (params.autoConfirm !== void 0) options.auto_confirm = params.autoConfirm;
        if (params.rawOutput !== void 0) options.raw_output = params.rawOutput;
        const optionsJson = Object.keys(options).length > 0 ? JSON.stringify(options) : null;
        let result = await provider.alkanesExecuteWithStrings(
          JSON.stringify(params.toAddresses),
          params.inputRequirements,
          params.protostones,
          params.feeRate ?? null,
          params.envelopeHex ?? null,
          optionsJson
        );
        result = typeof result === "string" ? JSON.parse(result) : result;
        const autoConfirm = params.autoConfirm !== false;
        if (autoConfirm) {
          if (result.readyToSignCommit) {
            const commitStateJson = JSON.stringify(result.readyToSignCommit);
            let commitResult = await provider.alkanesResumeCommitExecution(commitStateJson);
            commitResult = typeof commitResult === "string" ? JSON.parse(commitResult) : commitResult;
            if (commitResult.readyToSignReveal) {
              const revealStateJson = JSON.stringify(commitResult.readyToSignReveal);
              let revealResult = await provider.alkanesResumeRevealExecution(revealStateJson);
              revealResult = typeof revealResult === "string" ? JSON.parse(revealResult) : revealResult;
              return revealResult;
            }
            return commitResult;
          }
          if (result.readyToSign) {
            const paramsJson = JSON.stringify({
              fee_rate: params.feeRate,
              to_addresses: params.toAddresses,
              from_addresses: params.fromAddresses,
              change_address: params.changeAddress,
              alkanes_change_address: params.alkanesChangeAddress,
              input_requirements: [],
              protostones: [],
              envelope_data: null,
              raw_output: params.rawOutput ?? false,
              trace_enabled: params.traceEnabled ?? false,
              mine_enabled: params.mineEnabled ?? false,
              auto_confirm: true
            });
            const stateJson = JSON.stringify(result.readyToSign);
            let execResult = await provider.alkanesResumeExecution(stateJson, paramsJson);
            execResult = typeof execResult === "string" ? JSON.parse(execResult) : execResult;
            return execResult;
          }
        }
        return result;
      }
      /**
       * Wrap BTC to frBTC (typed parameters)
       *
       * @param params - Wrap parameters
       * @returns Transaction result
       *
       * @example
       * ```typescript
       * const result = await provider.frbtcWrapTyped({
       *   amount: 100000n,
       *   toAddress: myAddress,
       *   feeRate: 100,
       *   mineEnabled: true, // Auto-mine on regtest
       * });
       * ```
       */
      async frbtcWrapTyped(params) {
        const provider = await this.getProvider();
        const wrapParams = {
          amount: String(params.amount),
          to_address: params.toAddress,
          fee_rate: params.feeRate ?? 1,
          auto_confirm: params.autoConfirm ?? true,
          trace_enabled: params.traceEnabled ?? false,
          mine_enabled: params.mineEnabled ?? false
        };
        if (params.fromAddress) wrapParams.from_address = params.fromAddress;
        if (params.changeAddress) wrapParams.change_address = params.changeAddress;
        const result = await provider.alkanesWrapBtc(JSON.stringify(wrapParams));
        return typeof result === "string" ? JSON.parse(result) : result;
      }
      /**
       * Initialize a new AMM liquidity pool (typed parameters)
       *
       * @param params - Pool initialization parameters
       * @returns Transaction ID
       *
       * @example
       * ```typescript
       * const txid = await provider.alkanesInitPoolTyped({
       *   factoryId: { block: 4, tx: 65522 },
       *   token0: { block: 2, tx: 0 },
       *   token1: { block: 32, tx: 0 },
       *   amount0: '300000000',
       *   amount1: '50000',
       *   toAddress: myAddress,
       *   feeRate: 100,
       * });
       * ```
       */
      async alkanesInitPoolTyped(params) {
        const provider = await this.getProvider();
        const poolParams = {
          factory_id: params.factoryId,
          token0: params.token0,
          token1: params.token1,
          amount0: String(params.amount0),
          amount1: String(params.amount1),
          to_address: params.toAddress,
          fee_rate: params.feeRate ?? 1,
          trace: params.trace ?? false,
          auto_confirm: params.autoConfirm ?? true
        };
        if (params.minimumLp) poolParams.minimum_lp = String(params.minimumLp);
        if (params.fromAddress) poolParams.from_address = params.fromAddress;
        if (params.changeAddress) poolParams.change_address = params.changeAddress;
        return provider.alkanesInitPool(JSON.stringify(poolParams));
      }
      /**
       * Execute an AMM swap (typed parameters)
       *
       * @param params - Swap parameters
       * @returns Transaction ID
       */
      async alkanesSwapTyped(params) {
        const provider = await this.getProvider();
        const swapParams = {
          factory_id: params.factoryId,
          path: params.path,
          input_amount: String(params.inputAmount),
          minimum_output: String(params.minimumOutput),
          expires: params.expires,
          to_address: params.toAddress,
          fee_rate: params.feeRate ?? 1,
          trace: params.trace ?? false,
          auto_confirm: params.autoConfirm ?? true
        };
        if (params.fromAddress) swapParams.from_address = params.fromAddress;
        if (params.changeAddress) swapParams.change_address = params.changeAddress;
        return provider.alkanesSwap(JSON.stringify(swapParams));
      }
    };
  }
});

// src/wasm/alkanes_web_sys.js
var require_alkanes_web_sys = __commonJS({
  "src/wasm/alkanes_web_sys.js"(exports2, module2) {
    "use strict";
    var imports = {};
    imports["__wbindgen_placeholder__"] = module2.exports;
    var cachedUint8ArrayMemory0 = null;
    function getUint8ArrayMemory0() {
      if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
      }
      return cachedUint8ArrayMemory0;
    }
    var cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
    cachedTextDecoder.decode();
    function decodeText(ptr, len) {
      return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
    }
    function getStringFromWasm0(ptr, len) {
      ptr = ptr >>> 0;
      return decodeText(ptr, len);
    }
    var WASM_VECTOR_LEN = 0;
    var cachedTextEncoder = new TextEncoder();
    if (!("encodeInto" in cachedTextEncoder)) {
      cachedTextEncoder.encodeInto = function(arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
          read: arg.length,
          written: buf.length
        };
      };
    }
    function passStringToWasm0(arg, malloc, realloc) {
      if (realloc === void 0) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr2 = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr2, ptr2 + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr2;
      }
      let len = arg.length;
      let ptr = malloc(len, 1) >>> 0;
      const mem = getUint8ArrayMemory0();
      let offset = 0;
      for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 127) break;
        mem[ptr + offset] = code;
      }
      if (offset !== len) {
        if (offset !== 0) {
          arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);
        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
      }
      WASM_VECTOR_LEN = offset;
      return ptr;
    }
    var cachedDataViewMemory0 = null;
    function getDataViewMemory0() {
      if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || cachedDataViewMemory0.buffer.detached === void 0 && cachedDataViewMemory0.buffer !== wasm.memory.buffer) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
      }
      return cachedDataViewMemory0;
    }
    function isLikeNone(x) {
      return x === void 0 || x === null;
    }
    function debugString(val) {
      const type = typeof val;
      if (type == "number" || type == "boolean" || val == null) {
        return `${val}`;
      }
      if (type == "string") {
        return `"${val}"`;
      }
      if (type == "symbol") {
        const description = val.description;
        if (description == null) {
          return "Symbol";
        } else {
          return `Symbol(${description})`;
        }
      }
      if (type == "function") {
        const name = val.name;
        if (typeof name == "string" && name.length > 0) {
          return `Function(${name})`;
        } else {
          return "Function";
        }
      }
      if (Array.isArray(val)) {
        const length = val.length;
        let debug = "[";
        if (length > 0) {
          debug += debugString(val[0]);
        }
        for (let i = 1; i < length; i++) {
          debug += ", " + debugString(val[i]);
        }
        debug += "]";
        return debug;
      }
      const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
      let className;
      if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
      } else {
        return toString.call(val);
      }
      if (className == "Object") {
        try {
          return "Object(" + JSON.stringify(val) + ")";
        } catch (_) {
          return "Object";
        }
      }
      if (val instanceof Error) {
        return `${val.name}: ${val.message}
${val.stack}`;
      }
      return className;
    }
    function addToExternrefTable0(obj) {
      const idx = wasm.__externref_table_alloc();
      wasm.__wbindgen_externrefs.set(idx, obj);
      return idx;
    }
    function handleError(f, args) {
      try {
        return f.apply(this, args);
      } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
      }
    }
    function getArrayU8FromWasm0(ptr, len) {
      ptr = ptr >>> 0;
      return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
    }
    var CLOSURE_DTORS = typeof FinalizationRegistry === "undefined" ? { register: () => {
    }, unregister: () => {
    } } : new FinalizationRegistry((state) => state.dtor(state.a, state.b));
    function makeMutClosure(arg0, arg1, dtor, f) {
      const state = { a: arg0, b: arg1, cnt: 1, dtor };
      const real = (...args) => {
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
          return f(a, state.b, ...args);
        } finally {
          state.a = a;
          real._wbg_cb_unref();
        }
      };
      real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
          state.dtor(state.a, state.b);
          state.a = 0;
          CLOSURE_DTORS.unregister(state);
        }
      };
      CLOSURE_DTORS.register(real, state, state);
      return real;
    }
    function takeFromExternrefTable0(idx) {
      const value = wasm.__wbindgen_externrefs.get(idx);
      wasm.__externref_table_dealloc(idx);
      return value;
    }
    function passArray8ToWasm0(arg, malloc) {
      const ptr = malloc(arg.length * 1, 1) >>> 0;
      getUint8ArrayMemory0().set(arg, ptr / 1);
      WASM_VECTOR_LEN = arg.length;
      return ptr;
    }
    function passArrayJsValueToWasm0(array, malloc) {
      const ptr = malloc(array.length * 4, 4) >>> 0;
      for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
      }
      WASM_VECTOR_LEN = array.length;
      return ptr;
    }
    exports2.encryptMnemonic = function(mnemonic, passphrase) {
      const ptr0 = passStringToWasm0(mnemonic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ret = wasm.encryptMnemonic(ptr0, len0, ptr1, len1);
      return ret;
    };
    exports2.analyze_psbt = function(psbt_base64, network_str) {
      let deferred4_0;
      let deferred4_1;
      try {
        const ptr0 = passStringToWasm0(psbt_base64, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(network_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.analyze_psbt(ptr0, len0, ptr1, len1);
        var ptr3 = ret[0];
        var len3 = ret[1];
        if (ret[3]) {
          ptr3 = 0;
          len3 = 0;
          throw takeFromExternrefTable0(ret[2]);
        }
        deferred4_0 = ptr3;
        deferred4_1 = len3;
        return getStringFromWasm0(ptr3, len3);
      } finally {
        wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
      }
    };
    exports2.simulate_alkane_call = function(alkane_id_str, wasm_hex, cellpack_hex) {
      const ptr0 = passStringToWasm0(alkane_id_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(wasm_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ptr2 = passStringToWasm0(cellpack_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len2 = WASM_VECTOR_LEN;
      const ret = wasm.simulate_alkane_call(ptr0, len0, ptr1, len1, ptr2, len2);
      return ret;
    };
    exports2.get_alkane_bytecode = function(network, block, tx, block_tag) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ret = wasm.get_alkane_bytecode(ptr0, len0, block, tx, ptr1, len1);
      return ret;
    };
    exports2.analyze_runestone = function(tx_hex) {
      let deferred3_0;
      let deferred3_1;
      try {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.analyze_runestone(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
          ptr2 = 0;
          len2 = 0;
          throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
      } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
      }
    };
    exports2.decode_psbt = function(psbt_base64) {
      let deferred3_0;
      let deferred3_1;
      try {
        const ptr0 = passStringToWasm0(psbt_base64, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.decode_psbt(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
          ptr2 = 0;
          len2 = 0;
          throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
      } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
      }
    };
    exports2.brc20_prog_deploy_contract = function(network, foundry_json, params_json) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(foundry_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ptr2 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len2 = WASM_VECTOR_LEN;
      const ret = wasm.brc20_prog_deploy_contract(ptr0, len0, ptr1, len1, ptr2, len2);
      return ret;
    };
    exports2.brc20_prog_transact = function(network, contract_address, function_signature, calldata, params_json) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(contract_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ptr2 = passStringToWasm0(function_signature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len2 = WASM_VECTOR_LEN;
      const ptr3 = passStringToWasm0(calldata, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len3 = WASM_VECTOR_LEN;
      const ptr4 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len4 = WASM_VECTOR_LEN;
      const ret = wasm.brc20_prog_transact(ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
      return ret;
    };
    exports2.brc20_prog_wrap_btc = function(network, amount, target_contract, function_signature, calldata, params_json) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(target_contract, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ptr2 = passStringToWasm0(function_signature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len2 = WASM_VECTOR_LEN;
      const ptr3 = passStringToWasm0(calldata, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len3 = WASM_VECTOR_LEN;
      const ptr4 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len4 = WASM_VECTOR_LEN;
      const ret = wasm.brc20_prog_wrap_btc(ptr0, len0, amount, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
      return ret;
    };
    exports2.frbtc_wrap = function(network, amount, params_json) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ret = wasm.frbtc_wrap(ptr0, len0, amount, ptr1, len1);
      return ret;
    };
    exports2.frbtc_unwrap = function(network, amount, vout, recipient_address, params_json) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(recipient_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ptr2 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len2 = WASM_VECTOR_LEN;
      const ret = wasm.frbtc_unwrap(ptr0, len0, amount, vout, ptr1, len1, ptr2, len2);
      return ret;
    };
    exports2.frbtc_wrap_and_execute = function(network, amount, script_bytecode, params_json) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(script_bytecode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ptr2 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len2 = WASM_VECTOR_LEN;
      const ret = wasm.frbtc_wrap_and_execute(ptr0, len0, amount, ptr1, len1, ptr2, len2);
      return ret;
    };
    exports2.frbtc_wrap_and_execute2 = function(network, amount, target_address, function_signature, calldata_args, params_json) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ptr1 = passStringToWasm0(target_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      const ptr2 = passStringToWasm0(function_signature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len2 = WASM_VECTOR_LEN;
      const ptr3 = passStringToWasm0(calldata_args, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len3 = WASM_VECTOR_LEN;
      const ptr4 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len4 = WASM_VECTOR_LEN;
      const ret = wasm.frbtc_wrap_and_execute2(ptr0, len0, amount, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
      return ret;
    };
    exports2.frbtc_get_signer_address = function(network) {
      const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      const ret = wasm.frbtc_get_signer_address(ptr0, len0);
      return ret;
    };
    function wasm_bindgen__convert__closures_____invoke__h5943629905d90057(arg0, arg1, arg2) {
      wasm.wasm_bindgen__convert__closures_____invoke__h5943629905d90057(arg0, arg1, arg2);
    }
    function wasm_bindgen__convert__closures_____invoke__h8d00541115e24acb(arg0, arg1) {
      wasm.wasm_bindgen__convert__closures_____invoke__h8d00541115e24acb(arg0, arg1);
    }
    function wasm_bindgen__convert__closures_____invoke__h95fdbac5e4c1bfb6(arg0, arg1, arg2, arg3) {
      wasm.wasm_bindgen__convert__closures_____invoke__h95fdbac5e4c1bfb6(arg0, arg1, arg2, arg3);
    }
    var __wbindgen_enum_RequestCache = ["default", "no-store", "reload", "no-cache", "force-cache", "only-if-cached"];
    var __wbindgen_enum_RequestCredentials = ["omit", "same-origin", "include"];
    var __wbindgen_enum_RequestMode = ["same-origin", "no-cors", "cors", "navigate"];
    var KeystoreFinalization = typeof FinalizationRegistry === "undefined" ? { register: () => {
    }, unregister: () => {
    } } : new FinalizationRegistry((ptr) => wasm.__wbg_keystore_free(ptr >>> 0, 1));
    var Keystore = class {
      __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KeystoreFinalization.unregister(this);
        return ptr;
      }
      free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_keystore_free(ptr, 0);
      }
      /**
       * @param {any} val
       */
      constructor(val) {
        const ret = wasm.keystore_from_js(val);
        if (ret[2]) {
          throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        KeystoreFinalization.register(this, this.__wbg_ptr, this);
        return this;
      }
      /**
       * @returns {any}
       */
      to_js() {
        const ret = wasm.keystore_to_js(this.__wbg_ptr);
        if (ret[2]) {
          throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
      }
      /**
       * @returns {string}
       */
      accountXpub() {
        let deferred1_0;
        let deferred1_1;
        try {
          const ret = wasm.keystore_accountXpub(this.__wbg_ptr);
          deferred1_0 = ret[0];
          deferred1_1 = ret[1];
          return getStringFromWasm0(ret[0], ret[1]);
        } finally {
          wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
      }
      /**
       * @returns {any}
       */
      hdPaths() {
        const ret = wasm.keystore_hdPaths(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {string}
       */
      masterFingerprint() {
        let deferred1_0;
        let deferred1_1;
        try {
          const ret = wasm.keystore_masterFingerprint(this.__wbg_ptr);
          deferred1_0 = ret[0];
          deferred1_1 = ret[1];
          return getStringFromWasm0(ret[0], ret[1]);
        } finally {
          wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
      }
      /**
       * @param {string} passphrase
       * @returns {Promise<any>}
       */
      decryptMnemonic(passphrase) {
        const ptr0 = passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.keystore_decryptMnemonic(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
    };
    if (Symbol.dispose) Keystore.prototype[Symbol.dispose] = Keystore.prototype.free;
    exports2.Keystore = Keystore;
    var PbkdfParamsFinalization = typeof FinalizationRegistry === "undefined" ? { register: () => {
    }, unregister: () => {
    } } : new FinalizationRegistry((ptr) => wasm.__wbg_pbkdfparams_free(ptr >>> 0, 1));
    var PbkdfParams = class {
      __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        PbkdfParamsFinalization.unregister(this);
        return ptr;
      }
      free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_pbkdfparams_free(ptr, 0);
      }
      /**
       * @param {any} val
       */
      constructor(val) {
        const ret = wasm.pbkdfparams_from_js(val);
        if (ret[2]) {
          throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        PbkdfParamsFinalization.register(this, this.__wbg_ptr, this);
        return this;
      }
      /**
       * @returns {any}
       */
      to_js() {
        const ret = wasm.pbkdfparams_to_js(this.__wbg_ptr);
        if (ret[2]) {
          throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
      }
    };
    if (Symbol.dispose) PbkdfParams.prototype[Symbol.dispose] = PbkdfParams.prototype.free;
    exports2.PbkdfParams = PbkdfParams;
    var WasmBrowserWalletProviderFinalization = typeof FinalizationRegistry === "undefined" ? { register: () => {
    }, unregister: () => {
    } } : new FinalizationRegistry((ptr) => wasm.__wbg_wasmbrowserwalletprovider_free(ptr >>> 0, 1));
    var WasmBrowserWalletProvider = class _WasmBrowserWalletProvider {
      static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(_WasmBrowserWalletProvider.prototype);
        obj.__wbg_ptr = ptr;
        WasmBrowserWalletProviderFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
      }
      __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmBrowserWalletProviderFinalization.unregister(this);
        return ptr;
      }
      free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmbrowserwalletprovider_free(ptr, 0);
      }
      /**
       * Create a new BrowserWalletProvider from a JavaScript wallet adapter
       *
       * @param adapter - A JavaScript object implementing the JsWalletAdapter interface
       * @param network - Network string ("mainnet", "testnet", "signet", "regtest")
       * @returns Promise<WasmBrowserWalletProvider>
       * @param {JsWalletAdapter} adapter
       * @param {string} network
       */
      constructor(adapter, network) {
        const ptr0 = passStringToWasm0(network, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_new(adapter, ptr0, len0);
        return ret;
      }
      /**
       * Get the connected wallet address
       * @returns {string | undefined}
       */
      getAddress() {
        const ret = wasm.wasmbrowserwalletprovider_getAddress(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
          v1 = getStringFromWasm0(ret[0], ret[1]).slice();
          wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
      }
      /**
       * Get the wallet public key
       * @returns {Promise<string>}
       */
      getPublicKey() {
        const ret = wasm.wasmbrowserwalletprovider_getPublicKey(this.__wbg_ptr);
        return ret;
      }
      /**
       * Sign a PSBT (hex encoded)
       * @param {string} psbt_hex
       * @param {any} options
       * @returns {Promise<string>}
       */
      signPsbt(psbt_hex, options) {
        const ptr0 = passStringToWasm0(psbt_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_signPsbt(this.__wbg_ptr, ptr0, len0, options);
        return ret;
      }
      /**
       * Sign a message
       * @param {string} message
       * @param {string | null} [address]
       * @returns {Promise<string>}
       */
      signMessage(message, address) {
        const ptr0 = passStringToWasm0(message, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_signMessage(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Broadcast a transaction
       * @param {string} tx_hex
       * @returns {Promise<string>}
       */
      broadcastTransaction(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmbrowserwalletprovider_broadcastTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get balance
       * @returns {Promise<any>}
       */
      getBalance() {
        const ret = wasm.wasmbrowserwalletprovider_getBalance(this.__wbg_ptr);
        return ret;
      }
      /**
       * Get UTXOs
       * @param {boolean} include_frozen
       * @returns {Promise<any>}
       */
      getUtxos(include_frozen) {
        const ret = wasm.wasmbrowserwalletprovider_getUtxos(this.__wbg_ptr, include_frozen);
        return ret;
      }
      /**
       * Get enriched UTXOs with asset information
       * @returns {Promise<any>}
       */
      getEnrichedUtxos() {
        const ret = wasm.wasmbrowserwalletprovider_getEnrichedUtxos(this.__wbg_ptr);
        return ret;
      }
      /**
       * Get all balances (BTC + alkanes)
       * @returns {Promise<any>}
       */
      getAllBalances() {
        const ret = wasm.wasmbrowserwalletprovider_getAllBalances(this.__wbg_ptr);
        return ret;
      }
      /**
       * Get wallet info
       * @returns {any}
       */
      getWalletInfo() {
        const ret = wasm.wasmbrowserwalletprovider_getWalletInfo(this.__wbg_ptr);
        return ret;
      }
      /**
       * Get connection status
       * @returns {string}
       */
      getConnectionStatus() {
        let deferred1_0;
        let deferred1_1;
        try {
          const ret = wasm.wasmbrowserwalletprovider_getConnectionStatus(this.__wbg_ptr);
          deferred1_0 = ret[0];
          deferred1_1 = ret[1];
          return getStringFromWasm0(ret[0], ret[1]);
        } finally {
          wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
      }
      /**
       * Get current network
       * @returns {string}
       */
      getNetwork() {
        let deferred1_0;
        let deferred1_1;
        try {
          const ret = wasm.wasmbrowserwalletprovider_getNetwork(this.__wbg_ptr);
          deferred1_0 = ret[0];
          deferred1_1 = ret[1];
          return getStringFromWasm0(ret[0], ret[1]);
        } finally {
          wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
      }
      /**
       * Disconnect from the wallet
       * @returns {Promise<void>}
       */
      disconnect() {
        const ret = wasm.wasmbrowserwalletprovider_disconnect(this.__wbg_ptr);
        return ret;
      }
    };
    if (Symbol.dispose) WasmBrowserWalletProvider.prototype[Symbol.dispose] = WasmBrowserWalletProvider.prototype.free;
    exports2.WasmBrowserWalletProvider = WasmBrowserWalletProvider;
    var WebProviderFinalization = typeof FinalizationRegistry === "undefined" ? { register: () => {
    }, unregister: () => {
    } } : new FinalizationRegistry((ptr) => wasm.__wbg_webprovider_free(ptr >>> 0, 1));
    var WebProvider = class {
      __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WebProviderFinalization.unregister(this);
        return ptr;
      }
      free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_webprovider_free(ptr, 0);
      }
      /**
       * Create a new WebProvider from provider name and optional config overrides
       *
       * # Arguments
       * * `provider` - Network provider: "mainnet", "signet", "subfrost-regtest", "regtest"
       * * `config` - Optional JS object with RpcConfig fields to override defaults
       *
       * # Example (JavaScript)
       * ```js
       * // Simple - uses all defaults for signet
       * const provider = new WebProvider("signet");
       *
       * // With overrides
       * const provider = new WebProvider("signet", {
       *   bitcoin_rpc_url: "https://custom-rpc.example.com",
       *   esplora_url: "https://custom-esplora.example.com"
       * });
       * ```
       * @param {string} provider
       * @param {any | null} [config]
       */
      constructor(provider, config) {
        const ptr0 = passStringToWasm0(provider, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_new_js(ptr0, len0, isLikeNone(config) ? 0 : addToExternrefTable0(config));
        if (ret[2]) {
          throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WebProviderFinalization.register(this, this.__wbg_ptr, this);
        return this;
      }
      /**
       * @returns {string}
       */
      sandshrew_rpc_url() {
        let deferred1_0;
        let deferred1_1;
        try {
          const ret = wasm.webprovider_sandshrew_rpc_url(this.__wbg_ptr);
          deferred1_0 = ret[0];
          deferred1_1 = ret[1];
          return getStringFromWasm0(ret[0], ret[1]);
        } finally {
          wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
      }
      /**
       * @returns {string | undefined}
       */
      esplora_rpc_url() {
        const ret = wasm.webprovider_esplora_rpc_url(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
          v1 = getStringFromWasm0(ret[0], ret[1]).slice();
          wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
      }
      /**
       * @returns {string}
       */
      bitcoin_rpc_url() {
        let deferred1_0;
        let deferred1_1;
        try {
          const ret = wasm.webprovider_bitcoin_rpc_url(this.__wbg_ptr);
          deferred1_0 = ret[0];
          deferred1_1 = ret[1];
          return getStringFromWasm0(ret[0], ret[1]);
        } finally {
          wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
      }
      /**
       * @returns {string}
       */
      brc20_prog_rpc_url() {
        let deferred1_0;
        let deferred1_1;
        try {
          const ret = wasm.webprovider_brc20_prog_rpc_url(this.__wbg_ptr);
          deferred1_0 = ret[0];
          deferred1_1 = ret[1];
          return getStringFromWasm0(ret[0], ret[1]);
        } finally {
          wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
      }
      /**
       * Get enriched wallet balances using the balances.lua script
       *
       * This uses the built-in balances.lua script with automatic hash-based caching.
       * Returns comprehensive balance data including spendable UTXOs, asset UTXOs, and pending.
       * @param {string} address
       * @param {string | null} [protocol_tag]
       * @returns {Promise<any>}
       */
      getEnrichedBalances(address, protocol_tag) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(protocol_tag) ? 0 : passStringToWasm0(protocol_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getEnrichedBalances(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get all transactions for an address from Esplora
       * @param {string} address
       * @returns {Promise<any>}
       */
      getAddressTxs(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getAddressTxs(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get raw transaction hex
       * @param {string} txid
       * @returns {Promise<any>}
       */
      getTransactionHex(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getTransactionHex(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Trace alkanes execution for a protostone outpoint
       * @param {string} outpoint
       * @returns {Promise<any>}
       */
      traceOutpoint(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_traceOutpoint(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get address UTXOs
       * @param {string} address
       * @returns {Promise<any>}
       */
      getAddressUtxos(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getAddressUtxos(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Broadcast a raw transaction
       * @param {string} tx_hex
       * @returns {Promise<any>}
       */
      broadcastTransaction(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_broadcastTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get address transactions with complete runestone traces (CLI: esplora address-txs --runestone-trace)
       * @param {string} address
       * @param {boolean | null} [exclude_coinbase]
       * @returns {Promise<any>}
       */
      getAddressTxsWithTraces(address, exclude_coinbase) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getAddressTxsWithTraces(this.__wbg_ptr, ptr0, len0, isLikeNone(exclude_coinbase) ? 16777215 : exclude_coinbase ? 1 : 0);
        return ret;
      }
      /**
       * @param {string} inscription_id
       * @returns {Promise<any>}
       */
      ordInscription(inscription_id) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordInscription(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {number | null} [page]
       * @returns {Promise<any>}
       */
      ordInscriptions(page) {
        const ret = wasm.webprovider_ordInscriptions(this.__wbg_ptr, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      ordOutputs(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordOutputs(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} rune
       * @returns {Promise<any>}
       */
      ordRune(rune) {
        const ptr0 = passStringToWasm0(rune, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordRune(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      ordAddressInfo(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordAddressInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} query
       * @returns {Promise<any>}
       */
      ordBlockInfo(query) {
        const ptr0 = passStringToWasm0(query, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordBlockInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      ordBlockCount() {
        const ret = wasm.webprovider_ordBlockCount(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      ordBlocks() {
        const ret = wasm.webprovider_ordBlocks(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} inscription_id
       * @param {number | null} [page]
       * @returns {Promise<any>}
       */
      ordChildren(inscription_id, page) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordChildren(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
      }
      /**
       * @param {string} inscription_id
       * @returns {Promise<any>}
       */
      ordContent(inscription_id) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordContent(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} inscription_id
       * @param {number | null} [page]
       * @returns {Promise<any>}
       */
      ordParents(inscription_id, page) {
        const ptr0 = passStringToWasm0(inscription_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordParents(this.__wbg_ptr, ptr0, len0, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      ordTxInfo(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordTxInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Execute an alkanes smart contract
       * @param {string} params_json
       * @returns {Promise<any>}
       */
      alkanesExecute(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesExecute(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Execute an alkanes smart contract using CLI-style string parameters
       * This is the recommended method for executing alkanes contracts as it supports
       * the same parameter format as alkanes-cli.
       *
       * # Parameters
       * - `to_addresses`: JSON array of recipient addresses
       * - `input_requirements`: String format like "B:10000" or "2:0:1000" (alkane block:tx:amount)
       * - `protostones`: String format like "[32,0,77]:v0:v0" (cellpack:pointer:refund)
       * - `fee_rate`: Optional fee rate in sat/vB
       * - `envelope_hex`: Optional envelope data as hex string
       * - `options_json`: Optional JSON with additional options (trace_enabled, mine_enabled, auto_confirm, raw_output)
       * @param {string} to_addresses_json
       * @param {string} input_requirements
       * @param {string} protostones
       * @param {number | null} [fee_rate]
       * @param {string | null} [envelope_hex]
       * @param {string | null} [options_json]
       * @returns {Promise<any>}
       */
      alkanesExecuteWithStrings(to_addresses_json, input_requirements, protostones, fee_rate, envelope_hex, options_json) {
        const ptr0 = passStringToWasm0(to_addresses_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(input_requirements, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(protostones, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(envelope_hex) ? 0 : passStringToWasm0(envelope_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(options_json) ? 0 : passStringToWasm0(options_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len4 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesExecuteWithStrings(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, isLikeNone(fee_rate) ? 4294967297 : Math.fround(fee_rate), ptr3, len3, ptr4, len4);
        return ret;
      }
      /**
       * Resume execution after user confirmation (for simple transactions)
       * @param {string} state_json
       * @param {string} params_json
       * @returns {Promise<any>}
       */
      alkanesResumeExecution(state_json, params_json) {
        const ptr0 = passStringToWasm0(state_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesResumeExecution(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Resume execution after commit transaction confirmation
       * @param {string} state_json
       * @returns {Promise<any>}
       */
      alkanesResumeCommitExecution(state_json) {
        const ptr0 = passStringToWasm0(state_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesResumeCommitExecution(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Resume execution after reveal transaction confirmation
       * @param {string} state_json
       * @returns {Promise<any>}
       */
      alkanesResumeRevealExecution(state_json) {
        const ptr0 = passStringToWasm0(state_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesResumeRevealExecution(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Simulate an alkanes contract call (read-only)
       * @param {string} contract_id
       * @param {string} context_json
       * @param {string | null} [block_tag]
       * @returns {Promise<any>}
       */
      alkanesSimulate(contract_id, context_json, block_tag) {
        const ptr0 = passStringToWasm0(contract_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(context_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSimulate(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Wrap BTC to frBTC
       * @param {string} params_json
       * @returns {Promise<any>}
       */
      alkanesWrapBtc(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesWrapBtc(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Initialize a new AMM liquidity pool
       * @param {string} params_json
       * @returns {Promise<any>}
       */
      alkanesInitPool(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesInitPool(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Execute an AMM swap
       * @param {string} params_json
       * @returns {Promise<any>}
       */
      alkanesSwap(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSwap(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Reflect metadata for a range of alkanes
       * @param {number} block
       * @param {number} start_tx
       * @param {number} end_tx
       * @param {number | null} [concurrency]
       * @returns {Promise<any>}
       */
      alkanesReflectAlkaneRange(block, start_tx, end_tx, concurrency) {
        const ret = wasm.webprovider_alkanesReflectAlkaneRange(this.__wbg_ptr, block, start_tx, end_tx, !isLikeNone(concurrency), isLikeNone(concurrency) ? 0 : concurrency);
        return ret;
      }
      /**
       * Execute a tx-script with WASM bytecode
       * @param {string} wasm_hex
       * @param {string} inputs_json
       * @param {string | null} [block_tag]
       * @returns {Promise<any>}
       */
      alkanesTxScript(wasm_hex, inputs_json, block_tag) {
        const ptr0 = passStringToWasm0(wasm_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(inputs_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesTxScript(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get pool details for a specific pool
       * @param {string} pool_id
       * @returns {Promise<any>}
       */
      alkanesPoolDetails(pool_id) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesPoolDetails(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Calculate minimum unwrap amount for subfrost frBTC unwrapping
       * @param {number | null} [fee_rate_override]
       * @param {number | null} [premium]
       * @param {number | null} [expected_inputs]
       * @param {number | null} [expected_outputs]
       * @param {boolean | null} [raw]
       * @returns {Promise<any>}
       */
      subfrostMinimumUnwrap(fee_rate_override, premium, expected_inputs, expected_outputs, raw) {
        const ret = wasm.webprovider_subfrostMinimumUnwrap(this.__wbg_ptr, !isLikeNone(fee_rate_override), isLikeNone(fee_rate_override) ? 0 : fee_rate_override, !isLikeNone(premium), isLikeNone(premium) ? 0 : premium, !isLikeNone(expected_inputs), isLikeNone(expected_inputs) ? 0 : expected_inputs, !isLikeNone(expected_outputs), isLikeNone(expected_outputs) ? 0 : expected_outputs, isLikeNone(raw) ? 16777215 : raw ? 1 : 0);
        return ret;
      }
      /**
       * Get OPI block height
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI extras block height
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiExtrasBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiExtrasBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI database version
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiDbVersion(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiDbVersion(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI event hash version
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiEventHashVersion(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiEventHashVersion(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI balance on block
       * @param {string} base_url
       * @param {number} block_height
       * @param {string} pkscript
       * @param {string} ticker
       * @returns {Promise<any>}
       */
      opiBalanceOnBlock(base_url, block_height, pkscript, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBalanceOnBlock(this.__wbg_ptr, ptr0, len0, block_height, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get OPI activity on block
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiActivityOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiActivityOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI Bitcoin RPC results on block
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiBitcoinRpcResultsOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitcoinRpcResultsOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI current balance
       * @param {string} base_url
       * @param {string} ticker
       * @param {string | null} [address]
       * @param {string | null} [pkscript]
       * @returns {Promise<any>}
       */
      opiCurrentBalance(base_url, ticker, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiCurrentBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret;
      }
      /**
       * Get OPI valid tx notes of wallet
       * @param {string} base_url
       * @param {string | null} [address]
       * @param {string | null} [pkscript]
       * @returns {Promise<any>}
       */
      opiValidTxNotesOfWallet(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiValidTxNotesOfWallet(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get OPI valid tx notes of ticker
       * @param {string} base_url
       * @param {string} ticker
       * @returns {Promise<any>}
       */
      opiValidTxNotesOfTicker(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiValidTxNotesOfTicker(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI holders
       * @param {string} base_url
       * @param {string} ticker
       * @returns {Promise<any>}
       */
      opiHolders(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiHolders(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI hash of all activity
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI hash of all current balances
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiHashOfAllCurrentBalances(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiHashOfAllCurrentBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI event
       * @param {string} base_url
       * @param {string} event_hash
       * @returns {Promise<any>}
       */
      opiEvent(base_url, event_hash) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(event_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiEvent(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI IP address
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiIp(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiIp(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI raw endpoint
       * @param {string} base_url
       * @param {string} endpoint
       * @returns {Promise<any>}
       */
      opiRaw(base_url, endpoint) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(endpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRaw(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI Runes block height
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiRunesBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI Runes balance on block
       * @param {string} base_url
       * @param {number} block_height
       * @param {string} pkscript
       * @param {string} rune_id
       * @returns {Promise<any>}
       */
      opiRunesBalanceOnBlock(base_url, block_height, pkscript, rune_id) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(rune_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesBalanceOnBlock(this.__wbg_ptr, ptr0, len0, block_height, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get OPI Runes activity on block
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiRunesActivityOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesActivityOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI Runes current balance
       * @param {string} base_url
       * @param {string | null} [address]
       * @param {string | null} [pkscript]
       * @returns {Promise<any>}
       */
      opiRunesCurrentBalance(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesCurrentBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get OPI Runes unspent outpoints
       * @param {string} base_url
       * @param {string | null} [address]
       * @param {string | null} [pkscript]
       * @returns {Promise<any>}
       */
      opiRunesUnspentOutpoints(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesUnspentOutpoints(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get OPI Runes holders
       * @param {string} base_url
       * @param {string} rune_id
       * @returns {Promise<any>}
       */
      opiRunesHolders(base_url, rune_id) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(rune_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesHolders(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI Runes hash of all activity
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiRunesHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI Runes event
       * @param {string} base_url
       * @param {string} txid
       * @returns {Promise<any>}
       */
      opiRunesEvent(base_url, txid) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiRunesEvent(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI Bitmap block height
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiBitmapBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI Bitmap hash of all activity
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiBitmapHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI Bitmap hash of all bitmaps
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiBitmapHashOfAllBitmaps(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapHashOfAllBitmaps(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI Bitmap inscription ID
       * @param {string} base_url
       * @param {string} bitmap
       * @returns {Promise<any>}
       */
      opiBitmapInscriptionId(base_url, bitmap) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(bitmap, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiBitmapInscriptionId(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI POW20 block height
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiPow20BlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20BlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI POW20 balance on block
       * @param {string} base_url
       * @param {number} block_height
       * @param {string} pkscript
       * @param {string} ticker
       * @returns {Promise<any>}
       */
      opiPow20BalanceOnBlock(base_url, block_height, pkscript, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20BalanceOnBlock(this.__wbg_ptr, ptr0, len0, block_height, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get OPI POW20 activity on block
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiPow20ActivityOnBlock(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20ActivityOnBlock(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI POW20 current balance
       * @param {string} base_url
       * @param {string} ticker
       * @param {string | null} [address]
       * @param {string | null} [pkscript]
       * @returns {Promise<any>}
       */
      opiPow20CurrentBalance(base_url, ticker, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20CurrentBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret;
      }
      /**
       * Get OPI POW20 valid tx notes of wallet
       * @param {string} base_url
       * @param {string | null} [address]
       * @param {string | null} [pkscript]
       * @returns {Promise<any>}
       */
      opiPow20ValidTxNotesOfWallet(base_url, address, pkscript) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(pkscript) ? 0 : passStringToWasm0(pkscript, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20ValidTxNotesOfWallet(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * Get OPI POW20 valid tx notes of ticker
       * @param {string} base_url
       * @param {string} ticker
       * @returns {Promise<any>}
       */
      opiPow20ValidTxNotesOfTicker(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20ValidTxNotesOfTicker(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI POW20 holders
       * @param {string} base_url
       * @param {string} ticker
       * @returns {Promise<any>}
       */
      opiPow20Holders(base_url, ticker) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(ticker, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20Holders(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI POW20 hash of all activity
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiPow20HashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20HashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI POW20 hash of all current balances
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiPow20HashOfAllCurrentBalances(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiPow20HashOfAllCurrentBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI SNS block height
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiSnsBlockHeight(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsBlockHeight(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI SNS hash of all activity
       * @param {string} base_url
       * @param {number} block_height
       * @returns {Promise<any>}
       */
      opiSnsHashOfAllActivity(base_url, block_height) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsHashOfAllActivity(this.__wbg_ptr, ptr0, len0, block_height);
        return ret;
      }
      /**
       * Get OPI SNS hash of all registered names
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiSnsHashOfAllRegisteredNames(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsHashOfAllRegisteredNames(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get OPI SNS info
       * @param {string} base_url
       * @param {string} name
       * @returns {Promise<any>}
       */
      opiSnsInfo(base_url, name) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsInfo(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI SNS inscriptions of domain
       * @param {string} base_url
       * @param {string} domain
       * @returns {Promise<any>}
       */
      opiSnsInscriptionsOfDomain(base_url, domain) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(domain, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsInscriptionsOfDomain(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get OPI SNS registered namespaces
       * @param {string} base_url
       * @returns {Promise<any>}
       */
      opiSnsRegisteredNamespaces(base_url) {
        const ptr0 = passStringToWasm0(base_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_opiSnsRegisteredNamespaces(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get alkanes contract balance for an address
       * @param {string | null} [address]
       * @returns {Promise<any>}
       */
      alkanesBalance(address) {
        var ptr0 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesBalance(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get alkanes contract bytecode
       * @param {string} alkane_id
       * @param {string | null} [block_tag]
       * @returns {Promise<any>}
       */
      alkanesBytecode(alkane_id, block_tag) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesBytecode(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * Get all pools with details from an AMM factory (parallel optimized for browser)
       * @param {string} factory_id
       * @param {number | null} [chunk_size]
       * @param {number | null} [max_concurrent]
       * @returns {Promise<any>}
       */
      alkanesGetAllPoolsWithDetails(factory_id, chunk_size, max_concurrent) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesGetAllPoolsWithDetails(this.__wbg_ptr, ptr0, len0, !isLikeNone(chunk_size), isLikeNone(chunk_size) ? 0 : chunk_size, !isLikeNone(max_concurrent), isLikeNone(max_concurrent) ? 0 : max_concurrent);
        return ret;
      }
      /**
       * Get all pools from a factory (lightweight, IDs only)
       * @param {string} factory_id
       * @returns {Promise<any>}
       */
      alkanesGetAllPools(factory_id) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesGetAllPools(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get pool details including reserves using simulation
       * @param {string} pool_id
       * @returns {Promise<any>}
       */
      ammGetPoolDetails(pool_id) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ammGetPoolDetails(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} outpoint
       * @returns {Promise<any>}
       */
      alkanesTrace(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesTrace(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      traceProtostones(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_traceProtostones(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {number} height
       * @returns {Promise<any>}
       */
      traceBlock(height) {
        const ret = wasm.webprovider_traceBlock(this.__wbg_ptr, height);
        return ret;
      }
      /**
       * @param {string} address
       * @param {string | null} [block_tag]
       * @param {number | null} [protocol_tag]
       * @returns {Promise<any>}
       */
      alkanesByAddress(address, block_tag, protocol_tag) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesByAddress(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(protocol_tag), isLikeNone(protocol_tag) ? 0 : protocol_tag);
        return ret;
      }
      /**
       * @param {string} outpoint
       * @param {string | null} [block_tag]
       * @param {number | null} [protocol_tag]
       * @returns {Promise<any>}
       */
      alkanesByOutpoint(outpoint, block_tag, protocol_tag) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesByOutpoint(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(protocol_tag), isLikeNone(protocol_tag) ? 0 : protocol_tag);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      esploraGetTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTx(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      esploraGetTxStatus(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxStatus(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      esploraGetAddressInfo(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressInfo(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      esploraGetBlocksTipHeight() {
        const ret = wasm.webprovider_esploraGetBlocksTipHeight(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      esploraGetBlocksTipHash() {
        const ret = wasm.webprovider_esploraGetBlocksTipHash(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      esploraGetAddressUtxo(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressUtxo(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      esploraGetAddressTxs(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressTxs(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @param {string | null} [last_seen_txid]
       * @returns {Promise<any>}
       */
      esploraGetAddressTxsChain(address, last_seen_txid) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(last_seen_txid) ? 0 : passStringToWasm0(last_seen_txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressTxsChain(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * @param {bigint} block
       * @param {bigint} tx
       * @param {Uint8Array} path
       * @returns {Promise<any>}
       */
      getStorageAt(block, tx, path3) {
        const ptr0 = passArray8ToWasm0(path3, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_getStorageAt(this.__wbg_ptr, block, tx, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      esploraGetFeeEstimates() {
        const ret = wasm.webprovider_esploraGetFeeEstimates(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} tx_hex
       * @returns {Promise<any>}
       */
      esploraBroadcastTx(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraBroadcastTx(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      esploraGetTxHex(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxHex(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {number | null} [start_height]
       * @returns {Promise<any>}
       */
      esploraGetBlocks(start_height) {
        const ret = wasm.webprovider_esploraGetBlocks(this.__wbg_ptr, !isLikeNone(start_height), isLikeNone(start_height) ? 0 : start_height);
        return ret;
      }
      /**
       * @param {number} height
       * @returns {Promise<any>}
       */
      esploraGetBlockByHeight(height) {
        const ret = wasm.webprovider_esploraGetBlockByHeight(this.__wbg_ptr, height);
        return ret;
      }
      /**
       * @param {string} hash
       * @returns {Promise<any>}
       */
      esploraGetBlock(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlock(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} hash
       * @returns {Promise<any>}
       */
      esploraGetBlockStatus(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockStatus(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} hash
       * @returns {Promise<any>}
       */
      esploraGetBlockTxids(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockTxids(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} hash
       * @returns {Promise<any>}
       */
      esploraGetBlockHeader(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockHeader(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} hash
       * @returns {Promise<any>}
       */
      esploraGetBlockRaw(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockRaw(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} hash
       * @param {number} index
       * @returns {Promise<any>}
       */
      esploraGetBlockTxid(hash, index) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockTxid(this.__wbg_ptr, ptr0, len0, index);
        return ret;
      }
      /**
       * @param {string} hash
       * @param {number | null} [start_index]
       * @returns {Promise<any>}
       */
      esploraGetBlockTxs(hash, start_index) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetBlockTxs(this.__wbg_ptr, ptr0, len0, !isLikeNone(start_index), isLikeNone(start_index) ? 0 : start_index);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      esploraGetAddressTxsMempool(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressTxsMempool(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} prefix
       * @returns {Promise<any>}
       */
      esploraGetAddressPrefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetAddressPrefix(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      esploraGetTxRaw(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxRaw(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      esploraGetTxMerkleProof(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxMerkleProof(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      esploraGetTxMerkleblockProof(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxMerkleblockProof(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @param {number} index
       * @returns {Promise<any>}
       */
      esploraGetTxOutspend(txid, index) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxOutspend(this.__wbg_ptr, ptr0, len0, index);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      esploraGetTxOutspends(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraGetTxOutspends(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      esploraGetMempool() {
        const ret = wasm.webprovider_esploraGetMempool(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      esploraGetMempoolTxids() {
        const ret = wasm.webprovider_esploraGetMempoolTxids(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      esploraGetMempoolRecent() {
        const ret = wasm.webprovider_esploraGetMempoolRecent(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} tx_hex
       * @returns {Promise<any>}
       */
      esploraPostTx(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_esploraPostTx(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      bitcoindGetBlockCount() {
        const ret = wasm.webprovider_bitcoindGetBlockCount(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} tx_hex
       * @returns {Promise<any>}
       */
      bitcoindSendRawTransaction(tx_hex) {
        const ptr0 = passStringToWasm0(tx_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindSendRawTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {number} nblocks
       * @param {string} address
       * @returns {Promise<any>}
       */
      bitcoindGenerateToAddress(nblocks, address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGenerateToAddress(this.__wbg_ptr, nblocks, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      bitcoindGenerateFuture(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGenerateFuture(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      bitcoindGetBlockchainInfo() {
        const ret = wasm.webprovider_bitcoindGetBlockchainInfo(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      bitcoindGetNetworkInfo() {
        const ret = wasm.webprovider_bitcoindGetNetworkInfo(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} txid
       * @param {string | null} [block_hash]
       * @returns {Promise<any>}
       */
      bitcoindGetRawTransaction(txid, block_hash) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block_hash) ? 0 : passStringToWasm0(block_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetRawTransaction(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * @param {string} hash
       * @param {boolean} raw
       * @returns {Promise<any>}
       */
      bitcoindGetBlock(hash, raw) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetBlock(this.__wbg_ptr, ptr0, len0, raw);
        return ret;
      }
      /**
       * @param {number} height
       * @returns {Promise<any>}
       */
      bitcoindGetBlockHash(height) {
        const ret = wasm.webprovider_bitcoindGetBlockHash(this.__wbg_ptr, height);
        return ret;
      }
      /**
       * @param {string} hash
       * @returns {Promise<any>}
       */
      bitcoindGetBlockHeader(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetBlockHeader(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} hash
       * @returns {Promise<any>}
       */
      bitcoindGetBlockStats(hash) {
        const ptr0 = passStringToWasm0(hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetBlockStats(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      bitcoindGetMempoolInfo() {
        const ret = wasm.webprovider_bitcoindGetMempoolInfo(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {number} target
       * @returns {Promise<any>}
       */
      bitcoindEstimateSmartFee(target) {
        const ret = wasm.webprovider_bitcoindEstimateSmartFee(this.__wbg_ptr, target);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      bitcoindGetChainTips() {
        const ret = wasm.webprovider_bitcoindGetChainTips(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      bitcoindGetRawMempool() {
        const ret = wasm.webprovider_bitcoindGetRawMempool(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} txid
       * @param {number} vout
       * @param {boolean} include_mempool
       * @returns {Promise<any>}
       */
      bitcoindGetTxOut(txid, vout, include_mempool) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindGetTxOut(this.__wbg_ptr, ptr0, len0, vout, include_mempool);
        return ret;
      }
      /**
       * @param {string} hex
       * @returns {Promise<any>}
       */
      bitcoindDecodeRawTransaction(hex) {
        const ptr0 = passStringToWasm0(hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindDecodeRawTransaction(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} psbt
       * @returns {Promise<any>}
       */
      bitcoindDecodePsbt(psbt) {
        const ptr0 = passStringToWasm0(psbt, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_bitcoindDecodePsbt(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} contract_id
       * @param {string} view_fn
       * @param {Uint8Array | null} [params]
       * @param {string | null} [block_tag]
       * @returns {Promise<any>}
       */
      alkanesView(contract_id, view_fn, params, block_tag) {
        const ptr0 = passStringToWasm0(contract_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(view_fn, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(params) ? 0 : passArray8ToWasm0(params, wasm.__wbindgen_malloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesView(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret;
      }
      /**
       * @param {string} target
       * @param {any} config
       * @returns {Promise<any>}
       */
      alkanesInspect(target, config) {
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesInspect(this.__wbg_ptr, ptr0, len0, config);
        return ret;
      }
      /**
       * @param {string | null} [block_tag]
       * @returns {Promise<any>}
       */
      alkanesPendingUnwraps(block_tag) {
        var ptr0 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesPendingUnwraps(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} to
       * @param {string} data
       * @param {string | null} [block]
       * @returns {Promise<any>}
       */
      brc20progCall(to, data, block) {
        const ptr0 = passStringToWasm0(to, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(data, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progCall(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * @param {string} address
       * @param {string | null} [block]
       * @returns {Promise<any>}
       */
      brc20progGetBalance(address, block) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetBalance(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      brc20progGetCode(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetCode(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @param {string | null} [block]
       * @returns {Promise<any>}
       */
      brc20progGetTransactionCount(address, block) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetTransactionCount(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      brc20progBlockNumber() {
        const ret = wasm.webprovider_brc20progBlockNumber(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      brc20progChainId() {
        const ret = wasm.webprovider_brc20progChainId(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} tx_hash
       * @returns {Promise<any>}
       */
      brc20progGetTransactionReceipt(tx_hash) {
        const ptr0 = passStringToWasm0(tx_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetTransactionReceipt(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} tx_hash
       * @returns {Promise<any>}
       */
      brc20progGetTransactionByHash(tx_hash) {
        const ptr0 = passStringToWasm0(tx_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetTransactionByHash(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} block
       * @param {boolean} full_tx
       * @returns {Promise<any>}
       */
      brc20progGetBlockByNumber(block, full_tx) {
        const ptr0 = passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progGetBlockByNumber(this.__wbg_ptr, ptr0, len0, full_tx);
        return ret;
      }
      /**
       * @param {string} to
       * @param {string} data
       * @param {string | null} [block]
       * @returns {Promise<any>}
       */
      brc20progEstimateGas(to, data, block) {
        const ptr0 = passStringToWasm0(to, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(data, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(block) ? 0 : passStringToWasm0(block, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_brc20progEstimateGas(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * @param {any} filter
       * @returns {Promise<any>}
       */
      brc20progGetLogs(filter) {
        const ret = wasm.webprovider_brc20progGetLogs(this.__wbg_ptr, filter);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      brc20progWeb3ClientVersion() {
        const ret = wasm.webprovider_brc20progWeb3ClientVersion(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      metashrewHeight() {
        const ret = wasm.webprovider_metashrewHeight(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {number | null} [height]
       * @returns {Promise<any>}
       */
      metashrewStateRoot(height) {
        const ret = wasm.webprovider_metashrewStateRoot(this.__wbg_ptr, !isLikeNone(height), isLikeNone(height) ? 0 : height);
        return ret;
      }
      /**
       * @param {number} height
       * @returns {Promise<any>}
       */
      metashrewGetBlockHash(height) {
        const ret = wasm.webprovider_metashrewGetBlockHash(this.__wbg_ptr, height);
        return ret;
      }
      /**
       * Generic metashrew_view call
       *
       * Calls the metashrew_view RPC method with the given view function, payload, and block tag.
       * This is the low-level method for calling any metashrew view function.
       *
       * # Arguments
       * * `view_fn` - The view function name (e.g., "simulate", "protorunesbyaddress")
       * * `payload` - The hex-encoded payload (with or without 0x prefix)
       * * `block_tag` - The block tag ("latest" or a block height as string)
       *
       * # Returns
       * The hex-encoded response string from the view function
       * @param {string} view_fn
       * @param {string} payload
       * @param {string} block_tag
       * @returns {Promise<any>}
       */
      metashrewView(view_fn, payload, block_tag) {
        const ptr0 = passStringToWasm0(view_fn, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(payload, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_metashrewView(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret;
      }
      /**
       * @param {string} script
       * @returns {Promise<any>}
       */
      luaEvalScript(script) {
        const ptr0 = passStringToWasm0(script, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_luaEvalScript(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Execute a Lua script with arguments, using scripthash caching
       *
       * This method first tries to use the cached scripthash version (lua_evalsaved),
       * and falls back to the full script (lua_evalscript) if the hash isn't cached.
       * This is the recommended way to execute Lua scripts for better performance.
       *
       * # Arguments
       * * `script` - The Lua script content
       * * `args` - JSON-serialized array of arguments to pass to the script
       * @param {string} script
       * @param {any} args
       * @returns {Promise<any>}
       */
      luaEval(script, args) {
        const ptr0 = passStringToWasm0(script, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_luaEval(this.__wbg_ptr, ptr0, len0, args);
        return ret;
      }
      /**
       * @param {string} outpoint
       * @returns {Promise<any>}
       */
      ordList(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_ordList(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {number} sat
       * @returns {Promise<any>}
       */
      ordFind(sat) {
        const ret = wasm.webprovider_ordFind(this.__wbg_ptr, sat);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      runestoneDecodeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_runestoneDecodeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      runestoneAnalyzeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_runestoneAnalyzeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      protorunesDecodeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_protorunesDecodeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} txid
       * @returns {Promise<any>}
       */
      protorunesAnalyzeTx(txid) {
        const ptr0 = passStringToWasm0(txid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_protorunesAnalyzeTx(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Create a new wallet with an optional mnemonic phrase
       * If no mnemonic is provided, a new one will be generated
       * Returns wallet info including address and mnemonic
       *
       * Note: This sets the keystore on self synchronously so walletIsLoaded() returns true immediately
       * @param {string | null} [mnemonic]
       * @param {string | null} [passphrase]
       * @returns {any}
       */
      walletCreate(mnemonic, passphrase) {
        var ptr0 = isLikeNone(mnemonic) ? 0 : passStringToWasm0(mnemonic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(passphrase) ? 0 : passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletCreate(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[2]) {
          throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
      }
      /**
       * Load an existing wallet from storage
       * @param {string | null} [passphrase]
       * @returns {Promise<any>}
       */
      walletLoad(passphrase) {
        var ptr0 = isLikeNone(passphrase) ? 0 : passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletLoad(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get the wallet's primary address
       * @returns {Promise<any>}
       */
      walletGetAddress() {
        const ret = wasm.webprovider_walletGetAddress(this.__wbg_ptr);
        return ret;
      }
      /**
       * Get the wallet's BTC balance
       * Returns { confirmed: number, pending: number }
       * @param {string[] | null} [addresses]
       * @returns {Promise<any>}
       */
      walletGetBalance(addresses) {
        var ptr0 = isLikeNone(addresses) ? 0 : passArrayJsValueToWasm0(addresses, wasm.__wbindgen_malloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetBalance(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Load a wallet from mnemonic for signing transactions
       * This must be called before walletSend or other signing operations
       * @param {string} mnemonic_str
       * @param {string | null} [passphrase]
       */
      walletLoadMnemonic(mnemonic_str, passphrase) {
        const ptr0 = passStringToWasm0(mnemonic_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(passphrase) ? 0 : passStringToWasm0(passphrase, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletLoadMnemonic(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[1]) {
          throw takeFromExternrefTable0(ret[0]);
        }
      }
      /**
       * Check if wallet is loaded (has keystore for signing)
       * @returns {boolean}
       */
      walletIsLoaded() {
        const ret = wasm.webprovider_walletIsLoaded(this.__wbg_ptr);
        return ret !== 0;
      }
      /**
       * Get addresses from the loaded wallet keystore
       * Uses the Keystore.get_addresses method from alkanes-cli-common
       *
       * # Arguments
       * * `address_type` - Address type: "p2tr", "p2wpkh", "p2sh-p2wpkh", "p2pkh"
       * * `start_index` - Starting index for address derivation
       * * `count` - Number of addresses to derive
       * * `chain` - Chain index (0 for external/receiving, 1 for internal/change)
       *
       * # Returns
       * Array of address info objects with: { derivation_path, address, script_type, index, used }
       * @param {string} address_type
       * @param {number} start_index
       * @param {number} count
       * @param {number | null} [chain]
       * @returns {any}
       */
      walletGetAddresses(address_type, start_index, count, chain) {
        const ptr0 = passStringToWasm0(address_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetAddresses(this.__wbg_ptr, ptr0, len0, start_index, count, isLikeNone(chain) ? 4294967297 : chain >>> 0);
        if (ret[2]) {
          throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
      }
      /**
       * Send BTC to an address
       * params: { address: string, amount: number (satoshis), fee_rate?: number }
       * Wallet must be loaded first via walletLoadMnemonic
       * @param {string} params_json
       * @returns {Promise<any>}
       */
      walletSend(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletSend(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get UTXOs for the wallet
       * @param {string[] | null} [addresses]
       * @returns {Promise<any>}
       */
      walletGetUtxos(addresses) {
        var ptr0 = isLikeNone(addresses) ? 0 : passArrayJsValueToWasm0(addresses, wasm.__wbindgen_malloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetUtxos(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get transaction history for an address
       * @param {string | null} [address]
       * @returns {Promise<any>}
       */
      walletGetHistory(address) {
        var ptr0 = isLikeNone(address) ? 0 : passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletGetHistory(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} params_json
       * @returns {Promise<any>}
       */
      walletCreatePsbt(params_json) {
        const ptr0 = passStringToWasm0(params_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_walletCreatePsbt(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      walletExport() {
        const ret = wasm.webprovider_walletExport(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      walletBackup() {
        const ret = wasm.webprovider_walletBackup(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} pool_id
       * @param {string | null} [category]
       * @param {bigint | null} [limit]
       * @param {bigint | null} [offset]
       * @returns {Promise<any>}
       */
      dataApiGetPoolHistory(pool_id, category, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(category) ? 0 : passStringToWasm0(category, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolHistory(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
      }
      /**
       * @param {string} factory_id
       * @returns {Promise<any>}
       */
      dataApiGetPools(factory_id) {
        const ptr0 = passStringToWasm0(factory_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPools(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      dataApiGetAlkanesByAddress(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAlkanesByAddress(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @param {boolean} include_outpoints
       * @returns {Promise<any>}
       */
      dataApiGetAddressBalances(address, include_outpoints) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAddressBalances(this.__wbg_ptr, ptr0, len0, include_outpoints);
        return ret;
      }
      /**
       * @param {string} pool_id
       * @param {bigint | null} [limit]
       * @param {bigint | null} [offset]
       * @returns {Promise<any>}
       */
      dataApiGetAllHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAllHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
      }
      /**
       * @param {string} pool_id
       * @param {bigint | null} [limit]
       * @param {bigint | null} [offset]
       * @returns {Promise<any>}
       */
      dataApiGetSwapHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetSwapHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
      }
      /**
       * @param {string} pool_id
       * @param {bigint | null} [limit]
       * @param {bigint | null} [offset]
       * @returns {Promise<any>}
       */
      dataApiGetMintHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetMintHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
      }
      /**
       * @param {string} pool_id
       * @param {bigint | null} [limit]
       * @param {bigint | null} [offset]
       * @returns {Promise<any>}
       */
      dataApiGetBurnHistory(pool_id, limit, offset) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetBurnHistory(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit, !isLikeNone(offset), isLikeNone(offset) ? BigInt(0) : offset);
        return ret;
      }
      /**
       * @param {string} pool
       * @param {number | null} [start_time]
       * @param {number | null} [end_time]
       * @param {bigint | null} [limit]
       * @returns {Promise<any>}
       */
      dataApiGetTrades(pool, start_time, end_time, limit) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetTrades(this.__wbg_ptr, ptr0, len0, !isLikeNone(start_time), isLikeNone(start_time) ? 0 : start_time, !isLikeNone(end_time), isLikeNone(end_time) ? 0 : end_time, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit);
        return ret;
      }
      /**
       * @param {string} pool
       * @param {string} interval
       * @param {number | null} [start_time]
       * @param {number | null} [end_time]
       * @param {bigint | null} [limit]
       * @returns {Promise<any>}
       */
      dataApiGetCandles(pool, interval, start_time, end_time, limit) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetCandles(this.__wbg_ptr, ptr0, len0, ptr1, len1, !isLikeNone(start_time), isLikeNone(start_time) ? 0 : start_time, !isLikeNone(end_time), isLikeNone(end_time) ? 0 : end_time, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit);
        return ret;
      }
      /**
       * @param {string} pool
       * @returns {Promise<any>}
       */
      dataApiGetReserves(pool) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetReserves(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} alkane
       * @param {bigint} page
       * @param {bigint} limit
       * @returns {Promise<any>}
       */
      dataApiGetHolders(alkane, page, limit) {
        const ptr0 = passStringToWasm0(alkane, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetHolders(this.__wbg_ptr, ptr0, len0, page, limit);
        return ret;
      }
      /**
       * @param {string} alkane
       * @returns {Promise<any>}
       */
      dataApiGetHoldersCount(alkane) {
        const ptr0 = passStringToWasm0(alkane, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetHoldersCount(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} alkane
       * @param {string | null | undefined} prefix
       * @param {bigint} limit
       * @returns {Promise<any>}
       */
      dataApiGetKeys(alkane, prefix, limit) {
        const ptr0 = passStringToWasm0(alkane, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(prefix) ? 0 : passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetKeys(this.__wbg_ptr, ptr0, len0, ptr1, len1, limit);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      dataApiGetBitcoinPrice() {
        const ret = wasm.webprovider_dataApiGetBitcoinPrice(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {string} days
       * @returns {Promise<any>}
       */
      dataApiGetBitcoinMarketChart(days) {
        const ptr0 = passStringToWasm0(days, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetBitcoinMarketChart(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      dataApiHealth() {
        const ret = wasm.webprovider_dataApiHealth(this.__wbg_ptr);
        return ret;
      }
      /**
       * @param {bigint | null} [page]
       * @param {bigint | null} [limit]
       * @returns {Promise<any>}
       */
      dataApiGetAlkanes(page, limit) {
        const ret = wasm.webprovider_dataApiGetAlkanes(this.__wbg_ptr, !isLikeNone(page), isLikeNone(page) ? BigInt(0) : page, !isLikeNone(limit), isLikeNone(limit) ? BigInt(0) : limit);
        return ret;
      }
      /**
       * @param {string} alkane_id
       * @returns {Promise<any>}
       */
      dataApiGetAlkaneDetails(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetAlkaneDetails(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} pool_id
       * @returns {Promise<any>}
       */
      dataApiGetPoolById(pool_id) {
        const ptr0 = passStringToWasm0(pool_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetPoolById(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} outpoint
       * @returns {Promise<any>}
       */
      dataApiGetOutpointBalances(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_dataApiGetOutpointBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      dataApiGetBlockHeight() {
        const ret = wasm.webprovider_dataApiGetBlockHeight(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      dataApiGetBlockHash() {
        const ret = wasm.webprovider_dataApiGetBlockHash(this.__wbg_ptr);
        return ret;
      }
      /**
       * @returns {Promise<any>}
       */
      dataApiGetIndexerPosition() {
        const ret = wasm.webprovider_dataApiGetIndexerPosition(this.__wbg_ptr);
        return ret;
      }
      /**
       * Reflect alkane token metadata by querying standard opcodes
       *
       * This method queries the alkane contract with standard opcodes to retrieve
       * token metadata like name, symbol, total supply, cap, minted, and value per mint.
       *
       * # Arguments
       * * `alkane_id` - The alkane ID in "block:tx" format (e.g., "2:1234")
       *
       * # Returns
       * An AlkaneReflection object with all available metadata
       * @param {string} alkane_id
       * @returns {Promise<any>}
       */
      alkanesReflect(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesReflect(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string | null} [block_tag]
       * @returns {Promise<any>}
       */
      alkanesSequence(block_tag) {
        var ptr0 = isLikeNone(block_tag) ? 0 : passStringToWasm0(block_tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSequence(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * @param {string} address
       * @returns {Promise<any>}
       */
      alkanesSpendables(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_alkanesSpendables(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get current ESPO indexer height
       * @returns {Promise<any>}
       */
      espoGetHeight() {
        const ret = wasm.webprovider_espoGetHeight(this.__wbg_ptr);
        return ret;
      }
      /**
       * Ping the ESPO essentials module
       * @returns {Promise<any>}
       */
      espoPing() {
        const ret = wasm.webprovider_espoPing(this.__wbg_ptr);
        return ret;
      }
      /**
       * Get alkanes balances for an address from ESPO
       * @param {string} address
       * @param {boolean} include_outpoints
       * @returns {Promise<any>}
       */
      espoGetAddressBalances(address, include_outpoints) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAddressBalances(this.__wbg_ptr, ptr0, len0, include_outpoints);
        return ret;
      }
      /**
       * Get outpoints containing alkanes for an address from ESPO
       * @param {string} address
       * @returns {Promise<any>}
       */
      espoGetAddressOutpoints(address) {
        const ptr0 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetAddressOutpoints(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get alkanes balances at a specific outpoint from ESPO
       * @param {string} outpoint
       * @returns {Promise<any>}
       */
      espoGetOutpointBalances(outpoint) {
        const ptr0 = passStringToWasm0(outpoint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetOutpointBalances(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get holders of an alkane token from ESPO
       * @param {string} alkane_id
       * @param {number} page
       * @param {number} limit
       * @returns {Promise<any>}
       */
      espoGetHolders(alkane_id, page, limit) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetHolders(this.__wbg_ptr, ptr0, len0, page, limit);
        return ret;
      }
      /**
       * Get holder count for an alkane from ESPO
       * @param {string} alkane_id
       * @returns {Promise<any>}
       */
      espoGetHoldersCount(alkane_id) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetHoldersCount(this.__wbg_ptr, ptr0, len0);
        return ret;
      }
      /**
       * Get storage keys for an alkane contract from ESPO
       * @param {string} alkane_id
       * @param {number} page
       * @param {number} limit
       * @returns {Promise<any>}
       */
      espoGetKeys(alkane_id, page, limit) {
        const ptr0 = passStringToWasm0(alkane_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetKeys(this.__wbg_ptr, ptr0, len0, page, limit);
        return ret;
      }
      /**
       * Ping the ESPO AMM Data module
       * @returns {Promise<any>}
       */
      espoAmmdataPing() {
        const ret = wasm.webprovider_espoAmmdataPing(this.__wbg_ptr);
        return ret;
      }
      /**
       * Get OHLCV candlestick data for a pool from ESPO
       * @param {string} pool
       * @param {string | null} [timeframe]
       * @param {string | null} [side]
       * @param {number | null} [limit]
       * @param {number | null} [page]
       * @returns {Promise<any>}
       */
      espoGetCandles(pool, timeframe, side, limit, page) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(timeframe) ? 0 : passStringToWasm0(timeframe, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(side) ? 0 : passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetCandles(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
      }
      /**
       * Get trade history for a pool from ESPO
       * @param {string} pool
       * @param {number | null} [limit]
       * @param {number | null} [page]
       * @param {string | null} [side]
       * @param {string | null} [filter_side]
       * @param {string | null} [sort]
       * @param {string | null} [dir]
       * @returns {Promise<any>}
       */
      espoGetTrades(pool, limit, page, side, filter_side, sort, dir) {
        const ptr0 = passStringToWasm0(pool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(side) ? 0 : passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(filter_side) ? 0 : passStringToWasm0(filter_side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(sort) ? 0 : passStringToWasm0(sort, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(dir) ? 0 : passStringToWasm0(dir, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len4 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetTrades(this.__wbg_ptr, ptr0, len0, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, !isLikeNone(page), isLikeNone(page) ? 0 : page, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        return ret;
      }
      /**
       * Get all pools from ESPO
       * @param {number | null} [limit]
       * @param {number | null} [page]
       * @returns {Promise<any>}
       */
      espoGetPools(limit, page) {
        const ret = wasm.webprovider_espoGetPools(this.__wbg_ptr, !isLikeNone(limit), isLikeNone(limit) ? 0 : limit, !isLikeNone(page), isLikeNone(page) ? 0 : page);
        return ret;
      }
      /**
       * Find the best swap path between two tokens using ESPO
       * @param {string} token_in
       * @param {string} token_out
       * @param {string | null} [mode]
       * @param {string | null} [amount_in]
       * @param {string | null} [amount_out]
       * @param {string | null} [amount_out_min]
       * @param {string | null} [amount_in_max]
       * @param {string | null} [available_in]
       * @param {number | null} [fee_bps]
       * @param {number | null} [max_hops]
       * @returns {Promise<any>}
       */
      espoFindBestSwapPath(token_in, token_out, mode, amount_in, amount_out, amount_out_min, amount_in_max, available_in, fee_bps, max_hops) {
        const ptr0 = passStringToWasm0(token_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(token_out, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(mode) ? 0 : passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        var ptr3 = isLikeNone(amount_in) ? 0 : passStringToWasm0(amount_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len3 = WASM_VECTOR_LEN;
        var ptr4 = isLikeNone(amount_out) ? 0 : passStringToWasm0(amount_out, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len4 = WASM_VECTOR_LEN;
        var ptr5 = isLikeNone(amount_out_min) ? 0 : passStringToWasm0(amount_out_min, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len5 = WASM_VECTOR_LEN;
        var ptr6 = isLikeNone(amount_in_max) ? 0 : passStringToWasm0(amount_in_max, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len6 = WASM_VECTOR_LEN;
        var ptr7 = isLikeNone(available_in) ? 0 : passStringToWasm0(available_in, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len7 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoFindBestSwapPath(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6, ptr7, len7, !isLikeNone(fee_bps), isLikeNone(fee_bps) ? 0 : fee_bps, !isLikeNone(max_hops), isLikeNone(max_hops) ? 0 : max_hops);
        return ret;
      }
      /**
       * Find the best MEV swap opportunity for a token using ESPO
       * @param {string} token
       * @param {number | null} [fee_bps]
       * @param {number | null} [max_hops]
       * @returns {Promise<any>}
       */
      espoGetBestMevSwap(token, fee_bps, max_hops) {
        const ptr0 = passStringToWasm0(token, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.webprovider_espoGetBestMevSwap(this.__wbg_ptr, ptr0, len0, !isLikeNone(fee_bps), isLikeNone(fee_bps) ? 0 : fee_bps, !isLikeNone(max_hops), isLikeNone(max_hops) ? 0 : max_hops);
        return ret;
      }
    };
    if (Symbol.dispose) WebProvider.prototype[Symbol.dispose] = WebProvider.prototype.free;
    exports2.WebProvider = WebProvider;
    exports2.__wbg_Error_e83987f665cf5504 = function(arg0, arg1) {
      const ret = Error(getStringFromWasm0(arg0, arg1));
      return ret;
    };
    exports2.__wbg_Number_bb48ca12f395cd08 = function(arg0) {
      const ret = Number(arg0);
      return ret;
    };
    exports2.__wbg_String_8f0eb39a4a4c2f66 = function(arg0, arg1) {
      const ret = String(arg1);
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    exports2.__wbg___wbindgen_bigint_get_as_i64_f3ebc5a755000afd = function(arg0, arg1) {
      const v = arg1;
      const ret = typeof v === "bigint" ? v : void 0;
      getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
    };
    exports2.__wbg___wbindgen_boolean_get_6d5a1ee65bab5f68 = function(arg0) {
      const v = arg0;
      const ret = typeof v === "boolean" ? v : void 0;
      return isLikeNone(ret) ? 16777215 : ret ? 1 : 0;
    };
    exports2.__wbg___wbindgen_debug_string_df47ffb5e35e6763 = function(arg0, arg1) {
      const ret = debugString(arg1);
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    exports2.__wbg___wbindgen_in_bb933bd9e1b3bc0f = function(arg0, arg1) {
      const ret = arg0 in arg1;
      return ret;
    };
    exports2.__wbg___wbindgen_is_bigint_cb320707dcd35f0b = function(arg0) {
      const ret = typeof arg0 === "bigint";
      return ret;
    };
    exports2.__wbg___wbindgen_is_falsy_46b8d2f2aba49112 = function(arg0) {
      const ret = !arg0;
      return ret;
    };
    exports2.__wbg___wbindgen_is_function_ee8a6c5833c90377 = function(arg0) {
      const ret = typeof arg0 === "function";
      return ret;
    };
    exports2.__wbg___wbindgen_is_null_5e69f72e906cc57c = function(arg0) {
      const ret = arg0 === null;
      return ret;
    };
    exports2.__wbg___wbindgen_is_object_c818261d21f283a4 = function(arg0) {
      const val = arg0;
      const ret = typeof val === "object" && val !== null;
      return ret;
    };
    exports2.__wbg___wbindgen_is_string_fbb76cb2940daafd = function(arg0) {
      const ret = typeof arg0 === "string";
      return ret;
    };
    exports2.__wbg___wbindgen_is_undefined_2d472862bd29a478 = function(arg0) {
      const ret = arg0 === void 0;
      return ret;
    };
    exports2.__wbg___wbindgen_jsval_eq_6b13ab83478b1c50 = function(arg0, arg1) {
      const ret = arg0 === arg1;
      return ret;
    };
    exports2.__wbg___wbindgen_jsval_loose_eq_b664b38a2f582147 = function(arg0, arg1) {
      const ret = arg0 == arg1;
      return ret;
    };
    exports2.__wbg___wbindgen_number_get_a20bf9b85341449d = function(arg0, arg1) {
      const obj = arg1;
      const ret = typeof obj === "number" ? obj : void 0;
      getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
    };
    exports2.__wbg___wbindgen_string_get_e4f06c90489ad01b = function(arg0, arg1) {
      const obj = arg1;
      const ret = typeof obj === "string" ? obj : void 0;
      var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      var len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    exports2.__wbg___wbindgen_throw_b855445ff6a94295 = function(arg0, arg1) {
      throw new Error(getStringFromWasm0(arg0, arg1));
    };
    exports2.__wbg__wbg_cb_unref_2454a539ea5790d9 = function(arg0) {
      arg0._wbg_cb_unref();
    };
    exports2.__wbg_abort_28ad55c5825b004d = function(arg0, arg1) {
      arg0.abort(arg1);
    };
    exports2.__wbg_abort_e7eb059f72f9ed0c = function(arg0) {
      arg0.abort();
    };
    exports2.__wbg_append_b577eb3a177bc0fa = function() {
      return handleError(function(arg0, arg1, arg2, arg3, arg4) {
        arg0.append(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
      }, arguments);
    };
    exports2.__wbg_arrayBuffer_b375eccb84b4ddf3 = function() {
      return handleError(function(arg0) {
        const ret = arg0.arrayBuffer();
        return ret;
      }, arguments);
    };
    exports2.__wbg_call_525440f72fbfc0ea = function() {
      return handleError(function(arg0, arg1, arg2) {
        const ret = arg0.call(arg1, arg2);
        return ret;
      }, arguments);
    };
    exports2.__wbg_call_e45d2cf9fc925fcf = function() {
      return handleError(function(arg0, arg1, arg2, arg3) {
        const ret = arg0.call(arg1, arg2, arg3);
        return ret;
      }, arguments);
    };
    exports2.__wbg_call_e762c39fa8ea36bf = function() {
      return handleError(function(arg0, arg1) {
        const ret = arg0.call(arg1);
        return ret;
      }, arguments);
    };
    exports2.__wbg_clearTimeout_7a42b49784aea641 = function(arg0) {
      const ret = clearTimeout(arg0);
      return ret;
    };
    exports2.__wbg_connect_1ec2f4eb726c5e06 = function(arg0) {
      const ret = arg0.connect();
      return ret;
    };
    exports2.__wbg_crypto_574e78ad8b13b65f = function(arg0) {
      const ret = arg0.crypto;
      return ret;
    };
    exports2.__wbg_decrypt_0452782895e3c2f1 = function() {
      return handleError(function(arg0, arg1, arg2, arg3, arg4) {
        const ret = arg0.decrypt(arg1, arg2, getArrayU8FromWasm0(arg3, arg4));
        return ret;
      }, arguments);
    };
    exports2.__wbg_deriveBits_28ff8a809aa473ec = function() {
      return handleError(function(arg0, arg1, arg2, arg3) {
        const ret = arg0.deriveBits(arg1, arg2, arg3 >>> 0);
        return ret;
      }, arguments);
    };
    exports2.__wbg_disconnect_2ee62795d9b85438 = function(arg0) {
      const ret = arg0.disconnect();
      return ret;
    };
    exports2.__wbg_done_2042aa2670fb1db1 = function(arg0) {
      const ret = arg0.done;
      return ret;
    };
    exports2.__wbg_encrypt_36464dd547f58e9c = function() {
      return handleError(function(arg0, arg1, arg2, arg3, arg4) {
        const ret = arg0.encrypt(arg1, arg2, getArrayU8FromWasm0(arg3, arg4));
        return ret;
      }, arguments);
    };
    exports2.__wbg_entries_e171b586f8f6bdbf = function(arg0) {
      const ret = Object.entries(arg0);
      return ret;
    };
    exports2.__wbg_fetch_74a3e84ebd2c9a0e = function(arg0) {
      const ret = fetch(arg0);
      return ret;
    };
    exports2.__wbg_fetch_f8ba0e29a9d6de0d = function(arg0, arg1) {
      const ret = arg0.fetch(arg1);
      return ret;
    };
    exports2.__wbg_fromCodePoint_a1c5bb992dc05846 = function() {
      return handleError(function(arg0) {
        const ret = String.fromCodePoint(arg0 >>> 0);
        return ret;
      }, arguments);
    };
    exports2.__wbg_from_a4ad7cbddd0d7135 = function(arg0) {
      const ret = Array.from(arg0);
      return ret;
    };
    exports2.__wbg_getAccounts_364d2a097b56525c = function(arg0) {
      const ret = arg0.getAccounts();
      return ret;
    };
    exports2.__wbg_getBalance_3804a0e75a8ac301 = function(arg0) {
      const ret = arg0.getBalance();
      return ret;
    };
    exports2.__wbg_getInfo_8ee5a8fc05c8f277 = function(arg0) {
      const ret = arg0.getInfo();
      return ret;
    };
    exports2.__wbg_getInscriptions_0e1a3c5a6e0485f5 = function(arg0, arg1, arg2) {
      const ret = arg0.getInscriptions(arg1, arg2);
      return ret;
    };
    exports2.__wbg_getItem_89f57d6acc51a876 = function() {
      return handleError(function(arg0, arg1, arg2, arg3) {
        const ret = arg1.getItem(getStringFromWasm0(arg2, arg3));
        var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
      }, arguments);
    };
    exports2.__wbg_getNetwork_79834a70f80b58ba = function(arg0) {
      const ret = arg0.getNetwork();
      return ret;
    };
    exports2.__wbg_getPublicKey_19363ddc641d2495 = function(arg0) {
      const ret = arg0.getPublicKey();
      return ret;
    };
    exports2.__wbg_getRandomValues_6357e7b583eb49cc = function() {
      return handleError(function(arg0, arg1, arg2) {
        const ret = arg0.getRandomValues(getArrayU8FromWasm0(arg1, arg2));
        return ret;
      }, arguments);
    };
    exports2.__wbg_getRandomValues_b8f5dbd5f3995a9e = function() {
      return handleError(function(arg0, arg1) {
        arg0.getRandomValues(arg1);
      }, arguments);
    };
    exports2.__wbg_get_7bed016f185add81 = function(arg0, arg1) {
      const ret = arg0[arg1 >>> 0];
      return ret;
    };
    exports2.__wbg_get_efcb449f58ec27c2 = function() {
      return handleError(function(arg0, arg1) {
        const ret = Reflect.get(arg0, arg1);
        return ret;
      }, arguments);
    };
    exports2.__wbg_get_with_ref_key_1dc361bd10053bfe = function(arg0, arg1) {
      const ret = arg0[arg1];
      return ret;
    };
    exports2.__wbg_has_787fafc980c3ccdb = function() {
      return handleError(function(arg0, arg1) {
        const ret = Reflect.has(arg0, arg1);
        return ret;
      }, arguments);
    };
    exports2.__wbg_headers_b87d7eaba61c3278 = function(arg0) {
      const ret = arg0.headers;
      return ret;
    };
    exports2.__wbg_importKey_2be19189a1451235 = function() {
      return handleError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        const ret = arg0.importKey(getStringFromWasm0(arg1, arg2), arg3, arg4, arg5 !== 0, arg6);
        return ret;
      }, arguments);
    };
    exports2.__wbg_instanceof_ArrayBuffer_70beb1189ca63b38 = function(arg0) {
      let result;
      try {
        result = arg0 instanceof ArrayBuffer;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    };
    exports2.__wbg_instanceof_CryptoKey_9fbbefded7590b8c = function(arg0) {
      let result;
      try {
        result = arg0 instanceof CryptoKey;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    };
    exports2.__wbg_instanceof_Crypto_2574e69763b89701 = function(arg0) {
      let result;
      try {
        result = arg0 instanceof Crypto;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    };
    exports2.__wbg_instanceof_Map_8579b5e2ab5437c7 = function(arg0) {
      let result;
      try {
        result = arg0 instanceof Map;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    };
    exports2.__wbg_instanceof_Response_f4f3e87e07f3135c = function(arg0) {
      let result;
      try {
        result = arg0 instanceof Response;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    };
    exports2.__wbg_instanceof_Uint8Array_20c8e73002f7af98 = function(arg0) {
      let result;
      try {
        result = arg0 instanceof Uint8Array;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    };
    exports2.__wbg_instanceof_Window_4846dbb3de56c84c = function(arg0) {
      let result;
      try {
        result = arg0 instanceof Window;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    };
    exports2.__wbg_isArray_96e0af9891d0945d = function(arg0) {
      const ret = Array.isArray(arg0);
      return ret;
    };
    exports2.__wbg_isSafeInteger_d216eda7911dde36 = function(arg0) {
      const ret = Number.isSafeInteger(arg0);
      return ret;
    };
    exports2.__wbg_iterator_e5822695327a3c39 = function() {
      const ret = Symbol.iterator;
      return ret;
    };
    exports2.__wbg_key_38d01a092280ffc6 = function() {
      return handleError(function(arg0, arg1, arg2) {
        const ret = arg1.key(arg2 >>> 0);
        var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
      }, arguments);
    };
    exports2.__wbg_length_69bca3cb64fc8748 = function(arg0) {
      const ret = arg0.length;
      return ret;
    };
    exports2.__wbg_length_7534a213da0a65cd = function() {
      return handleError(function(arg0) {
        const ret = arg0.length;
        return ret;
      }, arguments);
    };
    exports2.__wbg_length_cdd215e10d9dd507 = function(arg0) {
      const ret = arg0.length;
      return ret;
    };
    exports2.__wbg_localStorage_3034501cd2b3da3f = function() {
      return handleError(function(arg0) {
        const ret = arg0.localStorage;
        return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
      }, arguments);
    };
    exports2.__wbg_msCrypto_a61aeb35a24c1329 = function(arg0) {
      const ret = arg0.msCrypto;
      return ret;
    };
    exports2.__wbg_new_0_f9740686d739025c = function() {
      const ret = /* @__PURE__ */ new Date();
      return ret;
    };
    exports2.__wbg_new_1acc0b6eea89d040 = function() {
      const ret = new Object();
      return ret;
    };
    exports2.__wbg_new_2531773dac38ebb3 = function() {
      return handleError(function() {
        const ret = new AbortController();
        return ret;
      }, arguments);
    };
    exports2.__wbg_new_3c3d849046688a66 = function(arg0, arg1) {
      try {
        var state0 = { a: arg0, b: arg1 };
        var cb0 = (arg02, arg12) => {
          const a = state0.a;
          state0.a = 0;
          try {
            return wasm_bindgen__convert__closures_____invoke__h95fdbac5e4c1bfb6(a, state0.b, arg02, arg12);
          } finally {
            state0.a = a;
          }
        };
        const ret = new Promise(cb0);
        return ret;
      } finally {
        state0.a = state0.b = 0;
      }
    };
    exports2.__wbg_new_5a79be3ab53b8aa5 = function(arg0) {
      const ret = new Uint8Array(arg0);
      return ret;
    };
    exports2.__wbg_new_68651c719dcda04e = function() {
      const ret = /* @__PURE__ */ new Map();
      return ret;
    };
    exports2.__wbg_new_9edf9838a2def39c = function() {
      return handleError(function() {
        const ret = new Headers();
        return ret;
      }, arguments);
    };
    exports2.__wbg_new_e17d9f43105b08be = function() {
      const ret = new Array();
      return ret;
    };
    exports2.__wbg_new_from_slice_92f4d78ca282a2d2 = function(arg0, arg1) {
      const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
      return ret;
    };
    exports2.__wbg_new_no_args_ee98eee5275000a4 = function(arg0, arg1) {
      const ret = new Function(getStringFromWasm0(arg0, arg1));
      return ret;
    };
    exports2.__wbg_new_with_length_01aa0dc35aa13543 = function(arg0) {
      const ret = new Uint8Array(arg0 >>> 0);
      return ret;
    };
    exports2.__wbg_new_with_str_and_init_0ae7728b6ec367b1 = function() {
      return handleError(function(arg0, arg1, arg2) {
        const ret = new Request(getStringFromWasm0(arg0, arg1), arg2);
        return ret;
      }, arguments);
    };
    exports2.__wbg_next_020810e0ae8ebcb0 = function() {
      return handleError(function(arg0) {
        const ret = arg0.next();
        return ret;
      }, arguments);
    };
    exports2.__wbg_next_2c826fe5dfec6b6a = function(arg0) {
      const ret = arg0.next;
      return ret;
    };
    exports2.__wbg_node_905d3e251edff8a2 = function(arg0) {
      const ret = arg0.node;
      return ret;
    };
    exports2.__wbg_now_793306c526e2e3b6 = function() {
      const ret = Date.now();
      return ret;
    };
    exports2.__wbg_of_035271b9e67a3bd9 = function(arg0) {
      const ret = Array.of(arg0);
      return ret;
    };
    exports2.__wbg_parse_2a704d6b78abb2b8 = function() {
      return handleError(function(arg0, arg1) {
        const ret = JSON.parse(getStringFromWasm0(arg0, arg1));
        return ret;
      }, arguments);
    };
    exports2.__wbg_process_dc0fbacc7c1c06f7 = function(arg0) {
      const ret = arg0.process;
      return ret;
    };
    exports2.__wbg_prototypesetcall_2a6620b6922694b2 = function(arg0, arg1, arg2) {
      Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
    };
    exports2.__wbg_pushPsbt_94963062529d842c = function(arg0, arg1, arg2) {
      const ret = arg0.pushPsbt(getStringFromWasm0(arg1, arg2));
      return ret;
    };
    exports2.__wbg_pushTx_4fc10aa38ba926b8 = function(arg0, arg1, arg2) {
      const ret = arg0.pushTx(getStringFromWasm0(arg1, arg2));
      return ret;
    };
    exports2.__wbg_push_df81a39d04db858c = function(arg0, arg1) {
      const ret = arg0.push(arg1);
      return ret;
    };
    exports2.__wbg_queueMicrotask_34d692c25c47d05b = function(arg0) {
      const ret = arg0.queueMicrotask;
      return ret;
    };
    exports2.__wbg_queueMicrotask_9d76cacb20c84d58 = function(arg0) {
      queueMicrotask(arg0);
    };
    exports2.__wbg_randomFillSync_ac0988aba3254290 = function() {
      return handleError(function(arg0, arg1) {
        arg0.randomFillSync(arg1);
      }, arguments);
    };
    exports2.__wbg_removeItem_0e1e70f1687b5304 = function() {
      return handleError(function(arg0, arg1, arg2) {
        arg0.removeItem(getStringFromWasm0(arg1, arg2));
      }, arguments);
    };
    exports2.__wbg_require_60cc747a6bc5215a = function() {
      return handleError(function() {
        const ret = module2.require;
        return ret;
      }, arguments);
    };
    exports2.__wbg_resolve_caf97c30b83f7053 = function(arg0) {
      const ret = Promise.resolve(arg0);
      return ret;
    };
    exports2.__wbg_setItem_64dfb54d7b20d84c = function() {
      return handleError(function(arg0, arg1, arg2, arg3, arg4) {
        arg0.setItem(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
      }, arguments);
    };
    exports2.__wbg_setTimeout_7bb3429662ab1e70 = function(arg0, arg1) {
      const ret = setTimeout(arg0, arg1);
      return ret;
    };
    exports2.__wbg_set_3f1d0b984ed272ed = function(arg0, arg1, arg2) {
      arg0[arg1] = arg2;
    };
    exports2.__wbg_set_907fb406c34a251d = function(arg0, arg1, arg2) {
      const ret = arg0.set(arg1, arg2);
      return ret;
    };
    exports2.__wbg_set_9e6516df7b7d0f19 = function(arg0, arg1, arg2) {
      arg0.set(getArrayU8FromWasm0(arg1, arg2));
    };
    exports2.__wbg_set_body_3c365989753d61f4 = function(arg0, arg1) {
      arg0.body = arg1;
    };
    exports2.__wbg_set_c213c871859d6500 = function(arg0, arg1, arg2) {
      arg0[arg1 >>> 0] = arg2;
    };
    exports2.__wbg_set_c2abbebe8b9ebee1 = function() {
      return handleError(function(arg0, arg1, arg2) {
        const ret = Reflect.set(arg0, arg1, arg2);
        return ret;
      }, arguments);
    };
    exports2.__wbg_set_cache_2f9deb19b92b81e3 = function(arg0, arg1) {
      arg0.cache = __wbindgen_enum_RequestCache[arg1];
    };
    exports2.__wbg_set_credentials_f621cd2d85c0c228 = function(arg0, arg1) {
      arg0.credentials = __wbindgen_enum_RequestCredentials[arg1];
    };
    exports2.__wbg_set_headers_6926da238cd32ee4 = function(arg0, arg1) {
      arg0.headers = arg1;
    };
    exports2.__wbg_set_method_c02d8cbbe204ac2d = function(arg0, arg1, arg2) {
      arg0.method = getStringFromWasm0(arg1, arg2);
    };
    exports2.__wbg_set_mode_52ef73cfa79639cb = function(arg0, arg1) {
      arg0.mode = __wbindgen_enum_RequestMode[arg1];
    };
    exports2.__wbg_set_signal_dda2cf7ccb6bee0f = function(arg0, arg1) {
      arg0.signal = arg1;
    };
    exports2.__wbg_signMessage_dfa4cbdda66532c1 = function(arg0, arg1, arg2, arg3, arg4) {
      const ret = arg0.signMessage(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
      return ret;
    };
    exports2.__wbg_signPsbt_1703f674396512f5 = function(arg0, arg1, arg2, arg3) {
      const ret = arg0.signPsbt(getStringFromWasm0(arg1, arg2), arg3);
      return ret;
    };
    exports2.__wbg_signPsbts_f6481ee8ea4a52f1 = function(arg0, arg1, arg2) {
      const ret = arg0.signPsbts(arg1, arg2);
      return ret;
    };
    exports2.__wbg_signal_4db5aa055bf9eb9a = function(arg0) {
      const ret = arg0.signal;
      return ret;
    };
    exports2.__wbg_static_accessor_GLOBAL_89e1d9ac6a1b250e = function() {
      const ret = typeof global === "undefined" ? null : global;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    exports2.__wbg_static_accessor_GLOBAL_THIS_8b530f326a9e48ac = function() {
      const ret = typeof globalThis === "undefined" ? null : globalThis;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    exports2.__wbg_static_accessor_SELF_6fdf4b64710cc91b = function() {
      const ret = typeof self === "undefined" ? null : self;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    exports2.__wbg_static_accessor_WINDOW_b45bfc5a37f6cfa2 = function() {
      const ret = typeof window === "undefined" ? null : window;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    };
    exports2.__wbg_status_de7eed5a7a5bfd5d = function(arg0) {
      const ret = arg0.status;
      return ret;
    };
    exports2.__wbg_stringify_b5fb28f6465d9c3e = function() {
      return handleError(function(arg0) {
        const ret = JSON.stringify(arg0);
        return ret;
      }, arguments);
    };
    exports2.__wbg_subarray_480600f3d6a9f26c = function(arg0, arg1, arg2) {
      const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
      return ret;
    };
    exports2.__wbg_subtle_a158c8cba320b8ed = function(arg0) {
      const ret = arg0.subtle;
      return ret;
    };
    exports2.__wbg_switchNetwork_c2498e59066e6f0b = function(arg0, arg1, arg2) {
      const ret = arg0.switchNetwork(getStringFromWasm0(arg1, arg2));
      return ret;
    };
    exports2.__wbg_text_dc33c15c17bdfb52 = function() {
      return handleError(function(arg0) {
        const ret = arg0.text();
        return ret;
      }, arguments);
    };
    exports2.__wbg_then_4f46f6544e6b4a28 = function(arg0, arg1) {
      const ret = arg0.then(arg1);
      return ret;
    };
    exports2.__wbg_then_70d05cf780a18d77 = function(arg0, arg1, arg2) {
      const ret = arg0.then(arg1, arg2);
      return ret;
    };
    exports2.__wbg_toISOString_48d92f5754d01b49 = function(arg0) {
      const ret = arg0.toISOString();
      return ret;
    };
    exports2.__wbg_url_b36d2a5008eb056f = function(arg0, arg1) {
      const ret = arg1.url;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    exports2.__wbg_value_692627309814bb8c = function(arg0) {
      const ret = arg0.value;
      return ret;
    };
    exports2.__wbg_versions_c01dfd4722a88165 = function(arg0) {
      const ret = arg0.versions;
      return ret;
    };
    exports2.__wbg_warn_1d74dddbe2fd1dbb = function(arg0) {
      console.warn(arg0);
    };
    exports2.__wbg_wasmbrowserwalletprovider_new = function(arg0) {
      const ret = WasmBrowserWalletProvider.__wrap(arg0);
      return ret;
    };
    exports2.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
      const ret = getStringFromWasm0(arg0, arg1);
      return ret;
    };
    exports2.__wbindgen_cast_4625c577ab2ec9ee = function(arg0) {
      const ret = BigInt.asUintN(64, arg0);
      return ret;
    };
    exports2.__wbindgen_cast_832dfe82587c7cfc = function(arg0, arg1) {
      const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h264445b1cd05789c, wasm_bindgen__convert__closures_____invoke__h8d00541115e24acb);
      return ret;
    };
    exports2.__wbindgen_cast_9ae0607507abb057 = function(arg0) {
      const ret = arg0;
      return ret;
    };
    exports2.__wbindgen_cast_cb9088102bce6b30 = function(arg0, arg1) {
      const ret = getArrayU8FromWasm0(arg0, arg1);
      return ret;
    };
    exports2.__wbindgen_cast_d6cd19b81560fd6e = function(arg0) {
      const ret = arg0;
      return ret;
    };
    exports2.__wbindgen_cast_e7b45dd881f38ce3 = function(arg0, arg1) {
      const ret = BigInt.asUintN(64, arg0) | BigInt.asUintN(64, arg1) << BigInt(64);
      return ret;
    };
    exports2.__wbindgen_cast_facf2bd55df381c1 = function(arg0, arg1) {
      const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h3ba04b4139aaae95, wasm_bindgen__convert__closures_____invoke__h5943629905d90057);
      return ret;
    };
    exports2.__wbindgen_init_externref_table = function() {
      const table = wasm.__wbindgen_externrefs;
      const offset = table.grow(4);
      table.set(0, void 0);
      table.set(offset + 0, void 0);
      table.set(offset + 1, null);
      table.set(offset + 2, true);
      table.set(offset + 3, false);
      ;
    };
    exports2.__wbindgen_object_is_undefined = function(arg0) {
      const ret = arg0 === void 0;
      return ret;
    };
    var wasmPath = `${__dirname}/alkanes_web_sys_bg.wasm`;
    var wasmBytes = require("fs").readFileSync(wasmPath);
    var wasmModule = new WebAssembly.Module(wasmBytes);
    var wasm = exports2.__wasm = new WebAssembly.Instance(wasmModule, imports).exports;
    wasm.__wbindgen_start();
  }
});

// src/cli/index.ts
var import_commander = require("commander");
var import_chalk4 = __toESM(require("chalk"));
init_formatting();

// src/cli/commands/wallet.ts
var import_chalk2 = __toESM(require("chalk"));

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
var cachedProvider = null;
var cachedNetwork = null;
async function createProvider2(options) {
  const config = await getConfig();
  const network = options.network || config.network || "mainnet";
  const rpcUrl = options.jsonrpcUrl || config.jsonrpcUrl;
  if (cachedProvider && cachedNetwork === network) {
    return cachedProvider;
  }
  const { AlkanesProvider: AlkanesProvider2 } = await Promise.resolve().then(() => (init_provider(), provider_exports));
  const providerConfig = {
    network,
    rpcUrl
  };
  const provider = new AlkanesProvider2(providerConfig);
  await provider.initialize();
  cachedProvider = provider;
  cachedNetwork = network;
  return provider;
}

// src/cli/utils/wallet.ts
var fs2 = __toESM(require("fs"));
var path2 = __toESM(require("path"));
function walletExists(walletPath) {
  const expandedPath = expandPath(walletPath);
  return fs2.existsSync(expandedPath);
}
function loadWalletFile(walletPath) {
  const expandedPath = expandPath(walletPath);
  if (!fs2.existsSync(expandedPath)) {
    throw new Error(`Wallet file not found: ${walletPath}`);
  }
  const content = fs2.readFileSync(expandedPath, "utf-8");
  return JSON.parse(content);
}
function saveWalletFile(walletPath, walletData) {
  const expandedPath = expandPath(walletPath);
  const dir = path2.dirname(expandedPath);
  if (!fs2.existsSync(dir)) {
    fs2.mkdirSync(dir, { recursive: true });
  }
  fs2.writeFileSync(expandedPath, JSON.stringify(walletData, null, 2));
  fs2.chmodSync(expandedPath, 384);
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

// src/cli/utils/address-resolver.ts
var VALID_ADDRESS_TYPES = ["p2tr", "p2wpkh", "p2sh-p2wpkh", "p2pkh"];
function isValidAddressType(type) {
  return VALID_ADDRESS_TYPES.includes(type);
}
function isRawBitcoinAddress(address) {
  if (/^1[a-km-zA-HJ-NP-Z1-9]{25,34}$/.test(address)) return true;
  if (/^3[a-km-zA-HJ-NP-Z1-9]{25,34}$/.test(address)) return true;
  if (/^(bc|tb|bcrt)1q[a-z0-9]{38,}$/.test(address)) return true;
  if (/^(bc|tb|bcrt)1p[a-z0-9]{38,}$/.test(address)) return true;
  return false;
}
function isShorthandIdentifier(input2) {
  const parts = input2.split(":");
  if (parts.length !== 2) return false;
  const [type, indexPart] = parts;
  if (!isValidAddressType(type)) return false;
  if (/^\d+$/.test(indexPart)) return true;
  if (/^\d+-\d+$/.test(indexPart)) return true;
  return false;
}
function isFullIdentifier(input2) {
  return /^\[.+\]$/.test(input2);
}
function containsIdentifiers(input2) {
  if (isShorthandIdentifier(input2)) return true;
  if (isFullIdentifier(input2)) return true;
  return false;
}
function parseShorthandIdentifier(input2) {
  const parts = input2.split(":");
  if (parts.length !== 2) return null;
  const [type, indexPart] = parts;
  if (!isValidAddressType(type)) return null;
  const indices = [];
  if (indexPart.includes("-")) {
    const [start, end] = indexPart.split("-").map(Number);
    if (isNaN(start) || isNaN(end) || start > end) return null;
    for (let i = start; i <= end; i++) {
      indices.push(i);
    }
  } else {
    const index = parseInt(indexPart, 10);
    if (isNaN(index)) return null;
    indices.push(index);
  }
  return { type, indices };
}
var AddressResolver = class {
  constructor(config = {}) {
    this.rawProvider = null;
    this.initialized = false;
    this.config = config;
  }
  /**
   * Initialize the resolver by loading the wallet
   */
  async initialize(createProvider3) {
    if (this.initialized) return true;
    const walletPath = expandPath(this.config.walletFile || "~/.alkanes/wallet.json");
    if (!walletExists(walletPath)) {
      return false;
    }
    try {
      const provider = await createProvider3({
        network: this.config.network,
        jsonrpcUrl: this.config.jsonrpcUrl
      });
      this.rawProvider = provider.rawProvider;
      const walletData = loadWalletFile(walletPath);
      if (!walletData || !walletData.mnemonic) {
        return false;
      }
      this.rawProvider.walletLoadMnemonic(walletData.mnemonic, this.config.passphrase || "");
      this.initialized = true;
      return true;
    } catch (err) {
      return false;
    }
  }
  /**
   * Get a single address from the wallet
   */
  getAddress(addressType, index) {
    if (!this.initialized || !this.rawProvider) return null;
    try {
      const addresses = this.rawProvider.walletGetAddresses(addressType, index, 1);
      if (addresses && addresses.length > 0) {
        return addresses[0].address;
      }
    } catch (err) {
    }
    return null;
  }
  /**
   * Get multiple addresses from the wallet
   */
  getAddresses(addressType, startIndex, count) {
    if (!this.initialized || !this.rawProvider) return [];
    try {
      const addresses = this.rawProvider.walletGetAddresses(addressType, startIndex, count);
      return addresses.map((a) => a.address);
    } catch (err) {
      return [];
    }
  }
  /**
   * Resolve a single address identifier to an actual Bitcoin address
   *
   * Handles:
   * - Raw addresses (returned as-is)
   * - Shorthand identifiers (p2tr:0)
   * - Full identifiers ([self:p2tr:0], [external:bc1q...])
   */
  async resolve(input2) {
    if (isRawBitcoinAddress(input2)) {
      return input2;
    }
    if (isFullIdentifier(input2)) {
      const inner = input2.slice(1, -1);
      const parts = inner.split(":");
      if (parts[0] === "external" && parts.length === 2) {
        return parts[1];
      }
      if (parts[0] === "self" && parts.length === 3) {
        const type = parts[1];
        const index = parseInt(parts[2], 10);
        if (isValidAddressType(type) && !isNaN(index)) {
          const address = this.getAddress(type, index);
          if (address) return address;
        }
      }
      throw new Error(`Cannot resolve identifier: ${input2}`);
    }
    if (isShorthandIdentifier(input2)) {
      const parsed = parseShorthandIdentifier(input2);
      if (!parsed) {
        throw new Error(`Invalid address identifier: ${input2}`);
      }
      if (parsed.indices.length === 1) {
        const address = this.getAddress(parsed.type, parsed.indices[0]);
        if (address) return address;
        throw new Error(`Cannot resolve address for ${input2} - wallet not loaded or address not found`);
      }
      const addresses = this.getAddresses(parsed.type, parsed.indices[0], parsed.indices.length);
      if (addresses.length > 0) {
        return addresses.join(",");
      }
      throw new Error(`Cannot resolve addresses for ${input2}`);
    }
    return input2;
  }
  /**
   * Resolve all identifiers in a string
   * Useful for resolving addresses in complex strings
   */
  async resolveAll(input2) {
    if (isShorthandIdentifier(input2) || isFullIdentifier(input2)) {
      return this.resolve(input2);
    }
    return input2;
  }
};
async function createAddressResolver(config, createProvider3) {
  const resolver = new AddressResolver(config);
  await resolver.initialize(createProvider3);
  return resolver;
}
async function resolveAddressWithProvider(address, provider, opts) {
  if (isRawBitcoinAddress(address)) {
    return address;
  }
  if (!containsIdentifiers(address)) {
    return address;
  }
  const walletPath = expandPath(opts.walletFile || "~/.alkanes/wallet.json");
  if (!walletExists(walletPath)) {
    throw new Error(
      `Wallet not found at ${walletPath}. Address identifier "${address}" requires a loaded wallet.`
    );
  }
  const walletData = loadWalletFile(walletPath);
  if (!walletData || !walletData.mnemonic) {
    throw new Error("Failed to load wallet or wallet has no mnemonic");
  }
  const rawProvider = provider.rawProvider;
  rawProvider.walletLoadMnemonic(walletData.mnemonic, opts.passphrase || "");
  const parsed = parseShorthandIdentifier(address);
  if (!parsed) {
    throw new Error(`Invalid address identifier: ${address}`);
  }
  if (parsed.indices.length === 1) {
    const addresses2 = rawProvider.walletGetAddresses(parsed.type, parsed.indices[0], 1);
    if (addresses2 && addresses2.length > 0) {
      return addresses2[0].address;
    }
    throw new Error(`Could not resolve address for ${address}`);
  }
  const addresses = rawProvider.walletGetAddresses(parsed.type, parsed.indices[0], parsed.indices.length);
  if (addresses && addresses.length > 0) {
    return addresses.map((a) => a.address).join(",");
  }
  throw new Error(`Could not resolve addresses for ${address}`);
}
async function resolveAddressesWithProvider(addresses, provider, opts) {
  if (!addresses) return void 0;
  const addrList = Array.isArray(addresses) ? addresses : [addresses];
  const resolved = [];
  for (const addr of addrList) {
    const resolvedAddr = await resolveAddressWithProvider(addr, provider, opts);
    resolved.push(...resolvedAddr.split(","));
  }
  return resolved;
}

// src/cli/commands/wallet.ts
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
        const provider = await createProvider2({
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
        const rawProvider = provider.rawProvider;
        const walletInfo = rawProvider.walletCreate(
          mnemonic || void 0,
          passphrase
        );
        saveWalletFile(walletPath, {
          mnemonic: walletInfo.mnemonic,
          network: globalOpts.provider || "mainnet",
          created_at: (/* @__PURE__ */ new Date()).toISOString()
        });
        spinner.succeed("Wallet created successfully!");
        console.log();
        success(`Wallet saved to: ${walletPath}`);
        info(`Network: ${walletInfo.network || globalOpts.provider || "mainnet"}`);
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const rawProvider = provider.rawProvider;
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          spinner.fail();
          error("Failed to load wallet or wallet has no mnemonic");
          return;
        }
        rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);
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
        const startIndex = indices[0];
        const count = indices.length;
        const addresses = rawProvider.walletGetAddresses(addressType, startIndex, count);
        console.log();
        const table = createTable(["Index", "Address Type", "Derivation Path", "Address"]);
        for (const addr of addresses) {
          table.push([String(addr.index), addr.script_type, addr.derivation_path, addr.address]);
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          esploraUrl: globalOpts.esploraUrl
        });
        const rawProvider = provider.rawProvider;
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          spinner.fail();
          error("Failed to load wallet or wallet has no mnemonic");
          return;
        }
        rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);
        const utxos = await rawProvider.walletGetUtxos();
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          esploraUrl: globalOpts.esploraUrl
        });
        const rawProvider = provider.rawProvider;
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          spinner.fail();
          error("Failed to load wallet or wallet has no mnemonic");
          return;
        }
        rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);
        const balance = await rawProvider.walletGetBalance();
        spinner.succeed("Balance calculated");
        console.log();
        const total = (balance.confirmed || 0) + (balance.pending || 0);
        success(`Total Balance: ${formatBTC(total)}`);
        info(`Confirmed: ${formatBTC(balance.confirmed || 0)}`);
        if (balance.pending && balance.pending > 0) {
          info(`Pending: ${formatBTC(balance.pending)}`);
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
  wallet.command("send <address> <amount>").description("Send BTC to an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--fee-rate <sats/vB>", "Fee rate in satoshis per virtual byte", "1").option("--from <spec>", "Source addresses (e.g., p2tr:0-5)").action(async (address, amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
      if (!walletExists(walletPath)) {
        error(`Wallet not found at ${walletPath}`);
        return;
      }
      const passphrase = globalOpts.passphrase || await password("Enter wallet passphrase:");
      const spinner = (0, import_ora.default)("Loading wallet...").start();
      try {
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const rawProvider = provider.rawProvider;
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          spinner.fail();
          error("Failed to load wallet or wallet has no mnemonic");
          return;
        }
        rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);
        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const resolvedFrom = options.from ? await resolveAddressesWithProvider([options.from], provider, {
          walletFile: globalOpts.walletFile,
          passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        }) : void 0;
        spinner.stop();
        if (!globalOpts.autoConfirm) {
          console.log();
          info(`Sending ${amount} BTC to ${resolvedAddress}`);
          if (address !== resolvedAddress) {
            info(`  (resolved from ${address})`);
          }
          info(`Fee rate: ${options.feeRate} sats/vB`);
          const confirmed = await confirm("Proceed with transaction?", false);
          if (!confirmed) {
            info("Transaction cancelled");
            return;
          }
        }
        spinner.start("Creating and broadcasting transaction...");
        const sendParams = {
          address: resolvedAddress,
          amount: Math.round(parseFloat(amount) * 1e8),
          // Convert BTC to satoshis
          fee_rate: parseFloat(options.feeRate),
          from: resolvedFrom
        };
        const txid = await rawProvider.walletSend(JSON.stringify(sendParams));
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const rawProvider = provider.rawProvider;
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          spinner.fail();
          error("Failed to load wallet or wallet has no mnemonic");
          return;
        }
        rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);
        const history = await rawProvider.walletGetHistory(options.address);
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        spinner.fail();
        error("PSBT signing is not yet available in the WASM CLI");
        info("Use a full node wallet for PSBT operations");
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        spinner.fail();
        error("UTXO freezing is not yet available in the WASM CLI");
        info("Use a full node wallet for UTXO management");
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        spinner.fail();
        error("UTXO unfreezing is not yet available in the WASM CLI");
        info("Use a full node wallet for UTXO management");
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        spinner.fail();
        error("Transaction creation (PSBT) is not yet available in the WASM CLI");
        info("Use walletSend for direct transactions or a full node wallet for PSBT operations");
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        spinner.fail();
        error("Transaction signing is not yet available in the WASM CLI");
        info("Use walletSend for direct transactions or a full node wallet for signing");
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const decoded = await provider.rawProvider.bitcoindDecodeRawTransaction(txHex);
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
        const txid = await provider.rawProvider.bitcoindSendRawTransaction(txHex);
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
      error("Fee estimation is not yet available in the WASM CLI");
      info("Use esplora fee-estimates for current network fee rates");
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
        const provider = await createProvider2({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          esploraUrl: globalOpts.esploraUrl
        });
        const rates = await provider.rawProvider.esploraGetFeeEstimates();
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
      error("Wallet sync is not yet available in the WASM CLI");
      info("The WASM wallet syncs automatically when querying balance/UTXOs");
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
      const walletData = loadWalletFile(walletPath);
      if (!walletData || !walletData.mnemonic) {
        error("Failed to load wallet or wallet has no mnemonic");
        return;
      }
      console.log();
      console.log(import_chalk2.default.yellow.bold("\u26A0 WARNING: Keep this mnemonic safe and private!"));
      console.log();
      console.log(import_chalk2.default.cyan(walletData.mnemonic));
      console.log();
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
  bitcoind.command("getblockcount").description("Get current block count").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block count...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const blockCount = await provider.bitcoin.getBlockCount();
      spinner.succeed();
      console.log(formatOutput(blockCount, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block count: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("generatetoaddress <nblocks> <address>").description("Generate blocks to an address (regtest only). Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (nblocks, address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)(`Generating ${nblocks} blocks...`).start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = address;
      if (containsIdentifiers(address)) {
        spinner.text = "Loading wallet...";
        const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
        if (!walletExists(walletPath)) {
          spinner.fail();
          error(`Wallet not found at ${walletPath}`);
          info("Create a wallet first with: alkanes-bindgen-cli wallet create");
          process.exit(1);
        }
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          spinner.fail();
          error("Failed to load wallet or wallet has no mnemonic");
          process.exit(1);
        }
        const rawProvider = provider.rawProvider;
        rawProvider.walletLoadMnemonic(walletData.mnemonic, globalOpts.passphrase || "");
        const resolver = await createAddressResolver({
          walletFile: walletPath,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        }, createProvider2);
        resolvedAddress = await resolver.resolve(address);
        spinner.text = `Generating ${nblocks} blocks to ${resolvedAddress}...`;
      }
      const hashes = await provider.bitcoin.generateToAddress(parseInt(nblocks), resolvedAddress);
      spinner.succeed(`Generated ${nblocks} blocks to ${resolvedAddress}`);
      console.log(formatOutput(hashes, { raw: options.raw }));
    } catch (err) {
      error(`Failed to generate blocks: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockchaininfo").description("Get blockchain information").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting blockchain info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const info2 = await provider.bitcoin.getBlockchainInfo();
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(info2, { raw: true }));
      } else {
        console.log(formatBlockchainInfo(info2));
      }
    } catch (err) {
      error(`Failed to get blockchain info: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getrawtransaction <txid>").description("Get raw transaction").option("--verbose", "Return decoded transaction").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const tx = await provider.bitcoin.getTransaction(txid);
      spinner.succeed();
      console.log(formatOutput(tx, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transaction: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblock <hash>").description("Get block by hash").option("--verbosity <level>", "Verbosity level (0-2)", "1").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const rawOutput = options.verbosity === "0";
      const block = await provider.bitcoin.getBlock(hash, rawOutput);
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(block, { raw: true }));
      } else if (typeof block === "object") {
        console.log(formatBlockInfo(block));
      } else {
        console.log(formatOutput(block, { raw: false }));
      }
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockhash <height>").description("Get block hash by height").option("--raw", "Output raw JSON").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const hash = await provider.bitcoin.getBlockHash(parseInt(height));
      spinner.succeed();
      console.log(formatOutput(hash, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block hash: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("sendrawtransaction <hex>").description("Broadcast a raw transaction").option("--raw", "Output raw JSON").action(async (hex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Broadcasting transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const txid = await provider.bitcoin.sendRawTransaction(hex);
      spinner.succeed("Transaction broadcast");
      if (options.raw) {
        console.log(formatOutput(txid, { raw: true }));
      } else {
        success(`TXID: ${txid}`);
      }
    } catch (err) {
      error(`Failed to broadcast transaction: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getnetworkinfo").description("Get network information").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting network info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const info2 = await provider.bitcoin.getNetworkInfo();
      spinner.succeed();
      console.log(formatOutput(info2, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get network info: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getmempoolinfo").description("Get mempool information").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting mempool info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const info2 = await provider.bitcoin.getMempoolInfo();
      spinner.succeed();
      console.log(formatOutput(info2, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get mempool info: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("generatefuture <address>").description("Generate a future block (regtest only). Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Generating future block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = address;
      if (containsIdentifiers(address)) {
        spinner.text = "Loading wallet...";
        const walletPath = expandPath(globalOpts.walletFile || "~/.alkanes/wallet.json");
        if (!walletExists(walletPath)) {
          spinner.fail();
          error(`Wallet not found at ${walletPath}`);
          info("Create a wallet first with: alkanes-bindgen-cli wallet create");
          process.exit(1);
        }
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          spinner.fail();
          error("Failed to load wallet or wallet has no mnemonic");
          process.exit(1);
        }
        const rawProvider = provider.rawProvider;
        rawProvider.walletLoadMnemonic(walletData.mnemonic, globalOpts.passphrase || "");
        const resolver = await createAddressResolver({
          walletFile: walletPath,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        }, createProvider2);
        resolvedAddress = await resolver.resolve(address);
        spinner.text = `Generating future block to ${resolvedAddress}...`;
      }
      const hash = await provider.bitcoin.generateFuture(resolvedAddress);
      spinner.succeed(`Future block generated to ${resolvedAddress}`);
      console.log(formatOutput(hash, { raw: options.raw }));
    } catch (err) {
      error(`Failed to generate future block: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockheader <hash>").description("Get block header by hash").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block header...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const header = await provider.bitcoin.getBlockHeader(hash);
      spinner.succeed();
      console.log(formatOutput(header, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block header: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getblockstats <hash>").description("Get block statistics by hash").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting block stats...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const stats = await provider.bitcoin.getBlockStats(hash);
      spinner.succeed();
      console.log(formatOutput(stats, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block stats: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("estimatesmartfee <blocks>").description("Estimate smart fee for confirmation in N blocks").option("--raw", "Output raw JSON").action(async (blocks, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Estimating fee...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const estimate = await provider.bitcoin.estimateSmartFee(parseInt(blocks));
      spinner.succeed();
      console.log(formatOutput(estimate, { raw: options.raw }));
    } catch (err) {
      error(`Failed to estimate fee: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getchaintips").description("Get chain tips information").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting chain tips...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const tips = await provider.bitcoin.getChainTips();
      spinner.succeed();
      console.log(formatOutput(tips, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get chain tips: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("decoderawtransaction <hex>").description("Decode a raw transaction hex").option("--raw", "Output raw JSON").action(async (hex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Decoding transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const decoded = await provider.bitcoin.decodeRawTransaction(hex);
      spinner.succeed();
      console.log(formatOutput(decoded, { raw: options.raw }));
    } catch (err) {
      error(`Failed to decode transaction: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("decodepsbt <psbt>").description("Decode a PSBT (base64)").option("--raw", "Output raw JSON").action(async (psbt, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Decoding PSBT...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const decoded = await provider.bitcoin.decodePsbt(psbt);
      spinner.succeed();
      console.log(formatOutput(decoded, { raw: options.raw }));
    } catch (err) {
      error(`Failed to decode PSBT: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("getrawmempool").description("Get raw mempool transactions").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting mempool transactions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const mempool = await provider.bitcoin.getRawMempool();
      spinner.succeed();
      console.log(formatOutput(mempool, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get mempool: ${err.message}`);
      process.exit(1);
    }
  });
  bitcoind.command("gettxout <txid> <vout>").description("Get transaction output details").option("--include-mempool", "Include mempool transactions", false).option("--raw", "Output raw JSON").action(async (txid, vout, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Getting transaction output...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const txout = await provider.bitcoin.getTxOut(txid, parseInt(vout), options.includeMempool);
      spinner.succeed();
      console.log(formatOutput(txout, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get tx out: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/alkanes.ts
var import_chalk3 = __toESM(require("chalk"));
init_formatting();
var import_ora3 = __toESM(require("ora"));
function registerAlkanesCommands(program2) {
  const alkanes = program2.command("alkanes").description("Alkanes smart contract operations");
  alkanes.command("getbytecode <alkane-id>").description("Get bytecode for an alkanes contract").option("--block-tag <tag>", 'Block tag (e.g., "latest" or height)').option("--raw", "Output raw JSON").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting bytecode...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const bytecode = await provider.alkanes.getBytecode(alkaneId, options.blockTag);
      spinner.succeed();
      console.log(formatOutput(bytecode, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get bytecode: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("balance").description("Get alkanes balance for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--address <address>", "Address to check (e.g., p2tr:0 or bc1q...)").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      let resolvedAddress = options.address;
      if (options.address) {
        resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const balance = await provider.alkanes.getBalance(resolvedAddress);
      spinner.succeed();
      if (options.address && options.address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
      }
      if (options.raw) {
        console.log(formatOutput(balance, { raw: true }));
      } else if (Array.isArray(balance)) {
        console.log(formatAlkaneBalances(balance));
      } else {
        console.log(formatOutput(balance, { raw: false }));
      }
    } catch (err) {
      error(`Failed to get balance: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("trace <outpoint>").description("Trace an alkanes transaction").option("--raw", "Output raw JSON").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Tracing transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const trace = await provider.alkanes.trace(outpoint);
      spinner.succeed();
      console.log(formatOutput(trace, { raw: options.raw }));
    } catch (err) {
      error(`Failed to trace: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("inspect <target>").description("Inspect alkanes bytecode").option("--disasm", "Enable disassembly to WAT format", false).option("--fuzz", "Enable fuzzing analysis", false).option("--fuzz-ranges <ranges>", "Opcode ranges for fuzzing").option("--raw", "Output raw JSON").action(async (target, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Inspecting bytecode...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const config = {
        disasm: options.disasm,
        fuzz: options.fuzz,
        fuzzRanges: options.fuzzRanges
      };
      const result = await provider.alkanes.inspect(target, config);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to inspect: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("inspect-bytecode <bytecode>").description("Inspect alkanes bytecode directly from file or hex string (no RPC fetch)").option("--alkane-id <id>", "Alkane ID for context (format: block:tx)", "0:0").option("--disasm", "Enable disassembly to WAT format", false).option("--fuzz", "Enable fuzzing analysis", false).option("--fuzz-ranges <ranges>", 'Opcode ranges for fuzzing (e.g., "0-100,200-300")').option("--meta", "Extract and display metadata", false).option("--codehash", "Compute and display codehash", false).option("--raw", "Output raw JSON").action(async (bytecode, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Inspecting bytecode...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      let bytecodeHex;
      const fs3 = await import("fs");
      if (fs3.existsSync(bytecode)) {
        const fileContent = fs3.readFileSync(bytecode);
        bytecodeHex = fileContent.toString("hex");
      } else {
        bytecodeHex = bytecode;
      }
      const config = {
        disasm: options.disasm,
        fuzz: options.fuzz,
        fuzz_ranges: options.fuzzRanges,
        meta: options.meta,
        codehash: options.codehash,
        raw: options.raw
      };
      const result = await provider.alkanes.inspectBytecode(bytecodeHex, options.alkaneId, config);
      spinner.succeed("Inspection complete");
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to inspect bytecode: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("simulate <contract-id>").description("Simulate alkanes execution (format: block:tx or block:tx:opcode)").option("--inputs <alkanes>", "Input alkanes as comma-separated triplets (e.g., 2:1:1000,2:2:500)").option("--height <height>", "Block height for simulation").option("--txindex <index>", "Transaction index (default: 1)", "1").option("--pointer <ptr>", "Pointer value (default: 0)", "0").option("--refund <ptr>", "Refund pointer (default: 0)", "0").option("--block-tag <tag>", "Block tag to query").option("--raw", "Output raw JSON").action(async (contractId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Simulating execution...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const parts = contractId.split(":");
      if (parts.length < 2 || parts.length > 3) {
        throw new Error("Invalid contract-id format. Use block:tx or block:tx:opcode (e.g., 2:112 or 2:112:10)");
      }
      const targetBlock = parseInt(parts[0], 10);
      const targetTx = parseInt(parts[1], 10);
      const calldataOpcode = parts.length === 3 ? parseInt(parts[2], 10) : 0;
      const alkanes2 = [];
      if (options.inputs) {
        const inputParts = options.inputs.split(",");
        for (const input2 of inputParts) {
          const [block, tx, amount] = input2.split(":").map((s) => parseInt(s, 10));
          if (isNaN(block) || isNaN(tx) || isNaN(amount)) {
            throw new Error(`Invalid input format: ${input2}. Use block:tx:amount`);
          }
          alkanes2.push({
            id: { block: { lo: block, hi: 0 }, tx: { lo: tx, hi: 0 } },
            value: { lo: amount, hi: 0 }
          });
        }
      }
      let height = options.height ? parseInt(options.height, 10) : 0;
      if (!height) {
        try {
          height = await provider.metashrew.height();
        } catch {
          height = 0;
        }
      }
      const calldata = [];
      let value = targetBlock;
      do {
        let byte = value & 127;
        value >>>= 7;
        if (value !== 0) byte |= 128;
        calldata.push(byte);
      } while (value !== 0);
      value = targetTx;
      do {
        let byte = value & 127;
        value >>>= 7;
        if (value !== 0) byte |= 128;
        calldata.push(byte);
      } while (value !== 0);
      value = calldataOpcode;
      do {
        let byte = value & 127;
        value >>>= 7;
        if (value !== 0) byte |= 128;
        calldata.push(byte);
      } while (value !== 0);
      const context = {
        alkanes: alkanes2,
        transaction: [],
        // Empty byte array
        block: [],
        // Empty byte array
        height,
        txindex: parseInt(options.txindex, 10),
        calldata: Array.from(calldata),
        // Pass as array of numbers
        vout: 0,
        pointer: parseInt(options.pointer, 10),
        refund_pointer: parseInt(options.refund, 10)
      };
      const contractIdStr = `${targetBlock}:${targetTx}`;
      const result = await provider.alkanes.simulate(contractIdStr, context, options.blockTag);
      spinner.succeed();
      if (typeof result === "string" && result.startsWith("0x") && !options.raw) {
        try {
          const hexData = result.slice(2);
          const bytes = Buffer.from(hexData, "hex");
          let pos = 0;
          let gasUsed = 0;
          let errorMsg = "";
          let executionData = "";
          while (pos < bytes.length) {
            const tag = bytes[pos++];
            const fieldNum = tag >> 3;
            const wireType = tag & 7;
            if (wireType === 0) {
              let value2 = 0;
              let shift = 0;
              while (pos < bytes.length) {
                const b = bytes[pos++];
                value2 |= (b & 127) << shift;
                if ((b & 128) === 0) break;
                shift += 7;
              }
              if (fieldNum === 2) gasUsed = value2;
            } else if (wireType === 2) {
              let len = 0;
              let shift = 0;
              while (pos < bytes.length) {
                const b = bytes[pos++];
                len |= (b & 127) << shift;
                if ((b & 128) === 0) break;
                shift += 7;
              }
              const data = bytes.slice(pos, pos + len);
              pos += len;
              if (fieldNum === 1) executionData = "0x" + data.toString("hex");
              if (fieldNum === 3) errorMsg = data.toString("utf8");
            }
          }
          console.log();
          if (errorMsg) {
            console.log(import_chalk3.default.red(`Error: ${errorMsg}`));
          } else {
            console.log(import_chalk3.default.green("\u2713 Simulation successful"));
          }
          if (gasUsed) console.log(`Gas used: ${gasUsed}`);
          if (executionData && executionData !== "0x") console.log(`Execution: ${executionData}`);
          console.log();
          console.log(import_chalk3.default.gray(`Raw: ${result}`));
        } catch {
          console.log(formatOutput(result, { raw: true }));
        }
      } else {
        console.log(formatOutput(result, { raw: options.raw }));
      }
    } catch (err) {
      error(`Failed to simulate: ${err.message || err}`);
      process.exit(1);
    }
  });
  alkanes.command("unwrap").description("Get pending unwraps").option("--block-tag <tag>", "Block tag").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting pending unwraps...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getPendingUnwraps(options.blockTag);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get pending unwraps: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("get-all-pools <factory-id>").description("Get all pools from an AMM factory").option("--raw", "Output raw JSON").action(async (factoryId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting pools...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getAllPools(factoryId);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get pools: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("all-pools-details <factory-id>").description("Get all pools with detailed information").option("--raw", "Output raw JSON").action(async (factoryId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting pool details...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getAllPoolsWithDetails(factoryId);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get pool details: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("reflect <alkane-id>").description("Reflect alkane metadata").option("--raw", "Output raw JSON").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Reflecting alkane...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.reflect(alkaneId);
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(result, { raw: true }));
      } else {
        console.log(formatReflectMetadata(result));
      }
    } catch (err) {
      error(`Failed to reflect: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("by-address <address>").description("Get alkanes by address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--block-tag <tag>", "Block tag").option("--protocol-tag <tag>", "Protocol tag").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting alkanes by address...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.alkanes.getByAddress(
        resolvedAddress,
        options.blockTag,
        options.protocolTag ? parseInt(options.protocolTag) : void 0
      );
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      if (options.raw) {
        console.log(formatOutput(result, { raw: true }));
      } else if (Array.isArray(result)) {
        console.log(formatAlkaneBalances(result));
      } else {
        console.log(formatOutput(result, { raw: false }));
      }
    } catch (err) {
      error(`Failed to get alkanes: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("by-outpoint <outpoint>").description("Get alkanes by outpoint").option("--block-tag <tag>", "Block tag").option("--protocol-tag <tag>", "Protocol tag").option("--raw", "Output raw JSON").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting alkanes by outpoint...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getByOutpoint(
        outpoint,
        options.blockTag,
        options.protocolTag ? parseInt(options.protocolTag) : void 0
      );
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get alkanes: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("traceblock <height>").description("Trace all alkanes transactions in a block").option("--raw", "Output raw JSON").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Tracing block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.traceBlock(parseInt(height));
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to trace block: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("sequence").description("Get sequence for the current block").option("--block-tag <tag>", "Block tag").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting sequence...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getSequence(options.blockTag);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get sequence: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("spendables <address>").description("Get spendable outpoints for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting spendables...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.alkanes.getSpendables(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get spendables: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("pool-details <pool-id>").description("Get detailed information about a specific pool").option("--raw", "Output raw JSON").action(async (poolId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting pool details...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getPoolDetails(poolId);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get pool details: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("reflect-alkane-range <block> <start-tx> <end-tx>").description("Reflect metadata for a range of alkanes in a block").option("--raw", "Output raw JSON").action(async (block, startTx, endTx, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Reflecting alkane range...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.reflectAlkaneRange(
        parseInt(block),
        parseInt(startTx),
        parseInt(endTx)
      );
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to reflect alkane range: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("execute").description("Execute an alkanes smart contract").option("--contract <id>", "Contract ID").option("--inputs <json>", "Input parameters JSON").option("--target <target>", "Target address").option("--pointer <pointer>", "Pointer value").option("--refund-pointer <pointer>", "Refund pointer").option("--feeRate <rate>", "Fee rate in sat/vB").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Executing contract...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const params = {
        contractId: options.contract,
        inputs: options.inputs ? JSON.parse(options.inputs) : [],
        target: options.target,
        pointer: options.pointer ? parseInt(options.pointer) : void 0,
        refundPointer: options.refundPointer ? parseInt(options.refundPointer) : void 0,
        feeRate: options.feeRate ? parseFloat(options.feeRate) : void 0
      };
      const result = await provider._provider.alkanesExecuteWithStrings(
        JSON.stringify(params.inputs),
        params.contractId,
        params.pointer || 0,
        params.refundPointer || 0,
        params.target || "",
        params.feeRate || 1
      );
      spinner.succeed("Contract executed");
      console.log(formatOutput(JSON.parse(result), { raw: options.raw }));
    } catch (err) {
      error(`Failed to execute: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("wrap-btc <amount>").description("Wrap BTC to frBTC").option("--feeRate <rate>", "Fee rate in sat/vB").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Wrapping BTC...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const params = {
        amount: parseInt(amount),
        feeRate: options.feeRate ? parseFloat(options.feeRate) : 1
      };
      const result = await provider._provider.alkanesWrapBtc(JSON.stringify(params));
      spinner.succeed("BTC wrapped");
      console.log(formatOutput(JSON.parse(result), { raw: options.raw }));
    } catch (err) {
      error(`Failed to wrap BTC: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("init-pool").description("Initialize a new AMM liquidity pool").option("--token0 <id>", "First token ID").option("--token1 <id>", "Second token ID").option("--amount0 <amount>", "Amount of first token").option("--amount1 <amount>", "Amount of second token").option("--feeRate <rate>", "Fee rate in sat/vB").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Initializing pool...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const params = {
        token0: options.token0,
        token1: options.token1,
        amount0: options.amount0,
        amount1: options.amount1,
        feeRate: options.feeRate ? parseFloat(options.feeRate) : 1
      };
      const txid = await provider._provider.alkanesInitPool(JSON.stringify(params));
      spinner.succeed("Pool initialized");
      if (options.raw) {
        console.log(formatOutput({ txid }, { raw: true }));
      } else {
        success(`TXID: ${txid}`);
      }
    } catch (err) {
      error(`Failed to init pool: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("swap").description("Execute an AMM token swap").option("--token-in <id>", "Token to swap from").option("--token-out <id>", "Token to swap to").option("--amount-in <amount>", "Amount to swap").option("--min-amount-out <amount>", "Minimum output amount").option("--feeRate <rate>", "Fee rate in sat/vB").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Executing swap...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const params = {
        tokenIn: options.tokenIn,
        tokenOut: options.tokenOut,
        amountIn: options.amountIn,
        minAmountOut: options.minAmountOut || "0",
        feeRate: options.feeRate ? parseFloat(options.feeRate) : 1
      };
      const txid = await provider._provider.alkanesSwap(JSON.stringify(params));
      spinner.succeed("Swap executed");
      if (options.raw) {
        console.log(formatOutput({ txid }, { raw: true }));
      } else {
        success(`TXID: ${txid}`);
      }
    } catch (err) {
      error(`Failed to swap: ${err.message}`);
      process.exit(1);
    }
  });
  alkanes.command("tx-script").description("Execute a tx-script with WASM bytecode").option("--bytecode <hex>", "WASM bytecode hex").option("--feeRate <rate>", "Fee rate in sat/vB").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Executing tx-script...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const params = {
        bytecode: options.bytecode,
        feeRate: options.feeRate ? parseFloat(options.feeRate) : 1
      };
      const result = await provider._provider.alkanesTxScript(JSON.stringify(params));
      spinner.succeed("tx-script executed");
      console.log(formatOutput(JSON.parse(result), { raw: options.raw }));
    } catch (err) {
      error(`Failed to execute tx-script: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/esplora.ts
init_formatting();
var import_ora4 = __toESM(require("ora"));
function registerEsploraCommands(program2) {
  const esplora = program2.command("esplora").description("Esplora REST API operations");
  esplora.command("tx <txid>").description("Get transaction by txid").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const tx = await provider.esplora.getTx(txid);
      spinner.succeed();
      console.log(formatOutput(tx, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transaction: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-status <txid>").description("Get transaction status").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transaction status...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const status = await provider.esplora.getTxStatus(txid);
      spinner.succeed();
      console.log(formatOutput(status, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transaction status: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address <address>").description("Get address information. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting address info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider
      });
      const addrInfo = await provider.esplora.getAddressInfo(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(addrInfo, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get address info: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-utxos <address>").description("Get UTXOs for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting UTXOs...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider
      });
      const utxos = await provider.esplora.getAddressUtxos(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(utxos, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get UTXOs: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs <address>").description("Get transactions for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transactions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider
      });
      const txs = await provider.esplora.getAddressTxs(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(txs, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transactions: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs-chain <address>").description("Get paginated transactions for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--last-seen <txid>", "Last seen txid for pagination").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transactions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider
      });
      const txs = await provider.esplora.getAddressTxsChain(resolvedAddress, options.lastSeen);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(txs, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transactions: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("blocks-tip-height").description("Get current block tip height").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting tip height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const height = await provider.esplora.getBlocksTipHeight();
      spinner.succeed();
      console.log(formatOutput(height, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get tip height: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("blocks-tip-hash").description("Get current block tip hash").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting tip hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const hash = await provider.esplora.getBlocksTipHash();
      spinner.succeed();
      console.log(formatOutput(hash, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get tip hash: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("fee-estimates").description("Get fee estimates").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting fee estimates...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const estimates = await provider.esplora.getFeeEstimates();
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(estimates, { raw: true }));
      } else {
        console.log(formatFeeEstimates(estimates));
      }
    } catch (err) {
      error(`Failed to get fee estimates: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("broadcast-tx <hex>").description("Broadcast a transaction").option("--raw", "Output raw JSON").action(async (hex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Broadcasting transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txid = await provider.esplora.broadcastTx(hex);
      spinner.succeed("Transaction broadcast");
      if (options.raw) {
        console.log(formatOutput({ txid }, { raw: true }));
      } else {
        success(`TXID: ${txid}`);
      }
    } catch (err) {
      error(`Failed to broadcast transaction: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-hex <txid>").description("Get raw transaction hex").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transaction hex...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const hex = await provider.esplora.getTxHex(txid);
      spinner.succeed();
      console.log(formatOutput(hex, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transaction hex: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("blocks [start-height]").description("Get blocks starting from height").option("--raw", "Output raw JSON").action(async (startHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting blocks...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const blocks = await provider.esplora.getBlocks(startHeight ? parseInt(startHeight) : void 0);
      spinner.succeed();
      console.log(formatOutput(blocks, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get blocks: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-height <height>").description("Get block hash by height").option("--raw", "Output raw JSON").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const hash = await provider.esplora.getBlockByHeight(parseInt(height));
      spinner.succeed();
      console.log(formatOutput(hash, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block <hash>").description("Get block by hash").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const block = await provider.esplora.getBlock(hash);
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(block, { raw: true }));
      } else {
        console.log(formatBlockInfo(block));
      }
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-status <hash>").description("Get block status").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block status...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const status = await provider.esplora.getBlockStatus(hash);
      spinner.succeed();
      console.log(formatOutput(status, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block status: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-txids <hash>").description("Get transaction IDs in block").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block txids...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txids = await provider.esplora.getBlockTxids(hash);
      spinner.succeed();
      console.log(formatOutput(txids, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block txids: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-header <hash>").description("Get block header").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block header...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const header = await provider.esplora.getBlockHeader(hash);
      spinner.succeed();
      console.log(formatOutput(header, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block header: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-raw <hash>").description("Get raw block data").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting raw block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const raw = await provider.esplora.getBlockRaw(hash);
      spinner.succeed();
      console.log(formatOutput(raw, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get raw block: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-txid <hash> <index>").description("Get transaction ID by block hash and index").option("--raw", "Output raw JSON").action(async (hash, index, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block txid...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txid = await provider.esplora.getBlockTxid(hash, parseInt(index));
      spinner.succeed();
      console.log(formatOutput(txid, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block txid: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("block-txs <hash> [start-index]").description("Get block transactions").option("--raw", "Output raw JSON").action(async (hash, startIndex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting block txs...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txs = await provider.esplora.getBlockTxs(hash, startIndex ? parseInt(startIndex) : void 0);
      spinner.succeed();
      console.log(formatOutput(txs, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block txs: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs-mempool <address>").description("Get mempool transactions for address").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting mempool transactions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txs = await provider.esplora.getAddressTxsMempool(address);
      spinner.succeed();
      console.log(formatOutput(txs, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get mempool transactions: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-prefix <prefix>").description("Search addresses by prefix").option("--raw", "Output raw JSON").action(async (prefix, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Searching addresses...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const addresses = await provider.esplora.getAddressPrefix(prefix);
      spinner.succeed();
      console.log(formatOutput(addresses, { raw: options.raw }));
    } catch (err) {
      error(`Failed to search addresses: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-raw <txid>").description("Get raw transaction").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting raw transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const raw = await provider.esplora.getTxRaw(txid);
      spinner.succeed();
      console.log(formatOutput(raw, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get raw transaction: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-merkle-proof <txid>").description("Get merkle proof for transaction").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting merkle proof...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const proof = await provider.esplora.getTxMerkleProof(txid);
      spinner.succeed();
      console.log(formatOutput(proof, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get merkle proof: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-merkleblock-proof <txid>").description("Get merkle block proof").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting merkleblock proof...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const proof = await provider.esplora.getTxMerkleblockProof(txid);
      spinner.succeed();
      console.log(formatOutput(proof, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get merkleblock proof: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-outspend <txid> <index>").description("Get outspend for transaction output").option("--raw", "Output raw JSON").action(async (txid, index, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting outspend...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const outspend = await provider.esplora.getTxOutspend(txid, parseInt(index));
      spinner.succeed();
      console.log(formatOutput(outspend, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get outspend: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("tx-outspends <txid>").description("Get all outspends for transaction").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting outspends...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const outspends = await provider.esplora.getTxOutspends(txid);
      spinner.succeed();
      console.log(formatOutput(outspends, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get outspends: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("mempool").description("Get mempool info").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting mempool info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const mempool = await provider.esplora.getMempool();
      spinner.succeed();
      console.log(formatOutput(mempool, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get mempool info: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("mempool-txids").description("Get mempool transaction IDs").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting mempool txids...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txids = await provider.esplora.getMempoolTxids();
      spinner.succeed();
      console.log(formatOutput(txids, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get mempool txids: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("mempool-recent").description("Get recent mempool transactions").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting recent mempool txs...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txs = await provider.esplora.getMempoolRecent();
      spinner.succeed();
      console.log(formatOutput(txs, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get recent mempool txs: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("post-tx <tx-hex>").description("Post transaction (alternative to broadcast)").option("--raw", "Output raw JSON").action(async (txHex, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Posting transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txid = await provider.esplora.broadcastTx(txHex);
      spinner.succeed("Transaction posted");
      if (options.raw) {
        console.log(formatOutput({ txid }, { raw: true }));
      } else {
        success(`TXID: ${txid}`);
      }
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
  ord.command("inscription <id>").description("Get inscription by ID").option("--raw", "Output raw JSON").action(async (id, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting inscription...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getInscription(id);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get inscription: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("inscriptions").description("List inscriptions").option("--page <number>", "Page number", "0").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting inscriptions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getInscriptions(parseInt(options.page));
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(result, { raw: true }));
      } else {
        console.log(formatInscriptions(result));
      }
    } catch (err) {
      error(`Failed to get inscriptions: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("outputs <address>").description("Get ordinal outputs for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting outputs...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getOutputs(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get outputs: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("rune <name>").description("Get rune information").option("--raw", "Output raw JSON").action(async (name, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting rune...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getRune(name);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get rune: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("list <outpoint>").description("List ordinals in an output").option("--raw", "Output raw JSON").action(async (outpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Listing ordinals...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.list(outpoint);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to list ordinals: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("find <sat>").description("Find ordinal by satoshi number").option("--raw", "Output raw JSON").action(async (sat, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Finding ordinal...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.find(parseInt(sat));
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to find ordinal: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("address-info <address>").description("Get address information. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting address info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getAddressInfo(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get address info: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("block-info <query>").description("Get block information (height or hash)").option("--raw", "Output raw JSON").action(async (query, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting block info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getBlockInfo(query);
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(result, { raw: true }));
      } else {
        console.log(formatBlockInfo(result));
      }
    } catch (err) {
      error(`Failed to get block info: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("block-count").description("Get latest block count").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting block count...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getBlockCount();
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block count: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("blocks").description("Get latest blocks").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting blocks...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getBlocks();
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get blocks: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("children <inscription-id>").description("Get children of an inscription").option("--page <number>", "Page number", "0").option("--raw", "Output raw JSON").action(async (inscriptionId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting children...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getChildren(inscriptionId, parseInt(options.page));
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get children: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("content <inscription-id>").description("Get inscription content").option("--raw", "Output raw JSON").action(async (inscriptionId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting content...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getContent(inscriptionId);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get content: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("parents <inscription-id>").description("Get parents of an inscription").option("--page <number>", "Page number", "0").option("--raw", "Output raw JSON").action(async (inscriptionId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting parents...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getParents(inscriptionId, parseInt(options.page));
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get parents: ${err.message}`);
      process.exit(1);
    }
  });
  ord.command("tx-info <txid>").description("Get transaction information").option("--raw", "Output raw JSON").action(async (txid, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting transaction info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getTxInfo(txid);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
  protorunes.command("by-address <address>").description("Get protorunes by address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--block-tag <tag>", 'Block tag (e.g., "latest" or height)').action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora7.default)("Getting protorunes...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.alkanesByAddress(
        resolvedAddress,
        options.blockTag || null,
        1
      );
      const protorunes2 = JSON.parse(result);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
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
      const provider = await createProvider2({
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
  metashrew.command("height").description("Get current metashrew height").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Getting height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const height = await provider.metashrew.getHeight();
      spinner.succeed();
      console.log(formatOutput(height, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get height: ${err.message}`);
      process.exit(1);
    }
  });
  metashrew.command("state-root").description("Get state root at height").option("--height <number>", "Block height").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Getting state root...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const height = options.height ? parseInt(options.height) : void 0;
      const stateRoot = await provider.metashrew.getStateRoot(height);
      spinner.succeed();
      console.log(formatOutput(stateRoot, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get state root: ${err.message}`);
      process.exit(1);
    }
  });
  metashrew.command("getblockhash <height>").description("Get block hash at height").option("--raw", "Output raw JSON").action(async (height, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Getting block hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const hash = await provider.metashrew.getBlockHash(parseInt(height));
      spinner.succeed();
      console.log(formatOutput(hash, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block hash: ${err.message}`);
      process.exit(1);
    }
  });
  metashrew.command("view <function> <payload> <block-tag>").description("Call metashrew view function").option("--raw", "Output raw JSON").action(async (fn, payload, blockTag, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora8.default)("Calling view function...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.metashrew.view(fn, payload, blockTag);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
  dataapi.command("address-balances <address>").description("Get alkanes balances for address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--include-outpoints", "Include outpoint details", false).action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting balances...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.data_api_get_address_balances_js(
        resolvedAddress,
        options.includeOutpoints
      );
      const balances = JSON.parse(result);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(balances, globalOpts));
    } catch (err) {
      error(`Failed to get balances: ${err.message}`);
      process.exit(1);
    }
  });
  dataapi.command("alkanes-by-address <address>").description("Get alkanes owned by address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting alkanes...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.data_api_get_alkanes_by_address_js(resolvedAddress);
      const alkanes = JSON.parse(result);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const height = await provider.espo.getHeight();
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const pong = await provider.espo.ping();
      spinner.succeed();
      console.log(formatOutput(pong, globalOpts));
    } catch (err) {
      error(`Failed to ping: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("address-balances <address>").description("Get balances for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--include-outpoints", "Include outpoint details", false).action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting balances...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const balances = await provider.espo.getAddressBalances(
        resolvedAddress,
        options.includeOutpoints
      );
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(balances, globalOpts));
    } catch (err) {
      error(`Failed to get balances: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("address-outpoints <address>").description("Get outpoints for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting outpoints...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const outpoints = await provider.espo.getAddressOutpoints(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const balances = await provider.espo.getOutpointBalances(outpoint);
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const holders = await provider.espo.getHolders(
        alkaneId,
        parseInt(options.page, 10),
        parseInt(options.limit, 10)
      );
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const count = await provider.espo.getHoldersCount(alkaneId);
      spinner.succeed();
      console.log(formatOutput({ count }, globalOpts));
    } catch (err) {
      error(`Failed to get holder count: ${err.message}`);
      process.exit(1);
    }
  });
  espo.command("keys <alkane-id>").description("Get storage keys for an alkane").option("--page <page>", "Page number", "0").option("--limit <limit>", "Limit results", "100").action(async (alkaneId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting keys...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const keys = await provider.espo.getKeys(
        alkaneId,
        parseInt(options.page, 10),
        parseInt(options.limit, 10)
      );
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const pong = await provider.espo.ammdataPing();
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const candles = await provider.espo.getCandles(
        pool,
        options.timeframe,
        options.side,
        options.limit ? parseInt(options.limit, 10) : void 0,
        options.page ? parseInt(options.page, 10) : void 0
      );
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const trades = await provider.espo.getTrades(
        pool,
        options.limit ? parseInt(options.limit, 10) : void 0,
        options.page ? parseInt(options.page, 10) : void 0,
        options.side,
        options.filterSide,
        options.sort,
        options.dir
      );
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const pools = await provider.espo.getPools(
        options.limit ? parseInt(options.limit, 10) : void 0,
        options.page ? parseInt(options.page, 10) : void 0
      );
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const path3 = await provider.espo.findBestSwapPath(
        tokenIn,
        tokenOut,
        options.mode,
        options.amountIn,
        options.amountOut,
        options.amountOutMin,
        options.amountInMax,
        options.availableIn,
        options.feeBps ? parseInt(options.feeBps, 10) : void 0,
        options.maxHops ? parseInt(options.maxHops, 10) : void 0
      );
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const mevSwap = await provider.espo.getBestMevSwap(
        token,
        options.feeBps ? parseInt(options.feeBps, 10) : void 0,
        options.maxHops ? parseInt(options.maxHops, 10) : void 0
      );
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
  const brc20Prog = program2.command("brc20-prog").description("Programmable BRC-20 operations");
  brc20Prog.command("balance <address>").description("Get balance for address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--block <tag>", "Block tag").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getBalance(resolvedAddress, options.block);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get balance: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("code <address>").description("Get contract code. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting code...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolvedAddress = await resolveAddressWithProvider(address, provider, {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getCode(resolvedAddress);
      spinner.succeed();
      if (address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${address})`);
      }
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get code: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("block-number").description("Get current block number").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting block number...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getBlockNumber();
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get block number: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("chain-id").description("Get chain ID").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting chain ID...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getChainId();
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get chain ID: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("tx-receipt <hash>").description("Get transaction receipt").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting transaction receipt...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getTxReceipt(hash);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transaction receipt: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("tx <hash>").description("Get transaction by hash").option("--raw", "Output raw JSON").action(async (hash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting transaction...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getTx(hash);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transaction: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("block <number>").description("Get block by number").option("--include-txs", "Include full transaction objects", false).option("--raw", "Output raw JSON").action(async (number, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getBlock(number, options.includeTxs);
      spinner.succeed();
      if (options.raw) {
        console.log(formatOutput(result, { raw: true }));
      } else {
        console.log(formatBlockInfo(result));
      }
    } catch (err) {
      error(`Failed to get block: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("call <to> <data>").description("Call contract function. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--from <address>", "Caller address (can be p2tr:0, p2wpkh:0, or raw address)").option("--block-tag <tag>", "Block tag (latest, pending, or number)").option("--raw", "Output raw JSON").action(async (to, data, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Calling contract...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedFrom = options.from;
      if (options.from) {
        resolvedFrom = await resolveAddressWithProvider(options.from, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.brc20prog.call(to, data, resolvedFrom, options.blockTag);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to call contract: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("estimate-gas <to> <data>").description("Estimate gas for transaction. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.").option("--from <address>", "Caller address (can be p2tr:0, p2wpkh:0, or raw address)").option("--raw", "Output raw JSON").action(async (to, data, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Estimating gas...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedFrom = options.from;
      if (options.from) {
        resolvedFrom = await resolveAddressWithProvider(options.from, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.brc20prog.estimateGas(to, data, resolvedFrom);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to estimate gas: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("wrap-btc <amount>").description("Wrap BTC to frBTC (simple wrap without execution)").option("--from <addresses...>", "Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)").option("--change <address>", "Change address (can be p2tr:0, p2wpkh:0, or raw address)").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Wrapping BTC to frBTC...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolverOpts = {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      };
      const resolvedFrom = options.from ? await resolveAddressesWithProvider(options.from, provider, resolverOpts) : void 0;
      const resolvedChange = options.change ? await resolveAddressWithProvider(options.change, provider, resolverOpts) : void 0;
      const params = {
        from_addresses: resolvedFrom,
        change_address: resolvedChange,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume,
        auto_confirm: true
      };
      const rawProvider = provider.rawProvider;
      const result = await rawProvider.frbtcWrap(BigInt(amount), JSON.stringify(params));
      spinner.succeed("BTC wrapped to frBTC successfully!");
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to wrap BTC: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("unwrap-btc <amount>").description("Unwrap frBTC to BTC (burns frBTC and queues BTC payment)").requiredOption("--to <address>", "Recipient address for the unwrapped BTC (can be p2tr:0, p2wpkh:0, or raw address)").option("--vout <index>", "Vout index for inscription output", parseInt, 0).option("--from <addresses...>", "Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)").option("--change <address>", "Change address (can be p2tr:0, p2wpkh:0, or raw address)").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Unwrapping frBTC to BTC...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolverOpts = {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      };
      const resolvedTo = await resolveAddressWithProvider(options.to, provider, resolverOpts);
      const resolvedFrom = options.from ? await resolveAddressesWithProvider(options.from, provider, resolverOpts) : void 0;
      const resolvedChange = options.change ? await resolveAddressWithProvider(options.change, provider, resolverOpts) : void 0;
      const params = {
        from_addresses: resolvedFrom,
        change_address: resolvedChange,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume,
        auto_confirm: true
      };
      const rawProvider = provider.rawProvider;
      const result = await rawProvider.frbtcUnwrap(
        BigInt(amount),
        BigInt(options.vout || 0),
        resolvedTo,
        JSON.stringify(params)
      );
      spinner.succeed("frBTC unwrap queued successfully!");
      console.log(formatOutput(result, { raw: options.raw }));
      success(`BTC will be sent to ${resolvedTo} by the subfrost operator`);
    } catch (err) {
      error(`Failed to unwrap frBTC: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("wrap-and-execute <amount>").description("Wrap BTC and deploy+execute a script (wrapAndExecute)").requiredOption("--script <bytecode>", "Script bytecode to deploy and execute (hex)").option("--from <addresses...>", "Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)").option("--change <address>", "Change address (can be p2tr:0, p2wpkh:0, or raw address)").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Wrapping BTC and executing script...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolverOpts = {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      };
      const resolvedFrom = options.from ? await resolveAddressesWithProvider(options.from, provider, resolverOpts) : void 0;
      const resolvedChange = options.change ? await resolveAddressWithProvider(options.change, provider, resolverOpts) : void 0;
      const params = {
        from_addresses: resolvedFrom,
        change_address: resolvedChange,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume,
        auto_confirm: true
      };
      const rawProvider = provider.rawProvider;
      const result = await rawProvider.frbtcWrapAndExecute(
        BigInt(amount),
        options.script,
        JSON.stringify(params)
      );
      spinner.succeed("BTC wrapped and script executed successfully!");
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to wrap and execute: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("wrap-and-execute2 <amount>").description("Wrap BTC and call an existing contract (wrapAndExecute2)").requiredOption("--target <address>", "Target contract address").requiredOption("--signature <sig>", 'Function signature (e.g., "deposit()")').option("--calldata <args>", "Comma-separated calldata arguments", "").option("--from <addresses...>", "Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)").option("--change <address>", "Change address (can be p2tr:0, p2wpkh:0, or raw address)").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Wrapping BTC and calling contract...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const resolverOpts = {
        walletFile: globalOpts.walletFile,
        passphrase: globalOpts.passphrase,
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      };
      const resolvedFrom = options.from ? await resolveAddressesWithProvider(options.from, provider, resolverOpts) : void 0;
      const resolvedChange = options.change ? await resolveAddressWithProvider(options.change, provider, resolverOpts) : void 0;
      const params = {
        from_addresses: resolvedFrom,
        change_address: resolvedChange,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume,
        auto_confirm: true
      };
      const rawProvider = provider.rawProvider;
      const result = await rawProvider.frbtcWrapAndExecute2(
        BigInt(amount),
        options.target,
        options.signature,
        options.calldata || "",
        JSON.stringify(params)
      );
      spinner.succeed("BTC wrapped and contract called successfully!");
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to wrap and execute2: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("signer-address").description("Get the FrBTC signer address for the current network").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting FrBTC signer address...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const rawProvider = provider.rawProvider;
      const signerAddress = await rawProvider.frbtcGetSignerAddress();
      spinner.succeed("FrBTC signer address retrieved!");
      if (options.raw) {
        console.log(formatOutput({ signer_address: signerAddress }, { raw: true }));
      } else {
        console.log(`FrBTC Signer Address`);
        console.log(`   Network: ${globalOpts.provider || "mainnet"}`);
        console.log(`   Signer Address: ${signerAddress}`);
      }
    } catch (err) {
      error(`Failed to get signer address: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/opi.ts
init_formatting();
var import_ora13 = __toESM(require("ora"));
function registerOpiCommands(program2) {
  const opi = program2.command("opi").description("Open Protocol Indexer operations");
  const DEFAULT_OPI_URL = "https://opi.alkanes.build";
  opi.command("block-height").description("Get current indexed block height").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting OPI block height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiBlockHeight(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get OPI block height: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("extras-block-height").description("Get extras indexed block height").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting OPI extras block height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiExtrasBlockHeight(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get OPI extras block height: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("db-version").description("Get database version").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting OPI database version...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiDbVersion(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get OPI database version: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("event-hash-version").description("Get event hash version").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting OPI event hash version...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiEventHashVersion(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get OPI event hash version: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("balance-on-block <block-height> <pkscript> <ticker>").description("Get balance on a specific block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, pkscript, ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting balance on block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiBalanceOnBlock(
        options.opiUrl,
        parseFloat(blockHeight),
        pkscript,
        ticker
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get balance on block: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("activity-on-block <block-height>").description("Get activity on a specific block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting activity on block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiActivityOnBlock(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get activity on block: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("bitcoin-rpc-results-on-block <block-height>").description("Get Bitcoin RPC results on a specific block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Bitcoin RPC results...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiBitcoinRpcResultsOnBlock(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get Bitcoin RPC results: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("current-balance <ticker>").description("Get current balance for a ticker").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address (can be p2tr:0, p2wpkh:0, or raw address)").option("--pkscript <pkscript>", "PK script").action(async (ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting current balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = options.address;
      if (options.address) {
        resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.opiCurrentBalance(
        options.opiUrl,
        ticker,
        resolvedAddress || null,
        options.pkscript || null
      );
      spinner.succeed();
      if (options.address && options.address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
      }
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get current balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("valid-tx-notes-of-wallet").description("Get valid transaction notes for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address (can be p2tr:0, p2wpkh:0, or raw address)").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting valid tx notes...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = options.address;
      if (options.address) {
        resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.opiValidTxNotesOfWallet(
        options.opiUrl,
        resolvedAddress || null,
        options.pkscript || null
      );
      spinner.succeed();
      if (options.address && options.address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
      }
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get valid tx notes: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("valid-tx-notes-of-ticker <ticker>").description("Get valid transaction notes for a ticker").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting valid tx notes for ticker...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiValidTxNotesOfTicker(
        options.opiUrl,
        ticker
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get valid tx notes: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("holders <ticker>").description("Get holders of a ticker").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting holders...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiHolders(options.opiUrl, ticker);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get holders: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("hash-of-all-activity <block-height>").description("Get hash of all activity on a block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting hash of all activity...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiHashOfAllActivity(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get hash of all activity: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("hash-of-all-current-balances").description("Get hash of all current balances").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting hash of all current balances...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiHashOfAllCurrentBalances(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get hash of all current balances: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("event <event-hash>").description("Get event details by hash").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (eventHash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting event details...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiEvent(options.opiUrl, eventHash);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get event: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("ip").description("Get OPI server IP address").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting OPI IP address...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiIp(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get OPI IP: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("raw <endpoint>").description("Make a raw request to an OPI endpoint").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (endpoint, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Making raw OPI request...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRaw(options.opiUrl, endpoint);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to make raw OPI request: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-block-height").description("Get Runes indexed block height").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Runes block height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesBlockHeight(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get Runes block height: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-balance-on-block <block-height> <pkscript> <rune-id>").description("Get Runes balance on a specific block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, pkscript, runeId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Runes balance on block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesBalanceOnBlock(
        options.opiUrl,
        parseFloat(blockHeight),
        pkscript,
        runeId
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get Runes balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-activity-on-block <block-height>").description("Get Runes activity on a specific block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Runes activity on block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesActivityOnBlock(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get Runes activity: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-current-balance").description("Get current Runes balance for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address (can be p2tr:0, p2wpkh:0, or raw address)").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting current Runes balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = options.address;
      if (options.address) {
        resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.opiRunesCurrentBalanceOfWallet(
        options.opiUrl,
        resolvedAddress || null,
        options.pkscript || null
      );
      spinner.succeed();
      if (options.address && options.address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
      }
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get Runes balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-unspent-outpoints").description("Get unspent Runes outpoints for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address (can be p2tr:0, p2wpkh:0, or raw address)").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting unspent Runes outpoints...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = options.address;
      if (options.address) {
        resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.opiRunesUnspentOutpointsOfWallet(
        options.opiUrl,
        resolvedAddress || null,
        options.pkscript || null
      );
      spinner.succeed();
      if (options.address && options.address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
      }
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get unspent outpoints: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-holders <rune-id>").description("Get holders of a Rune").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (runeId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Runes holders...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesHolders(options.opiUrl, runeId);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get Runes holders: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-hash-of-all-activity <block-height>").description("Get hash of all Runes activity on a block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Runes activity hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesHashOfAllActivity(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get Runes activity hash: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-event <event-hash>").description("Get Runes event details by hash").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (eventHash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Runes event details...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesEvent(options.opiUrl, eventHash);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get Runes event: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("bitmap-block-height").description("Get Bitmap indexed block height").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Bitmap block height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiBitmapBlockHeight(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get Bitmap block height: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("bitmap-hash-of-all-activity <block-height>").description("Get hash of all Bitmap activity on a block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Bitmap activity hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiBitmapHashOfAllActivity(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get Bitmap activity hash: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("bitmap-hash-of-all-bitmaps").description("Get hash of all registered Bitmaps").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting hash of all Bitmaps...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiBitmapHashOfAllBitmaps(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get Bitmaps hash: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("bitmap-inscription-id <bitmap-number>").description("Get inscription ID for a Bitmap number").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (bitmapNumber, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting Bitmap inscription ID...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiBitmapInscriptionId(
        options.opiUrl,
        parseFloat(bitmapNumber)
      );
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get Bitmap inscription ID: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-block-height").description("Get POW20 indexed block height").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting POW20 block height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20BlockHeight(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get POW20 block height: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-balance-on-block <block-height> <pkscript> <ticker>").description("Get POW20 balance on a specific block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, pkscript, ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting POW20 balance on block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20BalanceOnBlock(
        options.opiUrl,
        parseFloat(blockHeight),
        pkscript,
        ticker
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-activity-on-block <block-height>").description("Get POW20 activity on a specific block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting POW20 activity on block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20ActivityOnBlock(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 activity: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-current-balance").description("Get current POW20 balance for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address (can be p2tr:0, p2wpkh:0, or raw address)").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting current POW20 balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = options.address;
      if (options.address) {
        resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.opiPow20CurrentBalanceOfWallet(
        options.opiUrl,
        resolvedAddress || null,
        options.pkscript || null
      );
      spinner.succeed();
      if (options.address && options.address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
      }
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-valid-tx-notes-of-wallet").description("Get valid POW20 transaction notes for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address (can be p2tr:0, p2wpkh:0, or raw address)").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting valid POW20 tx notes...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      let resolvedAddress = options.address;
      if (options.address) {
        resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl
        });
      }
      const result = await provider.opiPow20ValidTxNotesOfWallet(
        options.opiUrl,
        resolvedAddress || null,
        options.pkscript || null
      );
      spinner.succeed();
      if (options.address && options.address !== resolvedAddress) {
        info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
      }
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 tx notes: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-valid-tx-notes-of-ticker <ticker>").description("Get valid POW20 transaction notes for a ticker").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting valid POW20 tx notes for ticker...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20ValidTxNotesOfTicker(
        options.opiUrl,
        ticker
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 tx notes: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-holders <ticker>").description("Get holders of a POW20 ticker").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting POW20 holders...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20Holders(options.opiUrl, ticker);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 holders: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-hash-of-all-activity <block-height>").description("Get hash of all POW20 activity on a block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting POW20 activity hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20HashOfAllActivity(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get POW20 activity hash: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-hash-of-all-current-balances").description("Get hash of all current POW20 balances").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting POW20 balances hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20HashOfAllCurrentBalances(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get POW20 balances hash: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-event <event-hash>").description("Get POW20 event details by hash").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (eventHash, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting POW20 event details...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20Event(options.opiUrl, eventHash);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 event: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("sns-block-height").description("Get SNS indexed block height").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting SNS block height...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiSnsBlockHeight(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get SNS block height: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("sns-hash-of-all-activity <block-height>").description("Get hash of all SNS activity on a block").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (blockHeight, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting SNS activity hash...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiSnsHashOfAllActivity(
        options.opiUrl,
        parseFloat(blockHeight)
      );
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get SNS activity hash: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("sns-hash-of-all-registered-names").description("Get hash of all registered SNS names").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting hash of all SNS names...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiSnsHashOfAllRegisteredNames(options.opiUrl);
      spinner.succeed();
      console.log(result);
    } catch (err) {
      error(`Failed to get SNS names hash: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("sns-info <domain>").description("Get SNS domain information").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (domain, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting SNS domain info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiSnsInfo(options.opiUrl, domain);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get SNS info: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("sns-inscriptions-of-domain <domain>").description("Get inscriptions for an SNS domain").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (domain, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting SNS domain inscriptions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiSnsInscriptionsOfDomain(options.opiUrl, domain);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get SNS inscriptions: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("sns-registered-namespaces").description("Get all registered SNS namespaces").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting SNS registered namespaces...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiSnsRegisteredNamespaces(options.opiUrl);
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get SNS namespaces: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/commands/subfrost.ts
init_formatting();
var import_ora14 = __toESM(require("ora"));
function registerSubfrostCommands(program2) {
  const subfrost = program2.command("subfrost").description("Subfrost operations (frBTC unwrap utilities)");
  subfrost.command("minimum-unwrap").description("Calculate minimum unwrap amount based on current fee rates").option("--fee-rate <rate>", "Fee rate override in sat/vB (otherwise fetches from network)").option("--premium <percent>", "Premium percentage (default: 0.1)", "0.1").option("--expected-inputs <n>", "Expected number of inputs", "10").option("--expected-outputs <n>", "Expected number of outputs", "10").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora14.default)("Calculating minimum unwrap amount...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const result = await provider.subfrostMinimumUnwrap(
        options.feeRate ? parseFloat(options.feeRate) : null,
        parseFloat(options.premium) / 100,
        // Convert percentage to decimal
        options.expectedInputs ? parseFloat(options.expectedInputs) : null,
        options.expectedOutputs ? parseFloat(options.expectedOutputs) : null,
        globalOpts.raw || false
      );
      spinner.succeed();
      if (globalOpts.raw) {
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } else {
        console.log(result);
      }
    } catch (err) {
      error(`Failed to calculate minimum unwrap: ${err.message}`);
      process.exit(1);
    }
  });
}

// src/cli/index.ts
var originalConsoleLog = console.log;
var originalConsoleInfo = console.info;
function setupLogging(verbose2) {
  setVerbosity(verbose2);
  console.log = (...args) => {
    const message = args.map((a) => String(a)).join(" ");
    if (message.includes("[INFO]") || message.includes("JsonRpcProvider::call") || message.includes("Raw RPC response")) {
      if (verbose2 >= 3) {
        originalConsoleLog.apply(console, [import_chalk4.default.dim(...args)]);
      }
      return;
    }
    originalConsoleLog.apply(console, args);
  };
  console.info = (...args) => {
    const message = args.map((a) => String(a)).join(" ");
    if (message.includes("[INFO]")) {
      if (verbose2 >= 2) {
        originalConsoleInfo.apply(console, [import_chalk4.default.dim(...args)]);
      }
      return;
    }
    originalConsoleInfo.apply(console, args);
  };
}
var program = new import_commander.Command();
program.name("alkanes-bindgen-cli").version("0.1.0").description("Alkanes Bindgen CLI - Bitcoin smart contracts (WASM/TypeScript version)").option("-p, --provider <network>", "Network: mainnet/testnet/signet/regtest", "mainnet").option("--wallet-file <path>", "Wallet file path", "~/.alkanes/wallet.json").option("--passphrase <password>", "Wallet passphrase").option("--jsonrpc-url <url>", "JSON-RPC URL").option("--esplora-url <url>", "Esplora API URL").option("--metashrew-url <url>", "Metashrew RPC URL").option("-v, --verbose", "Verbose output (-v, -vv, -vvv for increasing levels)", (_, prev) => prev + 1, 0).option("-y, --auto-confirm", "Skip confirmation prompts");
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
    const { decode_psbt } = await Promise.resolve().then(() => __toESM(require_alkanes_web_sys()));
    const globalOpts = command.parent?.opts() || {};
    const result = decode_psbt(psbt);
    const decoded = JSON.parse(result);
    const { formatOutput: formatOutput2 } = await Promise.resolve().then(() => (init_formatting(), formatting_exports));
    console.log(formatOutput2(decoded, globalOpts));
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
var preParseOpts = program.opts();
var verboseCount = process.argv.filter((arg) => arg === "-v" || arg === "--verbose").length + process.argv.filter((arg) => arg.match(/^-v+$/)).reduce((acc, arg) => acc + arg.length - 1, 0);
setupLogging(verboseCount);
program.parse(process.argv);
if (!process.argv.slice(2).length) {
  program.outputHelp();
}
