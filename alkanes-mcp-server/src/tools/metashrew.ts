/**
 * Metashrew tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerMetashrewTools(): void {
  createSimpleTool(
    'metashrew_height',
    'Get the current block height',
    ['metashrew', 'height'],
    {
      type: 'object',
      properties: {},
    }
  );

  createPositionalTool(
    'metashrew_getblockhash',
    'Get the block hash for a given height',
    ['metashrew', 'getblockhash'],
    ['height'],
    {
      type: 'object',
      properties: {
        height: { type: 'number', description: 'The block height' },
      },
      required: ['height'],
    }
  );

  createSimpleTool(
    'metashrew_getstateroot',
    'Get the state root for a given height',
    ['metashrew', 'getstateroot'],
    {
      type: 'object',
      properties: {
        height: { type: 'string', description: 'The block height, or "latest"' },
      },
    }
  );
}
