/**
 * Tool registry - exports all tools
 */

import { registerBitcoindTools } from './bitcoind.js';
import { registerWalletTools } from './wallet.js';
import { registerAlkanesTools } from './alkanes.js';
import { registerBrc20ProgTools } from './brc20_prog.js';
import { registerDataApiTools } from './dataapi.js';
import { registerOrdTools } from './ord.js';
import { registerOpiTools } from './opi.js';
import { registerEsploraTools } from './esplora.js';
import { registerMetashrewTools } from './metashrew.js';
import { registerLuaTools } from './lua.js';
import { registerProtorunesTools } from './protorunes.js';
import { registerRunestoneTools } from './runestone.js';
import { registerSubfrostTools } from './subfrost.js';
import { registerEspoTools } from './espo.js';

/**
 * Register all tools
 */
export function registerAllTools(): void {
  registerBitcoindTools();
  registerWalletTools();
  registerAlkanesTools();
  registerBrc20ProgTools();
  registerDataApiTools();
  registerOrdTools();
  registerOpiTools();
  registerEsploraTools();
  registerMetashrewTools();
  registerLuaTools();
  registerProtorunesTools();
  registerRunestoneTools();
  registerSubfrostTools();
  registerEspoTools();
}
