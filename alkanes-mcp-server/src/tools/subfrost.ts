/**
 * Subfrost tools
 */

import { createSimpleTool } from './helpers.js';

export function registerSubfrostTools(): void {
  createSimpleTool(
    'subfrost_minimum_unwrap',
    'Calculate minimum unwrap amount based on current fee rates',
    ['subfrost', 'minimum-unwrap'],
    {
      type: 'object',
      properties: {
        fee_rate: { type: 'number', description: 'Override fee rate in sat/vB' },
        premium: { type: 'number', description: 'Premium percentage charged by subfrost', default: 0.001 },
        expected_inputs: { type: 'number', description: 'Expected number of inputs', default: 10 },
        expected_outputs: { type: 'number', description: 'Expected number of outputs', default: 10 },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );
}
