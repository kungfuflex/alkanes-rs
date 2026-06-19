//! Bitcoin Script interpreter.
//! Maps to: src/script/interpreter.cpp in Bitcoin Core
//!
//! This is the most consensus-critical code in Bitcoin. It implements
//! the stack-based virtual machine that evaluates Script programs.

use crate::opcode::Opcode;
use crate::script::{
    Script, MAX_PUBKEYS_PER_MULTISIG, MAX_SCRIPT_ELEMENT_SIZE, MAX_STACK_SIZE,
    VALIDATION_WEIGHT_PER_SIGOP_PASSED,
};
use crate::script_error::ScriptError;
use crate::script_num::{ScriptNum, DEFAULT_MAX_NUM_SIZE};
use crate::verify_flags::ScriptVerifyFlags;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a Bitcoin compact-size integer to a `Vec<u8>`.
fn write_compact_size_to_vec(buf: &mut Vec<u8>, n: u64) {
    if n < 253 {
        buf.push(n as u8);
    } else if n <= 0xffff {
        buf.push(253);
        buf.extend_from_slice(&(n as u16).to_le_bytes());
    } else if n <= 0xffff_ffff {
        buf.push(254);
        buf.extend_from_slice(&(n as u32).to_le_bytes());
    } else {
        buf.push(255);
        buf.extend_from_slice(&n.to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum script length in bytes for pre-tapscript execution (10,000).
///
/// Tapscript (BIP 342) removes this limit.
pub const MAX_SCRIPT_SIZE: usize = 10_000;

/// Maximum number of non-push operations allowed per script (201).
pub const MAX_OPS_PER_SCRIPT: u32 = 201;

/// BIP 68 sequence locktime disable flag (bit 31).
///
/// When set on an input's `nSequence`, relative lock-time is not enforced.
pub const SEQUENCE_LOCKTIME_DISABLE_FLAG: u32 = 1 << 31;

/// BIP 68 sequence locktime type flag (bit 22).
///
/// When set, the relative lock-time is interpreted as 512-second intervals
/// rather than block heights.
pub const SEQUENCE_LOCKTIME_TYPE_FLAG: u32 = 1 << 22;

/// BIP 68 sequence locktime mask -- the lower 16 bits of `nSequence`.
///
/// Extracts the relative lock-time value (either blocks or time intervals).
pub const SEQUENCE_LOCKTIME_MASK: u32 = 0x0000ffff;

// Taproot constants (BIP 341)
/// The annex tag byte (0x50).
const ANNEX_TAG: u8 = 0x50;
/// Mask for extracting leaf version from control block byte 0.
const TAPROOT_LEAF_MASK: u8 = 0xfe;
/// Leaf version for tapscript (BIP 342).
const TAPROOT_LEAF_TAPSCRIPT: u8 = 0xc0;
/// Size of the control block base (1 byte leaf version + 32 byte internal key).
const TAPROOT_CONTROL_BASE_SIZE: usize = 33;
/// Size of each merkle path node in the control block.
const TAPROOT_CONTROL_NODE_SIZE: usize = 32;
/// Maximum number of merkle path nodes.
const TAPROOT_CONTROL_MAX_NODE_COUNT: usize = 128;
/// Maximum control block size.
const TAPROOT_CONTROL_MAX_SIZE: usize =
    TAPROOT_CONTROL_BASE_SIZE + TAPROOT_CONTROL_NODE_SIZE * TAPROOT_CONTROL_MAX_NODE_COUNT;
/// Offset added to witness serialized size for validation weight budget.
const VALIDATION_WEIGHT_OFFSET: i64 = 50;

// ---------------------------------------------------------------------------
// Stack type
// ---------------------------------------------------------------------------

/// A single value on the script stack -- an arbitrary-length byte vector.
pub type StackValue = Vec<u8>;

/// The script execution stack.
///
/// Wraps a `Vec<StackValue>` and provides Bitcoin Core-compatible access
/// patterns including negative-offset indexing from the top (e.g. -1 = top).
#[derive(Clone, Debug)]
pub struct ScriptStack {
    /// The underlying stack storage; index 0 is the bottom.
    stack: Vec<StackValue>,
}

impl ScriptStack {
    /// Creates an empty script stack.
    pub fn new() -> Self {
        ScriptStack { stack: Vec::new() }
    }

    /// Pushes a value onto the top of the stack.
    pub fn push(&mut self, val: StackValue) {
        self.stack.push(val);
    }

    /// Pops and returns the top value, or returns `ScriptError::InvalidStackOperation` if empty.
    pub fn pop(&mut self) -> Result<StackValue, ScriptError> {
        self.stack.pop().ok_or(ScriptError::InvalidStackOperation)
    }

    /// Access element relative to the top. offset=-1 is top, -2 is second, etc.
    /// This matches Bitcoin Core's `stacktop(i)` macro.
    pub fn top(&self, offset: isize) -> Result<&StackValue, ScriptError> {
        let idx = (self.stack.len() as isize) + offset;
        if idx < 0 || idx >= self.stack.len() as isize {
            return Err(ScriptError::InvalidStackOperation);
        }
        Ok(&self.stack[idx as usize])
    }

    /// Mutable access to element relative to the top.
    pub fn top_mut(&mut self, offset: isize) -> Result<&mut StackValue, ScriptError> {
        let len = self.stack.len() as isize;
        let idx = len + offset;
        if idx < 0 || idx >= len {
            return Err(ScriptError::InvalidStackOperation);
        }
        Ok(&mut self.stack[idx as usize])
    }

    /// Returns the number of elements on the stack.
    pub fn size(&self) -> usize {
        self.stack.len()
    }

    /// Returns `true` if the stack contains no elements.
    pub fn empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Erase elements in range [first, last).
    pub fn erase(&mut self, first: usize, last: usize) {
        self.stack.drain(first..last);
    }

    /// Insert value at the given absolute position.
    pub fn insert(&mut self, position: usize, val: StackValue) {
        self.stack.insert(position, val);
    }

    /// Swap two elements by absolute index.
    pub fn swap(&mut self, a: usize, b: usize) {
        self.stack.swap(a, b);
    }

    /// Swap two elements by negative offset from end. -1 = top, -2 = second from top.
    fn swap_top(&mut self, a: isize, b: isize) {
        let len = self.stack.len();
        let ia = (len as isize + a) as usize;
        let ib = (len as isize + b) as usize;
        self.stack.swap(ia, ib);
    }

    /// Remove element at negative offset from end.
    fn erase_top(&mut self, offset: isize) {
        let idx = (self.stack.len() as isize + offset) as usize;
        self.stack.remove(idx);
    }

    /// Remove range relative to end. erase_top_range(-6, -4) removes [end-6, end-4).
    fn erase_top_range(&mut self, from: isize, to: isize) {
        let len = self.stack.len();
        let f = (len as isize + from) as usize;
        let t = (len as isize + to) as usize;
        self.stack.drain(f..t);
    }

    /// Insert at position relative to end.
    fn insert_top(&mut self, offset: isize, val: StackValue) {
        let idx = (self.stack.len() as isize + offset) as usize;
        self.stack.insert(idx, val);
    }

    /// Get the underlying vec (for verify_script stack transfer).
    pub fn into_vec(self) -> Vec<StackValue> {
        self.stack
    }

    /// Create from a vec.
    pub fn from_vec(v: Vec<StackValue>) -> Self {
        ScriptStack { stack: v }
    }

    /// Get reference to backing vec.
    pub fn as_vec(&self) -> &Vec<StackValue> {
        &self.stack
    }

    /// Back (top of stack).
    pub fn back(&self) -> Result<&StackValue, ScriptError> {
        self.stack.last().ok_or(ScriptError::InvalidStackOperation)
    }
}

impl Default for ScriptStack {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// cast_to_bool - Consensus-critical boolean interpretation
// ---------------------------------------------------------------------------

/// Interpret a byte vector as a boolean (Bitcoin consensus rules).
/// False is zero or negative zero (0x80 in last byte with rest zero).
pub fn cast_to_bool(vch: &[u8]) -> bool {
    for (i, byte) in vch.iter().enumerate() {
        if *byte != 0 {
            // Negative zero
            if i == vch.len() - 1 && *byte == 0x80 {
                return false;
            }
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// SigVersion & ScriptExecutionData
// ---------------------------------------------------------------------------

/// Signature version controlling which hashing and validation rules apply.
///
/// Port of Bitcoin Core's `SigVersion` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigVersion {
    /// Pre-segwit scripts. Signature hashing uses the original algorithm.
    Base,
    /// Segwit v0 scripts (BIP 143). Uses the BIP 143 signature hash algorithm.
    WitnessV0,
    /// Taproot key-path spending (BIP 341). Uses the BIP 341 signature hash.
    Taproot,
    /// Tapscript (BIP 342). Uses BIP 342 signature hashing with leaf hash.
    Tapscript,
}

/// Mutable data carried through tapscript execution for signature checking.
///
/// Port of Bitcoin Core's `ScriptExecutionData`. Fields are lazily initialized
/// (the `*_init` booleans track whether the corresponding value has been set).
#[derive(Debug, Clone, Default)]
pub struct ScriptExecutionData {
    /// Whether `tapleaf_hash` has been initialized.
    pub tapleaf_hash_init: bool,
    /// The BIP 341 tapleaf hash for the currently executing tapscript.
    pub tapleaf_hash: [u8; 32],
    /// Whether `codeseparator_pos` has been initialized.
    pub codeseparator_pos_init: bool,
    /// Position of the last `OP_CODESEPARATOR` in the script (0xFFFFFFFF if none).
    pub codeseparator_pos: u32,
    /// Whether `validation_weight_left` has been initialized.
    pub validation_weight_left_init: bool,
    /// Remaining validation weight budget for BIP 342 sigop limiting.
    pub validation_weight_left: i64,
    /// Whether annex data has been initialized.
    pub annex_init: bool,
    /// Whether an annex was present in the witness stack.
    pub annex_present: bool,
    /// SHA-256 hash of the annex (if present).
    pub annex_hash: [u8; 32],
}

// ---------------------------------------------------------------------------
// Witness type (local, to avoid circular dependency on qubitcoin-consensus)
// ---------------------------------------------------------------------------

/// Witness data associated with a transaction input.
///
/// Port of Bitcoin Core's `CScriptWitness`. Contains the witness stack
/// items that accompany a segwit or taproot input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScriptWitness {
    /// The ordered list of witness stack items (byte vectors).
    pub stack: Vec<Vec<u8>>,
}

impl ScriptWitness {
    /// Creates an empty witness (no stack items).
    pub fn new() -> Self {
        ScriptWitness { stack: Vec::new() }
    }

    /// Returns `true` if the witness stack is empty (a "null" witness).
    pub fn is_null(&self) -> bool {
        self.stack.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SignatureChecker trait
// ---------------------------------------------------------------------------

/// Trait for verifying signatures and time-locks during script evaluation.
///
/// Port of Bitcoin Core's `BaseSignatureChecker` virtual class.
/// Implementors provide transaction-context-aware signature verification;
/// the default [`BaseSignatureChecker`] always returns `false`.
pub trait SignatureChecker {
    /// Verifies an ECDSA signature against `pubkey` using the given `script_code` and `sigversion`.
    fn check_ecdsa_signature(
        &self,
        sig: &[u8],
        pubkey: &[u8],
        script_code: &Script,
        sigversion: SigVersion,
    ) -> bool;

    /// Verifies a Schnorr signature against `pubkey` using BIP 341/342 sighash rules.
    ///
    /// On failure, sets `error` to the appropriate [`ScriptError`] variant.
    fn check_schnorr_signature(
        &self,
        sig: &[u8],
        pubkey: &[u8],
        sigversion: SigVersion,
        exec_data: &ScriptExecutionData,
        error: &mut ScriptError,
    ) -> bool;

    /// Checks whether the transaction's `nLockTime` satisfies `lock_time` (BIP 65).
    fn check_lock_time(&self, lock_time: &ScriptNum) -> bool;

    /// Checks whether the input's `nSequence` satisfies `sequence` (BIP 112).
    fn check_sequence(&self, sequence: &ScriptNum) -> bool;
}

/// Default signature checker that always returns false.
/// Used for script evaluation without transaction context.
pub struct BaseSignatureChecker;

impl SignatureChecker for BaseSignatureChecker {
    fn check_ecdsa_signature(
        &self,
        _sig: &[u8],
        _pubkey: &[u8],
        _script_code: &Script,
        _sigversion: SigVersion,
    ) -> bool {
        false
    }

    fn check_schnorr_signature(
        &self,
        _sig: &[u8],
        _pubkey: &[u8],
        _sigversion: SigVersion,
        _exec_data: &ScriptExecutionData,
        error: &mut ScriptError,
    ) -> bool {
        *error = ScriptError::SchnorrSig;
        false
    }

    fn check_lock_time(&self, _lock_time: &ScriptNum) -> bool {
        false
    }

    fn check_sequence(&self, _sequence: &ScriptNum) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Signature / pubkey encoding checks
// ---------------------------------------------------------------------------

fn is_compressed_or_uncompressed_pubkey(pubkey: &[u8]) -> bool {
    if pubkey.len() < 33 {
        return false;
    }
    if pubkey[0] == 0x04 {
        pubkey.len() == 65
    } else if pubkey[0] == 0x02 || pubkey[0] == 0x03 {
        pubkey.len() == 33
    } else {
        false
    }
}

fn is_compressed_pubkey(pubkey: &[u8]) -> bool {
    pubkey.len() == 33 && (pubkey[0] == 0x02 || pubkey[0] == 0x03)
}

fn is_valid_signature_encoding(sig: &[u8]) -> bool {
    if sig.len() < 9 || sig.len() > 73 {
        return false;
    }
    if sig[0] != 0x30 {
        return false;
    }
    if sig[1] as usize != sig.len() - 3 {
        return false;
    }
    let len_r = sig[3] as usize;
    if 5 + len_r >= sig.len() {
        return false;
    }
    let len_s = sig[5 + len_r] as usize;
    if len_r + len_s + 7 != sig.len() {
        return false;
    }
    if sig[2] != 0x02 {
        return false;
    }
    if len_r == 0 {
        return false;
    }
    if sig[4] & 0x80 != 0 {
        return false;
    }
    if len_r > 1 && sig[4] == 0x00 && sig[5] & 0x80 == 0 {
        return false;
    }
    if sig[len_r + 4] != 0x02 {
        return false;
    }
    if len_s == 0 {
        return false;
    }
    if sig[len_r + 6] & 0x80 != 0 {
        return false;
    }
    if len_s > 1 && sig[len_r + 6] == 0x00 && sig[len_r + 7] & 0x80 == 0 {
        return false;
    }
    true
}

fn is_defined_hashtype_signature(sig: &[u8]) -> bool {
    if sig.is_empty() {
        return false;
    }
    let hash_type = sig[sig.len() - 1] & !0x80; // mask off ANYONECANPAY
    (1..=3).contains(&hash_type) // ALL=1, NONE=2, SINGLE=3
}

fn check_signature_encoding(
    sig: &[u8],
    flags: &ScriptVerifyFlags,
    error: &mut ScriptError,
) -> bool {
    if sig.is_empty() {
        return true;
    }
    if (flags.contains(ScriptVerifyFlags::DERSIG)
        || flags.contains(ScriptVerifyFlags::LOW_S)
        || flags.contains(ScriptVerifyFlags::STRICTENC))
        && !is_valid_signature_encoding(sig)
    {
        *error = ScriptError::SigDer;
        return false;
    }
    if flags.contains(ScriptVerifyFlags::LOW_S) && !is_low_der_signature(sig, error) {
        // error is set by is_low_der_signature
        return false;
    }
    if flags.contains(ScriptVerifyFlags::STRICTENC) && !is_defined_hashtype_signature(sig) {
        *error = ScriptError::SigHashtype;
        return false;
    }
    true
}

/// Check that the S value in a DER signature is in the lower half of the curve order.
/// Port of Bitcoin Core's `IsLowDERSignature()`.
fn is_low_der_signature(sig: &[u8], error: &mut ScriptError) -> bool {
    if !is_valid_signature_encoding(sig) {
        *error = ScriptError::SigDer;
        return false;
    }
    // Strip the hash_type byte and check using secp256k1
    let der_sig = &sig[..sig.len() - 1];
    match qubitcoin_crypto::secp256k1::ecdsa::Signature::from_der(der_sig) {
        Ok(parsed) => {
            // secp256k1_ecdsa_signature_normalize returns 1 if the S value was NOT low.
            // In the secp256k1 Rust crate, normalize_s() returns the normalized sig.
            // We compare to see if normalization changed the sig.
            let mut check = parsed;
            check.normalize_s();
            if check != parsed {
                *error = ScriptError::SigHighS;
                return false;
            }
            true
        }
        Err(_) => {
            *error = ScriptError::SigDer;
            false
        }
    }
}

fn check_pubkey_encoding(
    pubkey: &[u8],
    flags: &ScriptVerifyFlags,
    sigversion: SigVersion,
    error: &mut ScriptError,
) -> bool {
    if flags.contains(ScriptVerifyFlags::STRICTENC) && !is_compressed_or_uncompressed_pubkey(pubkey)
    {
        *error = ScriptError::PubKeyType;
        return false;
    }
    if flags.contains(ScriptVerifyFlags::WITNESS_PUBKEYTYPE)
        && sigversion == SigVersion::WitnessV0
        && !is_compressed_pubkey(pubkey)
    {
        *error = ScriptError::WitnessPubKeyType;
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// CheckMinimalPush
// ---------------------------------------------------------------------------

fn check_minimal_push(data: &[u8], opcode: u8) -> bool {
    if data.is_empty() {
        return opcode == Opcode::Op0 as u8;
    } else if data.len() == 1 && data[0] >= 1 && data[0] <= 16 {
        return false; // should use OP_1..OP_16
    } else if data.len() == 1 && data[0] == 0x81 {
        return false; // should use OP_1NEGATE
    } else if data.len() <= 75 {
        return opcode as usize == data.len();
    } else if data.len() <= 255 {
        return opcode == Opcode::OpPushData1 as u8;
    } else if data.len() <= 65535 {
        return opcode == Opcode::OpPushData2 as u8;
    }
    true
}

// ---------------------------------------------------------------------------
// ConditionStack - optimized IF/ELSE/ENDIF tracking
// ---------------------------------------------------------------------------

/// Optimized condition stack matching Bitcoin Core's ConditionStack.
struct ConditionStack {
    stack_size: u32,
    first_false_pos: u32,
}

const NO_FALSE: u32 = u32::MAX;

impl ConditionStack {
    fn new() -> Self {
        ConditionStack {
            stack_size: 0,
            first_false_pos: NO_FALSE,
        }
    }

    fn empty(&self) -> bool {
        self.stack_size == 0
    }

    fn all_true(&self) -> bool {
        self.first_false_pos == NO_FALSE
    }

    fn push_back(&mut self, f: bool) {
        if self.first_false_pos == NO_FALSE && !f {
            self.first_false_pos = self.stack_size;
        }
        self.stack_size += 1;
    }

    fn pop_back(&mut self) {
        self.stack_size -= 1;
        if self.first_false_pos == self.stack_size {
            self.first_false_pos = NO_FALSE;
        }
    }

    fn toggle_top(&mut self) {
        if self.first_false_pos == NO_FALSE {
            self.first_false_pos = self.stack_size - 1;
        } else if self.first_false_pos == self.stack_size - 1 {
            self.first_false_pos = NO_FALSE;
        }
    }
}

// ---------------------------------------------------------------------------
// Disabled opcode check
// ---------------------------------------------------------------------------

fn is_disabled_opcode(opcode: u8) -> bool {
    matches!(
        Opcode::from_u8(opcode),
        Some(Opcode::OpCat)
            | Some(Opcode::OpSubStr)
            | Some(Opcode::OpLeft)
            | Some(Opcode::OpRight)
            | Some(Opcode::OpInvert)
            | Some(Opcode::OpAnd)
            | Some(Opcode::OpOr)
            | Some(Opcode::OpXor)
            | Some(Opcode::Op2Mul)
            | Some(Opcode::Op2Div)
            | Some(Opcode::OpMul)
            | Some(Opcode::OpDiv)
            | Some(Opcode::OpMod)
            | Some(Opcode::OpLShift)
            | Some(Opcode::OpRShift)
    )
}

// ---------------------------------------------------------------------------
// Helper: ScriptNum from stack value
// ---------------------------------------------------------------------------

fn stack_to_script_num(
    val: &[u8],
    require_minimal: bool,
    max_size: usize,
    error: &mut ScriptError,
) -> Result<ScriptNum, ()> {
    match ScriptNum::from_bytes(val, require_minimal, max_size) {
        Ok(n) => Ok(n),
        Err(_) => {
            *error = ScriptError::ScriptNum;
            Err(())
        }
    }
}

// ---------------------------------------------------------------------------
// eval_script - The core script VM
// ---------------------------------------------------------------------------

/// Evaluates a Bitcoin script on the provided `stack`.
///
/// This is a 1:1 port of Bitcoin Core's `EvalScript()`. Executes every
/// opcode in `script`, using `checker` for signature / timelock validation
/// and `flags` to control which consensus rules are enforced.
///
/// Returns `true` on success. On failure, `error` is set to the specific
/// [`ScriptError`] and the function returns `false`.
pub fn eval_script(
    stack: &mut ScriptStack,
    script: &Script,
    flags: &ScriptVerifyFlags,
    checker: &dyn SignatureChecker,
    sigversion: SigVersion,
    exec_data: &mut ScriptExecutionData,
    error: &mut ScriptError,
) -> bool {
    let vch_false: StackValue = vec![];
    let vch_true: StackValue = vec![1u8];

    debug_assert!(
        sigversion == SigVersion::Base
            || sigversion == SigVersion::WitnessV0
            || sigversion == SigVersion::Tapscript
    );

    let script_bytes = script.as_bytes();

    // Size limit for non-tapscript
    if (sigversion == SigVersion::Base || sigversion == SigVersion::WitnessV0)
        && script_bytes.len() > MAX_SCRIPT_SIZE
    {
        *error = ScriptError::ScriptSize;
        return false;
    }

    let mut pc: usize = 0;
    let pbegincodehash: &mut usize = &mut 0usize;
    let mut altstack = ScriptStack::new();
    let mut vf_exec = ConditionStack::new();
    let mut op_count: u32 = 0;
    let require_minimal = flags.contains(ScriptVerifyFlags::MINIMALDATA);
    let mut opcode_pos: u32 = 0;

    exec_data.codeseparator_pos = 0xFFFFFFFF;
    exec_data.codeseparator_pos_init = true;

    *error = ScriptError::UnknownError;

    while pc < script_bytes.len() {
        let f_exec = vf_exec.all_true();

        // Read instruction
        let _saved_pc = pc;
        let get_op_result = script.get_op(pc);
        let (opcode, push_data, new_pc) = match get_op_result {
            Some(r) => r,
            None => {
                *error = ScriptError::BadOpcode;
                return false;
            }
        };
        pc = new_pc;

        // Push data size limit
        if push_data.len() > MAX_SCRIPT_ELEMENT_SIZE {
            *error = ScriptError::PushSize;
            return false;
        }

        // Op count limit (non-tapscript)
        if (sigversion == SigVersion::Base || sigversion == SigVersion::WitnessV0)
            && opcode > Opcode::Op16 as u8
        {
            op_count += 1;
            if op_count > MAX_OPS_PER_SCRIPT {
                *error = ScriptError::OpCount;
                return false;
            }
        }

        // Disabled opcodes always fail, even in non-executed branches
        if is_disabled_opcode(opcode) {
            *error = ScriptError::DisabledOpcode;
            return false;
        }

        // OP_CODESEPARATOR in non-segwit with CONST_SCRIPTCODE
        if opcode == Opcode::OpCodeSeparator as u8
            && sigversion == SigVersion::Base
            && flags.contains(ScriptVerifyFlags::CONST_SCRIPTCODE)
        {
            *error = ScriptError::OpCodeSeparator;
            return false;
        }

        // Push data opcodes (0x00..0x4e)
        if f_exec && opcode <= Opcode::OpPushData4 as u8 {
            if require_minimal && !check_minimal_push(&push_data, opcode) {
                *error = ScriptError::MinimalData;
                return false;
            }
            stack.push(push_data);
        } else if f_exec || (opcode >= Opcode::OpIf as u8 && opcode <= Opcode::OpEndIf as u8) {
            match opcode {
                // OP_1NEGATE, OP_1..OP_16
                op if op == Opcode::Op1Negate as u8 => {
                    stack.push(ScriptNum::new(-1).to_bytes());
                }
                op if (Opcode::Op1 as u8..=Opcode::Op16 as u8).contains(&op) => {
                    let n = (op as i64) - (Opcode::Op1 as u8 as i64) + 1;
                    stack.push(ScriptNum::new(n).to_bytes());
                }

                // --- Flow control ---
                op if op == Opcode::OpNop as u8 => {}

                op if op == Opcode::OpCheckLockTimeVerify as u8 => {
                    if !flags.contains(ScriptVerifyFlags::CHECKLOCKTIMEVERIFY) {
                        // treat as NOP
                    } else {
                        if stack.size() < 1 {
                            *error = ScriptError::InvalidStackOperation;
                            return false;
                        }
                        let lock_time = match stack_to_script_num(
                            stack.top(-1).unwrap(),
                            require_minimal,
                            5,
                            error,
                        ) {
                            Ok(n) => n,
                            Err(()) => return false,
                        };
                        if lock_time.get_i64() < 0 {
                            *error = ScriptError::NegativeLocktime;
                            return false;
                        }
                        if !checker.check_lock_time(&lock_time) {
                            *error = ScriptError::UnsatisfiedLocktime;
                            return false;
                        }
                    }
                }

                op if op == Opcode::OpCheckSequenceVerify as u8 => {
                    if !flags.contains(ScriptVerifyFlags::CHECKSEQUENCEVERIFY) {
                        // treat as NOP
                    } else {
                        if stack.size() < 1 {
                            *error = ScriptError::InvalidStackOperation;
                            return false;
                        }
                        let sequence = match stack_to_script_num(
                            stack.top(-1).unwrap(),
                            require_minimal,
                            5,
                            error,
                        ) {
                            Ok(n) => n,
                            Err(()) => return false,
                        };
                        if sequence.get_i64() < 0 {
                            *error = ScriptError::NegativeLocktime;
                            return false;
                        }
                        // If disabled flag set, treat as NOP
                        if (sequence.get_i64() as u32) & SEQUENCE_LOCKTIME_DISABLE_FLAG != 0 {
                            // NOP
                        } else if !checker.check_sequence(&sequence) {
                            *error = ScriptError::UnsatisfiedLocktime;
                            return false;
                        }
                    }
                }

                op if op == Opcode::OpNop1 as u8
                    || (Opcode::OpNop4 as u8..=Opcode::OpNop10 as u8).contains(&op) =>
                {
                    if flags.contains(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_NOPS) {
                        *error = ScriptError::DiscourageUpgradableNops;
                        return false;
                    }
                }

                op if op == Opcode::OpIf as u8 || op == Opcode::OpNotIf as u8 => {
                    let mut f_value = false;
                    if f_exec {
                        if stack.size() < 1 {
                            *error = ScriptError::InvalidStackOperation;
                            return false;
                        }
                        let vch = stack.top(-1).unwrap().clone();
                        // Tapscript: minimal IF enforcement (consensus)
                        if sigversion == SigVersion::Tapscript {
                            if vch.len() > 1 || (vch.len() == 1 && vch[0] != 1) {
                                *error = ScriptError::TapscriptMinimalIf;
                                return false;
                            }
                        }
                        // Witness v0: minimal IF enforcement (policy)
                        if sigversion == SigVersion::WitnessV0
                            && flags.contains(ScriptVerifyFlags::MINIMALIF)
                        {
                            if vch.len() > 1 {
                                *error = ScriptError::MinimalIf;
                                return false;
                            }
                            if vch.len() == 1 && vch[0] != 1 {
                                *error = ScriptError::MinimalIf;
                                return false;
                            }
                        }
                        f_value = cast_to_bool(&vch);
                        if op == Opcode::OpNotIf as u8 {
                            f_value = !f_value;
                        }
                        let _ = stack.pop();
                    }
                    vf_exec.push_back(f_value);
                }

                op if op == Opcode::OpElse as u8 => {
                    if vf_exec.empty() {
                        *error = ScriptError::UnbalancedConditional;
                        return false;
                    }
                    vf_exec.toggle_top();
                }

                op if op == Opcode::OpEndIf as u8 => {
                    if vf_exec.empty() {
                        *error = ScriptError::UnbalancedConditional;
                        return false;
                    }
                    vf_exec.pop_back();
                }

                op if op == Opcode::OpVerify as u8 => {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    if cast_to_bool(stack.top(-1).unwrap()) {
                        let _ = stack.pop();
                    } else {
                        *error = ScriptError::Verify;
                        return false;
                    }
                }

                op if op == Opcode::OpReturn as u8 => {
                    *error = ScriptError::OpReturn;
                    return false;
                }

                // --- Stack ops ---
                op if op == Opcode::OpToAltStack as u8 => {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v = stack.pop().unwrap();
                    altstack.push(v);
                }

                op if op == Opcode::OpFromAltStack as u8 => {
                    if altstack.size() < 1 {
                        *error = ScriptError::InvalidAltstackOperation;
                        return false;
                    }
                    let v = altstack.pop().unwrap();
                    stack.push(v);
                }

                op if op == Opcode::Op2Drop as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let _ = stack.pop();
                    let _ = stack.pop();
                }

                op if op == Opcode::Op2Dup as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v1 = stack.top(-2).unwrap().clone();
                    let v2 = stack.top(-1).unwrap().clone();
                    stack.push(v1);
                    stack.push(v2);
                }

                op if op == Opcode::Op3Dup as u8 => {
                    if stack.size() < 3 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v1 = stack.top(-3).unwrap().clone();
                    let v2 = stack.top(-2).unwrap().clone();
                    let v3 = stack.top(-1).unwrap().clone();
                    stack.push(v1);
                    stack.push(v2);
                    stack.push(v3);
                }

                op if op == Opcode::Op2Over as u8 => {
                    if stack.size() < 4 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v1 = stack.top(-4).unwrap().clone();
                    let v2 = stack.top(-3).unwrap().clone();
                    stack.push(v1);
                    stack.push(v2);
                }

                op if op == Opcode::Op2Rot as u8 => {
                    if stack.size() < 6 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v1 = stack.top(-6).unwrap().clone();
                    let v2 = stack.top(-5).unwrap().clone();
                    stack.erase_top_range(-6, -4);
                    stack.push(v1);
                    stack.push(v2);
                }

                op if op == Opcode::Op2Swap as u8 => {
                    if stack.size() < 4 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    stack.swap_top(-4, -2);
                    stack.swap_top(-3, -1);
                }

                op if op == Opcode::OpIfDup as u8 => {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let vch = stack.top(-1).unwrap().clone();
                    if cast_to_bool(&vch) {
                        stack.push(vch);
                    }
                }

                op if op == Opcode::OpDepth as u8 => {
                    let n = ScriptNum::new(stack.size() as i64);
                    stack.push(n.to_bytes());
                }

                op if op == Opcode::OpDrop as u8 => {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let _ = stack.pop();
                }

                op if op == Opcode::OpDup as u8 => {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let vch = stack.top(-1).unwrap().clone();
                    stack.push(vch);
                }

                op if op == Opcode::OpNip as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    stack.erase_top(-2);
                }

                op if op == Opcode::OpOver as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let vch = stack.top(-2).unwrap().clone();
                    stack.push(vch);
                }

                op if op == Opcode::OpPick as u8 || op == Opcode::OpRoll as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let n_val = stack.top(-1).unwrap().clone();
                    let n = match stack_to_script_num(
                        &n_val,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(sn) => sn.getint(),
                        Err(()) => return false,
                    };
                    let _ = stack.pop();
                    if n < 0 || n as usize >= stack.size() {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let vch = stack.top(-(n as isize) - 1).unwrap().clone();
                    if op == Opcode::OpRoll as u8 {
                        let idx = stack.size() - (n as usize) - 1;
                        stack.erase(idx, idx + 1);
                    }
                    stack.push(vch);
                }

                op if op == Opcode::OpRot as u8 => {
                    if stack.size() < 3 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    stack.swap_top(-3, -2);
                    stack.swap_top(-2, -1);
                }

                op if op == Opcode::OpSwap as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    stack.swap_top(-2, -1);
                }

                op if op == Opcode::OpTuck as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let vch = stack.top(-1).unwrap().clone();
                    stack.insert_top(-2, vch);
                }

                // --- Splice ---
                op if op == Opcode::OpSize as u8 => {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let sz = stack.top(-1).unwrap().len();
                    stack.push(ScriptNum::new(sz as i64).to_bytes());
                }

                // --- Bitwise logic ---
                op if op == Opcode::OpEqual as u8 || op == Opcode::OpEqualVerify as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v1 = stack.top(-2).unwrap().clone();
                    let v2 = stack.top(-1).unwrap().clone();
                    let equal = v1 == v2;
                    let _ = stack.pop();
                    let _ = stack.pop();
                    stack.push(if equal {
                        vch_true.clone()
                    } else {
                        vch_false.clone()
                    });
                    if op == Opcode::OpEqualVerify as u8 {
                        if equal {
                            let _ = stack.pop();
                        } else {
                            *error = ScriptError::EqualVerify;
                            return false;
                        }
                    }
                }

                // --- Numeric: unary ---
                op if op == Opcode::Op1Add as u8
                    || op == Opcode::Op1Sub as u8
                    || op == Opcode::OpNegate as u8
                    || op == Opcode::OpAbs as u8
                    || op == Opcode::OpNot as u8
                    || op == Opcode::Op0NotEqual as u8 =>
                {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let val = stack.top(-1).unwrap().clone();
                    let mut bn = match stack_to_script_num(
                        &val,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n,
                        Err(()) => return false,
                    };
                    match op {
                        x if x == Opcode::Op1Add as u8 => bn = bn + ScriptNum::new(1),
                        x if x == Opcode::Op1Sub as u8 => bn = bn - ScriptNum::new(1),
                        x if x == Opcode::OpNegate as u8 => bn = -bn,
                        x if x == Opcode::OpAbs as u8 => {
                            if bn.get_i64() < 0 {
                                bn = -bn;
                            }
                        }
                        x if x == Opcode::OpNot as u8 => {
                            bn = ScriptNum::new(if bn.get_i64() == 0 { 1 } else { 0 });
                        }
                        x if x == Opcode::Op0NotEqual as u8 => {
                            bn = ScriptNum::new(if bn.get_i64() != 0 { 1 } else { 0 });
                        }
                        _ => unreachable!(),
                    }
                    let _ = stack.pop();
                    stack.push(bn.to_bytes());
                }

                // --- Numeric: binary ---
                op if op == Opcode::OpAdd as u8
                    || op == Opcode::OpSub as u8
                    || op == Opcode::OpBoolAnd as u8
                    || op == Opcode::OpBoolOr as u8
                    || op == Opcode::OpNumEqual as u8
                    || op == Opcode::OpNumEqualVerify as u8
                    || op == Opcode::OpNumNotEqual as u8
                    || op == Opcode::OpLessThan as u8
                    || op == Opcode::OpGreaterThan as u8
                    || op == Opcode::OpLessThanOrEqual as u8
                    || op == Opcode::OpGreaterThanOrEqual as u8
                    || op == Opcode::OpMin as u8
                    || op == Opcode::OpMax as u8 =>
                {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v1 = stack.top(-2).unwrap().clone();
                    let v2 = stack.top(-1).unwrap().clone();
                    let bn1 = match stack_to_script_num(
                        &v1,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n,
                        Err(()) => return false,
                    };
                    let bn2 = match stack_to_script_num(
                        &v2,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n,
                        Err(()) => return false,
                    };
                    let bn = match op {
                        x if x == Opcode::OpAdd as u8 => bn1 + bn2,
                        x if x == Opcode::OpSub as u8 => bn1 - bn2,
                        x if x == Opcode::OpBoolAnd as u8 => {
                            ScriptNum::new(if bn1.get_i64() != 0 && bn2.get_i64() != 0 {
                                1
                            } else {
                                0
                            })
                        }
                        x if x == Opcode::OpBoolOr as u8 => {
                            ScriptNum::new(if bn1.get_i64() != 0 || bn2.get_i64() != 0 {
                                1
                            } else {
                                0
                            })
                        }
                        x if x == Opcode::OpNumEqual as u8
                            || x == Opcode::OpNumEqualVerify as u8 =>
                        {
                            ScriptNum::new(if bn1.get_i64() == bn2.get_i64() { 1 } else { 0 })
                        }
                        x if x == Opcode::OpNumNotEqual as u8 => {
                            ScriptNum::new(if bn1.get_i64() != bn2.get_i64() { 1 } else { 0 })
                        }
                        x if x == Opcode::OpLessThan as u8 => {
                            ScriptNum::new(if bn1.get_i64() < bn2.get_i64() { 1 } else { 0 })
                        }
                        x if x == Opcode::OpGreaterThan as u8 => {
                            ScriptNum::new(if bn1.get_i64() > bn2.get_i64() { 1 } else { 0 })
                        }
                        x if x == Opcode::OpLessThanOrEqual as u8 => {
                            ScriptNum::new(if bn1.get_i64() <= bn2.get_i64() { 1 } else { 0 })
                        }
                        x if x == Opcode::OpGreaterThanOrEqual as u8 => {
                            ScriptNum::new(if bn1.get_i64() >= bn2.get_i64() { 1 } else { 0 })
                        }
                        x if x == Opcode::OpMin as u8 => {
                            if bn1.get_i64() < bn2.get_i64() {
                                bn1
                            } else {
                                bn2
                            }
                        }
                        x if x == Opcode::OpMax as u8 => {
                            if bn1.get_i64() > bn2.get_i64() {
                                bn1
                            } else {
                                bn2
                            }
                        }
                        _ => unreachable!(),
                    };
                    let _ = stack.pop();
                    let _ = stack.pop();
                    stack.push(bn.to_bytes());

                    if op == Opcode::OpNumEqualVerify as u8 {
                        if cast_to_bool(stack.top(-1).unwrap()) {
                            let _ = stack.pop();
                        } else {
                            *error = ScriptError::NumEqualVerify;
                            return false;
                        }
                    }
                }

                // OP_WITHIN
                op if op == Opcode::OpWithin as u8 => {
                    if stack.size() < 3 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let v1 = stack.top(-3).unwrap().clone();
                    let v2 = stack.top(-2).unwrap().clone();
                    let v3 = stack.top(-1).unwrap().clone();
                    let bn1 = match stack_to_script_num(
                        &v1,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n,
                        Err(()) => return false,
                    };
                    let bn2 = match stack_to_script_num(
                        &v2,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n,
                        Err(()) => return false,
                    };
                    let bn3 = match stack_to_script_num(
                        &v3,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n,
                        Err(()) => return false,
                    };
                    let f_value = bn2.get_i64() <= bn1.get_i64() && bn1.get_i64() < bn3.get_i64();
                    let _ = stack.pop();
                    let _ = stack.pop();
                    let _ = stack.pop();
                    stack.push(if f_value {
                        vch_true.clone()
                    } else {
                        vch_false.clone()
                    });
                }

                // --- Crypto ---
                op if op == Opcode::OpRipemd160 as u8
                    || op == Opcode::OpSha1 as u8
                    || op == Opcode::OpSha256 as u8
                    || op == Opcode::OpHash160 as u8
                    || op == Opcode::OpHash256 as u8 =>
                {
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let vch = stack.top(-1).unwrap().clone();
                    let hash_result: Vec<u8> = match op {
                        x if x == Opcode::OpRipemd160 as u8 => {
                            qubitcoin_crypto::hash::ripemd160_hash(&vch).to_vec()
                        }
                        x if x == Opcode::OpSha1 as u8 => {
                            qubitcoin_crypto::hash::sha1_hash(&vch).to_vec()
                        }
                        x if x == Opcode::OpSha256 as u8 => {
                            qubitcoin_crypto::hash::sha256_hash(&vch).to_vec()
                        }
                        x if x == Opcode::OpHash160 as u8 => {
                            qubitcoin_crypto::hash::hash160(&vch).to_vec()
                        }
                        x if x == Opcode::OpHash256 as u8 => {
                            qubitcoin_crypto::hash::hash256(&vch).to_vec()
                        }
                        _ => unreachable!(),
                    };
                    let _ = stack.pop();
                    stack.push(hash_result);
                }

                // OP_CODESEPARATOR
                op if op == Opcode::OpCodeSeparator as u8 => {
                    *pbegincodehash = pc;
                    exec_data.codeseparator_pos = opcode_pos;
                }

                // OP_CHECKSIG / OP_CHECKSIGVERIFY
                op if op == Opcode::OpCheckSig as u8 || op == Opcode::OpCheckSigVerify as u8 => {
                    if stack.size() < 2 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }

                    let vch_sig = stack.top(-2).unwrap().clone();
                    let vch_pubkey = stack.top(-1).unwrap().clone();

                    let mut f_success = true;

                    // EvalChecksig logic (pre-tapscript path)
                    if sigversion == SigVersion::Base || sigversion == SigVersion::WitnessV0 {
                        // Build script code from pbegincodehash to end
                        let mut script_code = Script::from_slice(&script_bytes[*pbegincodehash..]);

                        // Drop the signature in pre-segwit scripts but not segwit scripts
                        // (Bitcoin Core's FindAndDelete)
                        if sigversion == SigVersion::Base {
                            let mut sig_script = Script::new();
                            sig_script.push_data(&vch_sig);
                            let found = script_code.find_and_delete(sig_script.as_bytes());
                            if found > 0
                                && flags.contains(ScriptVerifyFlags::CONST_SCRIPTCODE)
                            {
                                *error = ScriptError::SigFindAndDelete;
                                return false;
                            }
                        }

                        if !check_signature_encoding(&vch_sig, flags, error) {
                            return false;
                        }
                        if !check_pubkey_encoding(&vch_pubkey, flags, sigversion, error) {
                            return false;
                        }
                        f_success = checker.check_ecdsa_signature(
                            &vch_sig,
                            &vch_pubkey,
                            &script_code,
                            sigversion,
                        );

                        if !f_success
                            && flags.contains(ScriptVerifyFlags::NULLFAIL)
                            && !vch_sig.is_empty()
                        {
                            *error = ScriptError::SigNullFail;
                            return false;
                        }
                    } else if sigversion == SigVersion::Tapscript {
                        // Tapscript checksig
                        f_success = !vch_sig.is_empty();
                        if f_success {
                            if exec_data.validation_weight_left_init {
                                exec_data.validation_weight_left -=
                                    VALIDATION_WEIGHT_PER_SIGOP_PASSED;
                                if exec_data.validation_weight_left < 0 {
                                    *error = ScriptError::TapscriptValidationWeight;
                                    return false;
                                }
                            }
                        }
                        if vch_pubkey.is_empty() {
                            *error = ScriptError::TapscriptEmptyPubkey;
                            return false;
                        } else if vch_pubkey.len() == 32 {
                            if f_success
                                && !checker.check_schnorr_signature(
                                    &vch_sig,
                                    &vch_pubkey,
                                    sigversion,
                                    exec_data,
                                    error,
                                )
                            {
                                return false;
                            }
                        } else {
                            if flags.contains(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_PUBKEYTYPE) {
                                *error = ScriptError::DiscourageUpgradablePubkeyType;
                                return false;
                            }
                        }
                    }

                    let _ = stack.pop();
                    let _ = stack.pop();
                    stack.push(if f_success {
                        vch_true.clone()
                    } else {
                        vch_false.clone()
                    });
                    if op == Opcode::OpCheckSigVerify as u8 {
                        if f_success {
                            let _ = stack.pop();
                        } else {
                            *error = ScriptError::CheckSigVerify;
                            return false;
                        }
                    }
                }

                // OP_CHECKSIGADD (BIP 342 Tapscript only)
                op if op == Opcode::OpCheckSigAdd as u8 => {
                    if sigversion == SigVersion::Base || sigversion == SigVersion::WitnessV0 {
                        *error = ScriptError::BadOpcode;
                        return false;
                    }
                    if stack.size() < 3 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    let sig = stack.top(-3).unwrap().clone();
                    let num_val = stack.top(-2).unwrap().clone();
                    let pubkey = stack.top(-1).unwrap().clone();

                    let num = match stack_to_script_num(
                        &num_val,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n,
                        Err(()) => return false,
                    };

                    let success = !sig.is_empty();
                    if success {
                        if exec_data.validation_weight_left_init {
                            exec_data.validation_weight_left -= VALIDATION_WEIGHT_PER_SIGOP_PASSED;
                            if exec_data.validation_weight_left < 0 {
                                *error = ScriptError::TapscriptValidationWeight;
                                return false;
                            }
                        }
                    }
                    if pubkey.is_empty() {
                        *error = ScriptError::TapscriptEmptyPubkey;
                        return false;
                    } else if pubkey.len() == 32 {
                        if success
                            && !checker.check_schnorr_signature(
                                &sig, &pubkey, sigversion, exec_data, error,
                            )
                        {
                            return false;
                        }
                    } else {
                        if flags.contains(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_PUBKEYTYPE) {
                            *error = ScriptError::DiscourageUpgradablePubkeyType;
                            return false;
                        }
                    }

                    let _ = stack.pop();
                    let _ = stack.pop();
                    let _ = stack.pop();
                    let result = num + ScriptNum::new(if success { 1 } else { 0 });
                    stack.push(result.to_bytes());
                }

                // OP_CHECKMULTISIG / OP_CHECKMULTISIGVERIFY
                op if op == Opcode::OpCheckMultiSig as u8
                    || op == Opcode::OpCheckMultiSigVerify as u8 =>
                {
                    if sigversion == SigVersion::Tapscript {
                        *error = ScriptError::TapscriptCheckMultiSig;
                        return false;
                    }

                    let mut i = 1i32;
                    if (stack.size() as i32) < i {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }

                    let n_keys_val = stack.top(-i as isize).unwrap().clone();
                    let n_keys_count = match stack_to_script_num(
                        &n_keys_val,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n.getint(),
                        Err(()) => return false,
                    };
                    if n_keys_count < 0 || n_keys_count as usize > MAX_PUBKEYS_PER_MULTISIG {
                        *error = ScriptError::PubkeyCount;
                        return false;
                    }
                    op_count += n_keys_count as u32;
                    if op_count > MAX_OPS_PER_SCRIPT {
                        *error = ScriptError::OpCount;
                        return false;
                    }
                    i += 1;
                    let ikey = i;
                    let mut ikey2 = n_keys_count + 2;
                    i += n_keys_count;
                    if (stack.size() as i32) < i {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }

                    let n_sigs_val = stack.top(-i as isize).unwrap().clone();
                    let mut n_sigs_count = match stack_to_script_num(
                        &n_sigs_val,
                        require_minimal,
                        DEFAULT_MAX_NUM_SIZE,
                        error,
                    ) {
                        Ok(n) => n.getint(),
                        Err(()) => return false,
                    };
                    if n_sigs_count < 0 || n_sigs_count > n_keys_count {
                        *error = ScriptError::SigCount;
                        return false;
                    }
                    i += 1;
                    let mut isig = i;
                    i += n_sigs_count;
                    if (stack.size() as i32) < i {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }

                    // Build script code
                    let mut script_code = Script::from_slice(&script_bytes[*pbegincodehash..]);

                    // Drop the signatures in pre-segwit scripts but not segwit scripts
                    // (Bitcoin Core's FindAndDelete for CHECKMULTISIG)
                    if sigversion == SigVersion::Base {
                        for k in 0..n_sigs_count {
                            let vch_sig = stack.top(-(isig as isize + k as isize)).unwrap().clone();
                            let mut sig_script = Script::new();
                            sig_script.push_data(&vch_sig);
                            let found = script_code.find_and_delete(sig_script.as_bytes());
                            if found > 0
                                && flags.contains(ScriptVerifyFlags::CONST_SCRIPTCODE)
                            {
                                *error = ScriptError::SigFindAndDelete;
                                return false;
                            }
                        }
                    }

                    let mut f_success = true;
                    let mut ikey_cur = ikey;
                    let mut n_keys_remaining = n_keys_count;

                    while f_success && n_sigs_count > 0 {
                        let vch_sig = stack.top(-(isig as isize)).unwrap().clone();
                        let vch_pubkey = stack.top(-(ikey_cur as isize)).unwrap().clone();

                        if !check_signature_encoding(&vch_sig, flags, error) {
                            return false;
                        }
                        if !check_pubkey_encoding(&vch_pubkey, flags, sigversion, error) {
                            return false;
                        }

                        let f_ok = checker.check_ecdsa_signature(
                            &vch_sig,
                            &vch_pubkey,
                            &script_code,
                            sigversion,
                        );
                        if f_ok {
                            isig += 1;
                            n_sigs_count -= 1;
                        }
                        ikey_cur += 1;
                        n_keys_remaining -= 1;

                        if n_sigs_count > n_keys_remaining {
                            f_success = false;
                        }
                    }

                    // Clean up stack of actual arguments
                    while i > 1 {
                        i -= 1;
                        if !f_success && flags.contains(ScriptVerifyFlags::NULLFAIL) && ikey2 == 0 {
                            if !stack.top(-1).unwrap().is_empty() {
                                *error = ScriptError::SigNullFail;
                                return false;
                            }
                        }
                        if ikey2 > 0 {
                            ikey2 -= 1;
                        }
                        let _ = stack.pop();
                    }

                    // Bug: CHECKMULTISIG consumes one extra argument
                    if stack.size() < 1 {
                        *error = ScriptError::InvalidStackOperation;
                        return false;
                    }
                    if flags.contains(ScriptVerifyFlags::NULLDUMMY)
                        && !stack.top(-1).unwrap().is_empty()
                    {
                        *error = ScriptError::SigNullDummy;
                        return false;
                    }
                    let _ = stack.pop();

                    stack.push(if f_success {
                        vch_true.clone()
                    } else {
                        vch_false.clone()
                    });
                    if op == Opcode::OpCheckMultiSigVerify as u8 {
                        if f_success {
                            let _ = stack.pop();
                        } else {
                            *error = ScriptError::CheckMultiSigVerify;
                            return false;
                        }
                    }
                }

                _ => {
                    *error = ScriptError::BadOpcode;
                    return false;
                }
            }
        }

        // Stack size limit
        if stack.size() + altstack.size() > MAX_STACK_SIZE {
            *error = ScriptError::StackSize;
            return false;
        }

        opcode_pos += 1;
    }

    if !vf_exec.empty() {
        *error = ScriptError::UnbalancedConditional;
        return false;
    }

    *error = ScriptError::Ok;
    true
}

/// Convenience wrapper around [`eval_script`] that creates a default
/// [`ScriptExecutionData`] internally.
///
/// Use this when tapscript execution data is not needed (e.g. for
/// `SigVersion::Base` or `SigVersion::WitnessV0`).
pub fn eval_script_simple(
    stack: &mut ScriptStack,
    script: &Script,
    flags: &ScriptVerifyFlags,
    checker: &dyn SignatureChecker,
    sigversion: SigVersion,
    error: &mut ScriptError,
) -> bool {
    let mut exec_data = ScriptExecutionData::default();
    eval_script(
        stack,
        script,
        flags,
        checker,
        sigversion,
        &mut exec_data,
        error,
    )
}

// ---------------------------------------------------------------------------
// verify_script
// ---------------------------------------------------------------------------

/// Verifies a complete transaction input script.
///
/// Evaluates `script_sig` (the unlocking script), then `script_pubkey`
/// (the locking script from the UTXO), applying P2SH, segwit, and
/// taproot rules as dictated by `flags` and `witness`.
///
/// This is a 1:1 port of Bitcoin Core's `VerifyScript()`. Returns `true`
/// on success; on failure, `error` is set to the specific [`ScriptError`].
pub fn verify_script(
    script_sig: &Script,
    script_pubkey: &Script,
    witness: &ScriptWitness,
    flags: &ScriptVerifyFlags,
    checker: &dyn SignatureChecker,
    error: &mut ScriptError,
) -> bool {
    *error = ScriptError::UnknownError;

    // SigPushOnly check
    if flags.contains(ScriptVerifyFlags::SIGPUSHONLY) && !script_sig.is_push_only() {
        *error = ScriptError::SigPushOnly;
        return false;
    }

    // Evaluate scriptSig
    let mut stack = ScriptStack::new();
    if !eval_script_simple(
        &mut stack,
        script_sig,
        flags,
        checker,
        SigVersion::Base,
        error,
    ) {
        return false;
    }

    // Copy stack for P2SH
    let stack_copy = if flags.contains(ScriptVerifyFlags::P2SH) {
        Some(stack.as_vec().clone())
    } else {
        None
    };

    // Evaluate scriptPubKey with the stack from scriptSig
    if !eval_script_simple(
        &mut stack,
        script_pubkey,
        flags,
        checker,
        SigVersion::Base,
        error,
    ) {
        return false;
    }

    // Check result
    if stack.empty() {
        *error = ScriptError::EvalFalse;
        return false;
    }
    if !cast_to_bool(stack.back().unwrap()) {
        *error = ScriptError::EvalFalse;
        return false;
    }

    // Bare witness programs
    let mut had_witness = false;
    if flags.contains(ScriptVerifyFlags::WITNESS) {
        if let Some((wit_version, wit_program)) = script_pubkey.is_witness_program() {
            had_witness = true;
            if !script_sig.is_empty() {
                *error = ScriptError::WitnessMalleated;
                return false;
            }
            if !verify_witness_program(
                witness,
                wit_version,
                wit_program,
                flags,
                checker,
                error,
                false,
            ) {
                return false;
            }
            // Bypass cleanstack for witness programs
            while stack.size() > 1 {
                let _ = stack.pop();
            }
        }
    }

    // P2SH
    if flags.contains(ScriptVerifyFlags::P2SH) && script_pubkey.is_p2sh() {
        if !script_sig.is_push_only() {
            *error = ScriptError::SigPushOnly;
            return false;
        }

        // Restore the stack from before scriptPubKey evaluation
        let mut stack = ScriptStack::from_vec(stack_copy.unwrap());

        if stack.empty() {
            *error = ScriptError::EvalFalse;
            return false;
        }

        let serialized = stack.back().unwrap().clone();
        let pub_key2 = Script::from_slice(&serialized);
        let _ = stack.pop();

        if !eval_script_simple(
            &mut stack,
            &pub_key2,
            flags,
            checker,
            SigVersion::Base,
            error,
        ) {
            return false;
        }

        if stack.empty() {
            *error = ScriptError::EvalFalse;
            return false;
        }
        if !cast_to_bool(stack.back().unwrap()) {
            *error = ScriptError::EvalFalse;
            return false;
        }

        // P2SH witness program
        if flags.contains(ScriptVerifyFlags::WITNESS) {
            if let Some((wit_version, wit_program)) = pub_key2.is_witness_program() {
                had_witness = true;
                // scriptSig must be exactly a push of the redeemScript
                let mut expected_sig = Script::new();
                expected_sig.push_data(&serialized);
                if script_sig.as_bytes() != expected_sig.as_bytes() {
                    *error = ScriptError::WitnessMalleatedP2sh;
                    return false;
                }
                if !verify_witness_program(
                    witness,
                    wit_version,
                    wit_program,
                    flags,
                    checker,
                    error,
                    true,
                ) {
                    return false;
                }
                while stack.size() > 1 {
                    let _ = stack.pop();
                }
            }
        }

        // Clean stack check for P2SH
        if flags.contains(ScriptVerifyFlags::CLEANSTACK) {
            if stack.size() != 1 {
                *error = ScriptError::CleanStack;
                return false;
            }
        }

        // Witness unexpected check for P2SH path
        if flags.contains(ScriptVerifyFlags::WITNESS) {
            if !had_witness && !witness.is_null() {
                *error = ScriptError::WitnessUnexpected;
                return false;
            }
        }

        *error = ScriptError::Ok;
        return true;
    }

    // Clean stack check (non-P2SH path)
    if flags.contains(ScriptVerifyFlags::CLEANSTACK) {
        if stack.size() != 1 {
            *error = ScriptError::CleanStack;
            return false;
        }
    }

    // Witness unexpected
    if flags.contains(ScriptVerifyFlags::WITNESS) {
        if !had_witness && !witness.is_null() {
            *error = ScriptError::WitnessUnexpected;
            return false;
        }
    }

    *error = ScriptError::Ok;
    true
}

// ---------------------------------------------------------------------------
// verify_witness_program
// ---------------------------------------------------------------------------

fn verify_witness_program(
    witness: &ScriptWitness,
    wit_version: u8,
    wit_program: &[u8],
    flags: &ScriptVerifyFlags,
    checker: &dyn SignatureChecker,
    error: &mut ScriptError,
    is_p2sh: bool,
) -> bool {
    let mut exec_data = ScriptExecutionData::default();

    if wit_version == 0 {
        if wit_program.len() == 32 {
            // P2WSH
            if witness.stack.is_empty() {
                *error = ScriptError::WitnessProgramWitnessEmpty;
                return false;
            }
            let script_bytes = &witness.stack[witness.stack.len() - 1];
            let exec_script = Script::from_slice(script_bytes);
            let hash = qubitcoin_crypto::hash::sha256_hash(script_bytes);
            if hash != wit_program {
                *error = ScriptError::WitnessProgramMismatch;
                return false;
            }
            // Stack is witness items except the last (which is the script)
            let mut stack = ScriptStack::new();
            for item in &witness.stack[..witness.stack.len() - 1] {
                stack.push(item.clone());
            }
            return execute_witness_script(
                &mut stack,
                &exec_script,
                flags,
                SigVersion::WitnessV0,
                checker,
                &mut exec_data,
                error,
            );
        } else if wit_program.len() == 20 {
            // P2WPKH
            if witness.stack.len() != 2 {
                *error = ScriptError::WitnessProgramMismatch;
                return false;
            }
            // Create implied P2PKH script
            let mut exec_script = Script::new();
            exec_script.push_opcode(Opcode::OpDup);
            exec_script.push_opcode(Opcode::OpHash160);
            exec_script.push_data(wit_program);
            exec_script.push_opcode(Opcode::OpEqualVerify);
            exec_script.push_opcode(Opcode::OpCheckSig);

            let mut stack = ScriptStack::new();
            for item in &witness.stack {
                stack.push(item.clone());
            }
            return execute_witness_script(
                &mut stack,
                &exec_script,
                flags,
                SigVersion::WitnessV0,
                checker,
                &mut exec_data,
                error,
            );
        } else {
            *error = ScriptError::WitnessProgramWrongLength;
            return false;
        }
    } else if wit_version == 1 && wit_program.len() == 32 && !is_p2sh {
        // BIP 341 Taproot: 32-byte non-P2SH witness v1 program
        if !flags.contains(ScriptVerifyFlags::TAPROOT) {
            *error = ScriptError::Ok;
            return true;
        }
        if witness.stack.is_empty() {
            *error = ScriptError::WitnessProgramWitnessEmpty;
            return false;
        }

        // Work with a mutable copy of the witness stack items
        let mut stack_items: Vec<Vec<u8>> = witness.stack.clone();

        // --- Annex detection ---
        if stack_items.len() >= 2
            && !stack_items.last().unwrap().is_empty()
            && stack_items.last().unwrap()[0] == ANNEX_TAG
        {
            let annex = stack_items.pop().unwrap();
            // Bitcoin Core serializes the annex as a vector (compact-size length
            // prefix + data) before hashing:
            //   execdata.m_annex_hash = (HashWriter{} << annex).GetSHA256();
            let mut serialized = Vec::with_capacity(9 + annex.len());
            write_compact_size_to_vec(&mut serialized, annex.len() as u64);
            serialized.extend_from_slice(&annex);
            exec_data.annex_hash = qubitcoin_crypto::hash::sha256_hash(&serialized);
            exec_data.annex_present = true;
        } else {
            exec_data.annex_present = false;
        }
        exec_data.annex_init = true;

        if stack_items.len() == 1 {
            // --- Key-path spending ---
            // The single remaining stack element is the signature.
            // `wit_program` is the 32-byte output key (x-only pubkey).
            if !checker.check_schnorr_signature(
                &stack_items[0],
                wit_program,
                SigVersion::Taproot,
                &exec_data,
                error,
            ) {
                return false; // error is set by checker
            }
            *error = ScriptError::Ok;
            return true;
        } else {
            // --- Script-path spending ---
            let control = stack_items.pop().unwrap();
            let script = stack_items.pop().unwrap();

            // Validate control block size
            if control.len() < TAPROOT_CONTROL_BASE_SIZE
                || control.len() > TAPROOT_CONTROL_MAX_SIZE
                || ((control.len() - TAPROOT_CONTROL_BASE_SIZE) % TAPROOT_CONTROL_NODE_SIZE) != 0
            {
                *error = ScriptError::TaprootWrongControlSize;
                return false;
            }

            // Compute tapleaf hash
            let leaf_version = control[0] & TAPROOT_LEAF_MASK;
            exec_data.tapleaf_hash = compute_tapleaf_hash(leaf_version, &script);

            // Verify taproot commitment
            if !verify_taproot_commitment(&control, wit_program, &exec_data.tapleaf_hash) {
                *error = ScriptError::WitnessProgramMismatch;
                return false;
            }
            exec_data.tapleaf_hash_init = true;

            if leaf_version == TAPROOT_LEAF_TAPSCRIPT {
                // Tapscript (BIP 342): leaf version 0xc0
                let exec_script = Script::from_slice(&script);

                // BIP 342: OP_SUCCESSx pre-scan.
                // If ANY opcode in the script is an OP_SUCCESS opcode, the
                // script succeeds immediately (overrides everything, including
                // size limits).  With DISCOURAGE_OP_SUCCESS this is rejected
                // as non-standard.
                {
                    let mut pc = 0usize;
                    let script_bytes = exec_script.as_bytes();
                    while pc < script_bytes.len() {
                        match exec_script.get_op(pc) {
                            Some((opcode, _data, next_pc)) => {
                                if is_op_success(opcode) {
                                    if flags.contains(ScriptVerifyFlags::DISCOURAGE_OP_SUCCESS) {
                                        *error = ScriptError::DiscourageOpSuccess;
                                        return false;
                                    }
                                    *error = ScriptError::Ok;
                                    return true;
                                }
                                pc = next_pc;
                            }
                            None => {
                                // Note: this condition would not be reached
                                // if an unknown OP_SUCCESS was found earlier.
                                *error = ScriptError::BadOpcode;
                                return false;
                            }
                        }
                    }
                }

                // Compute validation weight budget = GetSerializeSize(witness.stack) + VALIDATION_WEIGHT_OFFSET
                exec_data.validation_weight_left =
                    compute_witness_size(&witness.stack) as i64 + VALIDATION_WEIGHT_OFFSET;
                exec_data.validation_weight_left_init = true;

                // The remaining stack_items become the execution stack
                let mut exec_stack = ScriptStack::new();
                for item in &stack_items {
                    exec_stack.push(item.clone());
                }
                return execute_witness_script(
                    &mut exec_stack,
                    &exec_script,
                    flags,
                    SigVersion::Tapscript,
                    checker,
                    &mut exec_data,
                    error,
                );
            }

            // Unknown leaf version: future soft-fork
            if flags.contains(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_TAPROOT_VERSION) {
                *error = ScriptError::DiscourageUpgradableTaprootVersion;
                return false;
            }
            *error = ScriptError::Ok;
            return true;
        }
    } else {
        if flags.contains(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM) {
            *error = ScriptError::DiscourageUpgradableWitnessProgram;
            return false;
        }
        *error = ScriptError::Ok;
        return true;
    }
}

// ---------------------------------------------------------------------------
// Taproot helper functions (BIP 341)
// ---------------------------------------------------------------------------

/// Test whether an opcode is an OP_SUCCESSx opcode as defined by BIP 342.
///
/// These opcodes cause immediate script success in tapscript context,
/// providing a clean mechanism for future soft-fork upgrades.
///
/// Port of Bitcoin Core's `IsOpSuccess()`.
fn is_op_success(opcode: u8) -> bool {
    opcode == 80
        || opcode == 98
        || (opcode >= 126 && opcode <= 129)
        || (opcode >= 131 && opcode <= 134)
        || (opcode >= 137 && opcode <= 138)
        || (opcode >= 141 && opcode <= 142)
        || (opcode >= 149 && opcode <= 153)
        || (opcode >= 187 && opcode <= 254)
}

/// Compute the tapleaf hash: tagged_hash("TapLeaf", [leaf_version, compact_size(script.len()), script])
fn compute_tapleaf_hash(leaf_version: u8, script: &[u8]) -> [u8; 32] {
    let mut data = Vec::new();
    data.push(leaf_version);
    // Write compact size of script length
    let mut compact_buf = Vec::new();
    qubitcoin_serialize::write_compact_size(&mut compact_buf, script.len() as u64).unwrap();
    data.extend_from_slice(&compact_buf);
    data.extend_from_slice(script);
    qubitcoin_crypto::hash::tagged_hash(b"TapLeaf", &data)
}

/// Compute the tapbranch hash from two 32-byte nodes, sorting lexicographically.
fn compute_tapbranch_hash(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut data = Vec::with_capacity(64);
    if a < b {
        data.extend_from_slice(a);
        data.extend_from_slice(b);
    } else {
        data.extend_from_slice(b);
        data.extend_from_slice(a);
    }
    qubitcoin_crypto::hash::tagged_hash(b"TapBranch", &data)
}

/// Compute the merkle root from the control block path and tapleaf hash.
fn compute_taproot_merkle_root(control: &[u8], tapleaf_hash: &[u8; 32]) -> [u8; 32] {
    let path_len = (control.len() - TAPROOT_CONTROL_BASE_SIZE) / TAPROOT_CONTROL_NODE_SIZE;
    let mut k = *tapleaf_hash;
    for i in 0..path_len {
        let offset = TAPROOT_CONTROL_BASE_SIZE + TAPROOT_CONTROL_NODE_SIZE * i;
        let node = &control[offset..offset + TAPROOT_CONTROL_NODE_SIZE];
        k = compute_tapbranch_hash(&k, node);
    }
    k
}

/// Verify the taproot commitment: tweak the internal pubkey by the merkle root
/// and check it matches the output key (program).
fn verify_taproot_commitment(control: &[u8], program: &[u8], tapleaf_hash: &[u8; 32]) -> bool {
    use qubitcoin_crypto::secp256k1::{self, Scalar, Secp256k1, XOnlyPublicKey};

    // Internal pubkey from control block bytes [1..33]
    let internal_key = match XOnlyPublicKey::from_slice(&control[1..TAPROOT_CONTROL_BASE_SIZE]) {
        Ok(k) => k,
        Err(_) => return false,
    };

    // Output key from program
    let output_key = match XOnlyPublicKey::from_slice(program) {
        Ok(k) => k,
        Err(_) => return false,
    };

    // Compute merkle root
    let merkle_root = compute_taproot_merkle_root(control, tapleaf_hash);

    // Compute the tweak: tagged_hash("TapTweak", internal_key || merkle_root)
    let mut tweak_data = Vec::with_capacity(64);
    tweak_data.extend_from_slice(&internal_key.serialize());
    tweak_data.extend_from_slice(&merkle_root);
    let tweak_hash = qubitcoin_crypto::hash::tagged_hash(b"TapTweak", &tweak_data);

    // Create scalar from tweak hash (big-endian)
    let tweak_scalar = match Scalar::from_be_bytes(tweak_hash) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Parity from the control block first byte (bit 0)
    let parity = match secp256k1::Parity::from_u8(control[0] & 1) {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Verify: internal_key tweaked by tweak_scalar should equal output_key with the given parity
    let secp = Secp256k1::verification_only();
    internal_key.tweak_add_check(&secp, &output_key, parity, tweak_scalar)
}

/// Compute the serialized size of witness stack items.
/// This matches Bitcoin Core's `GetSerializeSize(witness.stack)`:
/// compact_size(n_items) + sum(compact_size(item.len()) + item.len())
fn compute_witness_size(stack: &[Vec<u8>]) -> usize {
    let mut size = qubitcoin_serialize::compact_size_len(stack.len() as u64);
    for item in stack {
        size += qubitcoin_serialize::compact_size_len(item.len() as u64) + item.len();
    }
    size
}

// ---------------------------------------------------------------------------
// execute_witness_script
// ---------------------------------------------------------------------------

fn execute_witness_script(
    stack: &mut ScriptStack,
    exec_script: &Script,
    flags: &ScriptVerifyFlags,
    sigversion: SigVersion,
    checker: &dyn SignatureChecker,
    exec_data: &mut ScriptExecutionData,
    error: &mut ScriptError,
) -> bool {
    // Check element sizes in initial stack
    for item in stack.as_vec() {
        if item.len() > MAX_SCRIPT_ELEMENT_SIZE {
            *error = ScriptError::PushSize;
            return false;
        }
    }

    // Tapscript: check initial witness stack doesn't exceed MAX_STACK_SIZE.
    // Bitcoin Core checks this in ExecuteWitnessScript for SigVersion::TAPSCRIPT.
    if sigversion == SigVersion::Tapscript && stack.size() > MAX_STACK_SIZE {
        *error = ScriptError::StackSize;
        return false;
    }

    if !eval_script(
        stack,
        exec_script,
        flags,
        checker,
        sigversion,
        exec_data,
        error,
    ) {
        return false;
    }

    if stack.size() != 1 {
        *error = ScriptError::CleanStack;
        return false;
    }

    if !cast_to_bool(stack.top(-1).unwrap()) {
        *error = ScriptError::EvalFalse;
        return false;
    }

    *error = ScriptError::Ok;
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcode::Opcode;
    use crate::script::Script;

    /// Helper: build script, evaluate, return (success, error, stack).
    fn run_script(script: &Script) -> (bool, ScriptError, ScriptStack) {
        let mut stack = ScriptStack::new();
        let flags = ScriptVerifyFlags::NONE;
        let checker = BaseSignatureChecker;
        let mut error = ScriptError::Ok;
        let ok = eval_script_simple(
            &mut stack,
            script,
            &flags,
            &checker,
            SigVersion::Base,
            &mut error,
        );
        (ok, error, stack)
    }

    fn run_script_flags(
        script: &Script,
        flags: &ScriptVerifyFlags,
    ) -> (bool, ScriptError, ScriptStack) {
        let mut stack = ScriptStack::new();
        let checker = BaseSignatureChecker;
        let mut error = ScriptError::Ok;
        let ok = eval_script_simple(
            &mut stack,
            script,
            flags,
            &checker,
            SigVersion::Base,
            &mut error,
        );
        (ok, error, stack)
    }

    // --- Stack tests ---

    #[test]
    fn test_cast_to_bool() {
        assert!(!cast_to_bool(&[]));
        assert!(!cast_to_bool(&[0]));
        assert!(!cast_to_bool(&[0, 0]));
        assert!(!cast_to_bool(&[0x80])); // negative zero
        assert!(cast_to_bool(&[1]));
        assert!(cast_to_bool(&[0x80, 0x00])); // -128 is truthy
        assert!(cast_to_bool(&[0, 1]));
    }

    #[test]
    fn test_stack_basic() {
        let mut s = ScriptStack::new();
        assert!(s.empty());
        s.push(vec![1, 2, 3]);
        assert_eq!(s.size(), 1);
        assert_eq!(s.top(-1).unwrap(), &vec![1, 2, 3]);
        let v = s.pop().unwrap();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(s.empty());
    }

    #[test]
    fn test_stack_pop_empty() {
        let mut s = ScriptStack::new();
        assert!(s.pop().is_err());
    }

    // --- Push data tests ---

    #[test]
    fn test_op_0_pushes_empty() {
        let mut script = Script::new();
        script.push_opcode(Opcode::Op0);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 1);
        assert_eq!(stack.top(-1).unwrap(), &Vec::<u8>::new());
    }

    #[test]
    fn test_op_1_through_16() {
        for n in 1..=16i64 {
            let mut script = Script::new();
            script.push_int(n);
            let (ok, _, stack) = run_script(&script);
            assert!(ok);
            assert_eq!(stack.size(), 1);
            assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(n).to_bytes());
        }
    }

    #[test]
    fn test_op_1negate() {
        let mut script = Script::new();
        script.push_int(-1);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(-1).to_bytes());
    }

    #[test]
    fn test_push_data() {
        let mut script = Script::new();
        let data = vec![0xab; 10];
        script.push_data(&data);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &data);
    }

    // --- Arithmetic tests ---

    #[test]
    fn test_op_add() {
        let mut script = Script::new();
        script.push_int(3);
        script.push_int(4);
        script.push_opcode(Opcode::OpAdd);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(7).to_bytes());
    }

    #[test]
    fn test_op_sub() {
        let mut script = Script::new();
        script.push_int(10);
        script.push_int(3);
        script.push_opcode(Opcode::OpSub);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(7).to_bytes());
    }

    #[test]
    fn test_op_1add() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_opcode(Opcode::Op1Add);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(6).to_bytes());
    }

    #[test]
    fn test_op_1sub() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_opcode(Opcode::Op1Sub);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(4).to_bytes());
    }

    #[test]
    fn test_op_negate() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_opcode(Opcode::OpNegate);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(-5).to_bytes());
    }

    #[test]
    fn test_op_abs() {
        let mut script = Script::new();
        script.push_int(-5);
        script.push_opcode(Opcode::OpAbs);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(5).to_bytes());
    }

    #[test]
    fn test_op_not() {
        // NOT(0) = 1
        let mut script = Script::new();
        script.push_int(0);
        script.push_opcode(Opcode::OpNot);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());

        // NOT(5) = 0
        let mut script = Script::new();
        script.push_int(5);
        script.push_opcode(Opcode::OpNot);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(0).to_bytes());
    }

    #[test]
    fn test_op_0notequal() {
        let mut script = Script::new();
        script.push_int(0);
        script.push_opcode(Opcode::Op0NotEqual);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(0).to_bytes());

        let mut script = Script::new();
        script.push_int(5);
        script.push_opcode(Opcode::Op0NotEqual);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());
    }

    #[test]
    fn test_op_booland() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(1);
        script.push_opcode(Opcode::OpBoolAnd);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());

        let mut script = Script::new();
        script.push_int(1);
        script.push_int(0);
        script.push_opcode(Opcode::OpBoolAnd);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(0).to_bytes());
    }

    #[test]
    fn test_op_boolor() {
        let mut script = Script::new();
        script.push_int(0);
        script.push_int(0);
        script.push_opcode(Opcode::OpBoolOr);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(0).to_bytes());

        let mut script = Script::new();
        script.push_int(1);
        script.push_int(0);
        script.push_opcode(Opcode::OpBoolOr);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());
    }

    #[test]
    fn test_op_numequal() {
        let mut script = Script::new();
        script.push_int(3);
        script.push_int(3);
        script.push_opcode(Opcode::OpNumEqual);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(cast_to_bool(stack.top(-1).unwrap()));

        let mut script = Script::new();
        script.push_int(3);
        script.push_int(4);
        script.push_opcode(Opcode::OpNumEqual);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(!cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_lessthan() {
        let mut script = Script::new();
        script.push_int(3);
        script.push_int(4);
        script.push_opcode(Opcode::OpLessThan);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_greaterthan() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_int(3);
        script.push_opcode(Opcode::OpGreaterThan);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_within() {
        // 3 is within [2, 5)
        let mut script = Script::new();
        script.push_int(3);
        script.push_int(2);
        script.push_int(5);
        script.push_opcode(Opcode::OpWithin);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(cast_to_bool(stack.top(-1).unwrap()));

        // 5 is NOT within [2, 5) (exclusive upper bound)
        let mut script = Script::new();
        script.push_int(5);
        script.push_int(2);
        script.push_int(5);
        script.push_opcode(Opcode::OpWithin);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(!cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_min_max() {
        let mut script = Script::new();
        script.push_int(3);
        script.push_int(7);
        script.push_opcode(Opcode::OpMin);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(3).to_bytes());

        let mut script = Script::new();
        script.push_int(3);
        script.push_int(7);
        script.push_opcode(Opcode::OpMax);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(7).to_bytes());
    }

    // --- Flow control tests ---

    #[test]
    fn test_op_if_true_branch() {
        // OP_1 OP_IF OP_2 OP_ELSE OP_3 OP_ENDIF
        let mut script = Script::new();
        script.push_int(1);
        script.push_opcode(Opcode::OpIf);
        script.push_int(2);
        script.push_opcode(Opcode::OpElse);
        script.push_int(3);
        script.push_opcode(Opcode::OpEndIf);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
    }

    #[test]
    fn test_op_if_false_branch() {
        // OP_0 OP_IF OP_2 OP_ELSE OP_3 OP_ENDIF
        let mut script = Script::new();
        script.push_int(0);
        script.push_opcode(Opcode::OpIf);
        script.push_int(2);
        script.push_opcode(Opcode::OpElse);
        script.push_int(3);
        script.push_opcode(Opcode::OpEndIf);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(3).to_bytes());
    }

    #[test]
    fn test_op_notif() {
        // OP_0 OP_NOTIF OP_2 OP_ENDIF (should execute)
        let mut script = Script::new();
        script.push_int(0);
        script.push_opcode(Opcode::OpNotIf);
        script.push_int(2);
        script.push_opcode(Opcode::OpEndIf);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
    }

    #[test]
    fn test_nested_if() {
        // OP_1 OP_IF OP_1 OP_IF OP_5 OP_ENDIF OP_ENDIF
        let mut script = Script::new();
        script.push_int(1);
        script.push_opcode(Opcode::OpIf);
        script.push_int(1);
        script.push_opcode(Opcode::OpIf);
        script.push_int(5);
        script.push_opcode(Opcode::OpEndIf);
        script.push_opcode(Opcode::OpEndIf);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(5).to_bytes());
    }

    #[test]
    fn test_unbalanced_if() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_opcode(Opcode::OpIf);
        script.push_int(2);
        // missing ENDIF
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::UnbalancedConditional);
    }

    // --- Equal/EqualVerify tests ---

    #[test]
    fn test_op_equal() {
        let mut script = Script::new();
        script.push_data(&[1, 2, 3]);
        script.push_data(&[1, 2, 3]);
        script.push_opcode(Opcode::OpEqual);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_equal_not_equal() {
        let mut script = Script::new();
        script.push_data(&[1, 2, 3]);
        script.push_data(&[4, 5, 6]);
        script.push_opcode(Opcode::OpEqual);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(!cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_equalverify_success() {
        let mut script = Script::new();
        script.push_data(&[1, 2, 3]);
        script.push_data(&[1, 2, 3]);
        script.push_opcode(Opcode::OpEqualVerify);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 0);
    }

    #[test]
    fn test_op_equalverify_fail() {
        let mut script = Script::new();
        script.push_data(&[1, 2, 3]);
        script.push_data(&[4, 5, 6]);
        script.push_opcode(Opcode::OpEqualVerify);
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::EqualVerify);
    }

    // --- Stack manipulation tests ---

    #[test]
    fn test_op_dup() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_opcode(Opcode::OpDup);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 2);
        assert_eq!(stack.top(-1).unwrap(), stack.top(-2).unwrap());
    }

    #[test]
    fn test_op_drop() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_opcode(Opcode::OpDrop);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 1);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());
    }

    #[test]
    fn test_op_swap() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_opcode(Opcode::OpSwap);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());
        assert_eq!(stack.top(-2).unwrap(), &ScriptNum::new(2).to_bytes());
    }

    #[test]
    fn test_op_over() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_opcode(Opcode::OpOver);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 3);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());
    }

    #[test]
    fn test_op_rot() {
        // [1, 2, 3] -> [2, 3, 1]
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_int(3);
        script.push_opcode(Opcode::OpRot);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(1).to_bytes());
        assert_eq!(stack.top(-2).unwrap(), &ScriptNum::new(3).to_bytes());
        assert_eq!(stack.top(-3).unwrap(), &ScriptNum::new(2).to_bytes());
    }

    #[test]
    fn test_op_nip() {
        // [1, 2] -> [2]
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_opcode(Opcode::OpNip);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 1);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
    }

    #[test]
    fn test_op_tuck() {
        // [1, 2] -> [2, 1, 2]
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_opcode(Opcode::OpTuck);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 3);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
        assert_eq!(stack.top(-2).unwrap(), &ScriptNum::new(1).to_bytes());
        assert_eq!(stack.top(-3).unwrap(), &ScriptNum::new(2).to_bytes());
    }

    #[test]
    fn test_op_2dup() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_opcode(Opcode::Op2Dup);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 4);
    }

    #[test]
    fn test_op_3dup() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_int(3);
        script.push_opcode(Opcode::Op3Dup);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 6);
    }

    #[test]
    fn test_op_2drop() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_int(3);
        script.push_opcode(Opcode::Op2Drop);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 1);
    }

    #[test]
    fn test_op_2over() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_int(3);
        script.push_int(4);
        script.push_opcode(Opcode::Op2Over);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 6);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
        assert_eq!(stack.top(-2).unwrap(), &ScriptNum::new(1).to_bytes());
    }

    #[test]
    fn test_op_2swap() {
        // [1, 2, 3, 4] -> [3, 4, 1, 2]
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_int(3);
        script.push_int(4);
        script.push_opcode(Opcode::Op2Swap);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
        assert_eq!(stack.top(-2).unwrap(), &ScriptNum::new(1).to_bytes());
        assert_eq!(stack.top(-3).unwrap(), &ScriptNum::new(4).to_bytes());
        assert_eq!(stack.top(-4).unwrap(), &ScriptNum::new(3).to_bytes());
    }

    #[test]
    fn test_op_pick() {
        let mut script = Script::new();
        script.push_int(10);
        script.push_int(20);
        script.push_int(30);
        script.push_int(2); // pick 3rd from top
        script.push_opcode(Opcode::OpPick);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 4);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(10).to_bytes());
    }

    #[test]
    fn test_op_roll() {
        let mut script = Script::new();
        script.push_int(10);
        script.push_int(20);
        script.push_int(30);
        script.push_int(2); // roll 3rd from top to top
        script.push_opcode(Opcode::OpRoll);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 3);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(10).to_bytes());
    }

    #[test]
    fn test_op_depth() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_opcode(Opcode::OpDepth);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
    }

    #[test]
    fn test_op_ifdup_true() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_opcode(Opcode::OpIfDup);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 2);
    }

    #[test]
    fn test_op_ifdup_false() {
        let mut script = Script::new();
        script.push_int(0);
        script.push_opcode(Opcode::OpIfDup);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 1);
    }

    #[test]
    fn test_op_size() {
        let mut script = Script::new();
        script.push_data(&[1, 2, 3, 4, 5]);
        script.push_opcode(Opcode::OpSize);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(5).to_bytes());
    }

    #[test]
    fn test_op_toaltstack_fromaltstack() {
        let mut script = Script::new();
        script.push_int(42);
        script.push_opcode(Opcode::OpToAltStack);
        script.push_opcode(Opcode::OpFromAltStack);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 1);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(42).to_bytes());
    }

    // --- Crypto tests ---

    #[test]
    fn test_op_sha256() {
        let mut script = Script::new();
        script.push_data(b"abc");
        script.push_opcode(Opcode::OpSha256);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        let expected = qubitcoin_crypto::hash::sha256_hash(b"abc");
        assert_eq!(stack.top(-1).unwrap(), &expected.to_vec());
    }

    #[test]
    fn test_op_hash160() {
        let mut script = Script::new();
        script.push_data(b"abc");
        script.push_opcode(Opcode::OpHash160);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        let expected = qubitcoin_crypto::hash::hash160(b"abc");
        assert_eq!(stack.top(-1).unwrap(), &expected.to_vec());
    }

    #[test]
    fn test_op_hash256() {
        let mut script = Script::new();
        script.push_data(b"abc");
        script.push_opcode(Opcode::OpHash256);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        let expected = qubitcoin_crypto::hash::hash256(b"abc");
        assert_eq!(stack.top(-1).unwrap(), &expected.to_vec());
    }

    #[test]
    fn test_op_ripemd160() {
        let mut script = Script::new();
        script.push_data(b"abc");
        script.push_opcode(Opcode::OpRipemd160);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        let expected = qubitcoin_crypto::hash::ripemd160_hash(b"abc");
        assert_eq!(stack.top(-1).unwrap(), &expected.to_vec());
    }

    #[test]
    fn test_op_sha1() {
        let mut script = Script::new();
        script.push_data(b"abc");
        script.push_opcode(Opcode::OpSha1);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        let expected = qubitcoin_crypto::hash::sha1_hash(b"abc");
        assert_eq!(stack.top(-1).unwrap(), &expected.to_vec());
    }

    // --- OP_CHECKSIG with BaseSignatureChecker (always returns false) ---

    #[test]
    fn test_op_checksig_base_checker() {
        let mut script = Script::new();
        script.push_data(&[0u8; 72]); // fake sig
        script.push_data(&[0u8; 33]); // fake pubkey
        script.push_opcode(Opcode::OpCheckSig);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        // BaseSignatureChecker returns false
        assert!(!cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_checksigverify_base_checker_fails() {
        let mut script = Script::new();
        script.push_data(&[0u8; 72]);
        script.push_data(&[0u8; 33]);
        script.push_opcode(Opcode::OpCheckSigVerify);
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::CheckSigVerify);
    }

    // --- OP_RETURN ---

    #[test]
    fn test_op_return() {
        let mut script = Script::new();
        script.push_opcode(Opcode::OpReturn);
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::OpReturn);
    }

    // --- OP_VERIFY ---

    #[test]
    fn test_op_verify_true() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_opcode(Opcode::OpVerify);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 0);
    }

    #[test]
    fn test_op_verify_false() {
        let mut script = Script::new();
        script.push_int(0);
        script.push_opcode(Opcode::OpVerify);
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::Verify);
    }

    // --- Disabled opcodes ---

    #[test]
    fn test_disabled_opcodes() {
        for op in &[
            Opcode::OpCat,
            Opcode::OpMul,
            Opcode::OpLShift,
            Opcode::Op2Mul,
        ] {
            let mut script = Script::new();
            script.push_int(1);
            script.push_opcode(Opcode::OpIf); // wrap in false branch
            script.push_opcode(*op);
            script.push_opcode(Opcode::OpEndIf);
            let (ok, err, _) = run_script(&script);
            // Disabled opcodes fail even in unexecuted branches
            assert!(!ok, "Expected failure for {:?}", op);
            assert_eq!(err, ScriptError::DisabledOpcode);
        }
    }

    // --- Stack size limit ---

    #[test]
    fn test_stack_size_limit() {
        let mut script = Script::new();
        // Push 1001 items (exceeds MAX_STACK_SIZE = 1000)
        for _ in 0..=1000 {
            script.push_int(1);
        }
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::StackSize);
    }

    // --- Op count limit ---

    #[test]
    fn test_op_count_limit() {
        let mut script = Script::new();
        script.push_int(1);
        // OP_NOP is > OP_16, so it counts toward op limit
        for _ in 0..=201 {
            script.push_opcode(Opcode::OpNop);
        }
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::OpCount);
    }

    // --- NOP tests ---

    #[test]
    fn test_op_nop() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_opcode(Opcode::OpNop);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 1);
    }

    #[test]
    fn test_discourage_upgradable_nops() {
        let mut script = Script::new();
        script.push_int(1);
        script.push_opcode(Opcode::OpNop1);
        let flags = ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_NOPS;
        let (ok, err, _) = run_script_flags(&script, &flags);
        assert!(!ok);
        assert_eq!(err, ScriptError::DiscourageUpgradableNops);
    }

    // --- NumEqualVerify ---

    #[test]
    fn test_op_numequalverify_success() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_int(5);
        script.push_opcode(Opcode::OpNumEqualVerify);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 0);
    }

    #[test]
    fn test_op_numequalverify_fail() {
        let mut script = Script::new();
        script.push_int(5);
        script.push_int(6);
        script.push_opcode(Opcode::OpNumEqualVerify);
        let (ok, err, _) = run_script(&script);
        assert!(!ok);
        assert_eq!(err, ScriptError::NumEqualVerify);
    }

    // --- verify_script tests ---

    #[test]
    fn test_verify_script_p2pkh_trivial() {
        // Build a script that always succeeds: scriptSig pushes OP_TRUE,
        // scriptPubKey is empty -- result is OP_TRUE on stack
        let mut script_sig = Script::new();
        script_sig.push_int(1);
        let script_pubkey = Script::new();
        let witness = ScriptWitness::new();
        let flags = ScriptVerifyFlags::NONE;
        let checker = BaseSignatureChecker;
        let mut error = ScriptError::Ok;

        let ok = verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &flags,
            &checker,
            &mut error,
        );
        assert!(ok);
    }

    #[test]
    fn test_verify_script_eval_false() {
        // scriptSig pushes 0, scriptPubKey is empty -> eval_false
        let mut script_sig = Script::new();
        script_sig.push_int(0);
        let script_pubkey = Script::new();
        let witness = ScriptWitness::new();
        let flags = ScriptVerifyFlags::NONE;
        let checker = BaseSignatureChecker;
        let mut error = ScriptError::Ok;

        let ok = verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &flags,
            &checker,
            &mut error,
        );
        assert!(!ok);
        assert_eq!(error, ScriptError::EvalFalse);
    }

    #[test]
    fn test_verify_script_p2sh_simple() {
        // Create a P2SH script where the redeem script is just OP_1
        let redeem_script_bytes = vec![Opcode::Op1 as u8];
        let redeem_hash = qubitcoin_crypto::hash::hash160(&redeem_script_bytes);

        // scriptPubKey: OP_HASH160 <hash> OP_EQUAL
        let script_pubkey = crate::script::build_p2sh(&redeem_hash);

        // scriptSig: push the redeem script
        let mut script_sig = Script::new();
        script_sig.push_data(&redeem_script_bytes);

        let witness = ScriptWitness::new();
        let flags = ScriptVerifyFlags::P2SH;
        let checker = BaseSignatureChecker;
        let mut error = ScriptError::Ok;

        let ok = verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &flags,
            &checker,
            &mut error,
        );
        assert!(ok);
    }

    #[test]
    fn test_verify_script_p2sh_fail() {
        // Create a P2SH script where the redeem script is OP_0 (fails)
        let redeem_script_bytes = vec![Opcode::Op0 as u8];
        let redeem_hash = qubitcoin_crypto::hash::hash160(&redeem_script_bytes);

        let script_pubkey = crate::script::build_p2sh(&redeem_hash);

        let mut script_sig = Script::new();
        script_sig.push_data(&redeem_script_bytes);

        let witness = ScriptWitness::new();
        let flags = ScriptVerifyFlags::P2SH;
        let checker = BaseSignatureChecker;
        let mut error = ScriptError::Ok;

        let ok = verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &flags,
            &checker,
            &mut error,
        );
        assert!(!ok);
        assert_eq!(error, ScriptError::EvalFalse);
    }

    #[test]
    fn test_verify_script_p2pkh_structure() {
        // Test P2PKH structure: scriptSig pushes sig + pubkey,
        // scriptPubKey is OP_DUP OP_HASH160 <hash> OP_EQUALVERIFY OP_CHECKSIG
        // With BaseSignatureChecker, CHECKSIG always returns false.
        // So this should fail at CHECKSIGVERIFY effectively (or return false from CHECKSIG).

        let fake_pubkey = vec![0x02; 33]; // compressed pubkey prefix
        let pubkey_hash = qubitcoin_crypto::hash::hash160(&fake_pubkey);

        let script_pubkey = crate::script::build_p2pkh(&pubkey_hash);

        let mut script_sig = Script::new();
        script_sig.push_data(&vec![0u8; 72]); // fake sig
        script_sig.push_data(&fake_pubkey);

        let witness = ScriptWitness::new();
        let flags = ScriptVerifyFlags::NONE;
        let checker = BaseSignatureChecker;
        let mut error = ScriptError::Ok;

        let ok = verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &flags,
            &checker,
            &mut error,
        );
        // Should fail because BaseSignatureChecker returns false for CHECKSIG
        assert!(!ok);
        assert_eq!(error, ScriptError::EvalFalse);
    }

    // --- 2ROT test ---

    #[test]
    fn test_op_2rot() {
        // [1, 2, 3, 4, 5, 6] -> [3, 4, 5, 6, 1, 2]
        let mut script = Script::new();
        script.push_int(1);
        script.push_int(2);
        script.push_int(3);
        script.push_int(4);
        script.push_int(5);
        script.push_int(6);
        script.push_opcode(Opcode::Op2Rot);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert_eq!(stack.size(), 6);
        assert_eq!(stack.top(-1).unwrap(), &ScriptNum::new(2).to_bytes());
        assert_eq!(stack.top(-2).unwrap(), &ScriptNum::new(1).to_bytes());
        assert_eq!(stack.top(-3).unwrap(), &ScriptNum::new(6).to_bytes());
    }

    // --- CHECKMULTISIG with BaseSignatureChecker ---

    #[test]
    fn test_op_checkmultisig_0_of_0() {
        // 0-of-0 multisig should succeed
        let mut script = Script::new();
        script.push_int(0); // dummy element (bug compat)
        script.push_int(0); // nsigs
        script.push_int(0); // nkeys
        script.push_opcode(Opcode::OpCheckMultiSig);
        let (ok, _, stack) = run_script(&script);
        assert!(ok);
        assert!(cast_to_bool(stack.top(-1).unwrap()));
    }

    #[test]
    fn test_op_checkmultisig_requires_dummy() {
        // CHECKMULTISIG without dummy element should fail with empty stack
        let mut script = Script::new();
        script.push_int(0); // nsigs
        script.push_int(0); // nkeys
        script.push_opcode(Opcode::OpCheckMultiSig);
        let (ok, _, _) = run_script(&script);
        // It should succeed: stack has nsigs (0), nkeys (0), then consume 1 more dummy
        // But with only 2 items on stack and needing 3 (nkeys + nsigs + dummy), this fails
        assert!(!ok);
    }

    // --- Minimal data enforcement ---

    #[test]
    fn test_minimal_data_enforcement() {
        // Push 1 using OP_PUSHDATA1 instead of direct push (non-minimal)
        let script = Script::from_bytes(vec![
            Opcode::OpPushData1 as u8,
            0x01,
            0x01, // PUSHDATA1 with 1 byte
        ]);
        let flags = ScriptVerifyFlags::MINIMALDATA;
        let (ok, err, _) = run_script_flags(&script, &flags);
        assert!(!ok);
        assert_eq!(err, ScriptError::MinimalData);
    }
}
