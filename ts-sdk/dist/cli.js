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
      async simulate(contractId, contextJson, blockTag) {
        const result = await this.provider.alkanesSimulate(contractId, contextJson, blockTag);
        return mapToObject(result);
      }
      async execute(paramsJson) {
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
      async getStorageAt(block, tx, path2) {
        const provider = await this.getProvider();
        return provider.getStorageAt(BigInt(block), BigInt(tx), Array.from(path2));
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
    };
  }
});

// src/cli/index.ts
var import_commander = require("commander");
var import_chalk3 = __toESM(require("chalk"));
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
        const provider = await createProvider2({
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
  bitcoind.command("generatetoaddress <nblocks> <address>").description("Generate blocks to an address (regtest only)").option("--raw", "Output raw JSON").action(async (nblocks, address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)(`Generating ${nblocks} blocks...`).start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const hashes = await provider.bitcoin.generateToAddress(parseInt(nblocks), address);
      spinner.succeed(`Generated ${nblocks} blocks`);
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
  bitcoind.command("generatefuture <address>").description("Generate a future block (regtest only)").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora2.default)("Generating future block...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const hash = await provider.bitcoin.generateFuture(address);
      spinner.succeed("Future block generated");
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
  alkanes.command("balance").description("Get alkanes balance for an address").option("--address <address>", "Address to check (defaults to wallet)").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const balance = await provider.alkanes.getBalance(options.address);
      spinner.succeed();
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
  alkanes.command("simulate <contract-id>").description("Simulate alkanes execution").option("--inputs <json>", "Input parameters JSON").option("--context <json>", "Execution context JSON").option("--block-tag <tag>", "Block tag").option("--raw", "Output raw JSON").action(async (contractId, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Simulating execution...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const contextJson = options.context || JSON.stringify({
        inputs: options.inputs ? JSON.parse(options.inputs) : []
      });
      const result = await provider.alkanes.simulate(contractId, contextJson, options.blockTag);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to simulate: ${err.message}`);
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
  alkanes.command("by-address <address>").description("Get alkanes by address").option("--block-tag <tag>", "Block tag").option("--protocol-tag <tag>", "Protocol tag").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting alkanes by address...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getByAddress(
        address,
        options.blockTag,
        options.protocolTag ? parseInt(options.protocolTag) : void 0
      );
      spinner.succeed();
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
  alkanes.command("spendables <address>").description("Get spendable outpoints for an address").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora3.default)("Getting spendables...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl,
        metashrewUrl: globalOpts.metashrewUrl
      });
      const result = await provider.alkanes.getSpendables(address);
      spinner.succeed();
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
  esplora.command("address <address>").description("Get address information").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting address info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const info2 = await provider.esplora.getAddressInfo(address);
      spinner.succeed();
      console.log(formatOutput(info2, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get address info: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-utxos <address>").description("Get UTXOs for an address").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting UTXOs...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const utxos = await provider.esplora.getAddressUtxos(address);
      spinner.succeed();
      console.log(formatOutput(utxos, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get UTXOs: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs <address>").description("Get transactions for an address").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transactions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txs = await provider.esplora.getAddressTxs(address);
      spinner.succeed();
      console.log(formatOutput(txs, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get transactions: ${err.message}`);
      process.exit(1);
    }
  });
  esplora.command("address-txs-chain <address>").description("Get paginated transactions for an address").option("--last-seen <txid>", "Last seen txid for pagination").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora4.default)("Getting transactions...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        esploraUrl: globalOpts.esploraUrl
      });
      const txs = await provider.esplora.getAddressTxsChain(address, options.lastSeen);
      spinner.succeed();
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
  ord.command("outputs <address>").description("Get ordinal outputs for an address").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting outputs...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getOutputs(address);
      spinner.succeed();
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
  ord.command("address-info <address>").description("Get address information").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora5.default)("Getting address info...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.ord.getAddressInfo(address);
      spinner.succeed();
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
  protorunes.command("by-address <address>").description("Get protorunes by address").option("--block-tag <tag>", 'Block tag (e.g., "latest" or height)').action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora7.default)("Getting protorunes...").start();
      const provider = await createProvider2({
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
  dataapi.command("address-balances <address>").description("Get alkanes balances for address").option("--include-outpoints", "Include outpoint details", false).action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora10.default)("Getting balances...").start();
      const provider = await createProvider2({
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
      const provider = await createProvider2({
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
  espo.command("address-balances <address>").description("Get balances for an address").option("--include-outpoints", "Include outpoint details", false).action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora11.default)("Getting balances...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const balances = await provider.espo.getAddressBalances(
        address,
        options.includeOutpoints
      );
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
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const outpoints = await provider.espo.getAddressOutpoints(address);
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
      const path2 = await provider.espo.findBestSwapPath(
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
      console.log(formatOutput(path2, globalOpts));
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
  brc20Prog.command("balance <address>").description("Get balance for address").option("--block <tag>", "Block tag").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getBalance(address, options.block);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to get balance: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("code <address>").description("Get contract code").option("--raw", "Output raw JSON").action(async (address, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting code...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.getCode(address);
      spinner.succeed();
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
  brc20Prog.command("call <to> <data>").description("Call contract function").option("--from <address>", "Caller address").option("--block-tag <tag>", "Block tag (latest, pending, or number)").option("--raw", "Output raw JSON").action(async (to, data, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Calling contract...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.call(to, data, options.from, options.blockTag);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to call contract: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("estimate-gas <to> <data>").description("Estimate gas for transaction").option("--from <address>", "Caller address").option("--raw", "Output raw JSON").action(async (to, data, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Estimating gas...").start();
      const provider = await createProvider2({
        network: globalOpts.provider,
        jsonrpcUrl: globalOpts.jsonrpcUrl
      });
      const result = await provider.brc20prog.estimateGas(to, data, options.from);
      spinner.succeed();
      console.log(formatOutput(result, { raw: options.raw }));
    } catch (err) {
      error(`Failed to estimate gas: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("wrap-btc <amount>").description("Wrap BTC to frBTC (simple wrap without execution)").option("--from <addresses...>", "Addresses to source UTXOs from").option("--change <address>", "Change address").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Wrapping BTC to frBTC...").start();
      const { frbtc_wrap } = await import("../../wasm/alkanes_web_sys.js");
      const params = {
        from_addresses: options.from,
        change_address: options.change,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume
      };
      const result = await frbtc_wrap(
        globalOpts.provider || "mainnet",
        BigInt(amount),
        JSON.stringify(params)
      );
      spinner.succeed("BTC wrapped to frBTC successfully!");
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, { raw: options.raw }));
    } catch (err) {
      error(`Failed to wrap BTC: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("unwrap-btc <amount>").description("Unwrap frBTC to BTC (burns frBTC and queues BTC payment)").requiredOption("--to <address>", "Recipient address for the unwrapped BTC").option("--vout <index>", "Vout index for inscription output", parseInt, 0).option("--from <addresses...>", "Addresses to source UTXOs from").option("--change <address>", "Change address").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Unwrapping frBTC to BTC...").start();
      const { frbtc_unwrap } = await import("../../wasm/alkanes_web_sys.js");
      const params = {
        from_addresses: options.from,
        change_address: options.change,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume
      };
      const result = await frbtc_unwrap(
        globalOpts.provider || "mainnet",
        BigInt(amount),
        BigInt(options.vout || 0),
        options.to,
        JSON.stringify(params)
      );
      spinner.succeed("frBTC unwrap queued successfully!");
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, { raw: options.raw }));
      success(`BTC will be sent to ${options.to} by the subfrost operator`);
    } catch (err) {
      error(`Failed to unwrap frBTC: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("wrap-and-execute <amount>").description("Wrap BTC and deploy+execute a script (wrapAndExecute)").requiredOption("--script <bytecode>", "Script bytecode to deploy and execute (hex)").option("--from <addresses...>", "Addresses to source UTXOs from").option("--change <address>", "Change address").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Wrapping BTC and executing script...").start();
      const { frbtc_wrap_and_execute } = await import("../../wasm/alkanes_web_sys.js");
      const params = {
        from_addresses: options.from,
        change_address: options.change,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume
      };
      const result = await frbtc_wrap_and_execute(
        globalOpts.provider || "mainnet",
        BigInt(amount),
        options.script,
        JSON.stringify(params)
      );
      spinner.succeed("BTC wrapped and script executed!");
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, { raw: options.raw }));
    } catch (err) {
      error(`Failed to wrap and execute: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("wrap-and-execute2 <amount>").description("Wrap BTC and call an existing contract (wrapAndExecute2)").requiredOption("--target <address>", "Target contract address").requiredOption("--signature <sig>", 'Function signature (e.g., "deposit()")').option("--calldata <args>", "Comma-separated calldata arguments", "").option("--from <addresses...>", "Addresses to source UTXOs from").option("--change <address>", "Change address").option("--fee-rate <rate>", "Fee rate in sat/vB", parseFloat).option("--use-slipstream", "Use MARA Slipstream for broadcasting").option("--use-rebar", "Use Rebar Shield for private relay").option("--rebar-tier <tier>", "Rebar fee tier (1 or 2)", parseInt).option("--resume <txid>", "Resume from existing commit transaction").option("--raw", "Output raw JSON").action(async (amount, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Wrapping BTC and calling contract...").start();
      const { frbtc_wrap_and_execute2 } = await import("../../wasm/alkanes_web_sys.js");
      const params = {
        from_addresses: options.from,
        change_address: options.change,
        fee_rate: options.feeRate,
        use_slipstream: options.useSlipstream,
        use_rebar: options.useRebar,
        rebar_tier: options.rebarTier,
        resume_from_commit: options.resume
      };
      const result = await frbtc_wrap_and_execute2(
        globalOpts.provider || "mainnet",
        BigInt(amount),
        options.target,
        options.signature,
        options.calldata || "",
        JSON.stringify(params)
      );
      spinner.succeed("BTC wrapped and contract called!");
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, { raw: options.raw }));
    } catch (err) {
      error(`Failed to wrap and execute2: ${err.message}`);
      process.exit(1);
    }
  });
  brc20Prog.command("signer-address").description("Get the FrBTC signer address for the current network").option("--raw", "Output raw JSON").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora12.default)("Getting FrBTC signer address...").start();
      const { frbtc_get_signer_address } = await import("../../wasm/alkanes_web_sys.js");
      const result = await frbtc_get_signer_address(globalOpts.provider || "mainnet");
      spinner.succeed("FrBTC signer address retrieved!");
      const parsed = JSON.parse(result);
      if (options.raw) {
        console.log(formatOutput(parsed, { raw: true }));
      } else {
        console.log(`\u{1F511} FrBTC Signer Address`);
        console.log(`   Network: ${parsed.network}`);
        console.log(`   FrBTC Contract: ${parsed.frbtc_contract}`);
        console.log(`   Signer Address: ${parsed.signer_address}`);
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
  opi.command("current-balance <ticker>").description("Get current balance for a ticker").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address").option("--pkscript <pkscript>", "PK script").action(async (ticker, options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting current balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiCurrentBalance(
        options.opiUrl,
        ticker,
        options.address || null,
        options.pkscript || null
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get current balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("valid-tx-notes-of-wallet").description("Get valid transaction notes for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting valid tx notes...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiValidTxNotesOfWallet(
        options.opiUrl,
        options.address || null,
        options.pkscript || null
      );
      spinner.succeed();
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
  opi.command("runes-current-balance").description("Get current Runes balance for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting current Runes balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesCurrentBalanceOfWallet(
        options.opiUrl,
        options.address || null,
        options.pkscript || null
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get Runes balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("runes-unspent-outpoints").description("Get unspent Runes outpoints for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting unspent Runes outpoints...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiRunesUnspentOutpointsOfWallet(
        options.opiUrl,
        options.address || null,
        options.pkscript || null
      );
      spinner.succeed();
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
  opi.command("pow20-current-balance").description("Get current POW20 balance for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting current POW20 balance...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20CurrentBalanceOfWallet(
        options.opiUrl,
        options.address || null,
        options.pkscript || null
      );
      spinner.succeed();
      const parsed = JSON.parse(result);
      console.log(formatOutput(parsed, globalOpts));
    } catch (err) {
      error(`Failed to get POW20 balance: ${err.message}`);
      process.exit(1);
    }
  });
  opi.command("pow20-valid-tx-notes-of-wallet").description("Get valid POW20 transaction notes for a wallet").option("--opi-url <url>", "OPI base URL", DEFAULT_OPI_URL).option("--address <address>", "Wallet address").option("--pkscript <pkscript>", "PK script").action(async (options, command) => {
    try {
      const globalOpts = command.parent?.parent?.opts() || {};
      const spinner = (0, import_ora13.default)("Getting valid POW20 tx notes...").start();
      const provider = await createProvider2({
        network: globalOpts.provider
      });
      const result = await provider.opiPow20ValidTxNotesOfWallet(
        options.opiUrl,
        options.address || null,
        options.pkscript || null
      );
      spinner.succeed();
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
        originalConsoleLog.apply(console, [import_chalk3.default.dim(...args)]);
      }
      return;
    }
    originalConsoleLog.apply(console, args);
  };
  console.info = (...args) => {
    const message = args.map((a) => String(a)).join(" ");
    if (message.includes("[INFO]")) {
      if (verbose2 >= 2) {
        originalConsoleInfo.apply(console, [import_chalk3.default.dim(...args)]);
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
    const { decode_psbt } = await import("../wasm/alkanes_web_sys.js");
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
