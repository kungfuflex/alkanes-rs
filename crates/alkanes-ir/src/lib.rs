#![no_std]
#![cfg_attr(not(target_arch = "spirv"), feature(error_in_core))]

#[cfg(not(target_arch = "spirv"))]
extern crate alloc;
#[cfg(all(feature = "std", not(target_arch = "spirv")))]
extern crate std;

#[macro_use]
mod for_each_op;
mod r#enum;
mod error;
mod immeditate;
pub mod index;
mod primitive;
mod span;
mod visit_regs;

#[cfg(test)]
mod tests;

use wasmi_core as core;

#[doc(inline)]
pub use self::{
    error::Error,
    immeditate::{AnyConst16, AnyConst32, Const16, Const32},
    index::Reg,
    primitive::{
        Address,
        Address32,
        BlockFuel,
        BranchOffset,
        BranchOffset16,
        Comparator,
        ComparatorAndOffset,
        IntoShiftAmount,
        Offset16,
        Offset64,
        Offset64Hi,
        Offset64Lo,
        Offset8,
        ShiftAmount,
        Sign,
    },
    r#enum::Instruction,
    span::{BoundedRegSpan, FixedRegSpan, RegSpan, RegSpanIter},
    visit_regs::VisitRegs,
};
