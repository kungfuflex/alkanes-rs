/**
 * Centralized tool registry
 */

import type { Tool } from '@modelcontextprotocol/sdk/types.js';
import { McpError, ErrorCode } from '@modelcontextprotocol/sdk/types.js';
import type { EnvironmentConfig } from '../config.js';
import { executeCommandJson } from '../executor.js';
import { formatErrorResponse, formatResponse } from '../response.js';

export type ToolHandler = (
  config: EnvironmentConfig,
  args: Record<string, unknown>
) => Promise<unknown>;

export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: Tool['inputSchema'];
  handler: ToolHandler;
}

const tools = new Map<string, ToolDefinition>();

export function registerTool(definition: ToolDefinition): void {
  tools.set(definition.name, definition);
}

export function getTool(name: string): ToolDefinition | undefined {
  return tools.get(name);
}

export function getAllTools(): Tool[] {
  return Array.from(tools.values()).map((def) => ({
    name: def.name,
    description: def.description,
    inputSchema: def.inputSchema,
  }));
}

export async function executeTool(
  name: string,
  config: EnvironmentConfig,
  args: Record<string, unknown>
): Promise<{ content: Array<{ type: string; text?: string }> }> {
  const tool = getTool(name);
  if (!tool) {
    throw new McpError(ErrorCode.MethodNotFound, `Tool not found: ${name}`);
  }

  try {
    const result = await tool.handler(config, args);
    return formatResponse(
      { stdout: JSON.stringify(result), stderr: '', exitCode: 0, success: true },
      true
    );
  } catch (error) {
    return formatErrorResponse(error);
  }
}
