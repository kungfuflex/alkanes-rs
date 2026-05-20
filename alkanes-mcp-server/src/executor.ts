/**
 * CLI command executor
 */

import { spawn } from 'child_process';
import { readFile } from 'fs/promises';
import { ExecutionError, TimeoutError, mapCliError } from './error.js';
import type { EnvironmentConfig } from './config.js';
import { buildArgs } from './utils.js';

export interface ExecutionResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  success: boolean;
}

export interface ExecutionOptions {
  timeout?: number;
  cwd?: string;
  env?: Record<string, string>;
}

/**
 * Execute alkanes-cli command
 */
export async function executeCommand(
  config: EnvironmentConfig,
  command: string[],
  options: ExecutionOptions = {}
): Promise<ExecutionResult> {
  const timeout = options.timeout || 600000; // 10 minutes default
  const cwd = options.cwd || process.cwd();

  return new Promise((resolve, reject) => {
    const cliPath = config.cli_path;
    const args = buildBaseArgs(config).concat(command);

    let stdout = '';
    let stderr = '';
    let timeoutHandle: NodeJS.Timeout | null = null;

    const child = spawn(cliPath, args, {
      cwd,
      env: {
        ...process.env,
        ...options.env,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    child.stdout?.on('data', (data: Buffer) => {
      stdout += data.toString();
    });

    child.stderr?.on('data', (data: Buffer) => {
      stderr += data.toString();
    });

    child.on('error', (error) => {
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }
      reject(
        mapCliError(
          new ExecutionError(`Failed to execute command: ${error.message}`, { cliPath, args }),
          command.join(' ')
        )
      );
    });

    child.on('exit', (code) => {
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }

      const exitCode = code ?? 1;
      const success = exitCode === 0;

      resolve({
        stdout: stdout.trim(),
        stderr: stderr.trim(),
        exitCode,
        success,
      });
    });

    // Set timeout
    timeoutHandle = setTimeout(() => {
      child.kill('SIGTERM');
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }
      reject(
        new TimeoutError(
          `Command timed out after ${timeout}ms`,
          { command: command.join(' '), timeout }
        )
      );
    }, timeout);
  });
}

/**
 * Build base arguments from environment configuration
 */
function buildBaseArgs(config: EnvironmentConfig): string[] {
  const args: string[] = [];

  // Provider
  if (config.provider) {
    args.push('-p', config.provider);
  }

  // Wallet configuration
  if (config.wallet_file) {
    args.push('--wallet-file', config.wallet_file);
  }

  if (config.passphrase) {
    args.push('--passphrase', config.passphrase);
  }

  if (config.hd_path) {
    args.push('--hd-path', config.hd_path);
  }

  if (config.wallet_address) {
    args.push('--wallet-address', config.wallet_address);
  }

  if (config.wallet_key) {
    args.push('--wallet-key', config.wallet_key);
  }

  if (config.wallet_key_file) {
    args.push('--wallet-key-file', config.wallet_key_file);
  }

  // RPC URLs
  if (config.jsonrpc_url) {
    args.push('--jsonrpc-url', config.jsonrpc_url);
  }

  if (config.titan_api_url) {
    args.push('--titan-api-url', config.titan_api_url);
  }

  if (config.subfrost_api_key) {
    args.push('--subfrost-api-key', config.subfrost_api_key);
  }

  if (config.bitcoin_rpc_url) {
    args.push('--bitcoin-rpc-url', config.bitcoin_rpc_url);
  }

  if (config.esplora_api_url) {
    args.push('--esplora-api-url', config.esplora_api_url);
  }

  if (config.ord_server_url) {
    args.push('--ord-server-url', config.ord_server_url);
  }

  if (config.metashrew_rpc_url) {
    args.push('--metashrew-rpc-url', config.metashrew_rpc_url);
  }

  if (config.brc20_prog_rpc_url) {
    args.push('--brc20-prog-rpc-url', config.brc20_prog_rpc_url);
  }

  if (config.frbtc_address) {
    args.push('--frbtc-address', config.frbtc_address);
  }

  if (config.data_api) {
    args.push('--data-api', config.data_api);
  }

  if (config.opi_url) {
    args.push('--opi-url', config.opi_url);
  }

  if (config.espo_rpc_url) {
    args.push('--espo-rpc-url', config.espo_rpc_url);
  }

  // Headers
  if (config.jsonrpc_headers) {
    for (const header of config.jsonrpc_headers) {
      args.push('--jsonrpc-header', header);
    }
  }

  if (config.opi_headers) {
    for (const header of config.opi_headers) {
      args.push('--opi-header', header);
    }
  }

  return args;
}

/**
 * Parse JSON response from CLI
 */
export function parseJsonResponse<T = unknown>(stdout: string): T {
  try {
    return JSON.parse(stdout) as T;
  } catch (error) {
    throw new ExecutionError(
      `Failed to parse JSON response: ${error instanceof Error ? error.message : String(error)}`,
      { stdout }
    );
  }
}

/**
 * Execute command and parse JSON response
 */
export async function executeCommandJson<T = unknown>(
  config: EnvironmentConfig,
  command: string[],
  options: ExecutionOptions = {}
): Promise<T> {
  const result = await executeCommand(config, command, options);

  if (!result.success) {
    throw new ExecutionError(
      `Command failed with exit code ${result.exitCode}`,
      {
        command: command.join(' '),
        stdout: result.stdout,
        stderr: result.stderr,
        exitCode: result.exitCode,
      }
    );
  }

  // Try to parse JSON from stdout
  try {
    return parseJsonResponse<T>(result.stdout);
  } catch {
    // If not JSON, return the raw stdout as a string
    return result.stdout as unknown as T;
  }
}
