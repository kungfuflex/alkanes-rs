# macOS Build Instructions

## Prerequisites

Before building alkanes-rs on macOS, you need to install LLVM via Homebrew.

### Install Homebrew

If you don't have Homebrew installed, install it first:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

### Install LLVM

```bash
brew install llvm
```

## Building

Once you have LLVM installed, prefix all your build commands with the `./osx-build.sh` script:

```bash
./osx-build.sh cargo build
```

```bash
./osx-build.sh cargo test
```

```bash
./osx-build.sh cargo run
```

The `osx-build.sh` script configures the necessary environment variables to use the Homebrew-installed LLVM toolchain, ensuring the build system works correctly on macOS.
