/**
 * Protorunes tools
 */

import { createPositionalTool } from './helpers.js';

export function registerProtorunesTools(): void {
  createPositionalTool(
    'protorunes_by_address',
    'Get protorunes by address',
    ['protorunes', 'by-address'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'Address to query' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        block_tag: { type: 'string', description: 'Block tag to query' },
        protocol_tag: { type: 'number', description: 'Protocol tag', default: 1 },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'protorunes_by_outpoint',
    'Get protorunes by outpoint',
    ['protorunes', 'by-outpoint'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'Outpoint to query' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
        block_tag: { type: 'string', description: 'Block tag to query' },
        protocol_tag: { type: 'number', description: 'Protocol tag', default: 1 },
      },
      required: ['outpoint'],
    }
  );
}
