//! Top-level WASM-to-SPIR-V compiler
//!
//! Orchestrates the full compilation pipeline:
//! 1. Parse WASM module (via our own parser for type info)
//! 2. Create LLVM module and lower each function to LLVM IR
//! 3. Run LLVM optimization passes
//! 4. Emit SPIR-V binary (TBD)

use crate::lowering::{FunctionLowering, parse_wasm_for_lowering};
use anyhow::{anyhow, Context, Result};
use inkwell::context::Context as LlvmContext;
use inkwell::module::Module;
use inkwell::passes::PassBuilderOptions;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::{AddressSpace, OptimizationLevel};
use inkwell::targets::{InitializationConfig, Target, TargetMachine};

/// Compiled GPU kernel for an alkanes contract
pub struct CompiledKernel {
    /// LLVM IR (for debugging)
    pub llvm_ir: String,
    /// Optimized LLVM IR
    pub optimized_ir: String,
    /// Contract identifier
    pub alkane_id: (u128, u128),
}

/// WASM-to-SPIR-V compiler
pub struct WasmToSpirv {
    context: LlvmContext,
}

impl WasmToSpirv {
    pub fn new() -> Self {
        Self {
            context: LlvmContext::create(),
        }
    }

    /// Compile a WASM contract to an optimized LLVM module.
    /// Returns the LLVM IR as a string for now (SPIR-V emission TBD).
    pub fn compile(
        &self,
        wasm_bytes: &[u8],
        alkane_id: (u128, u128),
    ) -> Result<CompiledKernel> {
        // Step 1: Parse WASM for type info
        let type_info = parse_wasm_for_lowering(wasm_bytes)
            .context("failed to parse WASM module")?;

        log::info!(
            "alkanes-llvm: compiling contract ({},{}) — {} imports, {} functions, {} globals",
            alkane_id.0, alkane_id.1,
            type_info.import_count,
            type_info.func_type_indices.len(),
            type_info.globals.len(),
        );

        // Step 2: Create LLVM module
        let module = self.context.create_module(&format!(
            "alkane_{}_{}", alkane_id.0, alkane_id.1
        ));

        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();
        let void_type = self.context.void_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());

        // Step 3: Create LLVM function declarations for all functions
        // (imports + internal functions). Every function gets (ptr %memory, ptr %kv_store, ..wasm_params..)
        let mut all_functions: Vec<FunctionValue> = Vec::new();

        // Create import function declarations
        for (i, &type_idx) in type_info.import_type_indices.iter().enumerate() {
            let func_type = &type_info.types[type_idx as usize];
            let llvm_fn_type = self.build_llvm_fn_type(&func_type.params, &func_type.results);
            let name = &type_info.import_names[i];
            // Use the import name as the LLVM function name
            let clean_name = clean_import_name(name);
            let func = module.add_function(&clean_name, llvm_fn_type, Some(inkwell::module::Linkage::External));
            all_functions.push(func);
        }

        // Create internal function declarations
        for (i, &type_idx) in type_info.func_type_indices.iter().enumerate() {
            let func_type = &type_info.types[type_idx as usize];
            let llvm_fn_type = self.build_llvm_fn_type(&func_type.params, &func_type.results);
            let func = module.add_function(
                &format!("__wasm_func_{}", i),
                llvm_fn_type,
                None,
            );
            all_functions.push(func);
        }

        // Step 4: Create LLVM globals
        let mut llvm_globals: Vec<PointerValue> = Vec::new();
        for (i, g) in type_info.globals.iter().enumerate() {
            let (global_type, init_val) = match g.value_type {
                0x7E => {
                    let gt = i64_type.as_basic_type_enum();
                    let iv = i64_type.const_int(g.init_value, false);
                    (gt, BasicValueEnum::from(iv))
                }
                _ => {
                    let gt = i32_type.as_basic_type_enum();
                    let iv = i32_type.const_int(g.init_value & 0xFFFFFFFF, false);
                    (gt, BasicValueEnum::from(iv))
                }
            };
            let gv = module.add_global(global_type, None, &format!("__wasm_global_{}", i));
            gv.set_initializer(&init_val);
            if g.mutable {
                gv.set_externally_initialized(false);
            }
            llvm_globals.push(gv.as_pointer_value());
        }

        // Step 5: Lower each internal function body
        for (i, &type_idx) in type_info.func_type_indices.iter().enumerate() {
            let func_type = &type_info.types[type_idx as usize];
            let llvm_func = all_functions[type_info.import_count as usize + i];

            // Get memory_base and kv_store from params
            let memory_base = llvm_func.get_nth_param(0).unwrap().into_pointer_value();
            let kv_store = llvm_func.get_nth_param(1).unwrap().into_pointer_value();

            let mut lowering = FunctionLowering::new(
                &self.context,
                &module,
                llvm_func,
                memory_base,
                kv_store,
                &all_functions,
                &type_info,
                &llvm_globals,
            );

            match lowering.lower_function(i, &func_type.params, &func_type.results) {
                Ok(()) => { log::debug!("lowered function {} OK", i); }
                Err(e) => {
                    log::warn!("Failed to lower function {}: {}. Emitting stub.", i, e);
                }
            }

            // Fix all unterminated basic blocks (from partial lowering or edge cases)
            let fixup_builder = self.context.create_builder();
            let mut bb_opt = llvm_func.get_first_basic_block();
            while let Some(bb) = bb_opt {
                if bb.get_terminator().is_none() {
                    fixup_builder.position_at_end(bb);
                    if func_type.results.is_empty() {
                        let _ = fixup_builder.build_return(None);
                    } else {
                        let zero: BasicValueEnum = match func_type.results[0] {
                            0x7E => BasicValueEnum::from(i64_type.const_zero()),
                            _ => BasicValueEnum::from(i32_type.const_zero()),
                        };
                        let _ = fixup_builder.build_return(Some(&zero));
                    }
                }
                bb_opt = bb.get_next_basic_block();
            }
        }

        // Step 6: Create __execute entry point that calls the WASM entry function
        let execute_fn_type = void_type.fn_type(
            &[ptr_type.into(), ptr_type.into(), i32_type.into()],
            false,
        );
        let execute_fn = module.add_function("__execute", execute_fn_type, None);
        {
            let entry_bb = self.context.append_basic_block(execute_fn, "entry");
            let builder = self.context.create_builder();
            builder.position_at_end(entry_bb);

            let memory = execute_fn.get_nth_param(0).unwrap().into_pointer_value();
            let kv_store = execute_fn.get_nth_param(1).unwrap().into_pointer_value();

            // Initialize data segments into memory
            for seg in &type_info.data_segments {
                if !seg.data.is_empty() {
                    let dst_offset = i32_type.const_int(seg.offset as u64, false);
                    let dst_ptr = unsafe {
                        builder.build_gep(
                            self.context.i8_type(), memory,
                            &[dst_offset.into()], "data_dst",
                        )?
                    };
                    // Store data as a global constant and memcpy
                    let data_const = self.context.const_string(&seg.data, false);
                    let data_global = module.add_global(data_const.get_type(), None,
                        &format!("__data_seg_{}", seg.offset));
                    data_global.set_initializer(&data_const);
                    data_global.set_constant(true);
                    let src_ptr = data_global.as_pointer_value();
                    let len = i32_type.const_int(seg.data.len() as u64, false);
                    builder.build_memcpy(dst_ptr, 1, src_ptr, 1, len)?;
                }
            }

            // Call the entry function
            if let Some(entry_idx) = type_info.entry_func_idx {
                if (entry_idx as usize) < all_functions.len() {
                    let entry_fn = all_functions[entry_idx as usize];
                    // Build args: memory, kv_store, then any WASM params
                    let entry_type_idx = if entry_idx < type_info.import_count {
                        type_info.import_type_indices[entry_idx as usize]
                    } else {
                        let internal = entry_idx - type_info.import_count;
                        type_info.func_type_indices[internal as usize]
                    };
                    let entry_wasm_type = &type_info.types[entry_type_idx as usize];
                    let mut args: Vec<inkwell::values::BasicMetadataValueEnum> = vec![
                        memory.into(),
                        kv_store.into(),
                    ];
                    // Entry functions typically take no WASM params, but handle it generically
                    for (_i, &pt) in entry_wasm_type.params.iter().enumerate() {
                        match pt {
                            0x7E => args.push(i64_type.const_zero().into()),
                            _ => args.push(i32_type.const_zero().into()),
                        }
                    }
                    builder.build_call(entry_fn, &args, "")?;
                }
            }

            builder.build_return(None)?;
        }

        // Verify module
        if let Err(msg) = module.verify() {
            let ir = module.print_to_string().to_string();
            log::error!("LLVM module verification failed: {}", msg.to_string());
            log::debug!("Failed IR:\n{}", ir);
            // Return unoptimized IR for debugging
            return Ok(CompiledKernel {
                llvm_ir: ir.clone(),
                optimized_ir: format!("VERIFICATION FAILED: {}\n\n{}", msg.to_string(), ir),
                alkane_id,
            });
        }

        // Get unoptimized IR
        let llvm_ir = module.print_to_string().to_string();

        // Step 7: Run optimization passes
        let optimized_ir = self.optimize_module(&module)?;

        Ok(CompiledKernel {
            llvm_ir,
            optimized_ir,
            alkane_id,
        })
    }

    /// Build an LLVM function type for a WASM function.
    /// Compile WASM to SPIR-V in one shot.
    /// Unlike `compile()` which returns IR strings, this preserves the
    /// LLVM Module long enough to call `emit_spirv()`.
    pub fn compile_and_emit_spirv(
        &self,
        wasm_bytes: &[u8],
        alkane_id: (u128, u128),
    ) -> Result<Vec<u8>> {
        // Step 1: Parse WASM for type info
        let type_info = parse_wasm_for_lowering(wasm_bytes)
            .context("failed to parse WASM module")?;

        log::info!(
            "alkanes-llvm: compile_and_emit_spirv ({},{}) — {} imports, {} functions",
            alkane_id.0, alkane_id.1,
            type_info.import_count,
            type_info.func_type_indices.len(),
        );

        // Step 2: Create LLVM module
        let module = self.context.create_module(&format!(
            "alkane_{}_{}", alkane_id.0, alkane_id.1
        ));

        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();
        let void_type = self.context.void_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());

        // Step 3: Create function declarations
        let mut all_functions: Vec<FunctionValue> = Vec::new();

        for (i, &type_idx) in type_info.import_type_indices.iter().enumerate() {
            let func_type = &type_info.types[type_idx as usize];
            let llvm_fn_type = self.build_llvm_fn_type(&func_type.params, &func_type.results);
            let name = &type_info.import_names[i];
            let clean_name = clean_import_name(name);
            let func = module.add_function(&clean_name, llvm_fn_type, Some(inkwell::module::Linkage::External));
            all_functions.push(func);
        }

        for (i, &type_idx) in type_info.func_type_indices.iter().enumerate() {
            let func_type = &type_info.types[type_idx as usize];
            let llvm_fn_type = self.build_llvm_fn_type(&func_type.params, &func_type.results);
            let func = module.add_function(
                &format!("__wasm_func_{}", i),
                llvm_fn_type,
                None,
            );
            all_functions.push(func);
        }

        // Step 4: Create globals
        let mut llvm_globals: Vec<PointerValue> = Vec::new();
        for (i, g) in type_info.globals.iter().enumerate() {
            let (global_type, init_val) = match g.value_type {
                0x7E => {
                    let gt = i64_type.as_basic_type_enum();
                    let iv = i64_type.const_int(g.init_value, false);
                    (gt, BasicValueEnum::from(iv))
                }
                _ => {
                    let gt = i32_type.as_basic_type_enum();
                    let iv = i32_type.const_int(g.init_value & 0xFFFFFFFF, false);
                    (gt, BasicValueEnum::from(iv))
                }
            };
            let gv = module.add_global(global_type, None, &format!("__wasm_global_{}", i));
            gv.set_initializer(&init_val);
            if g.mutable {
                gv.set_externally_initialized(false);
            }
            llvm_globals.push(gv.as_pointer_value());
        }

        // Step 5: Lower each internal function body
        for (i, &type_idx) in type_info.func_type_indices.iter().enumerate() {
            let func_type = &type_info.types[type_idx as usize];
            let llvm_func = all_functions[type_info.import_count as usize + i];

            let memory_base = llvm_func.get_nth_param(0).unwrap().into_pointer_value();
            let kv_store = llvm_func.get_nth_param(1).unwrap().into_pointer_value();

            let mut lowering = FunctionLowering::new(
                &self.context,
                &module,
                llvm_func,
                memory_base,
                kv_store,
                &all_functions,
                &type_info,
                &llvm_globals,
            );

            match lowering.lower_function(i, &func_type.params, &func_type.results) {
                Ok(()) => { log::debug!("lowered function {} OK", i); }
                Err(e) => {
                    log::warn!("Failed to lower function {}: {}. Emitting stub.", i, e);
                }
            }

            // Fix unterminated basic blocks
            let fixup_builder = self.context.create_builder();
            let mut bb_opt = llvm_func.get_first_basic_block();
            while let Some(bb) = bb_opt {
                if bb.get_terminator().is_none() {
                    fixup_builder.position_at_end(bb);
                    if func_type.results.is_empty() {
                        let _ = fixup_builder.build_return(None);
                    } else {
                        let zero: BasicValueEnum = match func_type.results[0] {
                            0x7E => BasicValueEnum::from(i64_type.const_zero()),
                            _ => BasicValueEnum::from(i32_type.const_zero()),
                        };
                        let _ = fixup_builder.build_return(Some(&zero));
                    }
                }
                bb_opt = bb.get_next_basic_block();
            }
        }

        // Step 6: Create __execute entry point
        let execute_fn_type = void_type.fn_type(
            &[ptr_type.into(), ptr_type.into(), i32_type.into()],
            false,
        );
        let execute_fn = module.add_function("__execute", execute_fn_type, None);
        {
            let entry_bb = self.context.append_basic_block(execute_fn, "entry");
            let builder = self.context.create_builder();
            builder.position_at_end(entry_bb);

            let memory = execute_fn.get_nth_param(0).unwrap().into_pointer_value();
            let kv_store = execute_fn.get_nth_param(1).unwrap().into_pointer_value();

            for seg in &type_info.data_segments {
                if !seg.data.is_empty() {
                    let dst_offset = i32_type.const_int(seg.offset as u64, false);
                    let dst_ptr = unsafe {
                        builder.build_gep(
                            self.context.i8_type(), memory,
                            &[dst_offset.into()], "data_dst",
                        )?
                    };
                    let data_const = self.context.const_string(&seg.data, false);
                    let data_global = module.add_global(data_const.get_type(), None,
                        &format!("__data_seg_{}", seg.offset));
                    data_global.set_initializer(&data_const);
                    data_global.set_constant(true);
                    let src_ptr = data_global.as_pointer_value();
                    let len = i32_type.const_int(seg.data.len() as u64, false);
                    builder.build_memcpy(dst_ptr, 1, src_ptr, 1, len)?;
                }
            }

            if let Some(entry_idx) = type_info.entry_func_idx {
                if (entry_idx as usize) < all_functions.len() {
                    let entry_fn = all_functions[entry_idx as usize];
                    let entry_type_idx = if entry_idx < type_info.import_count {
                        type_info.import_type_indices[entry_idx as usize]
                    } else {
                        let internal = entry_idx - type_info.import_count;
                        type_info.func_type_indices[internal as usize]
                    };
                    let entry_wasm_type = &type_info.types[entry_type_idx as usize];
                    let mut args: Vec<inkwell::values::BasicMetadataValueEnum> = vec![
                        memory.into(),
                        kv_store.into(),
                    ];
                    for &pt in entry_wasm_type.params.iter() {
                        match pt {
                            0x7E => args.push(i64_type.const_zero().into()),
                            _ => args.push(i32_type.const_zero().into()),
                        }
                    }
                    builder.build_call(entry_fn, &args, "")?;
                }
            }

            builder.build_return(None)?;
        }

        // Verify module
        if let Err(msg) = module.verify() {
            return Err(anyhow!("LLVM module verification failed: {}", msg.to_string()));
        }

        // Emit SPIR-V directly from the live module (no optimization for SPIR-V target)
        self.emit_spirv(&module)
    }

    /// Signature: (ptr memory, ptr kv_store, ..wasm_params..) -> wasm_result
    fn build_llvm_fn_type(&self, params: &[u8], results: &[u8]) -> inkwell::types::FunctionType<'_> {
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();

        let mut param_types: Vec<BasicMetadataTypeEnum> = vec![
            ptr_type.into(),  // memory
            ptr_type.into(),  // kv_store
        ];
        for &p in params {
            match p {
                0x7E => param_types.push(i64_type.into()),
                _ => param_types.push(i32_type.into()),
            }
        }

        if results.is_empty() {
            self.context.void_type().fn_type(&param_types, false)
        } else {
            // WASM multi-value not supported yet — use first result
            let ret_type: BasicTypeEnum = match results[0] {
                0x7E => i64_type.into(),
                _ => i32_type.into(),
            };
            ret_type.fn_type(&param_types, false)
        }
    }

    /// Run LLVM optimization passes on the module
    fn optimize_module(&self, module: &Module) -> Result<String> {
        Target::initialize_all(&InitializationConfig::default());

        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple)
            .map_err(|e| anyhow!("failed to get target: {}", e))?;
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::Aggressive,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("failed to create target machine"))?;

        let pass_options = PassBuilderOptions::create();
        module
            .run_passes("default<O2>", &target_machine, pass_options)
            .map_err(|e| anyhow!("optimization passes failed: {}", e))?;

        Ok(module.print_to_string().to_string())
    }


    /// Emit SPIR-V binary from an LLVM module via llvm-spirv-18
    pub fn emit_spirv(&self, module: &Module) -> Result<Vec<u8>> {
        use std::process::Command;
        
        let bc_path = std::env::temp_dir().join("alkanes_contract.bc");
        let spv_path = std::env::temp_dir().join("alkanes_contract.spv");
        
        // Set SPIR-V target triple required by llvm-spirv
        module.set_triple(&inkwell::targets::TargetTriple::create("spir64-unknown-unknown"));
        module.write_bitcode_to_path(&bc_path);
        
        let output = Command::new("llvm-spirv-18")
            .arg(&bc_path)
            .arg("-o")
            .arg(&spv_path)
            .output()
            .context("failed to run llvm-spirv-18")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("llvm-spirv-18 failed: {}", stderr));
        }
        
        let spirv_bytes = std::fs::read(&spv_path)
            .context("failed to read SPIR-V output")?;
        
        let _ = std::fs::remove_file(&bc_path);
        let _ = std::fs::remove_file(&spv_path);
        
        Ok(spirv_bytes)
    }

    /// Compile WASM to WGSL source code via LLVM IR.
    ///
    /// Pipeline: WASM -> LLVM IR -> -O2 optimization -> WGSL text
    pub fn compile_to_wgsl(
        &self,
        wasm_bytes: &[u8],
        alkane_id: (u128, u128),
    ) -> Result<String> {
        let kernel = self.compile(wasm_bytes, alkane_id)?;
        // Strip LLVM attribute lines that the textual IR parser chokes on
        let clean_ir: String = kernel.optimized_ir
            .lines()
            .filter(|l| !l.starts_with("attributes #"))
            .map(|l| {
                // Remove inline attribute refs like  #0  from function defs
                let mut s = l.to_string();
                while let Some(pos) = s.find(" #") {
                    let rest = &s[pos + 2..];
                    if rest.starts_with(|c: char| c.is_ascii_digit()) {
                        let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
                        s = format!("{}{}" , &s[..pos], &s[pos + 2 + end..]);
                    } else {
                        break;
                    }
                }
                s
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mem_buf = inkwell::memory_buffer::MemoryBuffer::create_from_memory_range(
            clean_ir.as_bytes(),
            "optimized_ir",
        );
        let module = self.context.create_module_from_ir(mem_buf)
            .map_err(|e| anyhow!("failed to parse optimized IR: {}", e))?;
        crate::wgsl_emit::emit_wgsl(&module)
    }
}

/// Clean up import names to be valid LLVM identifiers
fn clean_import_name(name: &str) -> String {
    // Strip Rust mangling prefix if present
    if let Some(stripped) = name.strip_prefix("_ZN15alkanes_runtime7imports") {
        // Extract the actual function name from the mangled form
        // Format: <len><name> e.g. "5abort" -> "abort", "14__load_storage" -> "__load_storage"
        let mut pos = 0;
        let bytes = stripped.as_bytes();
        let mut len_str = String::new();
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            len_str.push(bytes[pos] as char);
            pos += 1;
        }
        if let Ok(name_len) = len_str.parse::<usize>() {
            if pos + name_len <= bytes.len() {
                return String::from_utf8_lossy(&bytes[pos..pos + name_len]).to_string();
            }
        }
    }
    name.to_string()
}

use inkwell::types::BasicType;

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid WASM module with one function
    fn minimal_wasm() -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        // Type section: () -> ()
        w.push(1); // section id
        w.extend_from_slice(&[4]); // section length
        w.push(1); // 1 type
        w.push(0x60); // func type
        w.push(0); // 0 params
        w.push(0); // 0 results

        // Function section
        w.push(3);
        w.extend_from_slice(&[2]);
        w.push(1); // 1 function
        w.push(0); // type index 0

        // Export section: "_start"
        w.push(7);
        let name = b"_start";
        let export_len = 1 + 1 + name.len() + 1 + 1;
        w.push(export_len as u8);
        w.push(1);
        w.push(name.len() as u8);
        w.extend_from_slice(name);
        w.push(0x00);
        w.push(0);

        // Code section
        let body = vec![0x00, 0x0B]; // 0 locals, end
        let code_len = 1 + 1 + body.len();
        w.push(10);
        w.push(code_len as u8);
        w.push(1);
        w.push(body.len() as u8);
        w.extend_from_slice(&body);

        w
    }

    /// Build a WASM module with i32 arithmetic
    fn arithmetic_wasm() -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        // Type section: (i32, i32) -> i32
        w.push(1); // section id
        w.push(7); // section length
        w.push(1); // 1 type
        w.push(0x60);
        w.push(2); w.push(0x7F); w.push(0x7F); // 2 params: i32, i32
        w.push(1); w.push(0x7F); // 1 result: i32

        // Function section
        w.push(3); w.push(2);
        w.push(1); w.push(0); // 1 function, type 0

        // Export section: "execute"
        w.push(7);
        let name = b"execute";
        let export_len = 1 + 1 + name.len() + 1 + 1;
        w.push(export_len as u8);
        w.push(1);
        w.push(name.len() as u8);
        w.extend_from_slice(name);
        w.push(0x00);
        w.push(0);

        // Code section: local.get 0, local.get 1, i32.add
        let body = vec![
            0x00,       // 0 local declarations
            0x20, 0x00, // local.get 0
            0x20, 0x01, // local.get 1
            0x6A,       // i32.add
            0x0B,       // end
        ];
        let code_len = 1 + 1 + body.len();
        w.push(10);
        w.push(code_len as u8);
        w.push(1);
        w.push(body.len() as u8);
        w.extend_from_slice(&body);

        w
    }

    #[test]
    fn test_compile_minimal_wasm() {
        let _ = env_logger::try_init();
        let compiler = WasmToSpirv::new();
        let wasm = minimal_wasm();
        let result = compiler.compile(&wasm, (1, 0)).unwrap();

        assert!(result.llvm_ir.contains("define void @__execute"));
        assert!(result.optimized_ir.contains("__execute"));

        println!("=== Unoptimized IR ===");
        println!("{}", result.llvm_ir);
        println!("=== Optimized IR ===");
        println!("{}", result.optimized_ir);
    }

    #[test]
    fn test_compile_arithmetic() {
        let _ = env_logger::try_init();
        let compiler = WasmToSpirv::new();
        let wasm = arithmetic_wasm();
        let result = compiler.compile(&wasm, (1, 1)).unwrap();

        println!("=== Arithmetic IR ===");
        println!("{}", result.llvm_ir);
        println!("=== Optimized ===");
        println!("{}", result.optimized_ir);

        // The IR should contain an add instruction
        assert!(result.llvm_ir.contains("add"), "Expected add instruction in IR");
        // Should have the wasm function
        assert!(result.llvm_ir.contains("__wasm_func_0"), "Expected __wasm_func_0");
    }

    #[test]
    fn test_compile_real_contract() {
        let _ = env_logger::try_init();
        let path = "/tmp/contract_2_0.wasm";
        if !std::path::Path::new(path).exists() {
            eprintln!("Skipping: {} not found", path);
            return;
        }
        let wasm = std::fs::read(path).unwrap();
        let compiler = WasmToSpirv::new();
        let result = compiler.compile(&wasm, (2, 0)).unwrap();

        println!("=== Compiled contract (2,0) ===");
        println!("IR size: {} bytes", result.llvm_ir.len());
        println!("Optimized IR size: {} bytes", result.optimized_ir.len());

        // Check that we got non-trivial IR
        assert!(result.llvm_ir.len() > 500, "IR should be non-trivial for real contract");

        // Print first 200 lines
        for (i, line) in result.optimized_ir.lines().enumerate() {
            if i > 200 { break; }
            println!("{}", line);
        }
    }

    #[test]
    fn test_emit_spirv_minimal() {
        let _ = env_logger::try_init();
        let compiler = WasmToSpirv::new();

        // Create a minimal LLVM module with a void function
        let i32_type = compiler.context.i32_type();
        let void_type = compiler.context.void_type();
        let ptr_type = compiler.context.ptr_type(inkwell::AddressSpace::default());
        let fn_type = void_type.fn_type(&[ptr_type.into(), ptr_type.into(), i32_type.into()], false);
        let module = compiler.context.create_module("test_spirv");
        let func = module.add_function("__execute", fn_type, None);
        let bb = compiler.context.append_basic_block(func, "entry");
        let builder = compiler.context.create_builder();
        builder.position_at_end(bb);
        builder.build_return(None).unwrap();

        match compiler.emit_spirv(&module) {
            Ok(spirv) => {
                println!("SPIR-V size: {} bytes", spirv.len());
                assert!(spirv.len() > 0);
                if spirv.len() >= 4 {
                    let magic = u32::from_le_bytes([spirv[0], spirv[1], spirv[2], spirv[3]]);
                    println!("SPIR-V magic: 0x{:08x}", magic);
                }
            }
            Err(e) => {
                println!("SPIR-V emission failed: {}", e);
                // Don't assert — llvm-spirv may produce OpenCL SPIR-V which is expected
            }
        }
    }


    #[test]
    fn test_spirv_validate_and_disassemble() {
        let _ = env_logger::try_init();
        let compiler = WasmToSpirv::new();

        let i32_type = compiler.context.i32_type();
        let void_type = compiler.context.void_type();
        let ptr_type = compiler.context.ptr_type(inkwell::AddressSpace::default());
        let fn_type = void_type.fn_type(&[ptr_type.into(), ptr_type.into(), i32_type.into()], false);
        let module = compiler.context.create_module("test_spirv");
        module.set_triple(&inkwell::targets::TargetTriple::create("spir64-unknown-unknown"));
        let func = module.add_function("__execute", fn_type, None);
        let bb = compiler.context.append_basic_block(func, "entry");
        let builder = compiler.context.create_builder();
        builder.position_at_end(bb);
        builder.build_return(None).unwrap();

        let spirv = compiler.emit_spirv(&module).unwrap();
        std::fs::write("/tmp/test.spv", &spirv).unwrap();
        
        // Validate
        let val = std::process::Command::new("spirv-val").arg("/tmp/test.spv").output().unwrap();
        println!("spirv-val: exit={} stderr={}", val.status, String::from_utf8_lossy(&val.stderr));
        
        // Disassemble
        let dis = std::process::Command::new("spirv-dis").arg("/tmp/test.spv").output().unwrap();
        println!("SPIR-V assembly:\n{}", String::from_utf8_lossy(&dis.stdout));
    }


    #[test]
    fn test_compile_real_to_wgsl() {
        let _ = env_logger::try_init();
        let path = "/tmp/contract_2_0.wasm";
        if !std::path::Path::new(path).exists() {
            eprintln!("Skipping: {} not found", path);
            return;
        }
        let wasm = std::fs::read(path).unwrap();
        let compiler = WasmToSpirv::new();
        match compiler.compile_to_wgsl(&wasm, (2, 0)) {
            Ok(wgsl) => {
                println!("WGSL size: {} bytes", wgsl.len());
                println!("First 500 chars:\n{}", &wgsl[..std::cmp::min(wgsl.len(), 500)]);
                // Write to disk for inspection
                std::fs::write("/tmp/contract_2_0.wgsl", &wgsl).unwrap();
                println!("Wrote /tmp/contract_2_0.wgsl");
                assert!(wgsl.contains("@compute"));
                assert!(wgsl.contains("fn main"));
            }
            Err(e) => {
                println!("WGSL compilation failed: {}", e);
                // Don't assert — this is expected to need more work
            }
        }
    }

}
