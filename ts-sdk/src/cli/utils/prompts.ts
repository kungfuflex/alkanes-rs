/**
 * User prompt utilities for the CLI
 */

import inquirer from 'inquirer';

/**
 * Prompt user for confirmation
 */
export async function confirm(message: string, defaultValue: boolean = false): Promise<boolean> {
  const { confirmed } = await inquirer.prompt([
    {
      type: 'confirm',
      name: 'confirmed',
      message,
      default: defaultValue,
    },
  ]);

  return confirmed;
}

/**
 * Prompt user for text input
 */
export async function input(message: string, defaultValue?: string): Promise<string> {
  const { value } = await inquirer.prompt([
    {
      type: 'input',
      name: 'value',
      message,
      default: defaultValue,
    },
  ]);

  return value;
}

/**
 * Prompt user for password (hidden input)
 */
export async function password(message: string): Promise<string> {
  const { value } = await inquirer.prompt([
    {
      type: 'password',
      name: 'value',
      message,
      mask: '*',
    },
  ]);

  return value;
}

/**
 * Prompt user to select from a list
 */
export async function select(message: string, choices: string[]): Promise<string> {
  const { value } = await inquirer.prompt([
    {
      type: 'list',
      name: 'value',
      message,
      choices,
    },
  ]);

  return value;
}

/**
 * Check if auto-confirm flag is set (skip prompts)
 */
export function shouldSkipPrompt(autoConfirm?: boolean): boolean {
  return autoConfirm === true;
}
