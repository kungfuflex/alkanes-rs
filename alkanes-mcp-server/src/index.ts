#!/usr/bin/env node

/**
 * Alkanes CLI MCP Server
 * 
 * Exposes all alkanes-cli commands as MCP tools for AI agents
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  McpError,
  ErrorCode,
} from '@modelcontextprotocol/sdk/types.js';
import { loadConfig, loadConfigFromEnv, getEnvironmentConfig, type McpServerConfig } from './config.js';
import { registerAllTools } from './tools/mod.js';
import { getAllTools, executeTool } from './tools/registry.js';
import type { EnvironmentConfig } from './config.js';

// Load configuration from environment or MCP server config
let serverConfig: McpServerConfig;
let currentEnvironment: string | undefined;

try {
  // Try to load from environment variables (new format)
  // Supports both:
  // 1. New format: environments, default_environment, timeout_seconds as separate env vars
  // 2. Old format: MCP_SERVER_CONFIG as JSON string
  const loadedConfig = loadConfigFromEnv();
  if (loadedConfig) {
    serverConfig = loadedConfig;
  } else {
    // Default configuration - can be overridden via MCP server config
    serverConfig = {
      environments: {
        regtest: {
          cli_path: process.env.ALKANES_CLI_PATH || './target/release/alkanes-cli',
          provider: 'regtest',
        },
      },
      default_environment: 'regtest',
    };
  }
} catch (error) {
  console.error('Failed to load configuration:', error);
  process.exit(1);
}

// Register all tools first
registerAllTools();

// Create MCP server
const server = new Server(
  {
    name: 'alkanes-cli-mcp-server',
    version: '1.0.0',
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

// Get current environment configuration
function getCurrentConfig(): EnvironmentConfig {
  return getEnvironmentConfig(serverConfig, currentEnvironment);
}

// Handle list tools request
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: getAllTools(),
  };
});

// Handle tool calls
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  try {
    const config = getCurrentConfig();
    const result = await executeTool(
      request.params.name,
      config,
      request.params.arguments || {}
    );
    // MCP expects content array format
    return {
      content: result.content,
    };
  } catch (error) {
    if (error instanceof McpError) {
      throw error;
    }
    throw new McpError(
      ErrorCode.InternalError,
      `Error executing tool: ${error instanceof Error ? error.message : String(error)}`
    );
  }
});

// Note: Configuration updates would be handled via MCP notifications if needed
// For now, configuration is loaded from environment variables at startup

// Start server
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error('Alkanes CLI MCP Server running on stdio');
}

main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
