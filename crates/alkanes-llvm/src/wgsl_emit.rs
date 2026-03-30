//! LLVM IR → WGSL compute-shader emitter
//!
//! Walks an optimized LLVM IR module (via inkwell) and produces WGSL source
//! suitable for a `@compute` shader that operates on a flat u32 memory buffer.
//!
//! Limitations (v1):
//! - Only i32 / i64 integer types (no floats)
//! - Multi-block functions use a loop+switch "relooper" pattern
//! - Memory modelled as `array<u32>` addressed in 4-byte granularity

use anyhow::Result;
use inkwell::module::Module;
use inkwell::values::{
    AnyValue, BasicValueEnum, FunctionValue, InstructionOpcode, InstructionValue,
};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;


/// Compute functions reachable from entry within max_depth call levels.
fn reachable_functions(
    call_graph: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    entry: &str,
    max_depth: usize,
) -> std::collections::HashSet<String> {
    let mut reachable = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
    queue.push_back((entry.to_string(), 0));
    reachable.insert(entry.to_string());
    while let Some((func, depth)) = queue.pop_front() {
        if depth >= max_depth { continue; }
        if let Some(callees) = call_graph.get(&func) {
            for callee in callees {
                if reachable.insert(callee.clone()) {
                    queue.push_back((callee.clone(), depth + 1));
                }
            }
        }
    }
    reachable
}

/// Build a call graph: for each defined function, collect the set of functions it calls.
fn build_call_graph(module: &Module) -> HashMap<String, HashSet<String>> {
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
    let mut func = module.get_first_function();
    while let Some(f) = func {
        if f.get_first_basic_block().is_some() {
            let fname = f.get_name().to_str().unwrap_or("unknown").to_string();
            let callees = graph.entry(fname).or_default();
            // Walk all instructions looking for calls
            let mut bb = f.get_first_basic_block();
            while let Some(b) = bb {
                let mut inst = b.get_first_instruction();
                while let Some(i) = inst {
                    if i.get_opcode() == InstructionOpcode::Call {
                        let inst_str = i.print_to_string().to_string();
                        if let Some(target) = extract_call_target(&inst_str) {
                            if !target.starts_with("llvm.") {
                                callees.insert(target);
                            }
                        }
                    }
                    inst = i.get_next_instruction();
                }
                bb = b.get_next_basic_block();
            }
        }
        func = f.get_next_function();
    }
    graph
}

/// Find all strongly connected components using Tarjan's algorithm.
/// Returns SCCs as vectors of function names. SCCs with size > 1 contain mutual recursion.
fn find_sccs(graph: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    struct TarjanState<'a> {
        graph: &'a HashMap<String, HashSet<String>>,
        index_counter: usize,
        stack: Vec<String>,
        on_stack: HashSet<String>,
        index: HashMap<String, usize>,
        lowlink: HashMap<String, usize>,
        sccs: Vec<Vec<String>>,
    }

    fn strongconnect(state: &mut TarjanState, v: &str) {
        let idx = state.index_counter;
        state.index_counter += 1;
        state.index.insert(v.to_string(), idx);
        state.lowlink.insert(v.to_string(), idx);
        state.stack.push(v.to_string());
        state.on_stack.insert(v.to_string());

        if let Some(callees) = state.graph.get(v) {
            for w in callees.iter() {
                if !state.index.contains_key(w.as_str()) {
                    // w has not been visited; recurse
                    strongconnect(state, w);
                    let w_low = state.lowlink[w.as_str()];
                    let v_low = state.lowlink[v];
                    if w_low < v_low {
                        state.lowlink.insert(v.to_string(), w_low);
                    }
                } else if state.on_stack.contains(w.as_str()) {
                    let w_idx = state.index[w.as_str()];
                    let v_low = state.lowlink[v];
                    if w_idx < v_low {
                        state.lowlink.insert(v.to_string(), w_idx);
                    }
                }
            }
        }

        // If v is a root node, pop the SCC
        if state.lowlink[v] == state.index[v] {
            let mut scc = Vec::new();
            loop {
                let w = state.stack.pop().unwrap();
                state.on_stack.remove(&w);
                scc.push(w.clone());
                if w == v {
                    break;
                }
            }
            scc.reverse();
            state.sccs.push(scc);
        }
    }

    let mut state = TarjanState {
        graph,
        index_counter: 0,
        stack: Vec::new(),
        on_stack: HashSet::new(),
        index: HashMap::new(),
        lowlink: HashMap::new(),
        sccs: Vec::new(),
    };

    let nodes: Vec<String> = graph.keys().cloned().collect();
    for node in &nodes {
        if !state.index.contains_key(node.as_str()) {
            strongconnect(&mut state, node);
        }
    }

    state.sccs
}

/// Emit WGSL source code from an LLVM module.
///
/// The module should already be optimized (`-O2`).
pub fn emit_wgsl(module: &Module) -> Result<String> {
    let mut out = String::with_capacity(8192);

    // ── preamble ──────────────────────────────────────────────────────
    writeln!(out, "// Auto-generated WGSL from alkanes-llvm")?;
    writeln!(out, "@group(0) @binding(0) var<storage, read_write> memory: array<u32>;")?;
    writeln!(out, "@group(0) @binding(1) var<storage, read_write> kv_store: array<u32>;")?;
    writeln!(out, "struct ExecParams {{ height: u32, fuel: u32, message_count: u32 }}")?;
    writeln!(out, "@group(0) @binding(2) var<uniform> params: ExecParams;
const v_0: u32 = 0u;
")?;

    // Emit LLVM global variables as WGSL private vars
    let mut gv = module.get_first_global();
    while let Some(g) = gv {
        let name = g.get_name().to_str().unwrap_or("unknown");
        let clean = format!("g_{}", sanitize_name(name));
        // Try to get initial value
        if let Some(init) = g.get_initializer() {
            if let Ok(iv) = init.try_into() {
                let iv: inkwell::values::IntValue = iv;
                if let Some(val) = iv.get_zero_extended_constant() {
                    writeln!(out, "var<private> {}: u32 = {}u;", clean, val as u32)?;
                } else {
                    writeln!(out, "var<private> {}: u32 = 0u;", clean)?;
                }
            } else {
                writeln!(out, "var<private> {}: u32 = 0u; // non-int", clean)?;
            }
        } else {
            writeln!(out, "var<private> {}: u32 = 0u;", clean)?;
        }
        gv = g.get_next_global();
    }
    writeln!(out, "")?;
    writeln!(out)?;

    // ── detect and stub mutually-recursive functions ──────────────────
    let call_graph = build_call_graph(module);
    let sccs = find_sccs(&call_graph);
    let mut stubbed_funcs: HashSet<String> = HashSet::new();
    for scc in &sccs {
        if scc.len() > 1 {
            // Mutual recursion detected — stub all functions in this SCC
            eprintln!(
                "[wgsl_emit] Stubbing {} mutually-recursive functions: {:?}",
                scc.len(),
                scc
            );
            for name in scc {
                stubbed_funcs.insert(name.clone());
            }
        }
    }

    let total_funcs = {
        let mut count = 0;
        let mut f = module.get_first_function();
        while let Some(ff) = f { if ff.get_first_basic_block().is_some() { count += 1; } f = ff.get_next_function(); }
        count
    };
    // ── prune unreachable functions ──────────────────────────────────
    let reachable = reachable_functions(&call_graph, "__execute", 2);
    let do_pruning = total_funcs > 20; // only prune large modules
    // Also add host function names as reachable (they're called from reachable funcs)
    let mut pruned_funcs: std::collections::HashSet<String> = std::collections::HashSet::new();
    if do_pruning {
        let mut func = module.get_first_function();
        while let Some(f) = func {
            let name = f.get_name().to_str().unwrap_or("unknown");
            if f.get_first_basic_block().is_some() && !reachable.contains(name) {
                pruned_funcs.insert(name.to_string());
            }
            func = f.get_next_function();
        }
    }

    eprintln!("[wgsl_emit] Reachable from __execute (depth 4): {} / {} functions, pruning {}",
        reachable.len(), total_funcs, pruned_funcs.len());
    // Merge pruned into stubbed
    for name in &pruned_funcs {
        stubbed_funcs.insert(name.clone());
    }

    // ── host function stubs ──────────────────────────────────────────
    out.push_str("// Host function stubs\n");
    out.push_str("fn _request_context() -> u32 { return 0u; }\n");
    out.push_str("fn _load_context(dest: u32) -> u32 { return 0u; }\n");
    out.push_str("fn _request_storage(key_ptr: u32) -> u32 { return 0u; }\n");
    out.push_str("fn _load_storage(key_ptr: u32, dest: u32) -> u32 { return 0u; }\n");
    out.push_str("fn _height(dest: u32) -> u32 { return params.height; }\n");
    out.push_str("fn _balance(who: u32, what: u32, out_ptr: u32) {}\n");
    out.push_str("fn _sequence() -> u32 { return 0u; }\n");
    out.push_str("fn _fuel() -> u32 { return params.fuel; }\n");
    out.push_str("fn abort(a: u32, b: u32, c: u32, d: u32) {}\n");
    out.push_str("fn _log(log_ptr: u32) {}\n");
    out.push_str("\n");

    // ── translate every function ──────────────────────────────────────
    let mut func = module.get_first_function();
    while let Some(f) = func {
        // Skip declarations (no body) — these are imports
        if f.get_first_basic_block().is_some() {
            let name = f.get_name().to_str().unwrap_or("unknown");
            if stubbed_funcs.contains(name) {
                emit_stub_function(&f, &mut out)?;
            } else {
                let pre_len = out.len();
                emit_function(&f, &mut out)?;
                // Post-process: in multi-block functions, the pre-declared vars
                // conflict with "let" declarations in case blocks.
                // Replace "let v_" and "let t_" with assignment-only syntax.
                let func_body = &out[pre_len..];
                if func_body.contains("loop {") {
                    // This is a multi-block function with pre-declared vars
                    let fixed = out[pre_len..]
                        .replace("let v_", "v_")
                        .replace("let t_", "t_")
                        // Strip type annotations from assignments (var decls already have types)
                        .replace(": u32 = ", " = ")
                        .replace(": bool = ", " = ");
                    out.truncate(pre_len);
                    out.push_str(&fixed);
                }
            }
            writeln!(out)?;
        }
        func = f.get_next_function();
    }

    // ── compute entry point ──────────────────────────────────────────
    writeln!(out, "@compute @workgroup_size(64)")?;
    writeln!(out, "fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{")?;
    writeln!(out, "    let tid = gid.x;")?;
    writeln!(out, "    if tid >= params.message_count {{ return; }}")?;
    writeln!(out, "    _execute(tid);")?;
    writeln!(out, "}}")?;

    Ok(out)
}

// ── stub emitter for mutually-recursive functions ───────────────────

fn emit_stub_function(func: &FunctionValue, out: &mut String) -> Result<()> {
    let name = func.get_name().to_str().unwrap_or("unknown");
    let ret_type = func.get_type().get_return_type();
    let ret_wgsl = match ret_type {
        Some(t) => wgsl_type_for_basic_type(&t),
        None => "",
    };

    let namer = Namer::new();
    let param_count = func.count_params();
    let wasm_param_start = if param_count >= 2 { 2u32 } else { 0u32 };
    for idx in wasm_param_start..param_count {
        let param = func.get_nth_param(idx).unwrap();
        namer.register_param(&param, idx);
    }

    write!(out, "fn {}(", sanitize_name(name))?;
    let mut first = true;
    for idx in wasm_param_start..param_count {
        if !first { write!(out, ", ")?; }
        first = false;
        let param = func.get_nth_param(idx).unwrap();
        let pname = namer.name_for_param(idx);
        let pty = wgsl_type_for_basic_value(&param);
        write!(out, "{}: {}", pname, pty)?;
    }

    if ret_wgsl.is_empty() {
        writeln!(out, ") {{")?;
        writeln!(out, "    // stub: mutually-recursive function")?;
        writeln!(out, "    return;")?;
    } else {
        writeln!(out, ") -> {} {{", ret_wgsl)?;
        writeln!(out, "    // stub: mutually-recursive function")?;
        let default_val = match ret_wgsl {
            "bool" => "false",
            _ => "0u",
        };
        writeln!(out, "    return {};", default_val)?;
    }
    writeln!(out, "}}")?;
    Ok(())
}

// ── per-function emitter ─────────────────────────────────────────────

fn emit_function(func: &FunctionValue, out: &mut String) -> Result<()> {
    let name = func.get_name().to_str().unwrap_or("unknown");

    // Collect basic blocks
    let mut blocks: Vec<inkwell::basic_block::BasicBlock> = Vec::new();
    let mut bb = func.get_first_basic_block();
    while let Some(b) = bb {
        blocks.push(b);
        bb = b.get_next_basic_block();
    }

    // Map block → integer id (for relooper switch)
    let mut block_id: HashMap<inkwell::basic_block::BasicBlock, usize> = HashMap::new();
    for (i, b) in blocks.iter().enumerate() {
        block_id.insert(*b, i);
    }

    // Build a name map: instruction pointer-as-string → WGSL variable name
    let namer = Namer::new();

    // Pre-scan for phi nodes — these need `var` declarations
    let mut phi_vars: Vec<(String, &str)> = Vec::new(); // (var_name, wgsl_type)
    for b in &blocks {
        let mut inst = b.get_first_instruction();
        while let Some(i) = inst {
            if i.get_opcode() == InstructionOpcode::Phi {
                let vname = namer.name_for_inst(&i);
                let wty = wgsl_type_for_inst(&i);
                phi_vars.push((vname, wty));
            }
            inst = i.get_next_instruction();
        }
    }

    // For multi-block functions, pre-declare ALL instruction results as vars
    // This ensures any value can be accessed from any case block
    if blocks.len() > 1 {
        for bb in &blocks {
            let mut inst_iter = bb.get_first_instruction();
            while let Some(i) = inst_iter {
                let vname = namer.name_for_inst(&i);
                if (vname.starts_with("v_") || vname.starts_with("t_"))
                    && !phi_vars.iter().any(|(n, _)| n == &vname)
                {
                    phi_vars.push((vname, "u32"));
                }
                inst_iter = i.get_next_instruction();
            }
        }
    }
    if false && blocks.len() > 1 {
        // Build: for each instruction, which block defines it?
        let mut def_block: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (bi, b) in blocks.iter().enumerate() {
            let mut inst = b.get_first_instruction();
            while let Some(i) = inst {
                let vname = namer.name_for_inst(&i);
                def_block.insert(vname, bi);
                inst = i.get_next_instruction();
            }
        }
        // Check: for each instruction, are any of its operands defined in a different block?
        for (bi, b) in blocks.iter().enumerate() {
            let mut inst = b.get_first_instruction();
            while let Some(i) = inst {
                let num_ops = i.get_num_operands();
                for op_idx in 0..num_ops {
                    if let Some(op) = i.get_operand(op_idx) {
                        if let either::Either::Left(val) = op {
                            let op_name = value_to_wgsl(&val, &namer);
                            if let Some(&def_bi) = def_block.get(&op_name) {
                                if def_bi != bi && (op_name.starts_with("v_") || op_name.starts_with("t_")) {
                                    let wty = "u32";
                                    let entry = (op_name.clone(), wty);
                                    if !phi_vars.contains(&entry) {
                                        phi_vars.push(entry);
                                    }
                                }
                            }
                            if false {
                            }
                        }
                    }
                }
                inst = i.get_next_instruction();
            }
        }
    }

    // Deduplicate pre-declared vars by name
    {
        let mut seen = std::collections::HashSet::new();
        phi_vars.retain(|(name, _)| seen.insert(name.clone()));
    }

    // ── function signature ────────────────────────────────────────────
    let ret_type = func.get_type().get_return_type();
    let ret_wgsl = match ret_type {
        Some(t) => wgsl_type_for_basic_type(&t),
        None => "",
    };

    write!(out, "fn {}(", sanitize_name(name))?;
    let param_count = func.count_params();
    // First two params are memory/kv_store pointers — in WGSL these become implicit globals.
    // Remaining params are WASM-level values.
    let wasm_param_start = if param_count >= 2 { 2u32 } else { 0u32 };
    // Pre-register params so operand resolution finds the right names
    for idx in wasm_param_start..param_count {
        let param = func.get_nth_param(idx).unwrap();
        namer.register_param(&param, idx);
    }
    let mut first = true;
    for idx in wasm_param_start..param_count {
        if !first { write!(out, ", ")?; }
        first = false;
        let param = func.get_nth_param(idx).unwrap();
        let pname = namer.name_for_param(idx);
        let pty = wgsl_type_for_basic_value(&param);
        write!(out, "{}: {}", pname, pty)?;
    }
    if ret_wgsl.is_empty() {
        writeln!(out, ") {{")?;
    } else {
        writeln!(out, ") -> {} {{", ret_wgsl)?;
    }

    // Phi variable declarations
    let predeclared: std::collections::HashSet<String> = phi_vars.iter().map(|(n, _)| n.clone()).collect();
    for (vname, wty) in &phi_vars {
        writeln!(out, "    var {}: {};", vname, wty)?;
    }

    let single_block = blocks.len() == 1;

    if !single_block {
        writeln!(out, "    var state: u32 = 0u;")?;
        writeln!(out, "    loop {{")?;
        writeln!(out, "        switch state {{")?;
    }

    for (bi, blk) in blocks.iter().enumerate() {
        if !single_block {
            writeln!(out, "            case {}u {{", bi)?;
        }
        let indent = if single_block { "    " } else { "                " };

        let mut inst = blk.get_first_instruction();
        while let Some(i) = inst {
            emit_instruction(&i, &namer, &block_id, indent, out, single_block, func)?;
            inst = i.get_next_instruction();
        }

        if !single_block {
            writeln!(out, "            }}")?;
        }
    }

    if !single_block {
        writeln!(out, "            default {{")?;
        writeln!(out, "                break;")?;
        writeln!(out, "            }}")?;
        writeln!(out, "        }}")?;
        // break from loop after switch falls through default
        writeln!(out, "        break;")?;
        writeln!(out, "    }}")?;
    }

    // Add default return for functions that return u32
    // (ensures all paths return a value, but skip if last stmt is already return)
    let ret_ty = func.get_type().get_return_type();
    if ret_ty.is_some() {
        let last_line = out.lines().last().unwrap_or("");
        if !last_line.trim().starts_with("return") {
            out.push_str("    return 0u;
");
        }
    }
    writeln!(out, "}}")?;
    Ok(())
}

// ── instruction emitter ──────────────────────────────────────────────

fn emit_instruction(
    inst: &InstructionValue,
    namer: &Namer,
    block_id: &HashMap<inkwell::basic_block::BasicBlock, usize>,
    indent: &str,
    out: &mut String,
    single_block: bool,
    _func: &FunctionValue,
) -> Result<()> {
    let opcode = inst.get_opcode();
    match opcode {
        // ── arithmetic ───────────────────────────────────────────
        InstructionOpcode::Add => emit_binop(inst, namer, "+", indent, out),
        InstructionOpcode::Sub => emit_binop(inst, namer, "-", indent, out),
        InstructionOpcode::Mul => emit_binop(inst, namer, "*", indent, out),
        InstructionOpcode::And => emit_binop(inst, namer, "&", indent, out),
        InstructionOpcode::Or => emit_binop(inst, namer, "|", indent, out),
        InstructionOpcode::Xor => emit_binop(inst, namer, "^", indent, out),
        InstructionOpcode::Shl => emit_shift(inst, namer, "<<", indent, out),
        InstructionOpcode::LShr => emit_shift(inst, namer, ">>", indent, out),
        InstructionOpcode::AShr => {
            // Arithmetic shift right: cast to signed, shift, cast back
            let lhs = operand_name(inst, 0, namer);
            let rhs = operand_name(inst, 1, namer);
            let dst = namer.name_for_inst(inst);
            let wty = wgsl_type_for_inst(inst);
            let sty = if wty == "u32" { "i32" } else { "i64" };
            writeln!(out, "{}let {} = {}({}({}) >> {});", indent, dst, wty, sty, lhs, rhs)?;
            Ok(())
        }
        InstructionOpcode::UDiv => emit_binop(inst, namer, "/", indent, out),
        InstructionOpcode::URem => emit_binop(inst, namer, "%", indent, out),
        InstructionOpcode::SDiv => {
            let lhs = operand_name(inst, 0, namer);
            let rhs = operand_name(inst, 1, namer);
            let dst = namer.name_for_inst(inst);
            let wty = wgsl_type_for_inst(inst);
            let sty = if wty == "u32" { "i32" } else { "i64" };
            writeln!(out, "{}let {} = {}({}({}) / {}({}));", indent, dst, wty, sty, lhs, sty, rhs)?;
            Ok(())
        }
        InstructionOpcode::SRem => {
            let lhs = operand_name(inst, 0, namer);
            let rhs = operand_name(inst, 1, namer);
            let dst = namer.name_for_inst(inst);
            let wty = wgsl_type_for_inst(inst);
            let sty = if wty == "u32" { "i32" } else { "i64" };
            writeln!(out, "{}let {} = {}({}({}) % {}({}));", indent, dst, wty, sty, lhs, sty, rhs)?;
            Ok(())
        }

        // ── comparisons ──────────────────────────────────────────
        InstructionOpcode::ICmp => {
            let dst = namer.name_for_inst(inst);
            let lhs = operand_name(inst, 0, namer);
            let rhs = operand_name(inst, 1, namer);
            let pred = extract_icmp_predicate(inst);
            let op = match pred.as_str() {
                "eq" => "==",
                "ne" => "!=",
                "ugt" => ">",
                "uge" => ">=",
                "ult" => "<",
                "ule" => "<=",
                "sgt" => ">",
                "sge" => ">=",
                "slt" => "<",
                "sle" => "<=",
                _ => "==",
            };
            // ICmp: produce u32 (0 or 1) to avoid bool/u32 type mismatches
            writeln!(out, "{}let {} = u32({} {} {});", indent, dst, lhs, op, rhs)?;
            Ok(())
        }

        // ── casts ────────────────────────────────────────────────
        InstructionOpcode::Trunc => {
            let dst = namer.name_for_inst(inst);
            let src = operand_name(inst, 0, namer);
            let wty = wgsl_type_for_inst(inst);
            if wty == "u32" {
                writeln!(out, "{}let {} = {} & 0xFFFFFFFFu;", indent, dst, src)?;
            } else {
                writeln!(out, "{}let {} = {}({});", indent, dst, wty, src)?;
            }
            Ok(())
        }
        InstructionOpcode::ZExt => {
            let dst = namer.name_for_inst(inst);
            let src = operand_name(inst, 0, namer);
            let wty = wgsl_type_for_inst(inst);
            writeln!(out, "{}let {}: {} = {}({});", indent, dst, wty, wty, src)?;
            Ok(())
        }
        InstructionOpcode::SExt => {
            let dst = namer.name_for_inst(inst);
            let src = operand_name(inst, 0, namer);
            let wty = wgsl_type_for_inst(inst);
            // sign-extend: cast to signed first, then to unsigned target
            let _src_sty = "i32";
            let dst_sty = if wty == "u32" { "i32" } else { "i64" };
            writeln!(out, "{}let {} = {}({}({}));", indent, dst, wty, dst_sty, src)?;
            Ok(())
        }
        InstructionOpcode::BitCast | InstructionOpcode::IntToPtr | InstructionOpcode::PtrToInt => {
            let dst = namer.name_for_inst(inst);
            let src = operand_name(inst, 0, namer);
            writeln!(out, "{}let {} = {};", indent, dst, src)?;
            Ok(())
        }

        // ── memory ───────────────────────────────────────────────
        InstructionOpcode::GetElementPtr => {
            let dst = namer.name_for_inst(inst);
            let base = operand_name(inst, 0, namer);
            if inst.get_num_operands() >= 2 {
                let offset = operand_name(inst, 1, namer);
                writeln!(out, "{}let {} = {} + {};", indent, dst, base, offset)?;
            } else {
                writeln!(out, "{}let {} = {};", indent, dst, base)?;
            }
            Ok(())
        }
        InstructionOpcode::Load => {
            let dst = namer.name_for_inst(inst);
            let ptr = operand_name(inst, 0, namer);
            writeln!(out, "{}let {} = memory[{} / 4u];", indent, dst, ptr)?;
            Ok(())
        }
        InstructionOpcode::Store => {
            let val = operand_name(inst, 0, namer);
            let ptr = operand_name(inst, 1, namer);
            writeln!(out, "{}memory[{} / 4u] = u32({});", indent, ptr, val)?;
            Ok(())
        }
        InstructionOpcode::Alloca => {
            // Stack allocation becomes an offset in the memory buffer (simplified)
            let dst = namer.name_for_inst(inst);
            writeln!(out, "{}let {} = 0u; // alloca stub", indent, dst)?;
            Ok(())
        }

        // ── control flow ─────────────────────────────────────────
        InstructionOpcode::Br => {
            let num_ops = inst.get_num_operands();
            if num_ops == 1 {
                // Unconditional branch
                if let Some(target_bb) = get_branch_target_bb(inst, 0) {
                    if let Some(&tid) = block_id.get(&target_bb) {
                        if single_block {
                            // single block: unconditional branch is just fallthrough
                        } else {
                            writeln!(out, "{}state = {}u;", indent, tid)?;
                            writeln!(out, "{}continue;", indent)?;
                        }
                    }
                }
            } else {
                // Conditional: br i1 %cond, label %true, label %false
                // In LLVM IR: operand 0 = condition, but in the raw representation
                // the order might vary. We parse from the print representation.
                let cond = operand_name(inst, 0, namer);
                let true_bb = get_branch_target_bb(inst, 1);
                let false_bb = get_branch_target_bb(inst, 2);
                if let (Some(tbb), Some(fbb)) = (true_bb, false_bb) {
                    let tid = block_id.get(&tbb).copied().unwrap_or(0);
                    let fid = block_id.get(&fbb).copied().unwrap_or(0);
                    if single_block {
                        writeln!(out, "{}// conditional branch (simplified)", indent)?;
                    } else {
                        writeln!(out, "{}if ({} != 0u) {{", indent, cond)?;
                        writeln!(out, "{}    state = {}u;", indent, tid)?;
                        writeln!(out, "{}}} else {{", indent)?;
                        writeln!(out, "{}    state = {}u;", indent, fid)?;
                        writeln!(out, "{}}}", indent)?;
                        writeln!(out, "{}continue;", indent)?;
                    }
                }
            }
            Ok(())
        }
        InstructionOpcode::Return => {
            if inst.get_num_operands() == 0 {
                // Check if function returns a value
                if _func.get_type().get_return_type().is_some() {
                    writeln!(out, "{}return 0u;", indent)?;
                } else {
                    writeln!(out, "{}return;", indent)?;
                }
            } else {
                let val = operand_name(inst, 0, namer);
                writeln!(out, "{}return u32({});", indent, val)?;
            }
            Ok(())
        }
        InstructionOpcode::Switch => {
            let cond = operand_name(inst, 0, namer);
            writeln!(out, "{}// switch on {}", indent, cond)?;
            // Default target is operand 1
            if let Some(default_bb) = get_branch_target_bb(inst, 1) {
                let did = block_id.get(&default_bb).copied().unwrap_or(0);
                writeln!(out, "{}state = {}u; // default", indent, did)?;
            }
            if !single_block {
                writeln!(out, "{}continue;", indent)?;
            }
            Ok(())
        }
        InstructionOpcode::Unreachable => {
            writeln!(out, "{}// unreachable", indent)?;
            if _func.get_type().get_return_type().is_some() {
                writeln!(out, "{}return 0u;", indent)?;
            } else {
                writeln!(out, "{}return;", indent)?;
            }
            Ok(())
        }

        // ── select ───────────────────────────────────────────────
        InstructionOpcode::Select => {
            let dst = namer.name_for_inst(inst);
            let cond = operand_name(inst, 0, namer);
            let true_val = operand_name(inst, 1, namer);
            let false_val = operand_name(inst, 2, namer);
            // WGSL select requires reject/accept to have same type
            // Cast both to u32 to handle bool/u32 mismatches
            let s = inst.print_to_string().to_string();
            // Cast reject/accept to u32, but keep condition as bool
            // If condition is u32 (from our ICmp wrapping), convert to bool
            writeln!(out, "{}let {} = select(u32({}), u32({}), ({} != 0u));", indent, dst, false_val, true_val, cond)?;
            Ok(())
        }

        // ── call ─────────────────────────────────────────────────
        InstructionOpcode::Call => {
            emit_call(inst, namer, indent, out)
        }

        // ── phi ──────────────────────────────────────────────────
        InstructionOpcode::Phi => {
            // Phi assignments are handled at branch sites in a full relooper.
            // For now, emit a comment. The var was declared at function top.
            let dst = namer.name_for_inst(inst);
            writeln!(out, "{}// phi: {} assigned at predecessors", indent, dst)?;
            Ok(())
        }

        // ── catch-all ────────────────────────────────────────────
        _ => {
            let dst = namer.name_for_inst(inst);
            writeln!(out, "{}let {} = 0u; // TODO: {:?}", indent, dst, opcode)?;
            Ok(())
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn emit_binop(
    inst: &InstructionValue,
    namer: &Namer,
    op: &str,
    indent: &str,
    out: &mut String,
) -> Result<()> {
    let dst = namer.name_for_inst(inst);
    let lhs = operand_name(inst, 0, namer);
    let rhs = operand_name(inst, 1, namer);
    writeln!(out, "{}let {} = {} {} {};", indent, dst, lhs, op, rhs)?;
    Ok(())
}

fn emit_shift(
    inst: &InstructionValue,
    namer: &Namer,
    op: &str,
    indent: &str,
    out: &mut String,
) -> Result<()> {
    let dst = namer.name_for_inst(inst);
    let lhs = operand_name(inst, 0, namer);
    let rhs = operand_name(inst, 1, namer);
    // Mask shift amount to 31 to avoid WGSL validation errors
    writeln!(out, "{}let {} = {} {} ({} & 31u);", indent, dst, lhs, op, rhs)?;
    Ok(())
}

fn emit_call(
    inst: &InstructionValue,
    namer: &Namer,
    indent: &str,
    out: &mut String,
) -> Result<()> {
    // The called function is the last operand in inkwell
    let num_ops = inst.get_num_operands();
    if num_ops == 0 {
        writeln!(out, "{}// empty call", indent)?;
        return Ok(());
    }

    // Try to extract the function name from the instruction's string repr
    let inst_str = inst.print_to_string().to_string();
    let callee_name = extract_call_target(&inst_str).unwrap_or_else(|| "unknown_fn".to_string());
    // Handle LLVM intrinsics
    if callee_name.starts_with("llvm.") || callee_name.contains("llvm.") {
        // Count leading zeros
        if callee_name.contains("ctlz") {
            let arg = if num_ops > 0 { operand_name(inst, 0, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = countLeadingZeros({});", result_name, arg)?;
            return Ok(());
        }
        // Count trailing zeros
        if callee_name.contains("cttz") {
            let arg = if num_ops > 0 { operand_name(inst, 0, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = countTrailingZeros({});", result_name, arg)?;
            return Ok(());
        }
        // Population count (ctpop)
        if callee_name.contains("ctpop") {
            let arg = if num_ops > 0 { operand_name(inst, 0, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = countOneBits({});", result_name, arg)?;
            return Ok(());
        }
        // Byte swap (bswap)
        if callee_name.contains("bswap") {
            let arg = if num_ops > 0 { operand_name(inst, 0, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = reverseBits({});", result_name, arg)?;
            return Ok(());
        }
        // abs
        if callee_name.contains("abs") {
            let arg = if num_ops > 0 { operand_name(inst, 0, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = abs({});", result_name, arg)?;
            return Ok(());
        }
        // memcpy/memmove/memset — skip (data initialization handled separately)
        if callee_name.contains("memcpy") || callee_name.contains("memmove") || callee_name.contains("memset") || callee_name.contains("lifetime") {
            writeln!(out, "    // intrinsic: {} (skipped)", callee_name)?;
            return Ok(());
        }
        // Funnel shift left (rotate): fshl(a, b, c) = (a << c) | (b >> (32-c))
        if callee_name.contains("fshl") {
            let a = operand_name(inst, 0, namer);
            let b = if num_ops > 1 { operand_name(inst, 1, namer) } else { a.clone() };
            let amt = if num_ops > 2 { operand_name(inst, 2, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            // WGSL doesn't have native rotate, implement as shift+or
            writeln!(out, "    let {} = ({} << ({} & 31u)) | ({} >> ((32u - ({} & 31u)) & 31u));", result_name, a, amt, b, amt)?;
            return Ok(());
        }
        // Funnel shift right: fshr(a, b, c) = (a >> c) | (b << (32-c))
        if callee_name.contains("fshr") {
            let a = operand_name(inst, 0, namer);
            let b = if num_ops > 1 { operand_name(inst, 1, namer) } else { a.clone() };
            let amt = if num_ops > 2 { operand_name(inst, 2, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = ({} >> ({} & 31u)) | ({} << ((32u - ({} & 31u)) & 31u));", result_name, a, amt, b, amt)?;
            return Ok(());
        }
        // umax(a, b) = max(a, b)
        if callee_name.contains("umax") {
            let a = operand_name(inst, 0, namer);
            let b = if num_ops > 1 { operand_name(inst, 1, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = max({}, {});", result_name, a, b)?;
            return Ok(());
        }
        // umin(a, b) = min(a, b)
        if callee_name.contains("umin") {
            let a = operand_name(inst, 0, namer);
            let b = if num_ops > 1 { operand_name(inst, 1, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = min({}, {});", result_name, a, b)?;
            return Ok(());
        }
        // usub.sat(a, b) = saturating subtract
        if callee_name.contains("usub.sat") {
            let a = operand_name(inst, 0, namer);
            let b = if num_ops > 1 { operand_name(inst, 1, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = select({} - {}, 0u, {} < {});", result_name, a, b, a, b)?;
            return Ok(());
        }
        // uadd.sat(a, b) = saturating add
        if callee_name.contains("uadd.sat") {
            let a = operand_name(inst, 0, namer);
            let b = if num_ops > 1 { operand_name(inst, 1, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = select({} + {}, 0xFFFFFFFFu, ({} + {}) < {});", result_name, a, b, a, b, a)?;
            return Ok(());
        }
        // umul.with.overflow — just do the multiply, ignore overflow flag
        if callee_name.contains("umul.with.overflow") || callee_name.contains("uadd.with.overflow") {
            let a = operand_name(inst, 0, namer);
            let b = if num_ops > 1 { operand_name(inst, 1, namer) } else { "0u".to_string() };
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = {} * {};", result_name, a, b)?;
            return Ok(());
        }
        // vector.reduce.add — sum of vector elements, just return first arg
        if callee_name.contains("vector.reduce") {
            let a = operand_name(inst, 0, namer);
            let result_name = namer.name_for_inst(inst);
            writeln!(out, "    let {} = {};", result_name, a)?;
            return Ok(());
        }
        // assume — no-op
        if callee_name.contains("assume") {
            writeln!(out, "    // assume (no-op)")?;
            return Ok(());
        }
        // Other intrinsics — skip with warning
        writeln!(out, "    // TODO intrinsic: {} (skipped)", callee_name)?;
        return Ok(());
    }
    let callee_clean = sanitize_name(&callee_name);

    // Gather argument operands (skip the last operand which is the callee)
    let arg_count = if num_ops > 0 { num_ops - 1 } else { 0 };
    let mut args = Vec::new();
    // Skip first two args (memory, kv_store pointers)
    let skip = if arg_count >= 2 { 2 } else { 0 };
    for i in skip..arg_count {
        args.push(operand_name(inst, i, namer));
    }

    // Check if call has a return value:
    // 1. "call void" means no return
    // 2. Any other call type (call i32, call ptr, etc.) has a return value
    let is_void_call = inst_str.contains("call void ") 
        || inst_str.contains("call void(");
    let has_result = !is_void_call;

    if has_result {
        let dst = namer.name_for_inst(inst);
        writeln!(out, "{}let {} = {}({});", indent, dst, callee_clean, args.join(", "))?;
    } else {
        writeln!(out, "{}{}({});", indent, callee_clean, args.join(", "))?;
    }
    Ok(())
}

/// Extract the called function name from an LLVM call instruction string
fn extract_call_target(inst_str: &str) -> Option<String> {
    // Patterns: "call i32 @funcname(" or "call void @funcname("
    // Find the first @name( after "call" — that's the callee
    // The key: look for @name( where name doesn't contain spaces/commas
    if let Some(call_pos) = inst_str.find("call ") {
        let after_call = &inst_str[call_pos..];
        // Find first @ that's followed by a function name (before the opening paren)
        if let Some(at_pos) = after_call.find('@') {
            let after_at = &after_call[at_pos + 1..];
            let end = after_at.find('(').unwrap_or(after_at.len());
            let name = after_at[..end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Extract the ICmp predicate from instruction text
fn extract_icmp_predicate(inst: &InstructionValue) -> String {
    let s = inst.print_to_string().to_string();
    // Format: "icmp eq i32 %a, %b" or "icmp ult ..."
    for kw in &["eq", "ne", "ugt", "uge", "ult", "ule", "sgt", "sge", "slt", "sle"] {
        let pattern = format!("icmp {} ", kw);
        if s.contains(&pattern) {
            return kw.to_string();
        }
    }
    "eq".to_string()
}

/// Get a basic block target from a branch instruction operand index
fn get_branch_target_bb<'ctx>(
    inst: &InstructionValue<'ctx>,
    operand_idx: u32,
) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
    // inkwell: for br instructions, operands that are basic blocks
    // appear as operands. We can parse from the instruction text.
    // Actually, let's use the LLVM API through print_to_string parsing.
    // This is ugly but reliable with inkwell's limited branch API.

    // Alternative: count successors
    let num_ops = inst.get_num_operands();
    if operand_idx >= num_ops {
        return None;
    }

    // For unconditional branch: operand 0 is the BB
    // For conditional branch: operand 0 is cond, operands 1,2 are BBs
    // But inkwell's get_operand returns BasicValueEnum, not BB.
    // We need to parse the text.
    let s = inst.print_to_string().to_string();

    match inst.get_opcode() {
        InstructionOpcode::Br => {
            // Parse "br label %bbname" or "br i1 %cond, label %true, label %false"
            let labels: Vec<&str> = s.split("label %").skip(1).collect();
            let target_idx = if labels.len() == 1 {
                // unconditional
                0usize
            } else {
                // conditional: operand_idx 1 = true (first label), 2 = false (second label)
                if operand_idx <= 1 { 0 } else { 1 }
            };
            if target_idx < labels.len() {
                let bb_name = labels[target_idx].split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.').next()?;
                // Find the BB in the parent function
                let func = inst.get_parent()?.get_parent()?;
                let mut bb = func.get_first_basic_block();
                while let Some(b) = bb {
                    if b.get_name().to_str().unwrap_or("") == bb_name {
                        return Some(b);
                    }
                    bb = b.get_next_basic_block();
                }
            }
            None
        }
        _ => None,
    }
}

/// Produce a WGSL name for an LLVM operand
fn operand_name(inst: &InstructionValue, idx: u32, namer: &Namer) -> String {
    if let Some(op) = inst.get_operand(idx) {
        match op {
            either::Either::Left(val) => value_to_wgsl(&val, namer),
            either::Either::Right(_bb) => format!("bb_{}", idx),
        }
    } else {
        format!("undef_{}", idx)
    }
}

/// Convert a BasicValueEnum to a WGSL expression string
fn value_to_wgsl(val: &BasicValueEnum, namer: &Namer) -> String {
    match val {
        BasicValueEnum::IntValue(iv) => {
            // Check namer cache first (handles params registered by register_param)
            let dbg_key = iv.print_to_string().to_string();
            if let Some(cached) = namer.cache.borrow().get(&dbg_key) {
                return cached.clone();
            }
            // Check if it's a constant
            if let Some(c) = iv.get_zero_extended_constant() {
                let bits = iv.get_type().get_bit_width();
                if bits <= 32 {
                    format!("{}u", c as u32)
                } else {
                    // WGSL doesn't have native u64; use u32 with a comment
                    format!("{}u /* i64 */", c as u32)
                }
            } else {
                // It's an SSA value — look up its name
                // Use the instruction that defines it
                if let Some(inst) = iv.as_instruction() {
                    namer.name_for_inst(&inst)
                } else {
                    // Could be a function parameter
                    let s = iv.print_to_string().to_string();
                    if s.contains("@") {
                        // Global variable reference
                        if let Some(at_pos) = s.find('@') {
                            let after_at = &s[at_pos + 1..];
                            let end = after_at.find(|c: char| c == ' ' || c == '=' || c == ',').unwrap_or(after_at.len());
                            format!("g_{}", sanitize_llvm_name(&after_at[..end]))
                        } else { "0u".to_string() }
                    } else {
                        sanitize_llvm_name(&s)
                    }
                }
            }
        }
        BasicValueEnum::PointerValue(pv) => {
            if let Some(inst) = pv.as_instruction() {
                namer.name_for_inst(&inst)
            } else if pv.is_null() {
                return "0u".to_string();
            } else if pv.is_const() {
                // Could be a global variable or null pointer
                let s = pv.print_to_string().to_string();
                if s.contains("@") {
                    // Extract the global name: @name = ...
                    if let Some(at_pos) = s.find('@') {
                        let after_at = &s[at_pos + 1..];
                        let end = after_at.find(|c: char| c == ' ' || c == '=' || c == ',').unwrap_or(after_at.len());
                        let name = &after_at[..end];
                        // Return as a WGSL global reference (index into memory)
                        format!("g_{}", sanitize_name(name))
                    } else {
                        "0u".to_string() // fallback
                    }
                } else if s.contains("null") {
                    "0u".to_string()
                } else {
                    sanitize_llvm_name(&s)
                }
            } else {
                let s = pv.print_to_string().to_string();
                sanitize_llvm_name(&s)
            }
        }
        _ => {
            let s = val.print_to_string().to_string();
            sanitize_llvm_name(&s)
        }
    }
}

/// Determine the WGSL type for an instruction's result
fn wgsl_type_for_inst(_inst: &InstructionValue) -> &'static str {
    "u32"
}

fn wgsl_type_for_basic_type(ty: &inkwell::types::BasicTypeEnum) -> &'static str {
    match ty {
        inkwell::types::BasicTypeEnum::IntType(it) => {
            match it.get_bit_width() {
                1 => "bool",
                64 => "u32", // flatten i64 → u32 for WGSL v1
                _ => "u32",
            }
        }
        inkwell::types::BasicTypeEnum::PointerType(_) => "u32",
        _ => "u32",
    }
}

fn wgsl_type_for_basic_value(val: &BasicValueEnum) -> &'static str {
    match val {
        BasicValueEnum::IntValue(iv) => {
            match iv.get_type().get_bit_width() {
                1 => "bool",
                64 => "u32",
                _ => "u32",
            }
        }
        BasicValueEnum::PointerValue(_) => "u32",
        _ => "u32",
    }
}

fn sanitize_name(name: &str) -> String {
    let mut s = name.replace('.', "_").replace('-', "_");
    // WGSL reserves identifiers starting with __
    while s.starts_with("__") {
        s = s[1..].to_string(); // strip one underscore
    }
    s
}

fn sanitize_llvm_name(s: &str) -> String {
    // Handle LLVM special constants
    if s.contains("zeroinitializer") || s.contains("<") {
        return "0u".to_string();
    }
    if s.contains("undef") || s.contains("poison") {
        return "0u".to_string();
    }
    let s = s.trim();
    // Strip type prefixes like "i32 ", "ptr "
    let s = if let Some(rest) = s.strip_prefix("i32 ") { rest }
    else if let Some(rest) = s.strip_prefix("i64 ") { rest }
    else if let Some(rest) = s.strip_prefix("ptr ") { rest }
    else if let Some(rest) = s.strip_prefix("i1 ") { rest }
    else { s };
    let s = s.trim();

    // If it starts with %, it's an SSA name
    if let Some(name) = s.strip_prefix('%') {
        return format!("v_{}", sanitize_name(name));
    }
    // If it's a numeric constant
    if s.parse::<i64>().is_ok() || s.parse::<u64>().is_ok() {
        return format!("{}u", s);
    }
    format!("v_{}", sanitize_name(s))
}

// ── name generator ───────────────────────────────────────────────────

struct Namer {
    // We use instruction address (as usize from print_to_string hash) → name
    counter: std::cell::RefCell<u32>,
    cache: std::cell::RefCell<HashMap<String, String>>,
}

impl Namer {
    fn new() -> Self {
        Self {
            counter: std::cell::RefCell::new(0),
            cache: std::cell::RefCell::new(HashMap::new()),
        }
    }

    fn name_for_inst(&self, inst: &InstructionValue) -> String {
        let key = inst.print_to_string().to_string();
        let mut cache = self.cache.borrow_mut();
        if let Some(name) = cache.get(&key) {
            return name.clone();
        }
        // Try to use the LLVM SSA name
        let name_str = inst.get_name().map(|c| c.to_str().unwrap_or("").to_string());
        let wgsl_name = if let Some(ref n) = name_str {
            if !n.is_empty() {
                format!("v_{}", sanitize_name(n))
            } else {
                let mut c = self.counter.borrow_mut();
                *c += 1;
                format!("t_{}", *c)
            }
        } else {
            let mut c = self.counter.borrow_mut();
            *c += 1;
            format!("t_{}", *c)
        };
        cache.insert(key, wgsl_name.clone());
        wgsl_name
    }

    fn name_for_param(&self, idx: u32) -> String {
        format!("p_{}", idx)
    }

    /// Pre-register a function parameter so operand lookups resolve correctly.
    fn register_param(&self, param: &BasicValueEnum, idx: u32) {
        let name = self.name_for_param(idx);
        let key1 = param.print_to_string().to_string();
        self.cache.borrow_mut().insert(key1, name.clone());
        // Also register the inner value's print representation (may differ)
        match param {
            BasicValueEnum::IntValue(iv) => {
                let key2 = iv.print_to_string().to_string();
                self.cache.borrow_mut().insert(key2, name);
            }
            BasicValueEnum::PointerValue(pv) => {
                let key2 = pv.print_to_string().to_string();
                self.cache.borrow_mut().insert(key2, name);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::WasmToSpirv;

    /// Build a WASM module: (i32, i32) -> i32 that does a + b
    fn arithmetic_wasm() -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        // Type section: (i32, i32) -> i32
        w.push(1);
        w.push(7);
        w.push(1);
        w.push(0x60);
        w.push(2); w.push(0x7F); w.push(0x7F);
        w.push(1); w.push(0x7F);

        // Function section
        w.push(3); w.push(2);
        w.push(1); w.push(0);

        // Export "execute"
        w.push(7);
        let name = b"execute";
        let export_len = 1 + 1 + name.len() + 1 + 1;
        w.push(export_len as u8);
        w.push(1);
        w.push(name.len() as u8);
        w.extend_from_slice(name);
        w.push(0x00);
        w.push(0);

        // Code: local.get 0, local.get 1, i32.add
        let body = vec![0x00, 0x20, 0x00, 0x20, 0x01, 0x6A, 0x0B];
        let code_len = 1 + 1 + body.len();
        w.push(10);
        w.push(code_len as u8);
        w.push(1);
        w.push(body.len() as u8);
        w.extend_from_slice(&body);
        w
    }

    #[test]
    fn test_arithmetic_wgsl() {
        let _ = env_logger::try_init();
        let compiler = WasmToSpirv::new();
        let wasm = arithmetic_wasm();
        let result = compiler.compile(&wasm, (1, 1)).unwrap();

        // Parse the optimized IR back through inkwell to get a Module
        // We need the module object, so re-compile and emit WGSL
        let wgsl = compiler.compile_to_wgsl(&wasm, (1, 1)).unwrap();

        println!("=== WGSL Output ===");
        println!("{}", wgsl);

        assert!(wgsl.contains("fn "), "WGSL should contain function declarations");
        assert!(wgsl.contains("+"), "WGSL should contain addition operator");
        assert!(wgsl.contains("memory"), "WGSL should reference memory buffer");
        assert!(wgsl.contains("@compute"), "WGSL should have compute entry point");
    }

    #[test]
    fn test_minimal_wgsl() {
        let _ = env_logger::try_init();
        let compiler = WasmToSpirv::new();

        // Minimal: () -> ()
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);
        w.push(1); w.extend_from_slice(&[4]); w.push(1); w.push(0x60); w.push(0); w.push(0);
        w.push(3); w.extend_from_slice(&[2]); w.push(1); w.push(0);
        let name = b"_start";
        w.push(7);
        let el = 1 + 1 + name.len() + 1 + 1;
        w.push(el as u8); w.push(1); w.push(name.len() as u8);
        w.extend_from_slice(name); w.push(0x00); w.push(0);
        let body = vec![0x00, 0x0B];
        let cl = 1 + 1 + body.len();
        w.push(10); w.push(cl as u8); w.push(1); w.push(body.len() as u8);
        w.extend_from_slice(&body);

        let wgsl = compiler.compile_to_wgsl(&w, (0, 0)).unwrap();
        println!("=== Minimal WGSL ===");
        println!("{}", wgsl);

        assert!(wgsl.contains("@compute"));
        assert!(wgsl.contains("fn _execute"));
    }
}
