/**
 * Helper functions for tool registration
 */

import type { Tool } from '@modelcontextprotocol/sdk/types.js';
import type { EnvironmentConfig } from '../config.js';
import { executeCommandJson } from '../executor.js';
import { registerTool, type ToolHandler } from './registry.js';

/**
 * Create a simple tool that executes a CLI command
 */
export function createSimpleTool(
  name: string,
  description: string,
  command: string[],
  inputSchema: Tool['inputSchema'],
  buildArgs?: (args: Record<string, unknown>) => string[]
): void {
  const handler: ToolHandler = async (config, args) => {
    const cmd = [...command];
    
    // Add --raw if requested
    if (args?.raw) {
      cmd.push('--raw');
    }
    
    // Build additional arguments
    if (buildArgs) {
      const additionalArgs = buildArgs(args);
      cmd.push(...additionalArgs);
    } else {
      // Default: add all non-raw args as --key value
      for (const [key, value] of Object.entries(args || {})) {
        if (key === 'raw') continue;
        if (value !== undefined && value !== null) {
          if (typeof value === 'boolean' && value) {
            cmd.push(`--${key.replace(/_/g, '-')}`);
          } else if (typeof value === 'string' || typeof value === 'number') {
            cmd.push(`--${key.replace(/_/g, '-')}`, String(value));
          } else if (Array.isArray(value)) {
            for (const item of value) {
              cmd.push(`--${key.replace(/_/g, '-')}`, String(item));
            }
          }
        }
      }
    }
    
    return executeCommandJson(config, cmd);
  };
  
  registerTool({
    name,
    description,
    inputSchema,
    handler,
  });
}

/**
 * Create a tool with positional arguments
 */
export function createPositionalTool(
  name: string,
  description: string,
  command: string[],
  positionalArgs: string[],
  inputSchema: Tool['inputSchema'],
  buildFlags?: (args: Record<string, unknown>) => string[]
): void {
  const handler: ToolHandler = async (config, args) => {
    const cmd = [...command];
    
    // Add positional arguments in order
    for (const argName of positionalArgs) {
      const value = args?.[argName];
      if (value === undefined || value === null) {
        throw new Error(`Missing required argument: ${argName}`);
      }
      cmd.push(String(value));
    }
    
    // Add flags
    if (buildFlags) {
      const flags = buildFlags(args);
      cmd.push(...flags);
    } else {
      // Add --raw if requested
      if (args?.raw) {
        cmd.push('--raw');
      }
      
      // Add other boolean flags and options
      for (const [key, value] of Object.entries(args || {})) {
        if (positionalArgs.includes(key) || key === 'raw') continue;
        if (value === undefined || value === null) continue;
        
        if (typeof value === 'boolean' && value) {
          cmd.push(`--${key.replace(/_/g, '-')}`);
        } else if (typeof value === 'string' || typeof value === 'number') {
          cmd.push(`--${key.replace(/_/g, '-')}`, String(value));
        } else if (Array.isArray(value)) {
          for (const item of value) {
            cmd.push(`--${key.replace(/_/g, '-')}`, String(item));
          }
        }
      }
    }
    
    return executeCommandJson(config, cmd);
  };
  
  registerTool({
    name,
    description,
    inputSchema,
    handler,
  });
}
