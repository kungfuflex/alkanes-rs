//! alkanes-llvm: WASM-to-SPIR-V compiler via LLVM
//!
//! Takes alkanes contract WASM bytecodes and compiles them to optimized
//! SPIR-V compute shaders using LLVM's optimization passes.
//!
//! Pipeline: WASM bytecode → LLVM IR → -O2 optimization → SPIR-V
//!
//! The compiled SPIR-V is fed directly to wgpu for GPU execution.

#[allow(unused_imports)]
pub mod compiler;
#[allow(unused_variables, unused_assignments)]
pub mod lowering;

pub mod wgsl_emit;
pub use compiler::WasmToSpirv;
