/**
 * Ord tools
 */

import { createSimpleTool, createPositionalTool } from './helpers.js';

export function registerOrdTools(): void {
  createPositionalTool(
    'ord_inscription',
    'Get inscription by ID',
    ['ord', 'inscription'],
    ['id'],
    {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'The inscription ID' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['id'],
    }
  );

  createPositionalTool(
    'ord_inscriptions_in_block',
    'Get inscriptions for a block',
    ['ord', 'inscriptions-in-block'],
    ['hash'],
    {
      type: 'object',
      properties: {
        hash: { type: 'string', description: 'The block hash' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['hash'],
    }
  );

  createPositionalTool(
    'ord_address_info',
    'Get address information',
    ['ord', 'address-info'],
    ['address'],
    {
      type: 'object',
      properties: {
        address: { type: 'string', description: 'The address' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['address'],
    }
  );

  createPositionalTool(
    'ord_block_info',
    'Get block information',
    ['ord', 'block-info'],
    ['query'],
    {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'The block hash or height' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['query'],
    }
  );

  createSimpleTool(
    'ord_block_count',
    'Get latest block count',
    ['ord', 'block-count'],
    {
      type: 'object',
      properties: {},
    }
  );

  createSimpleTool(
    'ord_blocks',
    'Get latest blocks',
    ['ord', 'blocks'],
    {
      type: 'object',
      properties: {
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
    }
  );

  createPositionalTool(
    'ord_children',
    'Get children of an inscription',
    ['ord', 'children'],
    ['id'],
    {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'The inscription ID' },
        page: { type: 'number', description: 'Page number' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['id'],
    }
  );

  createPositionalTool(
    'ord_content',
    'Get inscription content',
    ['ord', 'content'],
    ['id'],
    {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'The inscription ID' },
      },
      required: ['id'],
    }
  );

  createPositionalTool(
    'ord_output',
    'Get output information',
    ['ord', 'output'],
    ['outpoint'],
    {
      type: 'object',
      properties: {
        outpoint: { type: 'string', description: 'The outpoint' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['outpoint'],
    }
  );

  createPositionalTool(
    'ord_parents',
    'Get parents of an inscription',
    ['ord', 'parents'],
    ['id'],
    {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'The inscription ID' },
        page: { type: 'number', description: 'Page number' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['id'],
    }
  );

  createPositionalTool(
    'ord_rune',
    'Get rune information',
    ['ord', 'rune'],
    ['rune'],
    {
      type: 'object',
      properties: {
        rune: { type: 'string', description: 'The rune name or ID' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['rune'],
    }
  );

  createPositionalTool(
    'ord_sat',
    'Get sat information',
    ['ord', 'sat'],
    ['sat'],
    {
      type: 'object',
      properties: {
        sat: { type: 'number', description: 'The sat number' },
        raw: { type: 'boolean', description: 'Show raw JSON output' },
      },
      required: ['sat'],
    }
  );

  createPositionalTool(
    'ord_tx_info',
    'Get transaction information',
    ['ord', 'tx-info'],
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
