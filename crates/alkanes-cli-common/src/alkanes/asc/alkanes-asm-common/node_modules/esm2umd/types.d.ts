/**
 * Transforms the given ESM code to UMD. Uses `moduleName` as the name of the
 * vanilla JS global.
 */
export function esm2umd(
  moduleName: string,
  esmCode: string,
  options?: Record<string, any>,
): string;
