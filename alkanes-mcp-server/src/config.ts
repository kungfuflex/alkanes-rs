/**
 * Configuration management for the MCP server
 */

import { ConfigurationError, ValidationError } from './error.js';
import { expandPath } from './utils.js';

export interface EnvironmentConfig {
  cli_path: string;
  provider: 'regtest' | 'signet' | 'mainnet';
  wallet_file?: string;
  passphrase?: string;
  jsonrpc_url?: string;
  data_api?: string;
  bitcoin_rpc_url?: string;
  esplora_api_url?: string;
  ord_server_url?: string;
  metashrew_rpc_url?: string;
  brc20_prog_rpc_url?: string;
  frbtc_address?: string;
  opi_url?: string;
  espo_rpc_url?: string;
  subfrost_api_key?: string;
  jsonrpc_headers?: string[];
  opi_headers?: string[];
  hd_path?: string;
  wallet_address?: string;
  wallet_key?: string;
  wallet_key_file?: string;
  titan_api_url?: string;
}

export interface McpServerConfig {
  environments: Record<string, EnvironmentConfig>;
  default_environment: string;
  timeout_seconds?: number;
}

/**
 * Resolve environment variable references in strings
 * Supports ${VAR_NAME} and $VAR_NAME syntax
 */
function resolveEnvVar(value: string): string {
  // Handle ${VAR_NAME} syntax
  value = value.replace(/\$\{([^}]+)\}/g, (_, varName) => {
    const envValue = process.env[varName];
    if (envValue === undefined) {
      throw new ConfigurationError(
        `Environment variable ${varName} is not set`
      );
    }
    return envValue;
  });

  // Handle $VAR_NAME syntax (but not ${VAR_NAME} which we already handled)
  value = value.replace(/\$([A-Z_][A-Z0-9_]*)/g, (_, varName) => {
    const envValue = process.env[varName];
    if (envValue === undefined) {
      throw new ConfigurationError(
        `Environment variable ${varName} is not set`
      );
    }
    return envValue;
  });

  return value;
}

/**
 * Expand and resolve configuration values
 */
function expandConfigValue(value: string | undefined): string | undefined {
  if (!value) return value;
  const expanded = expandPath(value);
  return resolveEnvVar(expanded);
}

/**
 * Validate environment configuration
 */
function validateEnvironmentConfig(
  name: string,
  config: EnvironmentConfig
): void {
  if (!config.cli_path) {
    throw new ValidationError(
      `Environment ${name}: cli_path is required`
    );
  }

  // Expand and validate cli_path
  const cliPath = expandConfigValue(config.cli_path);
  if (!cliPath) {
    throw new ValidationError(
      `Environment ${name}: cli_path cannot be empty`
    );
  }

  // Validate provider
  const validProviders = ['regtest', 'signet', 'mainnet'];
  if (!validProviders.includes(config.provider)) {
    throw new ValidationError(
      `Environment ${name}: provider must be one of ${validProviders.join(', ')}`
    );
  }

  // Validate URLs if provided
  const urlFields = [
    'jsonrpc_url',
    'data_api',
    'bitcoin_rpc_url',
    'esplora_api_url',
    'ord_server_url',
    'metashrew_rpc_url',
    'brc20_prog_rpc_url',
    'opi_url',
    'espo_rpc_url',
    'titan_api_url',
  ] as const;

  for (const field of urlFields) {
    const url = config[field];
    if (url && typeof url === 'string') {
      try {
        new URL(url);
      } catch {
        throw new ValidationError(
          `Environment ${name}: ${field} is not a valid URL: ${url}`
        );
      }
    }
  }

  // Validate wallet_file path if provided
  if (config.wallet_file) {
    const walletPath = expandConfigValue(config.wallet_file);
    if (walletPath && !walletPath.startsWith('~') && !walletPath.startsWith('/')) {
      // Relative paths are okay, but warn if they don't exist
      // We'll check existence at runtime
    }
  }
}

/**
 * Load configuration from environment variables
 * MCP clients serialize nested objects to JSON strings automatically
 */
export function loadConfigFromEnv(): McpServerConfig | null {
  // First, check for MCP_SERVER_CONFIG (single JSON string with full config)
  const mcpServerConfig = process.env.MCP_SERVER_CONFIG;
  if (mcpServerConfig) {
    try {
      const parsedConfig = JSON.parse(mcpServerConfig);
      return loadConfig(parsedConfig);
    } catch (error) {
      throw new ConfigurationError(
        `Failed to parse MCP_SERVER_CONFIG: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }

  // Fall back to individual environment variables
  const environmentsValue = process.env.environments;
  const defaultEnv = process.env.default_environment;
  const timeoutSeconds = process.env.timeout_seconds;

  if (!environmentsValue || !defaultEnv) {
    return null;
  }

  try {
    // Parse environments from JSON string (MCP clients serialize nested objects)
    const environments = JSON.parse(environmentsValue);

    const config: Record<string, unknown> = {
      environments,
      default_environment: defaultEnv,
    };

    if (timeoutSeconds) {
      const timeout = parseInt(timeoutSeconds, 10);
      if (!isNaN(timeout)) {
        config.timeout_seconds = timeout;
      }
    }

    return loadConfig(config);
  } catch (error) {
    throw new ConfigurationError(
      `Failed to parse environments from env var: ${error instanceof Error ? error.message : String(error)}`
    );
  }
}

/**
 * Load and validate MCP server configuration
 */
export function loadConfig(configJson: unknown): McpServerConfig {
  if (!configJson || typeof configJson !== 'object') {
    throw new ConfigurationError('Configuration must be an object');
  }

  const config = configJson as Record<string, unknown>;

  if (!config.environments || typeof config.environments !== 'object') {
    throw new ConfigurationError('Configuration must have an "environments" object');
  }

  const environments = config.environments as Record<string, unknown>;
  const defaultEnv = config.default_environment as string | undefined;

  if (!defaultEnv) {
    throw new ConfigurationError('Configuration must have a "default_environment"');
  }

  if (!environments[defaultEnv]) {
    throw new ConfigurationError(
      `Default environment "${defaultEnv}" not found in environments`
    );
  }

  // Validate all environments
  const validatedEnvironments: Record<string, EnvironmentConfig> = {};
  for (const [name, envConfig] of Object.entries(environments)) {
    if (!envConfig || typeof envConfig !== 'object') {
      throw new ValidationError(`Environment ${name}: must be an object`);
    }

    const expandedConfig: EnvironmentConfig = {
      ...(envConfig as EnvironmentConfig),
      cli_path: expandConfigValue((envConfig as EnvironmentConfig).cli_path) || '',
      wallet_file: expandConfigValue((envConfig as EnvironmentConfig).wallet_file),
      passphrase: expandConfigValue((envConfig as EnvironmentConfig).passphrase),
      jsonrpc_url: expandConfigValue((envConfig as EnvironmentConfig).jsonrpc_url),
      data_api: expandConfigValue((envConfig as EnvironmentConfig).data_api),
      bitcoin_rpc_url: expandConfigValue((envConfig as EnvironmentConfig).bitcoin_rpc_url),
      esplora_api_url: expandConfigValue((envConfig as EnvironmentConfig).esplora_api_url),
      ord_server_url: expandConfigValue((envConfig as EnvironmentConfig).ord_server_url),
      metashrew_rpc_url: expandConfigValue((envConfig as EnvironmentConfig).metashrew_rpc_url),
      brc20_prog_rpc_url: expandConfigValue((envConfig as EnvironmentConfig).brc20_prog_rpc_url),
      opi_url: expandConfigValue((envConfig as EnvironmentConfig).opi_url),
      espo_rpc_url: expandConfigValue((envConfig as EnvironmentConfig).espo_rpc_url),
      titan_api_url: expandConfigValue((envConfig as EnvironmentConfig).titan_api_url),
    };

    validateEnvironmentConfig(name, expandedConfig);
    validatedEnvironments[name] = expandedConfig;
  }

  return {
    environments: validatedEnvironments,
    default_environment: defaultEnv,
    timeout_seconds: (config.timeout_seconds as number | undefined) || 600,
  };
}

/**
 * Get environment configuration
 */
export function getEnvironmentConfig(
  config: McpServerConfig,
  environment?: string
): EnvironmentConfig {
  const envName = environment || config.default_environment;
  const envConfig = config.environments[envName];

  if (!envConfig) {
    throw new ConfigurationError(
      `Environment "${envName}" not found. Available: ${Object.keys(config.environments).join(', ')}`
    );
  }

  return envConfig;
}
