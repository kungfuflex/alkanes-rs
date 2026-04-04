//! LLVM IR to WGSL compute shader emitter.
//!
//! Walks an optimized LLVM module (post -O2) and emits equivalent WGSL code.
//! The IR at that point is clean SSA: mem2reg has eliminated allocas,
//! functions are inlined, and values are in SSA registers that map
//! directly to WGSL `let` bindings.

use anyhow::{anyhow, Result};
use inkwell::basic_block::BasicBlock;
use inkwell::module::Module;
use inkwell::types::{AnyTypeEnum, BasicTypeEnum};
use inkwell::values::{
    AsValueRef, BasicValueEnum, FunctionValue, InstructionOpcode, InstructionValue, PhiValue,
};
use inkwell::IntPredicate;
use llvm_sys::prelude::LLVMValueRef;
use std::collections::HashMap;
use std::fmt::Write;

/// Emit a complete WGSL compute shader from an LLVM module.
///
/// This is the main entry point. It walks all functions in the module
/// and emits equivalent WGSL code with buffer bindings and a compute entry point.
pub fn emit_wgsl(module: &Module<'_>) -> Result<String> {
    let mut state = EmitState::new();
    state.emit_prelude();
    state.emit_functions(module)?;
    state.emit_entry_point();
    Ok(state.output)
}

/// Internal state for the emitter.
struct EmitState {
    output: String,
    /// Maps LLVMValueRef pointer -> WGSL variable name (reset per-function).
    value_names: HashMap<usize, String>,
    /// Counter for anonymous values.
    name_counter: usize,
}

impl EmitState {
    fn new() -> Self {
        Self {
            output: String::new(),
            value_names: HashMap::new(),
            name_counter: 0,
        }
    }

    fn emit_prelude(&mut self) {
        self.output.push_str(
            "// Auto-generated WGSL compute shader from LLVM IR\n\
             \n\
             @group(0) @binding(0) var<storage, read_write> memory: array<u32>;\n\
             @group(0) @binding(1) var<storage, read_write> kv_store: array<u32>;\n\
             \n\
             struct ExecParams {\n\
             \x20   height: u32,\n\
             \x20   fuel: u32,\n\
             \x20   message_count: u32,\n\
             }\n\
             @group(0) @binding(2) var<uniform> params: ExecParams;\n\n",
        );
    }

    fn emit_entry_point(&mut self) {
        self.output.push_str(
            "@compute @workgroup_size(64)\n\
             fn main(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
             \x20   let tid = gid.x;\n\
             \x20   if tid >= params.message_count { return; }\n\
             }\n",
        );
    }

    fn emit_functions(&mut self, module: &Module<'_>) -> Result<()> {
        let mut func = module.get_first_function();
        while let Some(f) = func {
            if f.count_basic_blocks() > 0 {
                self.emit_function(f)?;
            }
            func = f.get_next_function();
        }
        Ok(())
    }

    fn emit_function(&mut self, func: FunctionValue<'_>) -> Result<()> {
        self.value_names.clear();
        self.name_counter = 0;

        let name = func.get_name().to_str().unwrap_or("unknown");
        let fn_type = func.get_type();

        // Build parameter list (skip pointer params — memory base pointers)
        let mut params_wgsl = Vec::new();
        for i in 0..func.count_params() {
            let param = func.get_nth_param(i).unwrap();
            let param_name = format!("p{}", i);
            self.register_value(param.as_value_ref(), param_name.clone());

            let ty = param.get_type();
            if let Some(wgsl_ty) = basic_type_to_wgsl(ty) {
                params_wgsl.push(format!("{}: {}", param_name, wgsl_ty));
            }
        }

        let ret_ty = fn_type
            .get_return_type()
            .and_then(|t| basic_type_to_wgsl(t));
        let ret_str = ret_ty
            .as_ref()
            .map(|t| format!(" -> {}", t))
            .unwrap_or_default();

        writeln!(
            self.output,
            "fn {}({}){} {{",
            name,
            params_wgsl.join(", "),
            ret_str
        )
        .unwrap();

        let bbs: Vec<_> = func.get_basic_blocks();
        if bbs.len() == 1 {
            self.emit_basic_block_body(&bbs[0], "    ")?;
        } else if bbs.len() > 1 {
            self.emit_multiblock_cfg(&bbs)?;
        }

        self.output.push_str("}\n\n");
        Ok(())
    }

    fn emit_basic_block_body(&mut self, bb: &BasicBlock<'_>, indent: &str) -> Result<()> {
        let mut inst = bb.get_first_instruction();
        while let Some(i) = inst {
            self.emit_instruction(&i, indent)?;
            inst = i.get_next_instruction();
        }
        Ok(())
    }

    fn emit_multiblock_cfg(&mut self, bbs: &[BasicBlock<'_>]) -> Result<()> {
        // Map BB pointers to state IDs
        let mut bb_ids: HashMap<usize, usize> = HashMap::new();
        for (idx, bb) in bbs.iter().enumerate() {
            bb_ids.insert(bb.as_mut_ptr() as usize, idx);
        }

        // Pre-declare phi variables
        for bb in bbs {
            let mut inst = bb.get_first_instruction();
            while let Some(i) = inst {
                if i.get_opcode() == InstructionOpcode::Phi {
                    let vname = self.get_or_assign_inst_name(&i);
                    let ty = instruction_result_type(&i);
                    writeln!(self.output, "    var {}: {};", vname, ty).unwrap();
                }
                inst = i.get_next_instruction();
            }
        }

        writeln!(self.output, "    var state: u32 = 0u;").unwrap();
        writeln!(self.output, "    loop {{").unwrap();
        writeln!(self.output, "        switch state {{").unwrap();

        for (idx, bb) in bbs.iter().enumerate() {
            writeln!(self.output, "            case {}u: {{", idx).unwrap();
            let mut inst = bb.get_first_instruction();
            while let Some(i) = inst {
                self.emit_instruction_in_switch(&i, &bb_ids, "                ")?;
                inst = i.get_next_instruction();
            }
            writeln!(self.output, "            }}").unwrap();
        }

        writeln!(self.output, "            default: {{ break; }}").unwrap();
        writeln!(self.output, "        }}").unwrap();
        writeln!(self.output, "    }}").unwrap();
        Ok(())
    }

    fn emit_instruction_in_switch(
        &mut self,
        inst: &InstructionValue<'_>,
        bb_ids: &HashMap<usize, usize>,
        indent: &str,
    ) -> Result<()> {
        match inst.get_opcode() {
            InstructionOpcode::Phi => Ok(()),
            InstructionOpcode::Br => self.emit_branch(inst, bb_ids, indent),
            InstructionOpcode::Return => self.emit_return(inst, indent),
            _ => self.emit_instruction_inner(inst, indent),
        }
    }

    fn emit_instruction(&mut self, inst: &InstructionValue<'_>, indent: &str) -> Result<()> {
        match inst.get_opcode() {
            InstructionOpcode::Return => self.emit_return(inst, indent),
            InstructionOpcode::Br => Ok(()),
            _ => self.emit_instruction_inner(inst, indent),
        }
    }

    fn emit_return(&mut self, inst: &InstructionValue<'_>, indent: &str) -> Result<()> {
        if inst.get_num_operands() > 0 {
            if let Some(either::Either::Left(val)) = inst.get_operand(0) {
                let val_name = self.value_name(val);
                writeln!(self.output, "{}return {};", indent, val_name).unwrap();
                return Ok(());
            }
        }
        writeln!(self.output, "{}return;", indent).unwrap();
        Ok(())
    }

    fn emit_branch(
        &mut self,
        inst: &InstructionValue<'_>,
        bb_ids: &HashMap<usize, usize>,
        indent: &str,
    ) -> Result<()> {
        let num_ops = inst.get_num_operands();
        if num_ops == 1 {
            // Unconditional
            let target_bb = inst
                .get_operand(0)
                .and_then(|op| op.right())
                .ok_or_else(|| anyhow!("br: missing target"))?;
            self.emit_phi_assignments(inst, &target_bb, indent)?;
            let tid = bb_id(&target_bb, bb_ids);
            writeln!(self.output, "{}state = {}u;", indent, tid).unwrap();
        } else {
            // Conditional: op0=cond, op1=false_bb, op2=true_bb (inkwell order)
            let cond = inst
                .get_operand(0)
                .and_then(|op| op.left())
                .ok_or_else(|| anyhow!("br: missing condition"))?;
            let cond_name = self.value_name(cond);

            let true_bb = inst.get_operand(2).and_then(|op| op.right());
            let false_bb = inst.get_operand(1).and_then(|op| op.right());

            if let (Some(tbb), Some(fbb)) = (true_bb, false_bb) {
                let inner = format!("{}    ", indent);
                writeln!(self.output, "{}if {} {{", indent, cond_name).unwrap();
                self.emit_phi_assignments(inst, &tbb, &inner)?;
                writeln!(self.output, "{}state = {}u;", inner, bb_id(&tbb, bb_ids)).unwrap();
                writeln!(self.output, "{}}} else {{", indent).unwrap();
                self.emit_phi_assignments(inst, &fbb, &inner)?;
                writeln!(self.output, "{}state = {}u;", inner, bb_id(&fbb, bb_ids)).unwrap();
                writeln!(self.output, "{}}}", indent).unwrap();
            }
        }
        Ok(())
    }

    fn emit_phi_assignments(
        &mut self,
        branch_inst: &InstructionValue<'_>,
        target_bb: &BasicBlock<'_>,
        indent: &str,
    ) -> Result<()> {
        let source_bb = branch_inst
            .get_parent()
            .ok_or_else(|| anyhow!("branch has no parent"))?;

        let mut inst = target_bb.get_first_instruction();
        while let Some(i) = inst {
            if i.get_opcode() != InstructionOpcode::Phi {
                break;
            }
            if let Ok(phi) = PhiValue::try_from(i) {
                let phi_name = self.get_or_assign_inst_name(&i);
                for idx in 0..phi.count_incoming() {
                    if let Some((value, bb)) = phi.get_incoming(idx) {
                        if bb == source_bb {
                            let val_name = self.value_name(value);
                            writeln!(self.output, "{}{} = {};", indent, phi_name, val_name)
                                .unwrap();
                            break;
                        }
                    }
                }
            }
            inst = i.get_next_instruction();
        }
        Ok(())
    }

    fn emit_instruction_inner(
        &mut self,
        inst: &InstructionValue<'_>,
        indent: &str,
    ) -> Result<()> {
        match inst.get_opcode() {
            InstructionOpcode::Add => self.emit_binop(inst, "+", indent),
            InstructionOpcode::Sub => self.emit_binop(inst, "-", indent),
            InstructionOpcode::Mul => self.emit_binop(inst, "*", indent),
            InstructionOpcode::And => self.emit_binop(inst, "&", indent),
            InstructionOpcode::Or => self.emit_binop(inst, "|", indent),
            InstructionOpcode::Xor => self.emit_binop(inst, "^", indent),
            InstructionOpcode::Shl => self.emit_binop(inst, "<<", indent),
            InstructionOpcode::LShr => self.emit_binop(inst, ">>", indent),
            InstructionOpcode::AShr => {
                let vname = self.get_or_assign_inst_name(inst);
                let lhs = self.operand_name(inst, 0)?;
                let rhs = self.operand_name(inst, 1)?;
                writeln!(
                    self.output,
                    "{}let {} = bitcast<u32>(bitcast<i32>({}) >> bitcast<i32>({}));",
                    indent, vname, lhs, rhs
                )
                .unwrap();
                Ok(())
            }
            InstructionOpcode::UDiv => self.emit_binop(inst, "/", indent),
            InstructionOpcode::URem => self.emit_binop(inst, "%", indent),
            InstructionOpcode::SDiv => {
                let vname = self.get_or_assign_inst_name(inst);
                let lhs = self.operand_name(inst, 0)?;
                let rhs = self.operand_name(inst, 1)?;
                writeln!(
                    self.output,
                    "{}let {} = bitcast<u32>(bitcast<i32>({}) / bitcast<i32>({}));",
                    indent, vname, lhs, rhs
                )
                .unwrap();
                Ok(())
            }
            InstructionOpcode::SRem => {
                let vname = self.get_or_assign_inst_name(inst);
                let lhs = self.operand_name(inst, 0)?;
                let rhs = self.operand_name(inst, 1)?;
                writeln!(
                    self.output,
                    "{}let {} = bitcast<u32>(bitcast<i32>({}) % bitcast<i32>({}));",
                    indent, vname, lhs, rhs
                )
                .unwrap();
                Ok(())
            }
            InstructionOpcode::ICmp => self.emit_icmp(inst, indent),
            InstructionOpcode::Load => {
                let vname = self.get_or_assign_inst_name(inst);
                let ptr = self.operand_name(inst, 0)?;
                writeln!(self.output, "{}let {} = memory[{} / 4u];", indent, vname, ptr).unwrap();
                Ok(())
            }
            InstructionOpcode::Store => {
                let val = self.operand_name(inst, 0)?;
                let ptr = self.operand_name(inst, 1)?;
                writeln!(self.output, "{}memory[{} / 4u] = {};", indent, ptr, val).unwrap();
                Ok(())
            }
            InstructionOpcode::GetElementPtr => {
                let vname = self.get_or_assign_inst_name(inst);
                if inst.get_num_operands() >= 2 {
                    let base = self.operand_name(inst, 0)?;
                    let offset = self.operand_name(inst, 1)?;
                    writeln!(self.output, "{}let {} = {} + {};", indent, vname, base, offset)
                        .unwrap();
                }
                Ok(())
            }
            InstructionOpcode::Trunc => {
                let vname = self.get_or_assign_inst_name(inst);
                let src = self.operand_name(inst, 0)?;
                let mask = match inst.get_type() {
                    AnyTypeEnum::IntType(it) => {
                        let bits = it.get_bit_width();
                        if bits < 32 {
                            Some((1u64 << bits) - 1)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(m) = mask {
                    writeln!(self.output, "{}let {} = {} & {}u;", indent, vname, src, m).unwrap();
                } else {
                    writeln!(self.output, "{}let {} = {};", indent, vname, src).unwrap();
                }
                Ok(())
            }
            InstructionOpcode::ZExt
            | InstructionOpcode::BitCast
            | InstructionOpcode::IntToPtr
            | InstructionOpcode::PtrToInt
            | InstructionOpcode::Freeze => {
                let vname = self.get_or_assign_inst_name(inst);
                let src = self.operand_name(inst, 0)?;
                writeln!(self.output, "{}let {} = {};", indent, vname, src).unwrap();
                Ok(())
            }
            InstructionOpcode::SExt => {
                let vname = self.get_or_assign_inst_name(inst);
                let src = self.operand_name(inst, 0)?;
                if let Some(either::Either::Left(val)) = inst.get_operand(0) {
                    if let BasicTypeEnum::IntType(it) = val.get_type() {
                        let bits = it.get_bit_width();
                        if bits == 1 {
                            writeln!(
                                self.output,
                                "{}let {} = select(0u, 0xFFFFFFFFu, {});",
                                indent, vname, src
                            )
                            .unwrap();
                            return Ok(());
                        } else if bits < 32 {
                            let shift = 32 - bits;
                            writeln!(
                                self.output,
                                "{}let {} = bitcast<u32>(bitcast<i32>({} << {}u) >> {}u);",
                                indent, vname, src, shift, shift
                            )
                            .unwrap();
                            return Ok(());
                        }
                    }
                }
                writeln!(self.output, "{}let {} = {};", indent, vname, src).unwrap();
                Ok(())
            }
            InstructionOpcode::Select => {
                let vname = self.get_or_assign_inst_name(inst);
                let cond = self.operand_name(inst, 0)?;
                let true_val = self.operand_name(inst, 1)?;
                let false_val = self.operand_name(inst, 2)?;
                writeln!(
                    self.output,
                    "{}let {} = select({}, {}, {});",
                    indent, vname, false_val, true_val, cond
                )
                .unwrap();
                Ok(())
            }
            InstructionOpcode::Call => self.emit_call(inst, indent),
            InstructionOpcode::Alloca => {
                let vname = self.get_or_assign_inst_name(inst);
                writeln!(self.output, "{}// alloca -> {}", indent, vname).unwrap();
                Ok(())
            }
            InstructionOpcode::Unreachable => {
                writeln!(self.output, "{}// unreachable", indent).unwrap();
                Ok(())
            }
            other => {
                writeln!(self.output, "{}// TODO: unhandled {:?}", indent, other).unwrap();
                Ok(())
            }
        }
    }

    fn emit_binop(
        &mut self,
        inst: &InstructionValue<'_>,
        op: &str,
        indent: &str,
    ) -> Result<()> {
        let vname = self.get_or_assign_inst_name(inst);
        let lhs = self.operand_name(inst, 0)?;
        let rhs = self.operand_name(inst, 1)?;
        writeln!(self.output, "{}let {} = {} {} {};", indent, vname, lhs, op, rhs).unwrap();
        Ok(())
    }

    fn emit_icmp(&mut self, inst: &InstructionValue<'_>, indent: &str) -> Result<()> {
        let vname = self.get_or_assign_inst_name(inst);
        let lhs = self.operand_name(inst, 0)?;
        let rhs = self.operand_name(inst, 1)?;
        let pred = inst.get_icmp_predicate();
        match pred {
            Some(IntPredicate::EQ) => writeln!(self.output, "{}let {} = {} == {};", indent, vname, lhs, rhs),
            Some(IntPredicate::NE) => writeln!(self.output, "{}let {} = {} != {};", indent, vname, lhs, rhs),
            Some(IntPredicate::UGT) => writeln!(self.output, "{}let {} = {} > {};", indent, vname, lhs, rhs),
            Some(IntPredicate::UGE) => writeln!(self.output, "{}let {} = {} >= {};", indent, vname, lhs, rhs),
            Some(IntPredicate::ULT) => writeln!(self.output, "{}let {} = {} < {};", indent, vname, lhs, rhs),
            Some(IntPredicate::ULE) => writeln!(self.output, "{}let {} = {} <= {};", indent, vname, lhs, rhs),
            Some(IntPredicate::SGT) => writeln!(self.output, "{}let {} = bitcast<i32>({}) > bitcast<i32>({});", indent, vname, lhs, rhs),
            Some(IntPredicate::SGE) => writeln!(self.output, "{}let {} = bitcast<i32>({}) >= bitcast<i32>({});", indent, vname, lhs, rhs),
            Some(IntPredicate::SLT) => writeln!(self.output, "{}let {} = bitcast<i32>({}) < bitcast<i32>({});", indent, vname, lhs, rhs),
            Some(IntPredicate::SLE) => writeln!(self.output, "{}let {} = bitcast<i32>({}) <= bitcast<i32>({});", indent, vname, lhs, rhs),
            None => writeln!(self.output, "{}// icmp: unknown predicate", indent),
        }.unwrap();
        Ok(())
    }

    fn emit_call(&mut self, inst: &InstructionValue<'_>, indent: &str) -> Result<()> {
        let num_ops = inst.get_num_operands();
        let callee_name = if num_ops > 0 {
            if let Some(either::Either::Left(val)) = inst.get_operand(num_ops - 1) {
                let name = unsafe {
                    let vref = val.as_value_ref();
                    let mut len: usize = 0;
                    let cname = llvm_sys::core::LLVMGetValueName2(vref, &mut len);
                    if cname.is_null() || len == 0 {
                        "unknown_func".to_string()
                    } else {
                        let slice = std::slice::from_raw_parts(cname as *const u8, len);
                        String::from_utf8_lossy(slice).to_string()
                    }
                };
                if name.is_empty() {
                    "unknown_func".to_string()
                } else {
                    name
                }
            } else {
                "unknown_func".to_string()
            }
        } else {
            "unknown_func".to_string()
        };

        let arg_count = if num_ops > 0 { num_ops - 1 } else { 0 };
        let mut args = Vec::new();
        for i in 0..arg_count {
            if let Ok(name) = self.operand_name(inst, i) {
                args.push(name);
            }
        }

        let has_return = !matches!(inst.get_type(), AnyTypeEnum::VoidType(_));
        if has_return {
            let vname = self.get_or_assign_inst_name(inst);
            writeln!(
                self.output,
                "{}let {} = {}({});",
                indent, vname, callee_name, args.join(", ")
            )
            .unwrap();
        } else {
            writeln!(
                self.output,
                "{}{}({});",
                indent, callee_name, args.join(", ")
            )
            .unwrap();
        }
        Ok(())
    }

    // =========================================================================
    // Value naming
    // =========================================================================

    fn register_value(&mut self, vref: LLVMValueRef, name: String) {
        self.value_names.insert(vref as usize, name);
    }

    fn get_or_assign_inst_name(&mut self, inst: &InstructionValue<'_>) -> String {
        let key = inst.as_value_ref() as usize;
        if let Some(name) = self.value_names.get(&key) {
            return name.clone();
        }

        let name_opt = inst.get_name().and_then(|n| n.to_str().ok());
        let wgsl_name = if let Some(n) = name_opt {
            if !n.is_empty() {
                format!("v_{}", sanitize_name(n))
            } else {
                let n = format!("v_{}", self.name_counter);
                self.name_counter += 1;
                n
            }
        } else {
            let n = format!("v_{}", self.name_counter);
            self.name_counter += 1;
            n
        };

        self.value_names.insert(key, wgsl_name.clone());
        wgsl_name
    }

    fn value_name(&self, val: BasicValueEnum<'_>) -> String {
        // Constant integers inline
        if let BasicValueEnum::IntValue(iv) = val {
            if let Some(c) = iv.get_zero_extended_constant() {
                if iv.get_type().get_bit_width() == 1 {
                    return if c == 0 {
                        "false".to_string()
                    } else {
                        "true".to_string()
                    };
                }
                return format!("{}u", c);
            }
        }

        let key = val.as_value_ref() as usize;
        if let Some(name) = self.value_names.get(&key) {
            return name.clone();
        }

        format!("v_unknown_{}", key & 0xFFFF)
    }

    fn operand_name(&mut self, inst: &InstructionValue<'_>, idx: u32) -> Result<String> {
        let op = inst
            .get_operand(idx)
            .ok_or_else(|| anyhow!("missing operand {}", idx))?;
        match op {
            either::Either::Left(val) => Ok(self.value_name(val)),
            either::Either::Right(_bb) => Ok("/* bb_ref */".to_string()),
        }
    }
}

// =========================================================================
// Free helpers
// =========================================================================

fn bb_id(bb: &BasicBlock<'_>, bb_ids: &HashMap<usize, usize>) -> usize {
    bb_ids.get(&(bb.as_mut_ptr() as usize)).copied().unwrap_or(0)
}

fn basic_type_to_wgsl(ty: BasicTypeEnum<'_>) -> Option<String> {
    match ty {
        BasicTypeEnum::IntType(it) => {
            let bits = it.get_bit_width();
            match bits {
                1 => Some("bool".to_string()),
                8 | 16 | 32 => Some("u32".to_string()),
                64 => Some("vec2<u32>".to_string()),
                _ => Some("u32".to_string()),
            }
        }
        BasicTypeEnum::PointerType(_) => None,
        BasicTypeEnum::FloatType(_) => Some("f32".to_string()),
        _ => None,
    }
}

fn instruction_result_type(inst: &InstructionValue<'_>) -> String {
    match inst.get_type() {
        AnyTypeEnum::IntType(it) => {
            if it.get_bit_width() == 1 {
                "bool".to_string()
            } else {
                "u32".to_string()
            }
        }
        _ => "u32".to_string(),
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;

    #[test]
    fn test_simple_add() {
        let context = Context::create();
        let module = context.create_module("test");
        let builder = context.create_builder();
        let i32_type = context.i32_type();

        let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
        let function = module.add_function("simple_add", fn_type, None);

        let entry = context.append_basic_block(function, "entry");
        builder.position_at_end(entry);

        let a = function.get_nth_param(0).unwrap().into_int_value();
        let b = function.get_nth_param(1).unwrap().into_int_value();
        a.set_name("a");
        b.set_name("b");

        let sum = builder.build_int_add(a, b, "sum").unwrap();
        builder.build_return(Some(&sum)).unwrap();

        let wgsl = emit_wgsl(&module).expect("emit failed");
        println!("=== Simple Add WGSL ===\n{}", wgsl);

        assert!(wgsl.contains("fn simple_add"));
        assert!(wgsl.contains("+"));
        assert!(wgsl.contains("return"));
    }

    #[test]
    fn test_arithmetic_ops() {
        let context = Context::create();
        let module = context.create_module("test");
        let builder = context.create_builder();
        let i32_type = context.i32_type();

        let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
        let function = module.add_function("arith", fn_type, None);

        let entry = context.append_basic_block(function, "entry");
        builder.position_at_end(entry);

        let a = function.get_nth_param(0).unwrap().into_int_value();
        let b = function.get_nth_param(1).unwrap().into_int_value();

        let sum = builder.build_int_add(a, b, "sum").unwrap();
        let diff = builder.build_int_sub(sum, b, "diff").unwrap();
        let prod = builder.build_int_mul(diff, a, "prod").unwrap();
        let masked = builder
            .build_and(prod, i32_type.const_int(0xFF, false), "masked")
            .unwrap();
        builder.build_return(Some(&masked)).unwrap();

        let wgsl = emit_wgsl(&module).expect("emit failed");
        println!("=== Arithmetic WGSL ===\n{}", wgsl);

        assert!(wgsl.contains("+"));
        assert!(wgsl.contains("-"));
        assert!(wgsl.contains("*"));
        assert!(wgsl.contains("&"));
    }

    #[test]
    fn test_conditional_branch() {
        let context = Context::create();
        let module = context.create_module("test");
        let builder = context.create_builder();
        let i32_type = context.i32_type();

        let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
        let function = module.add_function("max_val", fn_type, None);

        let entry = context.append_basic_block(function, "entry");
        let then_bb = context.append_basic_block(function, "then");
        let else_bb = context.append_basic_block(function, "else");
        let merge_bb = context.append_basic_block(function, "merge");

        builder.position_at_end(entry);
        let a = function.get_nth_param(0).unwrap().into_int_value();
        let b = function.get_nth_param(1).unwrap().into_int_value();
        let cmp = builder
            .build_int_compare(IntPredicate::UGT, a, b, "cmp")
            .unwrap();
        builder
            .build_conditional_branch(cmp, then_bb, else_bb)
            .unwrap();

        builder.position_at_end(then_bb);
        builder.build_unconditional_branch(merge_bb).unwrap();

        builder.position_at_end(else_bb);
        builder.build_unconditional_branch(merge_bb).unwrap();

        builder.position_at_end(merge_bb);
        let phi = builder.build_phi(i32_type, "result").unwrap();
        phi.add_incoming(&[(&a, then_bb), (&b, else_bb)]);
        builder
            .build_return(Some(&phi.as_basic_value()))
            .unwrap();

        let wgsl = emit_wgsl(&module).expect("emit failed");
        println!("=== Conditional WGSL ===\n{}", wgsl);

        assert!(wgsl.contains("switch state"));
        assert!(wgsl.contains("if "));
        assert!(wgsl.contains("return"));
    }
}
