/**
 * Lua script tools
 */

import { createSimpleTool } from './helpers.js';

export function registerLuaTools(): void {
  createSimpleTool(
    'lua_evalscript',
    'Execute a Lua script (tries cached hash first, falls back to full script)',
    ['lua', 'evalscript'],
    {
      type: 'object',
      properties: {
        script: { type: 'string', description: 'Path to Lua script file' },
        args: {
          type: 'array',
          items: { type: 'string' },
          description: 'Arguments to pass to the script',
        },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['script'],
    }
  );
}
