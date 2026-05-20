/**
 * Response formatting for MCP tools
 */

import type { ExecutionResult } from './executor.js';
import { parseJsonResponse } from './executor.js';

export interface FormattedResponse {
  content: Array<{
    type: 'text' | 'image' | 'resource';
    text?: string;
    data?: string;
    mimeType?: string;
  }>;
  isError?: boolean;
}

/**
 * Format CLI response for MCP tool response
 */
export function formatResponse(
  result: ExecutionResult,
  parseJson = true
): FormattedResponse {
  if (!result.success) {
    return {
      content: [
        {
          type: 'text',
          text: `Command failed with exit code ${result.exitCode}\n\nSTDOUT:\n${result.stdout}\n\nSTDERR:\n${result.stderr}`,
        },
      ],
      isError: true,
    };
  }

  // Try to parse as JSON if requested
  if (parseJson) {
    try {
      const json = parseJsonResponse(result.stdout);
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(json, null, 2),
          },
        ],
        isError: false,
      };
    } catch {
      // Not JSON, return as text
    }
  }

  return {
    content: [
      {
        type: 'text',
        text: result.stdout || result.stderr || 'Command executed successfully',
      },
    ],
    isError: false,
  };
}

/**
 * Format error response
 */
export function formatErrorResponse(error: unknown): FormattedResponse {
  const message = error instanceof Error ? error.message : String(error);
  const stack = error instanceof Error ? error.stack : undefined;

  return {
    content: [
      {
        type: 'text',
        text: stack ? `${message}\n\n${stack}` : message,
      },
    ],
    isError: true,
  };
}
