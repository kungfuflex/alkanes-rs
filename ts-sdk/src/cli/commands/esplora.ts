/**
 * Esplora command group
 * Esplora REST API operations for blockchain data
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerEsploraCommands(program: Command): void {
  const esplora = program.command('esplora').description('Esplora REST API operations');

  // tx
  esplora
    .command('tx <txid>')
    .description('Get transaction by txid')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_tx_js(txid);
        const tx = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(tx, globalOpts));
      } catch (err: any) {
        error(`Failed to get transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-status
  esplora
    .command('tx-status <txid>')
    .description('Get transaction status')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction status...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_tx_status_js(txid);
        const status = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(status, globalOpts));
      } catch (err: any) {
        error(`Failed to get transaction status: ${err.message}`);
        process.exit(1);
      }
    });

  // address
  esplora
    .command('address <address>')
    .description('Get address information')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_address_info_js(address);
        const info = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(info, globalOpts));
      } catch (err: any) {
        error(`Failed to get address info: ${err.message}`);
        process.exit(1);
      }
    });

  // address-utxos
  esplora
    .command('address-utxos <address>')
    .description('Get UTXOs for an address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting UTXOs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_address_utxo_js(address);
        const utxos = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(utxos, globalOpts));
      } catch (err: any) {
        error(`Failed to get UTXOs: ${err.message}`);
        process.exit(1);
      }
    });

  // address-txs
  esplora
    .command('address-txs <address>')
    .description('Get transactions for an address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_address_txs_js(address);
        const txs = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txs, globalOpts));
      } catch (err: any) {
        error(`Failed to get transactions: ${err.message}`);
        process.exit(1);
      }
    });

  // address-txs-chain
  esplora
    .command('address-txs-chain <address>')
    .description('Get paginated transactions for an address')
    .option('--last-seen <txid>', 'Last seen txid for pagination')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_address_txs_chain_js(
          address,
          options.lastSeen || null
        );
        const txs = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txs, globalOpts));
      } catch (err: any) {
        error(`Failed to get transactions: ${err.message}`);
        process.exit(1);
      }
    });

  // blocks-tip-height
  esplora
    .command('blocks-tip-height')
    .description('Get current block tip height')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting tip height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_blocks_tip_height_js();
        const height = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(height, globalOpts));
      } catch (err: any) {
        error(`Failed to get tip height: ${err.message}`);
        process.exit(1);
      }
    });

  // blocks-tip-hash
  esplora
    .command('blocks-tip-hash')
    .description('Get current block tip hash')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting tip hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_blocks_tip_hash_js();
        const hash = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(hash, globalOpts));
      } catch (err: any) {
        error(`Failed to get tip hash: ${err.message}`);
        process.exit(1);
      }
    });

  // fee-estimates
  esplora
    .command('fee-estimates')
    .description('Get fee estimates')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting fee estimates...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_fee_estimates_js();
        const estimates = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(estimates, globalOpts));
      } catch (err: any) {
        error(`Failed to get fee estimates: ${err.message}`);
        process.exit(1);
      }
    });

  // broadcast-tx
  esplora
    .command('broadcast-tx <hex>')
    .description('Broadcast a transaction')
    .action(async (hex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Broadcasting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_broadcast_tx_js(hex);
        const txid = JSON.parse(result);

        spinner.succeed('Transaction broadcast');
        success(`TXID: ${txid}`);
      } catch (err: any) {
        error(`Failed to broadcast transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-hex
  esplora
    .command('tx-hex <txid>')
    .description('Get raw transaction hex')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction hex...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esplora_get_tx_hex_js(txid);
        const hex = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(hex, globalOpts));
      } catch (err: any) {
        error(`Failed to get transaction hex: ${err.message}`);
        process.exit(1);
      }
    });

  // === BLOCK OPERATIONS ===

  // blocks
  esplora
    .command('blocks [start-height]')
    .description('Get blocks starting from height')
    .action(async (startHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting blocks...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetBlocks(startHeight ? parseFloat(startHeight) : null);
        const blocks = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(blocks, globalOpts));
      } catch (err: any) {
        error(`Failed to get blocks: ${err.message}`);
        process.exit(1);
      }
    });

  // block-height
  esplora
    .command('block-height <height>')
    .description('Get block hash by height')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const hash = await provider.esploraGetBlockByHeight(parseFloat(height));

        spinner.succeed();
        console.log(formatOutput(hash, globalOpts));
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // block
  esplora
    .command('block <hash>')
    .description('Get block by hash')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetBlock(hash);
        const block = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(block, globalOpts));
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // block-status
  esplora
    .command('block-status <hash>')
    .description('Get block status')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block status...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetBlockStatus(hash);
        const status = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(status, globalOpts));
      } catch (err: any) {
        error(`Failed to get block status: ${err.message}`);
        process.exit(1);
      }
    });

  // block-txids
  esplora
    .command('block-txids <hash>')
    .description('Get transaction IDs in block')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block txids...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetBlockTxids(hash);
        const txids = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txids, globalOpts));
      } catch (err: any) {
        error(`Failed to get block txids: ${err.message}`);
        process.exit(1);
      }
    });

  // block-header
  esplora
    .command('block-header <hash>')
    .description('Get block header')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block header...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const header = await provider.esploraGetBlockHeader(hash);

        spinner.succeed();
        console.log(formatOutput(header, globalOpts));
      } catch (err: any) {
        error(`Failed to get block header: ${err.message}`);
        process.exit(1);
      }
    });

  // block-raw
  esplora
    .command('block-raw <hash>')
    .description('Get raw block data')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting raw block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const raw = await provider.esploraGetBlockRaw(hash);

        spinner.succeed();
        console.log(formatOutput(raw, globalOpts));
      } catch (err: any) {
        error(`Failed to get raw block: ${err.message}`);
        process.exit(1);
      }
    });

  // block-txid
  esplora
    .command('block-txid <hash> <index>')
    .description('Get transaction ID by block hash and index')
    .action(async (hash, index, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block txid...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txid = await provider.esploraGetBlockTxid(hash, parseFloat(index));

        spinner.succeed();
        console.log(formatOutput(txid, globalOpts));
      } catch (err: any) {
        error(`Failed to get block txid: ${err.message}`);
        process.exit(1);
      }
    });

  // block-txs
  esplora
    .command('block-txs <hash> [start-index]')
    .description('Get block transactions')
    .action(async (hash, startIndex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block txs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetBlockTxs(hash, startIndex ? parseFloat(startIndex) : null);
        const txs = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txs, globalOpts));
      } catch (err: any) {
        error(`Failed to get block txs: ${err.message}`);
        process.exit(1);
      }
    });

  // === ADDRESS OPERATIONS ===

  // address-txs-mempool
  esplora
    .command('address-txs-mempool <address>')
    .description('Get mempool transactions for address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetAddressTxsMempool(address);
        const txs = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txs, globalOpts));
      } catch (err: any) {
        error(`Failed to get mempool transactions: ${err.message}`);
        process.exit(1);
      }
    });

  // address-prefix
  esplora
    .command('address-prefix <prefix>')
    .description('Search addresses by prefix')
    .action(async (prefix, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Searching addresses...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetAddressPrefix(prefix);
        const addresses = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(addresses, globalOpts));
      } catch (err: any) {
        error(`Failed to search addresses: ${err.message}`);
        process.exit(1);
      }
    });

  // === TRANSACTION OPERATIONS ===

  // tx-raw
  esplora
    .command('tx-raw <txid>')
    .description('Get raw transaction')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting raw transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const raw = await provider.esploraGetTxRaw(txid);

        spinner.succeed();
        console.log(formatOutput(raw, globalOpts));
      } catch (err: any) {
        error(`Failed to get raw transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-merkle-proof
  esplora
    .command('tx-merkle-proof <txid>')
    .description('Get merkle proof for transaction')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting merkle proof...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetTxMerkleProof(txid);
        const proof = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(proof, globalOpts));
      } catch (err: any) {
        error(`Failed to get merkle proof: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-merkleblock-proof
  esplora
    .command('tx-merkleblock-proof <txid>')
    .description('Get merkle block proof')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting merkleblock proof...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const proof = await provider.esploraGetTxMerkleblockProof(txid);

        spinner.succeed();
        console.log(formatOutput(proof, globalOpts));
      } catch (err: any) {
        error(`Failed to get merkleblock proof: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-outspend
  esplora
    .command('tx-outspend <txid> <index>')
    .description('Get outspend for transaction output')
    .action(async (txid, index, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outspend...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetTxOutspend(txid, parseFloat(index));
        const outspend = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(outspend, globalOpts));
      } catch (err: any) {
        error(`Failed to get outspend: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-outspends
  esplora
    .command('tx-outspends <txid>')
    .description('Get all outspends for transaction')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outspends...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetTxOutspends(txid);
        const outspends = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(outspends, globalOpts));
      } catch (err: any) {
        error(`Failed to get outspends: ${err.message}`);
        process.exit(1);
      }
    });

  // === MEMPOOL OPERATIONS ===

  // mempool
  esplora
    .command('mempool')
    .description('Get mempool info')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetMempool();
        const mempool = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(mempool, globalOpts));
      } catch (err: any) {
        error(`Failed to get mempool info: ${err.message}`);
        process.exit(1);
      }
    });

  // mempool-txids
  esplora
    .command('mempool-txids')
    .description('Get mempool transaction IDs')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool txids...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetMempoolTxids();
        const txids = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txids, globalOpts));
      } catch (err: any) {
        error(`Failed to get mempool txids: ${err.message}`);
        process.exit(1);
      }
    });

  // mempool-recent
  esplora
    .command('mempool-recent')
    .description('Get recent mempool transactions')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting recent mempool txs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.esploraGetMempoolRecent();
        const txs = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txs, globalOpts));
      } catch (err: any) {
        error(`Failed to get recent mempool txs: ${err.message}`);
        process.exit(1);
      }
    });

  // post-tx
  esplora
    .command('post-tx <tx-hex>')
    .description('Post transaction (alternative to broadcast)')
    .action(async (txHex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Posting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txid = await provider.esploraPostTx(txHex);

        spinner.succeed('Transaction posted');
        success(`TXID: ${txid}`);
      } catch (err: any) {
        error(`Failed to post transaction: ${err.message}`);
        process.exit(1);
      }
    });
}
