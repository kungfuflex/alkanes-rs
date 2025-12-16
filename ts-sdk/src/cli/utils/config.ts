/**
 * Configuration management for the CLI
 * Handles loading configuration from file, environment variables, and defaults
 */

import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

export interface CLIConfig {
  network?: string;
  jsonrpcUrl?: string;
  esploraUrl?: string;
  metashrewUrl?: string;
  walletFile?: string;
  subfrostApiKey?: string;
}

const DEFAULT_CONFIG_PATH = path.join(os.homedir(), '.alkanes', 'config.json');

/**
 * Load configuration from file
 */
export async function loadConfigFile(configPath?: string): Promise<CLIConfig> {
  const filePath = configPath || DEFAULT_CONFIG_PATH;

  try {
    if (fs.existsSync(filePath)) {
      const content = fs.readFileSync(filePath, 'utf-8');
      return JSON.parse(content);
    }
  } catch (error) {
    // If config file doesn't exist or is invalid, return empty config
    console.warn(`Warning: Could not load config from ${filePath}`);
  }

  return {};
}

/**
 * Load configuration from environment variables
 */
export function loadConfigFromEnv(): CLIConfig {
  return {
    network: process.env.ALKANES_NETWORK,
    jsonrpcUrl: process.env.JSONRPC_URL || process.env.BITCOIN_RPC_URL,
    esploraUrl: process.env.ESPLORA_URL,
    metashrewUrl: process.env.METASHREW_URL || process.env.SANDSHREW_URL,
    walletFile: process.env.WALLET_FILE,
    subfrostApiKey: process.env.SUBFROST_API_KEY,
  };
}

/**
 * Get merged configuration (CLI args > env > config file > defaults)
 * This function should be called with CLI options to get the final config
 */
export async function getConfig(configPath?: string): Promise<CLIConfig> {
  // Load from config file
  const fileConfig = await loadConfigFile(configPath);

  // Load from environment
  const envConfig = loadConfigFromEnv();

  // Merge (env overrides file)
  const merged: CLIConfig = {
    ...fileConfig,
    ...Object.fromEntries(
      Object.entries(envConfig).filter(([_, v]) => v !== undefined)
    ),
  };

  return merged;
}

/**
 * Save configuration to file
 */
export async function saveConfig(config: CLIConfig, configPath?: string): Promise<void> {
  const filePath = configPath || DEFAULT_CONFIG_PATH;
  const dir = path.dirname(filePath);

  // Ensure directory exists
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  fs.writeFileSync(filePath, JSON.stringify(config, null, 2));
}

/**
 * Get default wallet file path
 */
export function getDefaultWalletPath(): string {
  return path.join(os.homedir(), '.alkanes', 'wallet.json');
}

/**
 * Expand ~ in file paths
 */
export function expandPath(filePath: string): string {
  if (filePath.startsWith('~')) {
    return path.join(os.homedir(), filePath.slice(1));
  }
  return filePath;
}
