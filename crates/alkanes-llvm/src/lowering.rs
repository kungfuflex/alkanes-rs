//! WASM bytecode → LLVM IR lowering
//!
//! Translates WASM function bodies to LLVM IR functions.
//! WASM's stack machine is lowered to SSA form — the stack becomes
//! implicit through LLVM values, and mem2reg promotes locals to registers.

use anyhow::{anyhow, Result};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate};

/// Represents the WASM value stack during IR generation.
/// Values are LLVM SSA values, not memory locations.
pub(crate) struct ValueStack<'ctx> {
    values: Vec<BasicValueEnum<'ctx>>,
}

impl<'ctx> ValueStack<'ctx> {
    fn new() -> Self {
        Self { values: Vec::new() }
    }

    fn push(&mut self, val: BasicValueEnum<'ctx>) {
        self.values.push(val);
    }

    fn pop(&mut self) -> Result<BasicValueEnum<'ctx>> {
        self.values.pop().ok_or_else(|| anyhow!("stack underflow"))
    }

    fn pop_i32(&mut self) -> Result<IntValue<'ctx>> {
        self.pop()?.into_int_value_or_err()
    }

    fn pop_i64(&mut self) -> Result<IntValue<'ctx>> {
        self.pop()?.into_int_value_or_err()
    }

    fn len(&self) -> usize {
        self.values.len()
    }

    fn truncate(&mut self, len: usize) {
        self.values.truncate(len);
    }
}

trait IntoIntValueOrErr<'ctx> {
    fn into_int_value_or_err(self) -> Result<IntValue<'ctx>>;
}

impl<'ctx> IntoIntValueOrErr<'ctx> for BasicValueEnum<'ctx> {
    fn into_int_value_or_err(self) -> Result<IntValue<'ctx>> {
        match self {
            BasicValueEnum::IntValue(v) => Ok(v),
            _ => Err(anyhow!("expected int value on stack")),
        }
    }
}

/// Control flow frame for WASM block/loop/if structures
struct ControlFrame<'ctx> {
    /// The LLVM basic block to branch to on `br` (loop header for loops, merge for blocks)
    target: BasicBlock<'ctx>,
    /// The merge/continuation block after `end`
    merge: BasicBlock<'ctx>,
    /// Whether this is a loop (br goes back to target) or block (br goes to merge)
    is_loop: bool,
    /// Stack depth at entry (for unwinding on br)
    stack_depth: usize,
    /// Whether this frame is an if/else (else_bb is set)
    else_bb: Option<BasicBlock<'ctx>>,
    /// Whether the else branch has been entered
    else_entered: bool,
    /// Number of result values this block produces
    #[allow(dead_code)]
    result_count: usize,
    /// Whether the current block is unreachable (after br/br_if/return)
    unreachable: bool,
}

/// WASM function type signature
#[derive(Debug, Clone)]
pub struct WasmFuncType {
    pub params: Vec<u8>,   // WASM value types: 0x7F=i32, 0x7E=i64
    pub results: Vec<u8>,
}

/// Parsed WASM type + function information needed for lowering
#[derive(Debug, Clone)]
pub struct WasmTypeInfo {
    /// All function types from the type section
    pub types: Vec<WasmFuncType>,
    /// Type index for each function in the function section (non-imported)
    pub func_type_indices: Vec<u32>,
    /// Import type indices
    pub import_type_indices: Vec<u32>,
    /// Number of imported functions
    pub import_count: u32,
    /// Function body info: (offset_in_code_bytes, body_length, local_decls)
    /// local_decls: Vec<(count, value_type)>
    pub func_bodies: Vec<FuncBody>,
    /// Import names
    pub import_names: Vec<String>,
    /// Global info
    pub globals: Vec<GlobalInfo>,
    /// Data segments
    pub data_segments: Vec<DataSeg>,
    /// Raw code section bytes
    pub code_bytes: Vec<u8>,
    /// Entry function index (WASM absolute index)
    pub entry_func_idx: Option<u32>,
    /// Memory section: initial pages
    pub memory_initial_pages: u32,
    /// Table section: element entries for call_indirect
    pub table_elements: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct FuncBody {
    /// Offset within code_bytes where opcodes start (after local decls)
    pub code_start: usize,
    /// Offset within code_bytes where this body ends (the 0x0B end byte)
    pub code_end: usize,
    /// Local variable declarations: (count, value_type)
    pub local_decls: Vec<(u32, u8)>,
}

#[derive(Debug, Clone)]
pub struct GlobalInfo {
    pub value_type: u8,
    pub mutable: bool,
    pub init_value: u64,
}

#[derive(Debug, Clone)]
pub struct DataSeg {
    pub offset: u32,
    pub data: Vec<u8>,
}

/// Parse WASM binary to extract type info needed for LLVM lowering.
/// This is a more detailed parse than wasm_parser — we need types, local decls, etc.
pub fn parse_wasm_for_lowering(wasm: &[u8]) -> Result<WasmTypeInfo> {
    if wasm.len() < 8 || &wasm[0..4] != b"\0asm" {
        return Err(anyhow!("invalid WASM"));
    }
    let mut pos: usize = 8;

    let mut types = Vec::new();
    let mut func_type_indices = Vec::new();
    let mut import_count: u32 = 0;
    let mut import_type_indices = Vec::new();
    let mut import_names = Vec::new();
    let mut globals = Vec::new();
    let mut data_segments = Vec::new();
    let mut code_bytes = Vec::new();
    let mut func_bodies = Vec::new();
    let mut entry_func_idx: Option<u32> = None;
    let mut memory_initial_pages: u32 = 1;
    let mut table_elements: Vec<u32> = Vec::new();

    while pos < wasm.len() {
        let section_id = wasm[pos];
        pos += 1;
        let section_len = read_leb128_u32_at(wasm, &mut pos)?;
        let section_end = pos + section_len as usize;

        match section_id {
            // Type section
            1 => {
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..count {
                    if wasm[pos] != 0x60 {
                        return Err(anyhow!("expected func type 0x60"));
                    }
                    pos += 1;
                    let param_count = read_leb128_u32_at(wasm, &mut pos)?;
                    let mut params = Vec::new();
                    for _ in 0..param_count {
                        params.push(wasm[pos]);
                        pos += 1;
                    }
                    let result_count = read_leb128_u32_at(wasm, &mut pos)?;
                    let mut results = Vec::new();
                    for _ in 0..result_count {
                        results.push(wasm[pos]);
                        pos += 1;
                    }
                    types.push(WasmFuncType { params, results });
                }
            }
            // Import section
            2 => {
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..count {
                    let mlen = read_leb128_u32_at(wasm, &mut pos)?;
                    pos += mlen as usize; // skip module name
                    let flen = read_leb128_u32_at(wasm, &mut pos)?;
                    let field_name = String::from_utf8_lossy(&wasm[pos..pos + flen as usize]).to_string();
                    pos += flen as usize;
                    let kind = wasm[pos];
                    pos += 1;
                    match kind {
                        0x00 => {
                            let type_idx = read_leb128_u32_at(wasm, &mut pos)?;
                            import_type_indices.push(type_idx);
                            import_names.push(field_name);
                            import_count += 1;
                        }
                        0x01 => {
                            // table
                            pos += 1; // elem type
                            let flags = read_leb128_u32_at(wasm, &mut pos)?;
                            let _ = read_leb128_u32_at(wasm, &mut pos)?;
                            if flags & 1 != 0 { let _ = read_leb128_u32_at(wasm, &mut pos)?; }
                        }
                        0x02 => {
                            // memory
                            let flags = read_leb128_u32_at(wasm, &mut pos)?;
                            memory_initial_pages = read_leb128_u32_at(wasm, &mut pos)?;
                            if flags & 1 != 0 { let _ = read_leb128_u32_at(wasm, &mut pos)?; }
                        }
                        0x03 => {
                            pos += 1; // value type
                            pos += 1; // mutability
                        }
                        _ => {}
                    }
                }
            }
            // Function section
            3 => {
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..count {
                    let type_idx = read_leb128_u32_at(wasm, &mut pos)?;
                    func_type_indices.push(type_idx);
                }
            }
            // Table section
            4 => {
                // Just skip, we get elements from Element section
                pos = section_end;
            }
            // Memory section
            5 => {
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                if count > 0 {
                    let flags = read_leb128_u32_at(wasm, &mut pos)?;
                    memory_initial_pages = read_leb128_u32_at(wasm, &mut pos)?;
                    if flags & 1 != 0 { let _ = read_leb128_u32_at(wasm, &mut pos)?; }
                }
            }
            // Global section
            6 => {
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..count {
                    let vt = wasm[pos]; pos += 1;
                    let mt = wasm[pos] != 0; pos += 1;
                    let (init, consumed) = parse_init_expr(&wasm[pos..])?;
                    pos += consumed;
                    globals.push(GlobalInfo { value_type: vt, mutable: mt, init_value: init });
                }
            }
            // Export section
            7 => {
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..count {
                    let nlen = read_leb128_u32_at(wasm, &mut pos)?;
                    let name = &wasm[pos..pos + nlen as usize];
                    pos += nlen as usize;
                    let kind = wasm[pos]; pos += 1;
                    let idx = read_leb128_u32_at(wasm, &mut pos)?;
                    if kind == 0x00 && (name == b"_start" || name == b"execute" || name == b"__execute") {
                        entry_func_idx = Some(idx);
                    }
                }
            }
            // Element section
            9 => {
                let seg_count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..seg_count {
                    let flags = read_leb128_u32_at(wasm, &mut pos)?;
                    if flags == 0 {
                        // Active element segment for table 0
                        let (offset_val, consumed) = parse_init_expr(&wasm[pos..])?;
                        pos += consumed;
                        let elem_count = read_leb128_u32_at(wasm, &mut pos)?;
                        // Ensure table_elements is big enough
                        let needed = offset_val as usize + elem_count as usize;
                        if table_elements.len() < needed {
                            table_elements.resize(needed, 0);
                        }
                        for i in 0..elem_count as usize {
                            let func_idx = read_leb128_u32_at(wasm, &mut pos)?;
                            table_elements[offset_val as usize + i] = func_idx;
                        }
                    } else {
                        // Skip other element segment kinds
                        pos = section_end;
                        break;
                    }
                }
            }
            // Code section
            10 => {
                let code_section_start = pos;
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..count {
                    let body_len = read_leb128_u32_at(wasm, &mut pos)?;
                    let body_start = pos;
                    // Parse local declarations
                    let local_decl_count = read_leb128_u32_at(wasm, &mut pos)?;
                    let mut local_decls = Vec::new();
                    for _ in 0..local_decl_count {
                        let lc = read_leb128_u32_at(wasm, &mut pos)?;
                        let lt = wasm[pos]; pos += 1;
                        local_decls.push((lc, lt));
                    }
                    let code_start = pos - code_section_start;
                    let code_end = body_start + body_len as usize - code_section_start;
                    func_bodies.push(FuncBody { code_start, code_end, local_decls });
                    pos = body_start + body_len as usize;
                }
                code_bytes = wasm[code_section_start..section_end].to_vec();
            }
            // Data section
            11 => {
                let count = read_leb128_u32_at(wasm, &mut pos)?;
                for _ in 0..count {
                    let flags = read_leb128_u32_at(wasm, &mut pos)?;
                    match flags {
                        0 => {
                            let (offset_val, consumed) = parse_init_expr(&wasm[pos..])?;
                            pos += consumed;
                            let data_len = read_leb128_u32_at(wasm, &mut pos)?;
                            let data = wasm[pos..pos + data_len as usize].to_vec();
                            pos += data_len as usize;
                            data_segments.push(DataSeg { offset: offset_val as u32, data });
                        }
                        1 => {
                            let data_len = read_leb128_u32_at(wasm, &mut pos)?;
                            pos += data_len as usize;
                        }
                        2 => {
                            let _ = read_leb128_u32_at(wasm, &mut pos)?;
                            let (offset_val, consumed) = parse_init_expr(&wasm[pos..])?;
                            pos += consumed;
                            let data_len = read_leb128_u32_at(wasm, &mut pos)?;
                            let data = wasm[pos..pos + data_len as usize].to_vec();
                            pos += data_len as usize;
                            data_segments.push(DataSeg { offset: offset_val as u32, data });
                        }
                        _ => { pos = section_end; break; }
                    }
                }
            }
            _ => {}
        }
        pos = section_end;
    }

    Ok(WasmTypeInfo {
        types,
        func_type_indices,
        import_type_indices,
        import_count,
        import_names,
        globals,
        data_segments,
        code_bytes,
        func_bodies,
        entry_func_idx,
        memory_initial_pages,
        table_elements,
    })
}

/// Lower a single WASM function to LLVM IR
pub struct FunctionLowering<'a, 'ctx> {
    context: &'ctx Context,
    module: &'a Module<'ctx>,
    builder: Builder<'ctx>,
    /// WASM locals as LLVM alloca pointers (mem2reg will promote)
    locals: Vec<PointerValue<'ctx>>,
    /// WASM linear memory base pointer
    memory_base: PointerValue<'ctx>,
    /// KV store pointer (for host calls)
    kv_store: PointerValue<'ctx>,
    /// Value stack (SSA values)
    stack: ValueStack<'ctx>,
    /// Control flow stack
    control_stack: Vec<ControlFrame<'ctx>>,
    /// The LLVM function being built
    function: FunctionValue<'ctx>,
    /// All LLVM functions (indexed by WASM func index)
    all_functions: &'a [FunctionValue<'ctx>],
    /// WASM type info
    type_info: &'a WasmTypeInfo,
    /// LLVM globals (indexed by WASM global index)
    llvm_globals: &'a [PointerValue<'ctx>],
    /// WASM value types for each local (0x7F=i32, 0x7E=i64)
    local_types: Vec<u8>,
    /// Counter for unique block names
    block_counter: u32,
}

impl<'a, 'ctx> FunctionLowering<'a, 'ctx> {
    pub fn new(
        context: &'ctx Context,
        module: &'a Module<'ctx>,
        function: FunctionValue<'ctx>,
        memory_base: PointerValue<'ctx>,
        kv_store: PointerValue<'ctx>,
        all_functions: &'a [FunctionValue<'ctx>],
        type_info: &'a WasmTypeInfo,
        llvm_globals: &'a [PointerValue<'ctx>],
    ) -> Self {
        Self {
            context,
            module,
            builder: context.create_builder(),
            locals: Vec::new(),
            local_types: Vec::new(),
            memory_base,
            kv_store,
            stack: ValueStack::new(),
            control_stack: Vec::new(),
            function,
            all_functions,
            type_info,
            llvm_globals,
            block_counter: 0,
        }
    }

    fn next_bb(&mut self, prefix: &str) -> BasicBlock<'ctx> {
        self.block_counter += 1;
        self.context.append_basic_block(self.function, &format!("{}_{}", prefix, self.block_counter))
    }

    fn i32_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.i32_type()
    }

    fn i64_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.i64_type()
    }

    #[allow(dead_code)]
    fn ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.context.ptr_type(AddressSpace::default())
    }

    fn wasm_type_to_llvm(&self, vt: u8) -> BasicTypeEnum<'ctx> {
        match vt {
            0x7F => self.i32_type().into(),
            0x7E => self.i64_type().into(),
            _ => self.i32_type().into(), // default to i32 for unknown
        }
    }

    fn block_type_results(&self, block_type: i64) -> usize {
        if block_type == -0x40 {
            // void block
            0
        } else if block_type >= 0 {
            // type index
            let ty = &self.type_info.types[block_type as usize];
            ty.results.len()
        } else {
            // value type (0x7F etc) — single result
            1
        }
    }

    /// Lower a WASM function body to LLVM IR.
    /// `func_idx` is the internal function index (0-based, excluding imports).
    /// `param_types` are the WASM value types for parameters.
    /// `result_types` are the WASM value types for return values.
    pub fn lower_function(
        &mut self,
        func_idx: usize,
        param_types: &[u8],
        result_types: &[u8],
    ) -> Result<()> {
        let body = &self.type_info.func_bodies[func_idx];
        let code = &self.type_info.code_bytes;

        // Create entry block
        let entry_bb = self.context.append_basic_block(self.function, "entry");
        self.builder.position_at_end(entry_bb);

        // Allocate locals: first params, then declared locals
        // Parameters are passed as function args (after memory_base, kv_store)
        let mut local_idx = 0;
        // Params
        for (i, &vt) in param_types.iter().enumerate() {
            let llvm_ty = self.wasm_type_to_llvm(vt);
            let alloca = self.builder.build_alloca(llvm_ty, &format!("local_{}", local_idx))?;
            // Store the parameter value
            let param_val = self.function.get_nth_param((i + 2) as u32)  // +2 for memory_base, kv_store
                .ok_or_else(|| anyhow!("missing param {}", i))?;
            self.builder.build_store(alloca, param_val)?;
            self.locals.push(alloca);
            self.local_types.push(vt);
            local_idx += 1;
        }
        // Declared locals
        for &(count, vt) in &body.local_decls {
            let llvm_ty = self.wasm_type_to_llvm(vt);
            let zero: BasicValueEnum = match vt {
                0x7E => BasicValueEnum::from(self.i64_type().const_zero()),
                _ => BasicValueEnum::from(self.i32_type().const_zero()),
            };
            for _ in 0..count {
                let alloca = self.builder.build_alloca(llvm_ty, &format!("local_{}", local_idx))?;
                self.builder.build_store(alloca, zero)?;
                self.locals.push(alloca);
                self.local_types.push(vt);
                local_idx += 1;
            }
        }

        // Push a function-level control frame (the implicit block wrapping the entire function)
        let end_bb = self.next_bb("func_end");
        self.control_stack.push(ControlFrame {
            target: end_bb,
            merge: end_bb,
            is_loop: false,
            stack_depth: 0,
            else_bb: None,
            else_entered: false,
            result_count: result_types.len(),
            unreachable: false,
        });

        // Walk bytecodes
        let mut pos = body.code_start;
        let end_pos = body.code_end;

        while pos < end_pos {
            let opcode = code[pos];
            pos += 1;

            // If current frame is unreachable, skip non-control opcodes
            if let Some(frame) = self.control_stack.last() {
                if frame.unreachable {
                    match opcode {
                        0x02 | 0x03 | 0x04 | 0x05 | 0x0B | 0x0C | 0x0D | 0x0E | 0x0F => {
                            // Control flow opcodes — process them
                        }
                        _ => {
                            // Skip operand bytes for non-control opcodes in unreachable code
                            skip_opcode_operands(code, &mut pos, opcode)?;
                            continue;
                        }
                    }
                }
            }

            match opcode {
                // unreachable
                0x00 => {
                    self.builder.build_unreachable()?;
                    if let Some(frame) = self.control_stack.last_mut() {
                        frame.unreachable = true;
                    }
                }
                // nop
                0x01 => {}
                // block
                0x02 => {
                    let block_type = read_block_type(code, &mut pos)?;
                    let result_count = self.block_type_results(block_type);
                    let merge_bb = self.next_bb("block_merge");
                    self.control_stack.push(ControlFrame {
                        target: merge_bb,
                        merge: merge_bb,
                        is_loop: false,
                        stack_depth: self.stack.len(),
                        else_bb: None,
                        else_entered: false,
                        result_count,
                        unreachable: false,
                    });
                }
                // loop
                0x03 => {
                    let block_type = read_block_type(code, &mut pos)?;
                    let result_count = self.block_type_results(block_type);
                    let loop_header = self.next_bb("loop_header");
                    let merge_bb = self.next_bb("loop_merge");
                    self.builder.build_unconditional_branch(loop_header)?;
                    self.builder.position_at_end(loop_header);
                    self.control_stack.push(ControlFrame {
                        target: loop_header,
                        merge: merge_bb,
                        is_loop: true,
                        stack_depth: self.stack.len(),
                        else_bb: None,
                        else_entered: false,
                        result_count,
                        unreachable: false,
                    });
                }
                // if
                0x04 => {
                    let block_type = read_block_type(code, &mut pos)?;
                    let result_count = self.block_type_results(block_type);
                    let cond = self.stack.pop_i32()?;
                    let zero = self.i32_type().const_zero();
                    let cmp = self.builder.build_int_compare(IntPredicate::NE, cond, zero, "if_cond")?;
                    let then_bb = self.next_bb("if_then");
                    let else_bb = self.next_bb("if_else");
                    let merge_bb = self.next_bb("if_merge");
                    self.builder.build_conditional_branch(cmp, then_bb, else_bb)?;
                    self.builder.position_at_end(then_bb);
                    self.control_stack.push(ControlFrame {
                        target: merge_bb,
                        merge: merge_bb,
                        is_loop: false,
                        stack_depth: self.stack.len(),
                        else_bb: Some(else_bb),
                        else_entered: false,
                        result_count,
                        unreachable: false,
                    });
                }
                // else
                0x05 => {
                    let frame = self.control_stack.last_mut()
                        .ok_or_else(|| anyhow!("else without if"))?;
                    let else_bb = frame.else_bb
                        .ok_or_else(|| anyhow!("else without if"))?;
                    let merge_bb = frame.merge;
                    frame.else_entered = true;
                    let was_unreachable = frame.unreachable;
                    frame.unreachable = false;
                    if !was_unreachable {
                        self.builder.build_unconditional_branch(merge_bb)?;
                    }
                    self.builder.position_at_end(else_bb);
                    // Reset stack to block entry depth
                    let depth = frame.stack_depth;
                    self.stack.truncate(depth);
                }
                // end
                0x0B => {
                    if let Some(frame) = self.control_stack.pop() {
                        let was_unreachable = frame.unreachable;
                        // If there's an else_bb that was never entered, wire it to merge
                        if let Some(else_bb) = frame.else_bb {
                            if !frame.else_entered {
                                self.builder.position_at_end(else_bb);
                                self.builder.build_unconditional_branch(frame.merge)?;
                                if !was_unreachable {
                                    // Go back to where we were
                                    // The current block should branch to merge
                                }
                            }
                        }
                        // Branch to merge from current position (if reachable)
                        if !was_unreachable {
                            self.builder.build_unconditional_branch(frame.merge)?;
                        }
                        self.builder.position_at_end(frame.merge);

                        // If this was the function-level frame, emit return
                        if self.control_stack.is_empty() {
                            if result_types.is_empty() {
                                self.builder.build_return(None)?;
                            } else {
                                let ret_val = self.stack.pop()?;
                                self.builder.build_return(Some(&ret_val))?;
                            }
                        }
                    }
                }
                // br
                0x0C => {
                    let label_idx = read_leb128_u32(code, &mut pos)?;
                    self.emit_br(label_idx as usize)?;
                    if let Some(frame) = self.control_stack.last_mut() {
                        frame.unreachable = true;
                    }
                }
                // br_if
                0x0D => {
                    let label_idx = read_leb128_u32(code, &mut pos)?;
                    let cond = self.stack.pop_i32()?;
                    let zero = self.i32_type().const_zero();
                    let cmp = self.builder.build_int_compare(IntPredicate::NE, cond, zero, "br_if")?;
                    let target_idx = label_idx as usize;
                    let target_bb = self.br_target(target_idx)?;
                    let cont_bb = self.next_bb("br_if_cont");
                    self.builder.build_conditional_branch(cmp, target_bb, cont_bb)?;
                    self.builder.position_at_end(cont_bb);
                }
                // br_table
                0x0E => {
                    let count = read_leb128_u32(code, &mut pos)?;
                    let mut targets = Vec::new();
                    for _ in 0..count {
                        targets.push(read_leb128_u32(code, &mut pos)?);
                    }
                    let default_target = read_leb128_u32(code, &mut pos)?;
                    let idx = self.stack.pop_i32()?;
                    let default_bb = self.br_target(default_target as usize)?;
                    let cases: Vec<_> = targets.iter().enumerate().map(|(i, &t)| {
                        let bb = self.br_target(t as usize).unwrap();
                        (self.i32_type().const_int(i as u64, false), bb)
                    }).collect();
                    self.builder.build_switch(idx, default_bb, &cases)?;
                    if let Some(frame) = self.control_stack.last_mut() {
                        frame.unreachable = true;
                    }
                }
                // return
                0x0F => {
                    if result_types.is_empty() {
                        self.builder.build_return(None)?;
                    } else {
                        let ret_val = self.stack.pop()?;
                        self.builder.build_return(Some(&ret_val))?;
                    }
                    if let Some(frame) = self.control_stack.last_mut() {
                        frame.unreachable = true;
                    }
                }
                // call
                0x10 => {
                    let func_idx = read_leb128_u32(code, &mut pos)?;
                    self.emit_call(func_idx)?;
                }
                // call_indirect
                0x11 => {
                    let type_idx = read_leb128_u32(code, &mut pos)?;
                    let _table_idx = read_leb128_u32(code, &mut pos)?;
                    self.emit_call_indirect(type_idx)?;
                }
                // drop
                0x1A => {
                    self.stack.pop()?;
                }
                // select
                0x1B => {
                    let cond = self.stack.pop_i32()?;
                    let val2 = self.stack.pop()?;
                    let val1 = self.stack.pop()?;
                    let zero = self.i32_type().const_zero();
                    let cmp = self.builder.build_int_compare(IntPredicate::NE, cond, zero, "select")?;
                    let result = self.builder.build_select(cmp, val1, val2, "sel")?;
                    self.stack.push(result);
                }
                // local.get
                0x20 => {
                    let idx = read_leb128_u32(code, &mut pos)?;
                    
                    // Determine the type from the alloca
                    let val = self.load_local(idx as usize)?;
                    self.stack.push(val);
                }
                // local.set
                0x21 => {
                    let idx = read_leb128_u32(code, &mut pos)?;
                    let val = self.stack.pop()?;
                    self.builder.build_store(self.locals[idx as usize], val)?;
                }
                // local.tee
                0x22 => {
                    let idx = read_leb128_u32(code, &mut pos)?;
                    let val = self.stack.pop()?;
                    self.builder.build_store(self.locals[idx as usize], val)?;
                    self.stack.push(val);
                }
                // global.get
                0x23 => {
                    let idx = read_leb128_u32(code, &mut pos)?;
                    let global_ptr = self.llvm_globals[idx as usize];
                    let gt = &self.type_info.globals[idx as usize];
                    let llvm_ty = self.wasm_type_to_llvm(gt.value_type);
                    let val = self.builder.build_load(llvm_ty, global_ptr, &format!("g{}", idx))?;
                    self.stack.push(val);
                }
                // global.set
                0x24 => {
                    let idx = read_leb128_u32(code, &mut pos)?;
                    let val = self.stack.pop()?;
                    self.builder.build_store(self.llvm_globals[idx as usize], val)?;
                }
                // i32.load
                0x28 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load(self.i32_type().into(), offset)?;
                }
                // i64.load
                0x29 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load(self.i64_type().into(), offset)?;
                }
                // f32.load — TODO: float support
                0x2A => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    // Load as i32 for now
                    self.emit_load(self.i32_type().into(), offset)?;
                }
                // f64.load — TODO: float support
                0x2B => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load(self.i64_type().into(), offset)?;
                }
                // i32.load8_s
                0x2C => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(1, true, false, offset)?;
                }
                // i32.load8_u
                0x2D => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(1, false, false, offset)?;
                }
                // i32.load16_s
                0x2E => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(2, true, false, offset)?;
                }
                // i32.load16_u
                0x2F => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(2, false, false, offset)?;
                }
                // i64.load8_s
                0x30 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(1, true, true, offset)?;
                }
                // i64.load8_u
                0x31 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(1, false, true, offset)?;
                }
                // i64.load16_s
                0x32 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(2, true, true, offset)?;
                }
                // i64.load16_u
                0x33 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(2, false, true, offset)?;
                }
                // i64.load32_s
                0x34 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(4, true, true, offset)?;
                }
                // i64.load32_u
                0x35 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_load_extend(4, false, true, offset)?;
                }
                // i32.store
                0x36 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store(4, offset)?;
                }
                // i64.store
                0x37 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store(8, offset)?;
                }
                // f32.store — TODO: float
                0x38 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store(4, offset)?;
                }
                // f64.store — TODO: float
                0x39 => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store(8, offset)?;
                }
                // i32.store8
                0x3A => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store_trunc(1, offset)?;
                }
                // i32.store16
                0x3B => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store_trunc(2, offset)?;
                }
                // i64.store8
                0x3C => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store_trunc(1, offset)?;
                }
                // i64.store16
                0x3D => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store_trunc(2, offset)?;
                }
                // i64.store32
                0x3E => {
                    let _align = read_leb128_u32(code, &mut pos)?;
                    let offset = read_leb128_u32(code, &mut pos)?;
                    self.emit_store_trunc(4, offset)?;
                }
                // memory.size
                0x3F => {
                    let _mem_idx = read_leb128_u32(code, &mut pos)?;
                    // Return a constant for now — runtime would track this
                    let pages = self.i32_type().const_int(self.type_info.memory_initial_pages as u64, false);
                    self.stack.push(pages.into());
                }
                // memory.grow
                0x40 => {
                    let _mem_idx = read_leb128_u32(code, &mut pos)?;
                    let _delta = self.stack.pop_i32()?;
                    // Return -1 (failure) for now — static memory model
                    let neg1 = self.i32_type().const_int(u32::MAX as u64, false);
                    self.stack.push(neg1.into());
                }
                // i32.const
                0x41 => {
                    let val = read_leb128_i32(code, &mut pos)?;
                    let c = self.i32_type().const_int(val as u32 as u64, false);
                    self.stack.push(c.into());
                }
                // i64.const
                0x42 => {
                    let val = read_leb128_i64(code, &mut pos)?;
                    let c = self.i64_type().const_int(val as u64, false);
                    self.stack.push(c.into());
                }
                // f32.const — TODO
                0x43 => {
                    let bits = u32::from_le_bytes([code[pos], code[pos+1], code[pos+2], code[pos+3]]);
                    pos += 4;
                    let c = self.i32_type().const_int(bits as u64, false);
                    self.stack.push(c.into());
                }
                // f64.const — TODO
                0x44 => {
                    let bits = u64::from_le_bytes([
                        code[pos], code[pos+1], code[pos+2], code[pos+3],
                        code[pos+4], code[pos+5], code[pos+6], code[pos+7],
                    ]);
                    pos += 8;
                    let c = self.i64_type().const_int(bits, false);
                    self.stack.push(c.into());
                }
                // i32.eqz
                0x45 => {
                    let a = self.stack.pop_i32()?;
                    let zero = self.i32_type().const_zero();
                    let cmp = self.builder.build_int_compare(IntPredicate::EQ, a, zero, "eqz")?;
                    let r = self.builder.build_int_z_extend(cmp, self.i32_type(), "eqz_ext")?;
                    self.stack.push(r.into());
                }
                // i32.eq
                0x46 => { self.emit_i32_cmp(IntPredicate::EQ)?; }
                // i32.ne
                0x47 => { self.emit_i32_cmp(IntPredicate::NE)?; }
                // i32.lt_s
                0x48 => { self.emit_i32_cmp(IntPredicate::SLT)?; }
                // i32.lt_u
                0x49 => { self.emit_i32_cmp(IntPredicate::ULT)?; }
                // i32.gt_s
                0x4A => { self.emit_i32_cmp(IntPredicate::SGT)?; }
                // i32.gt_u
                0x4B => { self.emit_i32_cmp(IntPredicate::UGT)?; }
                // i32.le_s
                0x4C => { self.emit_i32_cmp(IntPredicate::SLE)?; }
                // i32.le_u
                0x4D => { self.emit_i32_cmp(IntPredicate::ULE)?; }
                // i32.ge_s
                0x4E => { self.emit_i32_cmp(IntPredicate::SGE)?; }
                // i32.ge_u
                0x4F => { self.emit_i32_cmp(IntPredicate::UGE)?; }

                // i64.eqz
                0x50 => {
                    let a = self.stack.pop_i64()?;
                    let zero = self.i64_type().const_zero();
                    let cmp = self.builder.build_int_compare(IntPredicate::EQ, a, zero, "eqz64")?;
                    let r = self.builder.build_int_z_extend(cmp, self.i32_type(), "eqz64_ext")?;
                    self.stack.push(r.into());
                }
                // i64.eq
                0x51 => { self.emit_i64_cmp(IntPredicate::EQ)?; }
                // i64.ne
                0x52 => { self.emit_i64_cmp(IntPredicate::NE)?; }
                // i64.lt_s
                0x53 => { self.emit_i64_cmp(IntPredicate::SLT)?; }
                // i64.lt_u
                0x54 => { self.emit_i64_cmp(IntPredicate::ULT)?; }
                // i64.gt_s
                0x55 => { self.emit_i64_cmp(IntPredicate::SGT)?; }
                // i64.gt_u
                0x56 => { self.emit_i64_cmp(IntPredicate::UGT)?; }
                // i64.le_s
                0x57 => { self.emit_i64_cmp(IntPredicate::SLE)?; }
                // i64.le_u
                0x58 => { self.emit_i64_cmp(IntPredicate::ULE)?; }
                // i64.ge_s
                0x59 => { self.emit_i64_cmp(IntPredicate::SGE)?; }
                // i64.ge_u
                0x5A => { self.emit_i64_cmp(IntPredicate::UGE)?; }

                // i32.clz
                0x67 => {
                    let a = self.stack.pop_i32()?;
                    let ctlz = inkwell::intrinsics::Intrinsic::find("llvm.ctlz.i32")
                        .ok_or_else(|| anyhow!("ctlz intrinsic not found"))?;
                    let ctlz_fn = ctlz.get_declaration(self.module, &[self.i32_type().into()])
                        .ok_or_else(|| anyhow!("ctlz decl failed"))?;
                    let false_val = self.context.bool_type().const_zero();
                    let r = self.builder.build_call(ctlz_fn, &[a.into(), false_val.into()], "clz")?;
                    self.stack.push(r.try_as_basic_value().left().unwrap());
                }
                // i32.ctz
                0x68 => {
                    let a = self.stack.pop_i32()?;
                    let cttz = inkwell::intrinsics::Intrinsic::find("llvm.cttz.i32")
                        .ok_or_else(|| anyhow!("cttz intrinsic not found"))?;
                    let cttz_fn = cttz.get_declaration(self.module, &[self.i32_type().into()])
                        .ok_or_else(|| anyhow!("cttz decl failed"))?;
                    let false_val = self.context.bool_type().const_zero();
                    let r = self.builder.build_call(cttz_fn, &[a.into(), false_val.into()], "ctz")?;
                    self.stack.push(r.try_as_basic_value().left().unwrap());
                }
                // i32.popcnt
                0x69 => {
                    let a = self.stack.pop_i32()?;
                    let ctpop = inkwell::intrinsics::Intrinsic::find("llvm.ctpop.i32")
                        .ok_or_else(|| anyhow!("ctpop intrinsic not found"))?;
                    let ctpop_fn = ctpop.get_declaration(self.module, &[self.i32_type().into()])
                        .ok_or_else(|| anyhow!("ctpop decl failed"))?;
                    let r = self.builder.build_call(ctpop_fn, &[a.into()], "popcnt")?;
                    self.stack.push(r.try_as_basic_value().left().unwrap());
                }
                // i32.add
                0x6A => { self.emit_i32_binop(BinOp::Add)?; }
                // i32.sub
                0x6B => { self.emit_i32_binop(BinOp::Sub)?; }
                // i32.mul
                0x6C => { self.emit_i32_binop(BinOp::Mul)?; }
                // i32.div_s
                0x6D => { self.emit_i32_binop(BinOp::DivS)?; }
                // i32.div_u
                0x6E => { self.emit_i32_binop(BinOp::DivU)?; }
                // i32.rem_s
                0x6F => { self.emit_i32_binop(BinOp::RemS)?; }
                // i32.rem_u
                0x70 => { self.emit_i32_binop(BinOp::RemU)?; }
                // i32.and
                0x71 => { self.emit_i32_binop(BinOp::And)?; }
                // i32.or
                0x72 => { self.emit_i32_binop(BinOp::Or)?; }
                // i32.xor
                0x73 => { self.emit_i32_binop(BinOp::Xor)?; }
                // i32.shl
                0x74 => { self.emit_i32_binop(BinOp::Shl)?; }
                // i32.shr_s
                0x75 => { self.emit_i32_binop(BinOp::ShrS)?; }
                // i32.shr_u
                0x76 => { self.emit_i32_binop(BinOp::ShrU)?; }
                // i32.rotl
                0x77 => { self.emit_i32_binop(BinOp::Rotl)?; }
                // i32.rotr
                0x78 => { self.emit_i32_binop(BinOp::Rotr)?; }

                // i64.clz
                0x79 => {
                    let a = self.stack.pop_i64()?;
                    let ctlz = inkwell::intrinsics::Intrinsic::find("llvm.ctlz.i64")
                        .ok_or_else(|| anyhow!("ctlz64 intrinsic not found"))?;
                    let ctlz_fn = ctlz.get_declaration(self.module, &[self.i64_type().into()])
                        .ok_or_else(|| anyhow!("ctlz64 decl failed"))?;
                    let false_val = self.context.bool_type().const_zero();
                    let r = self.builder.build_call(ctlz_fn, &[a.into(), false_val.into()], "clz64")?;
                    self.stack.push(r.try_as_basic_value().left().unwrap());
                }
                // i64.ctz
                0x7A => {
                    let a = self.stack.pop_i64()?;
                    let cttz = inkwell::intrinsics::Intrinsic::find("llvm.cttz.i64")
                        .ok_or_else(|| anyhow!("cttz64 intrinsic not found"))?;
                    let cttz_fn = cttz.get_declaration(self.module, &[self.i64_type().into()])
                        .ok_or_else(|| anyhow!("cttz64 decl failed"))?;
                    let false_val = self.context.bool_type().const_zero();
                    let r = self.builder.build_call(cttz_fn, &[a.into(), false_val.into()], "ctz64")?;
                    self.stack.push(r.try_as_basic_value().left().unwrap());
                }
                // i64.popcnt
                0x7B => {
                    let a = self.stack.pop_i64()?;
                    let ctpop = inkwell::intrinsics::Intrinsic::find("llvm.ctpop.i64")
                        .ok_or_else(|| anyhow!("ctpop64 intrinsic not found"))?;
                    let ctpop_fn = ctpop.get_declaration(self.module, &[self.i64_type().into()])
                        .ok_or_else(|| anyhow!("ctpop64 decl failed"))?;
                    let r = self.builder.build_call(ctpop_fn, &[a.into()], "popcnt64")?;
                    self.stack.push(r.try_as_basic_value().left().unwrap());
                }
                // i64.add
                0x7C => { self.emit_i64_binop(BinOp::Add)?; }
                // i64.sub
                0x7D => { self.emit_i64_binop(BinOp::Sub)?; }
                // i64.mul
                0x7E => { self.emit_i64_binop(BinOp::Mul)?; }
                // i64.div_s
                0x7F => { self.emit_i64_binop(BinOp::DivS)?; }
                // i64.div_u
                0x80 => { self.emit_i64_binop(BinOp::DivU)?; }
                // i64.rem_s
                0x81 => { self.emit_i64_binop(BinOp::RemS)?; }
                // i64.rem_u
                0x82 => { self.emit_i64_binop(BinOp::RemU)?; }
                // i64.and
                0x83 => { self.emit_i64_binop(BinOp::And)?; }
                // i64.or
                0x84 => { self.emit_i64_binop(BinOp::Or)?; }
                // i64.xor
                0x85 => { self.emit_i64_binop(BinOp::Xor)?; }
                // i64.shl
                0x86 => { self.emit_i64_binop(BinOp::Shl)?; }
                // i64.shr_s
                0x87 => { self.emit_i64_binop(BinOp::ShrS)?; }
                // i64.shr_u
                0x88 => { self.emit_i64_binop(BinOp::ShrU)?; }
                // i64.rotl
                0x89 => { self.emit_i64_binop(BinOp::Rotl)?; }
                // i64.rotr
                0x8A => { self.emit_i64_binop(BinOp::Rotr)?; }

                // i32.wrap_i64
                0xA7 => {
                    let a = self.stack.pop_i64()?;
                    let r = self.builder.build_int_truncate(a, self.i32_type(), "wrap")?;
                    self.stack.push(r.into());
                }
                // i64.extend_i32_s
                0xAC => {
                    let a = self.stack.pop_i32()?;
                    let r = self.builder.build_int_s_extend(a, self.i64_type(), "sext")?;
                    self.stack.push(r.into());
                }
                // i64.extend_i32_u
                0xAD => {
                    let a = self.stack.pop_i32()?;
                    let r = self.builder.build_int_z_extend(a, self.i64_type(), "zext")?;
                    self.stack.push(r.into());
                }

                // f32/f64 conversion opcodes — stub as bitcasts/reinterprets
                // i32.reinterpret_f32
                0xBC => { /* nop — we treat f32 as i32 */ }
                // i64.reinterpret_f64
                0xBD => { /* nop — we treat f64 as i64 */ }
                // f32.reinterpret_i32
                0xBE => { /* nop */ }
                // f64.reinterpret_i64
                0xBF => { /* nop */ }

                // i32.extend8_s
                0xC0 => {
                    let a = self.stack.pop_i32()?;
                    let t = self.builder.build_int_truncate(a, self.context.i8_type(), "tr8")?;
                    let r = self.builder.build_int_s_extend(t, self.i32_type(), "sext8")?;
                    self.stack.push(r.into());
                }
                // i32.extend16_s
                0xC1 => {
                    let a = self.stack.pop_i32()?;
                    let t = self.builder.build_int_truncate(a, self.context.i16_type(), "tr16")?;
                    let r = self.builder.build_int_s_extend(t, self.i32_type(), "sext16")?;
                    self.stack.push(r.into());
                }
                // i64.extend8_s
                0xC2 => {
                    let a = self.stack.pop_i64()?;
                    let t = self.builder.build_int_truncate(a, self.context.i8_type(), "tr8_64")?;
                    let r = self.builder.build_int_s_extend(t, self.i64_type(), "sext8_64")?;
                    self.stack.push(r.into());
                }
                // i64.extend16_s
                0xC3 => {
                    let a = self.stack.pop_i64()?;
                    let t = self.builder.build_int_truncate(a, self.context.i16_type(), "tr16_64")?;
                    let r = self.builder.build_int_s_extend(t, self.i64_type(), "sext16_64")?;
                    self.stack.push(r.into());
                }
                // i64.extend32_s
                0xC4 => {
                    let a = self.stack.pop_i64()?;
                    let t = self.builder.build_int_truncate(a, self.i32_type(), "tr32_64")?;
                    let r = self.builder.build_int_s_extend(t, self.i64_type(), "sext32_64")?;
                    self.stack.push(r.into());
                }

                // Float ops — stub: treat f32 as i32, f64 as i64 (no FP support yet)
                // f32 comparison ops 0x5B-0x60, f64 comparison ops 0x61-0x66
                0x5B..=0x66 => {
                    // Float comparison — pop 2, push i32 result (always 0)
                    self.stack.pop()?;
                    self.stack.pop()?;
                    self.stack.push(self.i32_type().const_zero().into());
                }
                // f32 unary ops
                0x8B..=0x91 => {
                    // f32 unary — pop 1, push 1 (identity)
                    // nop: value stays on stack
                }
                // f32 binary ops
                0x92..=0x98 => {
                    // f32 binary — pop 2, push 1
                    let _b = self.stack.pop()?;
                    // keep a on stack
                }
                // f64 unary ops
                0x99..=0x9F => {
                    // f64 unary — identity
                }
                // f64 binary ops
                0xA0..=0xA6 => {
                    // f64 binary — pop 2, push 1
                    let _b = self.stack.pop()?;
                }
                // f32/f64 conversion ops not yet handled
                0xA8..=0xAB => {
                    // trunc f->i: pop f, push i
                    let a = self.stack.pop()?;
                    // For now push back as-is (it's already an int since we treat f as i)
                    self.stack.push(a);
                }
                0xAE..=0xB1 => {
                    // convert i->f: pop i, push f (we keep as int)
                    let a = self.stack.pop()?;
                    self.stack.push(a);
                }
                // f32.demote_f64
                0xB6 => {
                    let a = self.stack.pop_i64()?;
                    let r = self.builder.build_int_truncate(a, self.i32_type(), "demote")?;
                    self.stack.push(r.into());
                }
                // f64.promote_f32
                0xBB => {
                    let a = self.stack.pop_i32()?;
                    let r = self.builder.build_int_z_extend(a, self.i64_type(), "promote")?;
                    self.stack.push(r.into());
                }

                // Multi-byte opcodes (0xFC prefix)
                0xFC => {
                    let sub_opcode = read_leb128_u32(code, &mut pos)?;
                    match sub_opcode {
                        // i32.trunc_sat_f32_s .. i64.trunc_sat_f64_u (0-7)
                        0..=7 => {
                            // Saturating truncation — identity for our int-as-float model
                            let a = self.stack.pop()?;
                            self.stack.push(a);
                        }
                        // memory.copy (10)
                        10 => {
                            let _dst_mem = read_leb128_u32(code, &mut pos)?;
                            let _src_mem = read_leb128_u32(code, &mut pos)?;
                            let n = self.stack.pop_i32()?;
                            let src = self.stack.pop_i32()?;
                            let dst = self.stack.pop_i32()?;
                            // Use llvm.memcpy
                            let dst_ptr = unsafe {
                                self.builder.build_gep(self.context.i8_type(), self.memory_base, &[dst.into()], "mcpy_dst")?
                            };
                            let src_ptr = unsafe {
                                self.builder.build_gep(self.context.i8_type(), self.memory_base, &[src.into()], "mcpy_src")?
                            };
                            self.builder.build_memmove(dst_ptr, 1, src_ptr, 1, n)?;
                        }
                        // memory.fill (11)
                        11 => {
                            let _mem = read_leb128_u32(code, &mut pos)?;
                            let n = self.stack.pop_i32()?;
                            let val = self.stack.pop_i32()?;
                            let dst = self.stack.pop_i32()?;
                            let dst_ptr = unsafe {
                                self.builder.build_gep(self.context.i8_type(), self.memory_base, &[dst.into()], "mfill_dst")?
                            };
                            let byte_val = self.builder.build_int_truncate(val, self.context.i8_type(), "fill_byte")?;
                            self.builder.build_memset(dst_ptr, 1, byte_val, n)?;
                        }
                        _ => {
                            log::warn!("unhandled 0xFC sub-opcode: {}", sub_opcode);
                        }
                    }
                }

                other => {
                    // Skip unknown opcodes — this is a best-effort lowering
                    log::warn!("unhandled WASM opcode: 0x{:02X} at offset {}", other, pos - 1);
                    // Try to skip operands
                    skip_opcode_operands(code, &mut pos, other)?;
                }
            }
        }

        Ok(())
    }

    fn emit_br(&mut self, label_idx: usize) -> Result<()> {
        let frame_idx = self.control_stack.len() - 1 - label_idx;
        let frame = &self.control_stack[frame_idx];
        let target = if frame.is_loop { frame.target } else { frame.merge };
        self.builder.build_unconditional_branch(target)?;
        Ok(())
    }

    fn br_target(&self, label_idx: usize) -> Result<BasicBlock<'ctx>> {
        let frame_idx = self.control_stack.len() - 1 - label_idx;
        let frame = &self.control_stack[frame_idx];
        Ok(if frame.is_loop { frame.target } else { frame.merge })
    }

    fn load_local(&self, idx: usize) -> Result<BasicValueEnum<'ctx>> {
        let alloca = self.locals[idx];
        // Determine type from the alloca's allocated type
        // We need to figure out if it's i32 or i64
        // Use the pointee type info from the alloca
        
        // Try loading as i32 first, check if element type is i64
        // Actually, inkwell allocas remember their type. We can use get_allocated_type
        // But that's not directly accessible. Instead, track types separately or
        // use the local variable info to determine the type.
        // For simplicity, try to infer from function body local decls and param types.
        // Actually we do know — we stored the alloca with a specific type.
        // We'll load using the correct type based on the wasm local index.
        let local_type = self.get_local_type(idx);
        let val = self.builder.build_load(local_type, alloca, &format!("l{}", idx))?;
        Ok(val)
    }

    fn get_local_type(&self, idx: usize) -> BasicTypeEnum<'ctx> {
        if idx < self.local_types.len() {
            self.wasm_type_to_llvm(self.local_types[idx])
        } else {
            self.i32_type().into()
        }
    }

    fn emit_load(&mut self, load_ty: BasicTypeEnum<'ctx>, offset: u32) -> Result<()> {
        let base = self.stack.pop_i32()?;
        let addr = if offset != 0 {
            let off = self.i32_type().const_int(offset as u64, false);
            self.builder.build_int_add(base, off, "addr")?
        } else {
            base
        };
        let ptr = unsafe {
            self.builder.build_gep(self.context.i8_type(), self.memory_base, &[addr.into()], "mptr")?
        };
        let val = self.builder.build_load(load_ty, ptr, "mload")?;
        self.stack.push(val);
        Ok(())
    }

    fn emit_load_extend(&mut self, bytes: u32, signed: bool, to_i64: bool, offset: u32) -> Result<()> {
        let base = self.stack.pop_i32()?;
        let addr = if offset != 0 {
            let off = self.i32_type().const_int(offset as u64, false);
            self.builder.build_int_add(base, off, "addr")?
        } else {
            base
        };
        let ptr = unsafe {
            self.builder.build_gep(self.context.i8_type(), self.memory_base, &[addr.into()], "mptr")?
        };
        let narrow_ty = match bytes {
            1 => self.context.i8_type(),
            2 => self.context.i16_type(),
            4 => self.i32_type(),
            _ => unreachable!(),
        };
        let narrow_val = self.builder.build_load(narrow_ty, ptr, "mload_narrow")?
            .into_int_value();
        let target_ty = if to_i64 { self.i64_type() } else { self.i32_type() };
        let extended = if signed {
            self.builder.build_int_s_extend(narrow_val, target_ty, "sext")?
        } else {
            self.builder.build_int_z_extend(narrow_val, target_ty, "zext")?
        };
        self.stack.push(extended.into());
        Ok(())
    }

    fn emit_store(&mut self, _bytes: u32, offset: u32) -> Result<()> {
        let val = self.stack.pop()?;
        let base = self.stack.pop_i32()?;
        let addr = if offset != 0 {
            let off = self.i32_type().const_int(offset as u64, false);
            self.builder.build_int_add(base, off, "saddr")?
        } else {
            base
        };
        let ptr = unsafe {
            self.builder.build_gep(self.context.i8_type(), self.memory_base, &[addr.into()], "sptr")?
        };
        self.builder.build_store(ptr, val)?;
        Ok(())
    }

    fn emit_store_trunc(&mut self, bytes: u32, offset: u32) -> Result<()> {
        let val = self.stack.pop()?.into_int_value();
        let base = self.stack.pop_i32()?;
        let addr = if offset != 0 {
            let off = self.i32_type().const_int(offset as u64, false);
            self.builder.build_int_add(base, off, "saddr")?
        } else {
            base
        };
        let ptr = unsafe {
            self.builder.build_gep(self.context.i8_type(), self.memory_base, &[addr.into()], "sptr")?
        };
        let narrow_ty = match bytes {
            1 => self.context.i8_type(),
            2 => self.context.i16_type(),
            4 => self.i32_type(),
            _ => unreachable!(),
        };
        let trunc_val = self.builder.build_int_truncate(val, narrow_ty, "trunc")?;
        self.builder.build_store(ptr, trunc_val)?;
        Ok(())
    }

    fn emit_i32_cmp(&mut self, pred: IntPredicate) -> Result<()> {
        let b = self.stack.pop_i32()?;
        let a = self.stack.pop_i32()?;
        let cmp = self.builder.build_int_compare(pred, a, b, "cmp")?;
        let r = self.builder.build_int_z_extend(cmp, self.i32_type(), "cmp_ext")?;
        self.stack.push(r.into());
        Ok(())
    }

    fn emit_i64_cmp(&mut self, pred: IntPredicate) -> Result<()> {
        let b = self.stack.pop_i64()?;
        let a = self.stack.pop_i64()?;
        let cmp = self.builder.build_int_compare(pred, a, b, "cmp64")?;
        let r = self.builder.build_int_z_extend(cmp, self.i32_type(), "cmp64_ext")?;
        self.stack.push(r.into());
        Ok(())
    }

    fn emit_i32_binop(&mut self, op: BinOp) -> Result<()> {
        let b = self.stack.pop_i32()?;
        let a = self.stack.pop_i32()?;
        let r = self.build_binop(a, b, op, 32)?;
        self.stack.push(r.into());
        Ok(())
    }

    fn emit_i64_binop(&mut self, op: BinOp) -> Result<()> {
        let b = self.stack.pop_i64()?;
        let a = self.stack.pop_i64()?;
        let r = self.build_binop(a, b, op, 64)?;
        self.stack.push(r.into());
        Ok(())
    }

    fn build_binop(&self, a: IntValue<'ctx>, b: IntValue<'ctx>, op: BinOp, bits: u32) -> Result<IntValue<'ctx>> {
        Ok(match op {
            BinOp::Add => self.builder.build_int_add(a, b, "add")?,
            BinOp::Sub => self.builder.build_int_sub(a, b, "sub")?,
            BinOp::Mul => self.builder.build_int_mul(a, b, "mul")?,
            BinOp::DivS => self.builder.build_int_signed_div(a, b, "divs")?,
            BinOp::DivU => self.builder.build_int_unsigned_div(a, b, "divu")?,
            BinOp::RemS => self.builder.build_int_signed_rem(a, b, "rems")?,
            BinOp::RemU => self.builder.build_int_unsigned_rem(a, b, "remu")?,
            BinOp::And => self.builder.build_and(a, b, "and")?,
            BinOp::Or => self.builder.build_or(a, b, "or")?,
            BinOp::Xor => self.builder.build_xor(a, b, "xor")?,
            BinOp::Shl => self.builder.build_left_shift(a, b, "shl")?,
            BinOp::ShrS => self.builder.build_right_shift(a, b, true, "shrs")?,
            BinOp::ShrU => self.builder.build_right_shift(a, b, false, "shru")?,
            BinOp::Rotl => {
                let fshl = inkwell::intrinsics::Intrinsic::find(&format!("llvm.fshl.i{}", bits))
                    .ok_or_else(|| anyhow!("fshl not found"))?;
                let int_ty: BasicTypeEnum = if bits == 32 { self.i32_type().into() } else { self.i64_type().into() };
                let fshl_fn = fshl.get_declaration(self.module, &[int_ty])
                    .ok_or_else(|| anyhow!("fshl decl failed"))?;
                let r = self.builder.build_call(fshl_fn, &[a.into(), a.into(), b.into()], "rotl")?;
                r.try_as_basic_value().left().unwrap().into_int_value()
            }
            BinOp::Rotr => {
                let fshr = inkwell::intrinsics::Intrinsic::find(&format!("llvm.fshr.i{}", bits))
                    .ok_or_else(|| anyhow!("fshr not found"))?;
                let int_ty: BasicTypeEnum = if bits == 32 { self.i32_type().into() } else { self.i64_type().into() };
                let fshr_fn = fshr.get_declaration(self.module, &[int_ty])
                    .ok_or_else(|| anyhow!("fshr decl failed"))?;
                let r = self.builder.build_call(fshr_fn, &[a.into(), a.into(), b.into()], "rotr")?;
                r.try_as_basic_value().left().unwrap().into_int_value()
            }
        })
    }

    fn emit_call(&mut self, func_idx: u32) -> Result<()> {
        // Get function type
        let type_idx = if (func_idx as u32) < self.type_info.import_count {
            self.type_info.import_type_indices[func_idx as usize]
        } else {
            let internal_idx = func_idx as usize - self.type_info.import_count as usize;
            self.type_info.func_type_indices[internal_idx]
        };
        let func_type = &self.type_info.types[type_idx as usize];

        // Pop arguments from stack (in reverse order)
        let mut args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
        // First two args are always memory_base and kv_store
        args.push(self.memory_base.into());
        args.push(self.kv_store.into());
        let mut wasm_args = Vec::new();
        for _ in 0..func_type.params.len() {
            wasm_args.push(self.stack.pop()?);
        }
        wasm_args.reverse();
        for a in wasm_args {
            args.push(a.into());
        }

        let llvm_fn = self.all_functions[func_idx as usize];
        let call = self.builder.build_call(llvm_fn, &args, "call")?;

        // Push results
        if !func_type.results.is_empty() {
            if let Some(ret_val) = call.try_as_basic_value().left() {
                self.stack.push(ret_val);
            }
        }

        Ok(())
    }

    fn emit_call_indirect(&mut self, type_idx: u32) -> Result<()> {
        let func_type = &self.type_info.types[type_idx as usize].clone();

        // Pop the table index (topmost stack value)
        let table_idx_val = self.stack.pop_i32()?;

        // Pop arguments based on the expected type signature
        let mut wasm_args = Vec::new();
        for _ in 0..func_type.params.len() {
            wasm_args.push(self.stack.pop()?);
        }
        wasm_args.reverse();

        // If no table elements, push zero result and continue
        if self.type_info.table_elements.is_empty() {
            if !func_type.results.is_empty() {
                let zero = match func_type.results[0] {
                    0x7E => BasicValueEnum::from(self.i64_type().const_zero()),
                    _ => BasicValueEnum::from(self.i32_type().const_zero()),
                };
                self.stack.push(zero);
            }
            return Ok(());
        }

        let has_result = !func_type.results.is_empty();

        // Create blocks for the dispatch structure
        let merge_bb = self.next_bb("calli_merge");
        let trap_bb = self.next_bb("calli_trap");

        // Collect case blocks -- only for table entries whose signature matches
        let mut case_bbs: Vec<(u64, BasicBlock<'ctx>, u32)> = Vec::new();
        for (i, &target_func_idx) in self.type_info.table_elements.iter().enumerate() {
            if target_func_idx == 0 && i > 0 {
                continue; // skip empty slots (except slot 0 which might be valid)
            }
            if (target_func_idx as usize) >= self.all_functions.len() {
                continue;
            }
            // Get the target function's type
            let target_type_idx = if target_func_idx < self.type_info.import_count {
                self.type_info.import_type_indices[target_func_idx as usize]
            } else {
                let internal_idx = target_func_idx as usize - self.type_info.import_count as usize;
                if internal_idx < self.type_info.func_type_indices.len() {
                    self.type_info.func_type_indices[internal_idx]
                } else {
                    continue;
                }
            };
            let target_func_type = &self.type_info.types[target_type_idx as usize];

            // Only dispatch to functions whose signature matches the expected type
            if target_func_type.params == func_type.params && target_func_type.results == func_type.results {
                let case_bb = self.next_bb(&format!("calli_{}", i));
                case_bbs.push((i as u64, case_bb, target_func_idx));
            }
            // Mismatched signatures fall through to trap (WASM spec behavior)
        }

        // Build switch in the current block
        let cases: Vec<(IntValue<'ctx>, BasicBlock<'ctx>)> = case_bbs.iter()
            .map(|(slot, bb, _)| (self.i32_type().const_int(*slot, false), *bb))
            .collect();
        self.builder.build_switch(table_idx_val, trap_bb, &cases)?;

        // Build trap block
        self.builder.position_at_end(trap_bb);
        if has_result {
            // For trap with result: branch to merge with a zero value
            let zero = match func_type.results[0] {
                0x7E => BasicValueEnum::from(self.i64_type().const_zero()),
                _ => BasicValueEnum::from(self.i32_type().const_zero()),
            };
            self.builder.build_unconditional_branch(merge_bb)?;

            // Build each case block and collect results for phi
            let mut call_results: Vec<(BasicValueEnum<'ctx>, BasicBlock<'ctx>)> = vec![(zero, trap_bb)];
            for &(_, case_bb, target_func_idx) in &case_bbs {
                self.builder.position_at_end(case_bb);
                let callee = self.all_functions[target_func_idx as usize];
                let mut args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
                args.push(self.memory_base.into());
                args.push(self.kv_store.into());
                for a in &wasm_args {
                    args.push((*a).into());
                }
                let call = self.builder.build_call(callee, &args, "icall")?;
                if let Some(rv) = call.try_as_basic_value().left() {
                    call_results.push((rv, case_bb));
                }
                self.builder.build_unconditional_branch(merge_bb)?;
            }

            // Build phi in merge block
            self.builder.position_at_end(merge_bb);
            let result_ty = match func_type.results[0] {
                0x7E => self.i64_type(),
                _ => self.i32_type(),
            };
            let phi = self.builder.build_phi(result_ty, "calli_result")?;
            for (val, bb) in &call_results {
                phi.add_incoming(&[(val, *bb)]);
            }
            self.stack.push(phi.as_basic_value());
        } else {
            // Trap block for void call_indirect: unreachable
            self.builder.build_unreachable()?;

            // Build each case block (void return)
            for &(_, case_bb, target_func_idx) in &case_bbs {
                self.builder.position_at_end(case_bb);
                let callee = self.all_functions[target_func_idx as usize];
                let mut args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
                args.push(self.memory_base.into());
                args.push(self.kv_store.into());
                for a in &wasm_args {
                    args.push((*a).into());
                }
                self.builder.build_call(callee, &args, "icall")?;
                self.builder.build_unconditional_branch(merge_bb)?;
            }

            self.builder.position_at_end(merge_bb);
        }

        Ok(())
    }
}

enum BinOp {
    Add, Sub, Mul, DivS, DivU, RemS, RemU,
    And, Or, Xor, Shl, ShrS, ShrU, Rotl, Rotr,
}

/// Read a WASM block type (single byte or LEB128 type index)
fn read_block_type(data: &[u8], pos: &mut usize) -> Result<i64> {
    let byte = data[*pos];
    match byte {
        0x40 => { *pos += 1; Ok(-0x40) }  // void
        0x7F => { *pos += 1; Ok(-1) }     // i32 result
        0x7E => { *pos += 1; Ok(-2) }     // i64 result
        0x7D => { *pos += 1; Ok(-3) }     // f32 result
        0x7C => { *pos += 1; Ok(-4) }     // f64 result
        _ => {
            // Type index (LEB128)
            let idx = read_leb128_u32(data, pos)?;
            Ok(idx as i64)
        }
    }
}

/// Skip operand bytes for an opcode in unreachable code
fn skip_opcode_operands(code: &[u8], pos: &mut usize, opcode: u8) -> Result<()> {
    match opcode {
        // Opcodes with no operands
        0x00 | 0x01 | 0x0F | 0x1A | 0x1B | 0x45..=0x9F | 0xA7..=0xC4 => {}
        // Opcodes with one LEB128 operand
        0x02 | 0x03 | 0x04 => {
            // block type
            read_block_type(code, pos)?;
        }
        0x0C | 0x0D | 0x10 | 0x20 | 0x21 | 0x22 | 0x23 | 0x24 | 0x3F | 0x40 => {
            read_leb128_u32(code, pos)?;
        }
        // call_indirect: type_idx + table_idx
        0x11 => {
            read_leb128_u32(code, pos)?;
            read_leb128_u32(code, pos)?;
        }
        // br_table
        0x0E => {
            let count = read_leb128_u32(code, pos)?;
            for _ in 0..=count {
                read_leb128_u32(code, pos)?;
            }
        }
        // Memory ops: align + offset
        0x28..=0x3E => {
            read_leb128_u32(code, pos)?;
            read_leb128_u32(code, pos)?;
        }
        // i32.const
        0x41 => { read_leb128_i32(code, pos)?; }
        // i64.const
        0x42 => { read_leb128_i64(code, pos)?; }
        // f32.const
        0x43 => { *pos += 4; }
        // f64.const
        0x44 => { *pos += 8; }
        // 0xFC prefix
        0xFC => {
            let sub = read_leb128_u32(code, pos)?;
            match sub {
                10 => { read_leb128_u32(code, pos)?; read_leb128_u32(code, pos)?; }
                11 => { read_leb128_u32(code, pos)?; }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

fn read_leb128_u32_at(data: &[u8], pos: &mut usize) -> Result<u32> {
    read_leb128_u32(data, pos)
}

/// Read a LEB128-encoded u32 from bytecode
fn read_leb128_u32(data: &[u8], pos: &mut usize) -> Result<u32> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    loop {
        if *pos >= data.len() {
            return Err(anyhow!("LEB128 overflow"));
        }
        let byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 35 {
            return Err(anyhow!("LEB128 too long"));
        }
    }
}

/// Read a signed LEB128-encoded i32 from bytecode
fn read_leb128_i32(data: &[u8], pos: &mut usize) -> Result<i32> {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    let mut byte: u8;
    loop {
        if *pos >= data.len() {
            return Err(anyhow!("LEB128 overflow"));
        }
        byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as i32) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
        if shift >= 35 {
            return Err(anyhow!("LEB128 too long"));
        }
    }
    // Sign extend
    if shift < 32 && (byte & 0x40) != 0 {
        result |= !0 << shift;
    }
    Ok(result)
}

/// Read a signed LEB128-encoded i64 from bytecode
fn read_leb128_i64(data: &[u8], pos: &mut usize) -> Result<i64> {
    let mut result: i64 = 0;
    let mut shift: u32 = 0;
    let mut byte: u8;
    loop {
        if *pos >= data.len() {
            return Err(anyhow!("LEB128 overflow"));
        }
        byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as i64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
        if shift >= 70 {
            return Err(anyhow!("LEB128 too long"));
        }
    }
    if shift < 64 && (byte & 0x40) != 0 {
        result |= !0i64 << shift;
    }
    Ok(result)
}

fn parse_init_expr(data: &[u8]) -> Result<(u64, usize)> {
    let mut p: usize = 0;
    let opcode = data[p];
    p += 1;
    let value = match opcode {
        0x41 => {
            let val = read_leb128_i32(data, &mut p)?;
            val as u32 as u64
        }
        0x42 => {
            let val = read_leb128_i64(data, &mut p)?;
            val as u64
        }
        0x23 => {
            let _ = read_leb128_u32(data, &mut p)?;
            0u64
        }
        _ => 0u64,
    };
    if p < data.len() && data[p] == 0x0B {
        p += 1;
    }
    Ok((value, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leb128_u32() {
        let data = vec![0x80, 0x01]; // 128
        let mut pos = 0;
        assert_eq!(read_leb128_u32(&data, &mut pos).unwrap(), 128);
        assert_eq!(pos, 2);
    }

    #[test]
    fn test_leb128_i32_negative() {
        let data = vec![0x7F]; // -1
        let mut pos = 0;
        assert_eq!(read_leb128_i32(&data, &mut pos).unwrap(), -1);
    }

    #[test]
    fn test_leb128_i64() {
        let data = vec![0xC0, 0xBB, 0x78]; // -123456
        let mut pos = 0;
        let val = read_leb128_i64(&data, &mut pos).unwrap();
        assert_eq!(val, -123456);
    }

    #[test]
    fn test_parse_wasm_types() {
        // Build a minimal WASM with type section
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);
        // Type section: 1 type, () -> ()
        w.push(1); w.push(4); // section id=1, len=4
        w.push(1); // 1 type
        w.push(0x60); w.push(0); w.push(0); // () -> ()
        // Function section
        w.push(3); w.push(2);
        w.push(1); w.push(0); // 1 function, type 0
        // Code section
        let body = vec![0x00, 0x0B]; // 0 locals, end
        w.push(10); // code section
        w.push((1 + 1 + body.len()) as u8);
        w.push(1); // 1 body
        w.push(body.len() as u8);
        w.extend_from_slice(&body);

        let info = parse_wasm_for_lowering(&w).unwrap();
        assert_eq!(info.types.len(), 1);
        assert_eq!(info.types[0].params.len(), 0);
        assert_eq!(info.types[0].results.len(), 0);
        assert_eq!(info.func_type_indices.len(), 1);
        assert_eq!(info.func_bodies.len(), 1);
    }
}
