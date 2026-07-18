//! Bitcoin Script types, interpreter, and verification engine.
//!
//! Maps to: `src/script/` in Bitcoin Core.
//!
//! This crate implements the complete Bitcoin Script subsystem:
//!
//! - [`Opcode`] -- enumeration of all opcodes (`OP_0`..`OP_NOP10`, `OP_CHECKSIGADD`).
//! - [`Script`] -- byte-vector wrapper with builder helpers and classification methods
//!   (P2PKH, P2SH, P2WPKH, P2WSH, P2TR detection, etc.).
//! - [`ScriptNum`] -- consensus-critical signed integer type with minimal encoding rules.
//! - [`ScriptError`] -- exhaustive set of script verification error codes.
//! - [`ScriptVerifyFlags`] -- bitflag set controlling which consensus / relay rules
//!   are enforced (`P2SH`, `DERSIG`, `WITNESS`, `TAPROOT`, and more).
//! - [`eval_script`] / [`verify_script`] -- the stack-based virtual machine and
//!   top-level script verification entry point.
//! - [`SignatureChecker`] trait -- abstraction for ECDSA and Schnorr signature
//!   verification during script evaluation.

/// Bitcoin Script interpreter (stack VM, `eval_script`, `verify_script`).
pub mod interpreter;
/// Complete enumeration of Bitcoin Script opcodes.
pub mod opcode;
/// The `Script` byte-vector type and standard script builders.
pub mod script;
/// Script verification error codes.
pub mod script_error;
/// Consensus-critical script number type (`CScriptNum`).
pub mod script_num;
/// Script verification flag constants (`SCRIPT_VERIFY_*`).
pub mod verify_flags;

pub use interpreter::{
    cast_to_bool, eval_script, eval_script_simple, verify_script, BaseSignatureChecker,
    ScriptExecutionData, ScriptStack, ScriptWitness, SigVersion, SignatureChecker,
};
pub use opcode::Opcode;
pub use script::{
    build_op_return, build_p2pkh, build_p2sh, build_p2tr, build_p2wpkh, build_p2wsh, Script,
    ScriptOpsIter, LOCKTIME_THRESHOLD, MAX_OPS_PER_SCRIPT, MAX_PUBKEYS_PER_MULTISIG,
    MAX_SCRIPT_ELEMENT_SIZE, MAX_SCRIPT_SIZE, MAX_STACK_SIZE,
};
pub use script_error::ScriptError;
pub use script_num::ScriptNum;
pub use verify_flags::ScriptVerifyFlags;
