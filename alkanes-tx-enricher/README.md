# Alkanes Transaction Enricher

A CLI tool to enrich Bitcoin transactions with protorune data from the ALKANES metaprotocol.

## Features

- Accepts raw transaction hex and block height
- Simulates the analysis of input outpoints with protorunes data at the block prior to confirmation
- Simulates the analysis of output outpoints with protorunes data at the confirmation block
- Decodes OP_RETURN outputs as Runestone/Protostone artifacts

## Installation

Make sure you have Rust and Cargo installed, then build the project:

```bash
cargo build --release
```

The binary will be available at `target/release/alkanes-tx-enricher`.

## Usage

```bash
# Basic usage
alkanes-tx-enricher --tx-hex <RAW_TX_HEX> --block-height <BLOCK_HEIGHT>

# With verbose output
alkanes-tx-enricher --tx-hex <RAW_TX_HEX> --block-height <BLOCK_HEIGHT> --verbose
```

### Example

```bash
alkanes-tx-enricher --tx-hex 02000000000101868b4d911d79a8f13612bb74df3f0b12766a5cf9b9ea30c04dd6050d18b2d95a1900000000ffffffff044a010000000000001600143369178e4af536e1dfd06ad8075048f59b7923fc0000000000000000506a5d4c4cff7f8194ec82d08bc0a886f5ea848890a4faed01ff7fc3d3af848891acfdc1e38688e5bbabc0b501ff7f89e182bceefbe8c7b6d9d7b6befbd9a88001ff7ffafddfdfdfdffffdfb9f8eefdf07b80b00000000000016001485fa2f8a4ebb17225ea647901effb3a90318e239e6f30700000000001600143369178e4af536e1dfd06ad8075048f59b7923fc02483045022100df6ca8750ce8d3af5e9458c341d1f388580f3965542e765e064e1c7c8c9ab41c022076201617dde7ae7de5fc9e8b3ec1198da703ee9f27c38c29238ed90d3d9ad0230121034015870024aac759c6089e4d2cc00e338608f4ff819f4433b42040e5a4cadce700000000 --block-height 880001
```

## Output Format

The tool provides output in the following sections:

### Transaction Inputs

For each input, the tool shows:
- Input index and outpoint (txid:vout)
- Indicates where it would query protorunes balances at the block height prior to confirmation

### Transaction Outputs

For each output, the tool shows:
- Output index, value in satoshis, and recipient address
- Indicates where it would query protorunes balances at the confirmation block height

### OP_RETURN Decoding

If the transaction contains an OP_RETURN output, the tool attempts to:
- Decode it as an Ordinals artifact
- If it's a Runestone, convert it to a Protostone
- Display the decoded data

## Dependencies

This tool relies on:
- Bitcoin and blockchain libraries (bitcoin, ordinals, metashrew-support, protorune-support)
- CLI libraries (clap)
- Serialization libraries (serde, serde_json)
- Utility libraries (anyhow, thiserror, log, env_logger)

## Environment Variables

You can set the following environment variables:
- `RUST_LOG`: Controls log level (e.g., `info`, `debug`, `trace`)

## License

MIT