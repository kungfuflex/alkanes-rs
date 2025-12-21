/**
 * Esplora command group
 * Esplora REST API operations for blockchain data
 *
 * The CLI uses the SDK's EsploraClient via provider.esplora for all operations.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import {
  formatOutput,
  formatFeeEstimates,
  formatBlockInfo,
  success,
  error,
} from '../utils/formatting.js';
import ora from 'ora';

export function registerEsploraCommands(program: Command): void {
  const esplora = program.command('esplora').description('Esplora REST API operations');

  // tx
  esplora
    .command('tx <txid>')
    .description('Get transaction by txid')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const tx = await provider.esplora.getTx(txid);

        spinner.succeed();
        console.log(formatOutput(tx, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-status
  esplora
    .command('tx-status <txid>')
    .description('Get transaction status')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction status...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const status = await provider.esplora.getTxStatus(txid);

        spinner.succeed();
        console.log(formatOutput(status, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get transaction status: ${err.message}`);
        process.exit(1);
      }
    });

  // address
  esplora
    .command('address <address>')
    .description('Get address information')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const info = await provider.esplora.getAddressInfo(address);

        spinner.succeed();
        console.log(formatOutput(info, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get address info: ${err.message}`);
        process.exit(1);
      }
    });

  // address-utxos
  esplora
    .command('address-utxos <address>')
    .description('Get UTXOs for an address')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting UTXOs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const utxos = await provider.esplora.getAddressUtxos(address);

        spinner.succeed();
        console.log(formatOutput(utxos, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get UTXOs: ${err.message}`);
        process.exit(1);
      }
    });

  // address-txs
  esplora
    .command('address-txs <address>')
    .description('Get transactions for an address')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txs = await provider.esplora.getAddressTxs(address);

        spinner.succeed();
        console.log(formatOutput(txs, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txs = await provider.esplora.getAddressTxsChain(address, options.lastSeen);

        spinner.succeed();
        console.log(formatOutput(txs, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get transactions: ${err.message}`);
        process.exit(1);
      }
    });

  // blocks-tip-height
  esplora
    .command('blocks-tip-height')
    .description('Get current block tip height')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting tip height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const height = await provider.esplora.getBlocksTipHeight();

        spinner.succeed();
        console.log(formatOutput(height, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get tip height: ${err.message}`);
        process.exit(1);
      }
    });

  // blocks-tip-hash
  esplora
    .command('blocks-tip-hash')
    .description('Get current block tip hash')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting tip hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const hash = await provider.esplora.getBlocksTipHash();

        spinner.succeed();
        console.log(formatOutput(hash, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get tip hash: ${err.message}`);
        process.exit(1);
      }
    });

  // fee-estimates
  esplora
    .command('fee-estimates')
    .description('Get fee estimates')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting fee estimates...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const estimates = await provider.esplora.getFeeEstimates();

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(estimates, { raw: true }));
        } else {
          console.log(formatFeeEstimates(estimates));
        }
      } catch (err: any) {
        error(`Failed to get fee estimates: ${err.message}`);
        process.exit(1);
      }
    });

  // broadcast-tx
  esplora
    .command('broadcast-tx <hex>')
    .description('Broadcast a transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (hex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Broadcasting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txid = await provider.esplora.broadcastTx(hex);

        spinner.succeed('Transaction broadcast');
        if (options.raw) {
          console.log(formatOutput({ txid }, { raw: true }));
        } else {
          success(`TXID: ${txid}`);
        }
      } catch (err: any) {
        error(`Failed to broadcast transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-hex
  esplora
    .command('tx-hex <txid>')
    .description('Get raw transaction hex')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction hex...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const hex = await provider.esplora.getTxHex(txid);

        spinner.succeed();
        console.log(formatOutput(hex, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (startHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting blocks...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const blocks = await provider.esplora.getBlocks(startHeight ? parseInt(startHeight) : undefined);

        spinner.succeed();
        console.log(formatOutput(blocks, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get blocks: ${err.message}`);
        process.exit(1);
      }
    });

  // block-height
  esplora
    .command('block-height <height>')
    .description('Get block hash by height')
    .option('--raw', 'Output raw JSON')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const hash = await provider.esplora.getBlockByHeight(parseInt(height));

        spinner.succeed();
        console.log(formatOutput(hash, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // block
  esplora
    .command('block <hash>')
    .description('Get block by hash')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const block = await provider.esplora.getBlock(hash);

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(block, { raw: true }));
        } else {
          console.log(formatBlockInfo(block));
        }
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // block-status
  esplora
    .command('block-status <hash>')
    .description('Get block status')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block status...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const status = await provider.esplora.getBlockStatus(hash);

        spinner.succeed();
        console.log(formatOutput(status, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block status: ${err.message}`);
        process.exit(1);
      }
    });

  // block-txids
  esplora
    .command('block-txids <hash>')
    .description('Get transaction IDs in block')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block txids...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txids = await provider.esplora.getBlockTxids(hash);

        spinner.succeed();
        console.log(formatOutput(txids, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block txids: ${err.message}`);
        process.exit(1);
      }
    });

  // block-header
  esplora
    .command('block-header <hash>')
    .description('Get block header')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block header...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const header = await provider.esplora.getBlockHeader(hash);

        spinner.succeed();
        console.log(formatOutput(header, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block header: ${err.message}`);
        process.exit(1);
      }
    });

  // block-raw
  esplora
    .command('block-raw <hash>')
    .description('Get raw block data')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting raw block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const raw = await provider.esplora.getBlockRaw(hash);

        spinner.succeed();
        console.log(formatOutput(raw, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get raw block: ${err.message}`);
        process.exit(1);
      }
    });

  // block-txid
  esplora
    .command('block-txid <hash> <index>')
    .description('Get transaction ID by block hash and index')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, index, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block txid...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txid = await provider.esplora.getBlockTxid(hash, parseInt(index));

        spinner.succeed();
        console.log(formatOutput(txid, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block txid: ${err.message}`);
        process.exit(1);
      }
    });

  // block-txs
  esplora
    .command('block-txs <hash> [start-index]')
    .description('Get block transactions')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, startIndex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block txs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txs = await provider.esplora.getBlockTxs(hash, startIndex ? parseInt(startIndex) : undefined);

        spinner.succeed();
        console.log(formatOutput(txs, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txs = await provider.esplora.getAddressTxsMempool(address);

        spinner.succeed();
        console.log(formatOutput(txs, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get mempool transactions: ${err.message}`);
        process.exit(1);
      }
    });

  // address-prefix
  esplora
    .command('address-prefix <prefix>')
    .description('Search addresses by prefix')
    .option('--raw', 'Output raw JSON')
    .action(async (prefix, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Searching addresses...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const addresses = await provider.esplora.getAddressPrefix(prefix);

        spinner.succeed();
        console.log(formatOutput(addresses, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting raw transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const raw = await provider.esplora.getTxRaw(txid);

        spinner.succeed();
        console.log(formatOutput(raw, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get raw transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-merkle-proof
  esplora
    .command('tx-merkle-proof <txid>')
    .description('Get merkle proof for transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting merkle proof...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const proof = await provider.esplora.getTxMerkleProof(txid);

        spinner.succeed();
        console.log(formatOutput(proof, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get merkle proof: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-merkleblock-proof
  esplora
    .command('tx-merkleblock-proof <txid>')
    .description('Get merkle block proof')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting merkleblock proof...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const proof = await provider.esplora.getTxMerkleblockProof(txid);

        spinner.succeed();
        console.log(formatOutput(proof, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get merkleblock proof: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-outspend
  esplora
    .command('tx-outspend <txid> <index>')
    .description('Get outspend for transaction output')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, index, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outspend...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const outspend = await provider.esplora.getTxOutspend(txid, parseInt(index));

        spinner.succeed();
        console.log(formatOutput(outspend, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get outspend: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-outspends
  esplora
    .command('tx-outspends <txid>')
    .description('Get all outspends for transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outspends...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const outspends = await provider.esplora.getTxOutspends(txid);

        spinner.succeed();
        console.log(formatOutput(outspends, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const mempool = await provider.esplora.getMempool();

        spinner.succeed();
        console.log(formatOutput(mempool, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get mempool info: ${err.message}`);
        process.exit(1);
      }
    });

  // mempool-txids
  esplora
    .command('mempool-txids')
    .description('Get mempool transaction IDs')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool txids...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txids = await provider.esplora.getMempoolTxids();

        spinner.succeed();
        console.log(formatOutput(txids, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get mempool txids: ${err.message}`);
        process.exit(1);
      }
    });

  // mempool-recent
  esplora
    .command('mempool-recent')
    .description('Get recent mempool transactions')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting recent mempool txs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txs = await provider.esplora.getMempoolRecent();

        spinner.succeed();
        console.log(formatOutput(txs, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get recent mempool txs: ${err.message}`);
        process.exit(1);
      }
    });

  // post-tx (alias for broadcast-tx)
  esplora
    .command('post-tx <tx-hex>')
    .description('Post transaction (alternative to broadcast)')
    .option('--raw', 'Output raw JSON')
    .action(async (txHex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Posting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const txid = await provider.esplora.broadcastTx(txHex);

        spinner.succeed('Transaction posted');
        if (options.raw) {
          console.log(formatOutput({ txid }, { raw: true }));
        } else {
          success(`TXID: ${txid}`);
        }
      } catch (err: any) {
        error(`Failed to post transaction: ${err.message}`);
        process.exit(1);
      }
    });
}
