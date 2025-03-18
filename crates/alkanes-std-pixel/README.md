# Alkane Pixels

A pixel-based NFT implementation for the Alkanes protocol.

## Overview

Alkane Pixels is a proof-of-concept NFT implementation using the Alkanes protocol. Each NFT represents a unique pixel with different colors and attributes. Users can mint, transfer, and view their pixel NFTs.

## Features

- Mint unique pixel NFTs with custom colors and patterns
- Transfer pixels between addresses
- View pixel metadata and images
- Track pixel ownership
- Calculate rarity scores based on pixel attributes

## Usage

### Initialization

Initialize the contract with opcode 0:

```
oyl alkane execute -data '2, 14, 0' -p oylnet
```

### Minting a Pixel

Mint a new pixel with opcode 1, specifying RGB color values and pattern:

```
oyl alkane execute -data '2, 14, 1, 255, 0, 0, 1' -p oylnet
```

This mints a red pixel (R=255, G=0, B=0) with pattern 1.

### Transferring a Pixel

Transfer a pixel to another address with opcode 2:

```
oyl alkane execute -data '2, 14, 2, 1, "RECIPIENT_ADDRESS"' -p oylnet
```

This transfers pixel ID 1 to the specified recipient address.

### Viewing Pixel Metadata

Get pixel metadata with opcode 3:

```
oyl alkane execute -data '2, 14, 3, 1' -p oylnet
```

This returns the metadata for pixel ID 1.

### Viewing Pixel Image

Get the pixel image with opcode 4:

```
oyl alkane execute -data '2, 14, 4, 1' -p oylnet
```

This returns the image data for pixel ID 1.

### Viewing Owned Pixels

Get the pixels owned by an address with opcode 5:

```
oyl alkane execute -data '2, 14, 5, "ADDRESS"' -p oylnet
```

This returns the list of pixel IDs owned by the specified address. If no address is provided, it returns the pixels owned by the caller.

## Opcodes

| Opcode | Description | Parameters |
|--------|-------------|------------|
| 0 | Initialize the contract | None |
| 1 | Mint a new pixel | R (0-255), G (0-255), B (0-255), Pattern (0-255) |
| 2 | Transfer a pixel | Pixel ID, Recipient Address |
| 3 | Get pixel metadata | Pixel ID |
| 4 | Get pixel image | Pixel ID |
| 5 | Get pixels owned by an address | Address (optional) |
| 99 | Get token name | None |
| 100 | Get token symbol | None |
| 101 | Get total supply | None |

## Pixel Metadata

Each pixel has the following metadata:

- **ID**: Unique identifier for the pixel
- **Color**: RGB color values (3 bytes)
- **Pattern**: Pattern type (0-255)
- **Rarity**: Rarity score (0-100)

## Development

### Building

```
cargo build --target wasm32-unknown-unknown --release
```

### Testing

```
cargo test
```

## License

MIT