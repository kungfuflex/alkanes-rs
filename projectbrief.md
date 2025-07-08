# Alkanes GPU Shader Project Brief

## Project Overview
The alkanes-gpu-shader project aims to implement GPU-accelerated execution of alkanes contracts using SPIR-V compute shaders. This enables parallel processing of alkanes messages on GPU hardware for significant performance improvements.

## Core Requirements

### 1. SPIR-V Compatibility
- Must compile to SPIR-V compute shaders using rust-gpu
- No heap allocation (fixed-size arrays only)
- No standard library dependencies incompatible with SPIR-V
- Uses spirv-std for GPU-specific functionality

### 2. WASM Interpreter Compatibility
- Implement minimal WASM interpreter for alkanes contracts
- Must be 1-to-1 compatible with wasmi fuel metering
- Support required WASM imports/exports interface specific to alkanes
- Handle host function calls matching alkanes VM exactly

### 3. Build System
- Uses spirv-builder to compile Rust to SPIR-V
- Command: `cargo run -p spirv-build ../crates/alkanes-gpu-shader`
- Must work with nightly Rust toolchain (nightly-2023-09-30)

## Current Status
- Basic GPU shader structure exists in crates/alkanes-gpu-shader
- Custom WASM interpreter implementation started
- Build currently failing due to wasmi API incompatibilities
- Need to resolve dependency issues and complete WASM interpreter

## Success Criteria
- GPU shader compiles to SPIR-V successfully
- WASM interpreter handles alkanes contracts correctly
- Fuel metering matches canonical wasmi implementation
- Host functions work identically to alkanes VM
