/**
 * Runestone tools
 */

import { createPositionalTool } from './helpers.js';

export function registerRunestoneTools(): void {
  createPositionalTool(
    'runestone_analyze',
    'Analyze a runestone in a transaction',
    ['runestone', 'analyze'],
    ['txid'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'The transaction ID' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid'],
    }
  );

  createPositionalTool(
    'runestone_trace',
    'Trace all protostones in a runestone transaction',
    ['runestone', 'trace'],
    ['txid'],
    {
      type: 'object',
      properties: {
        txid: { type: 'string', description: 'The transaction ID' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['txid'],
    }
  );
}
