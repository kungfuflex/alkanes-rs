/**
 * Error handling for the MCP server
 */

export class AlkanesMcpError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly details?: unknown
  ) {
    super(message);
    this.name = 'AlkanesMcpError';
  }
}

export class ConfigurationError extends AlkanesMcpError {
  constructor(message: string, details?: unknown) {
    super(message, 'CONFIG_ERROR', details);
    this.name = 'ConfigurationError';
  }
}

export class ExecutionError extends AlkanesMcpError {
  constructor(message: string, details?: unknown) {
    super(message, 'EXECUTION_ERROR', details);
    this.name = 'ExecutionError';
  }
}

export class TimeoutError extends AlkanesMcpError {
  constructor(message: string, details?: unknown) {
    super(message, 'TIMEOUT_ERROR', details);
    this.name = 'TimeoutError';
  }
}

export class ValidationError extends AlkanesMcpError {
  constructor(message: string, details?: unknown) {
    super(message, 'VALIDATION_ERROR', details);
    this.name = 'ValidationError';
  }
}

/**
 * Map CLI exit codes and errors to MCP error codes
 */
export function mapCliError(
  error: unknown,
  command: string
): AlkanesMcpError {
  if (error instanceof AlkanesMcpError) {
    return error;
  }

  const errorMessage = error instanceof Error ? error.message : String(error);

  // Check for common error patterns
  if (errorMessage.includes('timeout') || errorMessage.includes('timed out')) {
    return new TimeoutError(
      `Command timed out: ${command}`,
      { command, originalError: errorMessage }
    );
  }

  if (errorMessage.includes('not found') || errorMessage.includes('No such file')) {
    return new ConfigurationError(
      `CLI binary or file not found: ${errorMessage}`,
      { command, originalError: errorMessage }
    );
  }

  if (errorMessage.includes('permission denied')) {
    return new ExecutionError(
      `Permission denied: ${errorMessage}`,
      { command, originalError: errorMessage }
    );
  }

  // Default to execution error
  return new ExecutionError(
    `Command failed: ${command}`,
    { command, originalError: errorMessage }
  );
}
