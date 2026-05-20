/**
 * Utility functions
 */

import { homedir } from 'os';
import { resolve } from 'path';

/**
 * Expand ~ in file paths
 */
export function expandPath(path: string): string {
  if (path.startsWith('~/')) {
    return resolve(homedir(), path.slice(2));
  }
  if (path === '~') {
    return homedir();
  }
  return path;
}

/**
 * Sanitize file path to prevent directory traversal
 */
export function sanitizePath(path: string): string {
  const expanded = expandPath(path);
  const resolved = resolve(expanded);
  
  // Prevent directory traversal
  if (resolved.includes('..')) {
    throw new Error(`Invalid path: ${path}`);
  }
  
  return resolved;
}

/**
 * Build command arguments array from options object
 */
export function buildArgs(options: Record<string, unknown>): string[] {
  const args: string[] = [];

  for (const [key, value] of Object.entries(options)) {
    if (value === undefined || value === null) {
      continue;
    }

    const flag = `--${key.replace(/_/g, '-')}`;

    if (typeof value === 'boolean') {
      if (value) {
        args.push(flag);
      }
    } else if (typeof value === 'string') {
      args.push(flag, value);
    } else if (typeof value === 'number') {
      args.push(flag, value.toString());
    } else if (Array.isArray(value)) {
      for (const item of value) {
        if (typeof item === 'string') {
          args.push(flag, item);
        }
      }
    } else {
      args.push(flag, JSON.stringify(value));
    }
  }

  return args;
}
