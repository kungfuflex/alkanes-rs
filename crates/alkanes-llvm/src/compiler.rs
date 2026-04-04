//! Compiler pipeline: WASM bytecode → LLVM IR → optimized IR → WGSL.

use anyhow::{anyhow, Result};
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{InitializationConfig, Target, TargetMachine, TargetTriple, CodeModel, RelocMode};
use inkwell::types::BasicTypeEnum;
use inkwell::values::FunctionValue;
use inkwell::OptimizationLevel;

use crate::wgsl_emit::WgslEmitter;

/// Compiles WASM bytecode to WGSL via LLVM IR.
pub struct WasmToWgslCompiler {
    /// Optimization level (default: O2)
    opt_level: OptimizationLevel,
}

impl WasmToWgslCompiler {
    pub fn new() -> Self {
        Self {
            opt_level: OptimizationLevel::Default, // -O2
        }
    }

    /// Compile WASM bytecode to WGSL shader code.
    pub fn compile(&self, wasm_bytes: &[u8]) -> Result<String> {
        let context = Context::create();
        let module = self.wasm_to_llvm_ir(&context, wasm_bytes)?;

        // Run optimization passes
        self.optimize(&module)?;

        // Emit WGSL
        let mut emitter = WgslEmitter::new(&module);
        emitter.emit()
    }

    /// Translate WASM bytecode to LLVM IR.
    fn wasm_to_llvm_ir<'ctx>(
        &self,
        context: &'ctx Context,
        wasm_bytes: &[u8],
    ) -> Result<Module<'ctx>> {
        let module = context.create_module("alkanes_wgsl");
        let builder = context.create_builder();

        // Parse WASM and translate each function
        let parser = wasmparser::Parser::new(0);
        let mut func_types: Vec<wasmparser::FuncType> = Vec::new();
        let mut func_type_indices: Vec<u32> = Vec::new();
        let mut code_section_entries: Vec<wasmparser::FunctionBody<'_>> = Vec::new();

        for payload in parser.parse_all(wasm_bytes) {
            let payload = payload.map_err(|e| anyhow!("wasm parse error: {}", e))?;
            match payload {
                wasmparser::Payload::TypeSection(reader) => {
                    for rec_group in reader {
                        let rec_group = rec_group.map_err(|e| anyhow!("type section error: {}", e))?;
                        for sub_type in rec_group.into_types() {
                            if let wasmparser::CompositeInnerType::Func(ft) = sub_type.composite_type.inner {
                                func_types.push(ft);
                            }
                        }
                    }
                }
                wasmparser::Payload::FunctionSection(reader) => {
                    for func in reader {
                        let type_idx = func.map_err(|e| anyhow!("func section error: {}", e))?;
                        func_type_indices.push(type_idx);
                    }
                }
                wasmparser::Payload::CodeSectionEntry(body) => {
                    code_section_entries.push(body);
                }
                _ => {}
            }
        }

        // Create LLVM functions for each WASM function
        let i32_type = context.i32_type();
        let i64_type = context.i64_type();
        let ptr_type = context.ptr_type(inkwell::AddressSpace::default());

        for (idx, body) in code_section_entries.iter().enumerate() {
            let type_idx = func_type_indices.get(idx).copied().unwrap_or(0) as usize;
            let func_type = func_types
                .get(type_idx)
                .ok_or_else(|| anyhow!("missing type for function {}", idx))?;

            // Build LLVM function type
            let mut param_types: Vec<BasicTypeEnum> = Vec::new();
            // First two params are always ptr (memory base) in our convention
            param_types.push(ptr_type.into());
            param_types.push(ptr_type.into());

            for param in func_type.params() {
                match param {
                    wasmparser::ValType::I32 => param_types.push(i32_type.into()),
                    wasmparser::ValType::I64 => param_types.push(i64_type.into()),
                    _ => param_types.push(i32_type.into()),
                }
            }

            let param_types_ref: Vec<inkwell::types::BasicMetadataTypeEnum> = param_types
                .iter()
                .map(|t| (*t).into())
                .collect();

            let ret_type = func_type.results().first();
            let fn_type = match ret_type {
                Some(wasmparser::ValType::I32) => i32_type.fn_type(&param_types_ref, false),
                Some(wasmparser::ValType::I64) => i64_type.fn_type(&param_types_ref, false),
                None => context.void_type().fn_type(&param_types_ref, false),
                _ => i32_type.fn_type(&param_types_ref, false),
            };

            let func_name = format!("__wasm_func_{}", idx);
            let function = module.add_function(&func_name, fn_type, None);

            // Create entry basic block
            let entry = context.append_basic_block(function, "entry");
            builder.position_at_end(entry);

            // Translate WASM instructions to LLVM IR
            // For now, implement a basic stack machine translator
            self.translate_wasm_body(context, &builder, &module, &function, body, func_type)?;
        }

        Ok(module)
    }

    /// Translate a single WASM function body to LLVM IR.
    fn translate_wasm_body<'ctx>(
        &self,
        context: &'ctx Context,
        builder: &inkwell::builder::Builder<'ctx>,
        module: &Module<'ctx>,
        function: &FunctionValue<'ctx>,
        body: &wasmparser::FunctionBody<'_>,
        func_type: &wasmparser::FuncType,
    ) -> Result<()> {
        let i32_type = context.i32_type();
        let i64_type = context.i64_type();

        // Value stack for the WASM stack machine
        let mut stack: Vec<inkwell::values::BasicValueEnum<'ctx>> = Vec::new();

        // Local variables (params + locals)
        let mut locals: Vec<inkwell::values::PointerValue<'ctx>> = Vec::new();

        // Create allocas for all params
        let entry_bb = function.get_first_basic_block().unwrap();
        builder.position_at_end(entry_bb);

        let num_params = function.count_params();
        for i in 0..num_params {
            let param = function.get_nth_param(i).unwrap();
            let alloca = builder.build_alloca(param.get_type(), &format!("local_{}", i))
                .map_err(|e| anyhow!("alloca error: {}", e))?;
            builder.build_store(alloca, param)
                .map_err(|e| anyhow!("store error: {}", e))?;
            locals.push(alloca);
        }

        // Create allocas for function-body locals
        let mut local_reader = body.get_locals_reader()
            .map_err(|e| anyhow!("locals reader error: {}", e))?;
        for _ in 0..local_reader.get_count() {
            let (count, val_type) = local_reader.read()
                .map_err(|e| anyhow!("local read error: {}", e))?;
            let llvm_type: BasicTypeEnum = match val_type {
                wasmparser::ValType::I32 => i32_type.into(),
                wasmparser::ValType::I64 => i64_type.into(),
                _ => i32_type.into(),
            };
            for _ in 0..count {
                let idx = locals.len();
                let alloca = builder
                    .build_alloca(llvm_type, &format!("local_{}", idx))
                    .map_err(|e| anyhow!("alloca error: {}", e))?;
                // Initialize to zero
                let zero = match val_type {
                    wasmparser::ValType::I64 => context.i64_type().const_zero().into(),
                    _ => context.i32_type().const_zero().into(),
                };
                builder.build_store(alloca, zero)
                    .map_err(|e| anyhow!("store error: {}", e))?;
                locals.push(alloca);
            }
        }

        // Now translate operators
        let mut ops_reader = body.get_operators_reader()
            .map_err(|e| anyhow!("ops reader error: {}", e))?;

        while !ops_reader.eof() {
            let op = ops_reader.read()
                .map_err(|e| anyhow!("op read error: {}", e))?;

            match op {
                wasmparser::Operator::LocalGet { local_index } => {
                    let local = locals.get(local_index as usize)
                        .ok_or_else(|| anyhow!("local {} out of range", local_index))?;
                    let val = builder.build_load(
                        builder.build_load(i32_type, *local, "")
                            .map_err(|e| anyhow!("load type error: {}", e))?
                            .get_type(),
                        *local,
                        &format!("get_{}", local_index),
                    ).map_err(|e| anyhow!("load error: {}", e))?;
                    // Determine the type from the alloca
                    let loaded = builder.build_load(
                        if local_index < function.count_params() {
                            function.get_nth_param(local_index).unwrap().get_type()
                        } else {
                            i32_type.into() // default for body locals
                        },
                        *local,
                        &format!("get_{}", local_index),
                    ).map_err(|e| anyhow!("load error: {}", e))?;
                    stack.push(loaded);
                }
                wasmparser::Operator::LocalSet { local_index } => {
                    if let Some(val) = stack.pop() {
                        let local = locals.get(local_index as usize)
                            .ok_or_else(|| anyhow!("local {} out of range", local_index))?;
                        builder.build_store(*local, val)
                            .map_err(|e| anyhow!("store error: {}", e))?;
                    }
                }
                wasmparser::Operator::I32Const { value } => {
                    stack.push(i32_type.const_int(value as u64, false).into());
                }
                wasmparser::Operator::I64Const { value } => {
                    stack.push(i64_type.const_int(value as u64, false).into());
                }
                wasmparser::Operator::I32Add => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_int_add(a.into_int_value(), b.into_int_value(), "add")
                        .map_err(|e| anyhow!("add error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32Sub => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_int_sub(a.into_int_value(), b.into_int_value(), "sub")
                        .map_err(|e| anyhow!("sub error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32Mul => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_int_mul(a.into_int_value(), b.into_int_value(), "mul")
                        .map_err(|e| anyhow!("mul error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32And => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_and(a.into_int_value(), b.into_int_value(), "and")
                        .map_err(|e| anyhow!("and error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32Or => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_or(a.into_int_value(), b.into_int_value(), "or")
                        .map_err(|e| anyhow!("or error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32Xor => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_xor(a.into_int_value(), b.into_int_value(), "xor")
                        .map_err(|e| anyhow!("xor error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32Shl => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_left_shift(a.into_int_value(), b.into_int_value(), "shl")
                        .map_err(|e| anyhow!("shl error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32ShrU => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_right_shift(a.into_int_value(), b.into_int_value(), false, "lshr")
                        .map_err(|e| anyhow!("lshr error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::I32ShrS => {
                    let b = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let a = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
                    let result = builder
                        .build_right_shift(a.into_int_value(), b.into_int_value(), true, "ashr")
                        .map_err(|e| anyhow!("ashr error: {}", e))?;
                    stack.push(result.into());
                }
                wasmparser::Operator::End => {
                    // End of function or block — emit return if we have a value
                    // and this is likely the function end (simple heuristic for now)
                    if ops_reader.eof() {
                        if let Some(val) = stack.pop() {
                            builder.build_return(Some(&val))
                                .map_err(|e| anyhow!("return error: {}", e))?;
                        } else if func_type.results().is_empty() {
                            builder.build_return(None)
                                .map_err(|e| anyhow!("return error: {}", e))?;
                        }
                    }
                }
                _ => {
                    // Skip unsupported ops for now
                }
            }
        }

        // If no terminator was emitted, add one
        let last_bb = function.get_last_basic_block().unwrap();
        if last_bb.get_terminator().is_none() {
            builder.position_at_end(last_bb);
            if func_type.results().is_empty() {
                builder.build_return(None)
                    .map_err(|e| anyhow!("return error: {}", e))?;
            } else {
                // Return zero as fallback
                let zero = i32_type.const_zero();
                builder.build_return(Some(&zero))
                    .map_err(|e| anyhow!("return error: {}", e))?;
            }
        }

        Ok(())
    }

    /// Run LLVM optimization passes on the module.
    fn optimize(&self, module: &Module<'_>) -> Result<()> {
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| anyhow!("failed to init target: {}", e))?;

        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple)
            .map_err(|e| anyhow!("target from triple: {}", e))?;
        let machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                self.opt_level,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("failed to create target machine"))?;

        module
            .run_passes("default<O2>", &machine, PassBuilderOptions::create())
            .map_err(|e| anyhow!("pass manager error: {}", e))?;

        Ok(())
    }
}

impl Default for WasmToWgslCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal WASM module with a single function: (i32, i32) -> i32 = a + b
    fn make_add_wasm() -> Vec<u8> {
        // Minimal WASM binary for:
        // (module
        //   (func (export "add") (param i32 i32) (result i32)
        //     local.get 0
        //     local.get 1
        //     i32.add))
        let mut wasm = Vec::new();
        // Magic + version
        wasm.extend_from_slice(b"\x00asm");
        wasm.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

        // Type section (1 type: (i32, i32) -> i32)
        wasm.push(0x01); // section id
        wasm.push(0x07); // section size
        wasm.push(0x01); // num types
        wasm.push(0x60); // func type
        wasm.push(0x02); // num params
        wasm.push(0x7f); // i32
        wasm.push(0x7f); // i32
        wasm.push(0x01); // num results
        wasm.push(0x7f); // i32

        // Function section (1 function, type index 0)
        wasm.push(0x03); // section id
        wasm.push(0x02); // section size
        wasm.push(0x01); // num functions
        wasm.push(0x00); // type index

        // Code section
        wasm.push(0x0a); // section id
        wasm.push(0x09); // section size
        wasm.push(0x01); // num functions
        wasm.push(0x07); // function body size
        wasm.push(0x00); // num locals
        wasm.push(0x20); // local.get
        wasm.push(0x00); // index 0
        wasm.push(0x20); // local.get
        wasm.push(0x01); // index 1
        wasm.push(0x6a); // i32.add
        wasm.push(0x0b); // end

        wasm
    }

    #[test]
    fn test_wasm_add_to_wgsl() {
        let wasm = make_add_wasm();
        let compiler = WasmToWgslCompiler::new();
        let wgsl = compiler.compile(&wasm).expect("compilation failed");

        println!("=== Generated WGSL ===\n{}", wgsl);

        // Basic structure checks
        assert!(wgsl.contains("var<storage, read_write> memory"));
        assert!(wgsl.contains("@compute @workgroup_size(64)"));
        assert!(wgsl.contains("fn __wasm_func_0"));
        // Should contain an add operation (or the optimized equivalent)
        assert!(
            wgsl.contains("+") || wgsl.contains("add"),
            "WGSL should contain addition"
        );
        assert!(wgsl.contains("return"), "WGSL should contain a return");
    }

    #[test]
    fn test_wgsl_emitter_simple_ir() {
        // Create a simple LLVM module directly and test the emitter
        let context = Context::create();
        let module = context.create_module("test");
        let i32_type = context.i32_type();
        let builder = context.create_builder();

        // fn add(a: i32, b: i32) -> i32 { a + b }
        let fn_type = i32_type.fn_type(
            &[i32_type.into(), i32_type.into()],
            false,
        );
        let function = module.add_function("simple_add", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        builder.position_at_end(entry);

        let a = function.get_nth_param(0).unwrap().into_int_value();
        let b = function.get_nth_param(1).unwrap().into_int_value();
        a.set_name("a");
        b.set_name("b");

        let sum = builder.build_int_add(a, b, "sum").unwrap();
        builder.build_return(Some(&sum)).unwrap();

        let mut emitter = WgslEmitter::new(&module);
        let wgsl = emitter.emit().expect("emit failed");
        println!("=== Simple IR WGSL ===\n{}", wgsl);

        assert!(wgsl.contains("fn simple_add"));
        assert!(wgsl.contains("+"));
        assert!(wgsl.contains("return"));
    }
}
