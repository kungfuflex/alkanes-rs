//! Bitcoin Script opcodes.
//!
//! Maps to: `src/script/script.h` (`opcodetype` enum) in Bitcoin Core.
//!
//! Complete enumeration of all Bitcoin Script opcodes including
//! disabled, reserved, and tapscript opcodes.

/// All Bitcoin Script opcodes.
///
/// Port of Bitcoin Core's `opcodetype` enum. The discriminant values match the
/// byte values used in serialized scripts, so `Opcode as u8` gives the on-wire byte.
///
/// Opcodes 0x01..=0x4b are *direct data push* instructions (push the next N
/// bytes) and are handled by the script parser rather than this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Opcode {
    // -- Push value --

    /// Push an empty byte vector onto the stack (also known as `OP_FALSE`).
    Op0 = 0x00,
    /// The next byte contains the number of bytes to push.
    OpPushData1 = 0x4c,
    /// The next two bytes (little-endian) contain the number of bytes to push.
    OpPushData2 = 0x4d,
    /// The next four bytes (little-endian) contain the number of bytes to push.
    OpPushData4 = 0x4e,
    /// Push the number -1 onto the stack.
    Op1Negate = 0x4f,
    /// Reserved opcode. Transaction is invalid unless found in an unexecuted `OP_IF` branch.
    OpReserved = 0x50,
    /// Push the number 1 onto the stack (also known as `OP_TRUE`).
    Op1 = 0x51,
    /// Push the number 2 onto the stack.
    Op2 = 0x52,
    /// Push the number 3 onto the stack.
    Op3 = 0x53,
    /// Push the number 4 onto the stack.
    Op4 = 0x54,
    /// Push the number 5 onto the stack.
    Op5 = 0x55,
    /// Push the number 6 onto the stack.
    Op6 = 0x56,
    /// Push the number 7 onto the stack.
    Op7 = 0x57,
    /// Push the number 8 onto the stack.
    Op8 = 0x58,
    /// Push the number 9 onto the stack.
    Op9 = 0x59,
    /// Push the number 10 onto the stack.
    Op10 = 0x5a,
    /// Push the number 11 onto the stack.
    Op11 = 0x5b,
    /// Push the number 12 onto the stack.
    Op12 = 0x5c,
    /// Push the number 13 onto the stack.
    Op13 = 0x5d,
    /// Push the number 14 onto the stack.
    Op14 = 0x5e,
    /// Push the number 15 onto the stack.
    Op15 = 0x5f,
    /// Push the number 16 onto the stack.
    Op16 = 0x60,

    // -- Control flow --

    /// Does nothing.
    OpNop = 0x61,
    /// Reserved. Transaction is invalid unless in an unexecuted `OP_IF` branch.
    OpVer = 0x62,
    /// Execute the following statements only if the top stack value is true.
    OpIf = 0x63,
    /// Execute the following statements only if the top stack value is false.
    OpNotIf = 0x64,
    /// Reserved. Transaction is invalid even in an unexecuted `OP_IF` branch.
    OpVerIf = 0x65,
    /// Reserved. Transaction is invalid even in an unexecuted `OP_IF` branch.
    OpVerNotIf = 0x66,
    /// Execute the following statements if the preceding `OP_IF`/`OP_NOTIF` was not executed.
    OpElse = 0x67,
    /// End an `OP_IF`/`OP_NOTIF` block.
    OpEndIf = 0x68,
    /// Mark the transaction as invalid if the top stack value is false.
    OpVerify = 0x69,
    /// Mark the transaction as invalid. Used for provably unspendable outputs.
    OpReturn = 0x6a,

    // -- Stack manipulation --

    /// Move the top stack item to the alt stack.
    OpToAltStack = 0x6b,
    /// Move the top alt-stack item to the main stack.
    OpFromAltStack = 0x6c,
    /// Remove the top two stack items.
    Op2Drop = 0x6d,
    /// Duplicate the top two stack items.
    Op2Dup = 0x6e,
    /// Duplicate the top three stack items.
    Op3Dup = 0x6f,
    /// Copy items 3 and 4 to the top of the stack.
    Op2Over = 0x70,
    /// Move items 5 and 6 to the top of the stack.
    Op2Rot = 0x71,
    /// Swap the top two pairs of items.
    Op2Swap = 0x72,
    /// Duplicate the top stack value if it is non-zero.
    OpIfDup = 0x73,
    /// Push the number of stack items onto the stack.
    OpDepth = 0x74,
    /// Remove the top stack item.
    OpDrop = 0x75,
    /// Duplicate the top stack item.
    OpDup = 0x76,
    /// Remove the second-to-top stack item.
    OpNip = 0x77,
    /// Copy the second-to-top stack item to the top.
    OpOver = 0x78,
    /// Copy the item N levels back in the stack to the top.
    OpPick = 0x79,
    /// Move the item N levels back in the stack to the top.
    OpRoll = 0x7a,
    /// Rotate the top three items: (x1 x2 x3 -> x2 x3 x1).
    OpRot = 0x7b,
    /// Swap the top two stack items.
    OpSwap = 0x7c,
    /// Copy the top item and insert it before the second-to-top item.
    OpTuck = 0x7d,

    // -- Splice (disabled) --

    /// Concatenate two strings. **Disabled.**
    OpCat = 0x7e,
    /// Return a section of a string. **Disabled.**
    OpSubStr = 0x7f,
    /// Keep only characters left of a specified point. **Disabled.**
    OpLeft = 0x80,
    /// Keep only characters right of a specified point. **Disabled.**
    OpRight = 0x81,
    /// Push the byte-length of the top stack item.
    OpSize = 0x82,

    // -- Bitwise logic (disabled except `OP_EQUAL`) --

    /// Flip all bits of the input. **Disabled.**
    OpInvert = 0x83,
    /// Bitwise AND of two values. **Disabled.**
    OpAnd = 0x84,
    /// Bitwise OR of two values. **Disabled.**
    OpOr = 0x85,
    /// Bitwise XOR of two values. **Disabled.**
    OpXor = 0x86,
    /// Push 1 if the top two items are byte-for-byte equal, 0 otherwise.
    OpEqual = 0x87,
    /// Same as `OP_EQUAL` followed by `OP_VERIFY`.
    OpEqualVerify = 0x88,
    /// Reserved. Transaction is invalid unless in an unexecuted `OP_IF` branch.
    OpReserved1 = 0x89,
    /// Reserved. Transaction is invalid unless in an unexecuted `OP_IF` branch.
    OpReserved2 = 0x8a,

    // -- Numeric --

    /// Add 1 to the top stack item.
    Op1Add = 0x8b,
    /// Subtract 1 from the top stack item.
    Op1Sub = 0x8c,
    /// Multiply the top item by 2. **Disabled.**
    Op2Mul = 0x8d,
    /// Divide the top item by 2. **Disabled.**
    Op2Div = 0x8e,
    /// Negate the sign of the top stack item.
    OpNegate = 0x8f,
    /// Replace the top item with its absolute value.
    OpAbs = 0x90,
    /// If the top item is 0 or 1, flip it; otherwise push 0.
    OpNot = 0x91,
    /// Push 0 if the top item is 0, otherwise push 1.
    Op0NotEqual = 0x92,
    /// Pop two items, push their sum.
    OpAdd = 0x93,
    /// Pop two items, push a - b.
    OpSub = 0x94,
    /// Multiply two items. **Disabled.**
    OpMul = 0x95,
    /// Divide two items. **Disabled.**
    OpDiv = 0x96,
    /// Modulo of two items. **Disabled.**
    OpMod = 0x97,
    /// Left-shift. **Disabled.**
    OpLShift = 0x98,
    /// Right-shift. **Disabled.**
    OpRShift = 0x99,
    /// Push 1 if both inputs are non-zero, otherwise 0.
    OpBoolAnd = 0x9a,
    /// Push 1 if either input is non-zero, otherwise 0.
    OpBoolOr = 0x9b,
    /// Push 1 if the two numbers are equal, otherwise 0.
    OpNumEqual = 0x9c,
    /// Same as `OP_NUMEQUAL` followed by `OP_VERIFY`.
    OpNumEqualVerify = 0x9d,
    /// Push 1 if the two numbers are not equal, otherwise 0.
    OpNumNotEqual = 0x9e,
    /// Push 1 if a < b, otherwise 0.
    OpLessThan = 0x9f,
    /// Push 1 if a > b, otherwise 0.
    OpGreaterThan = 0xa0,
    /// Push 1 if a <= b, otherwise 0.
    OpLessThanOrEqual = 0xa1,
    /// Push 1 if a >= b, otherwise 0.
    OpGreaterThanOrEqual = 0xa2,
    /// Push the smaller of two items.
    OpMin = 0xa3,
    /// Push the larger of two items.
    OpMax = 0xa4,
    /// Push 1 if x is within [min, max), otherwise 0.
    OpWithin = 0xa5,

    // -- Crypto --

    /// Hash the top item with RIPEMD-160.
    OpRipemd160 = 0xa6,
    /// Hash the top item with SHA-1.
    OpSha1 = 0xa7,
    /// Hash the top item with SHA-256.
    OpSha256 = 0xa8,
    /// Hash the top item with SHA-256 then RIPEMD-160 (= Hash160).
    OpHash160 = 0xa9,
    /// Hash the top item with double SHA-256 (= Hash256).
    OpHash256 = 0xaa,
    /// Mark the boundary for signature hashing (affects `FindAndDelete`).
    OpCodeSeparator = 0xab,
    /// Pop a signature and public key; push 1 if the signature is valid, 0 otherwise.
    OpCheckSig = 0xac,
    /// Same as `OP_CHECKSIG` followed by `OP_VERIFY`.
    OpCheckSigVerify = 0xad,
    /// Pop M signatures and N public keys; push 1 if all signatures are valid.
    OpCheckMultiSig = 0xae,
    /// Same as `OP_CHECKMULTISIG` followed by `OP_VERIFY`.
    OpCheckMultiSigVerify = 0xaf,

    // -- Expansion / NOP --

    /// No operation. Reserved for future soft-fork upgrades.
    OpNop1 = 0xb0,
    /// Verify that the top stack value >= the transaction's `nLockTime` (BIP 65).
    OpCheckLockTimeVerify = 0xb1,
    /// Verify that the top stack value matches the input's `nSequence` (BIP 112).
    OpCheckSequenceVerify = 0xb2,
    /// No operation. Reserved for future soft-fork upgrades.
    OpNop4 = 0xb3,
    /// No operation. Reserved for future soft-fork upgrades.
    OpNop5 = 0xb4,
    /// No operation. Reserved for future soft-fork upgrades.
    OpNop6 = 0xb5,
    /// No operation. Reserved for future soft-fork upgrades.
    OpNop7 = 0xb6,
    /// No operation. Reserved for future soft-fork upgrades.
    OpNop8 = 0xb7,
    /// No operation. Reserved for future soft-fork upgrades.
    OpNop9 = 0xb8,
    /// No operation. Reserved for future soft-fork upgrades.
    OpNop10 = 0xb9,

    // -- BIP 342 Tapscript --

    /// Tapscript multi-signature accumulator: pops sig, pubkey, and n; pushes n+1 or n (BIP 342).
    OpCheckSigAdd = 0xba,

    // -- Invalid --

    /// Sentinel value representing an invalid or unrecognized opcode.
    OpInvalidOpcode = 0xff,
}

/// Alias: `OP_FALSE` is the same as [`Opcode::Op0`].
pub const OP_FALSE: Opcode = Opcode::Op0;
/// Alias: `OP_TRUE` is the same as [`Opcode::Op1`].
pub const OP_TRUE: Opcode = Opcode::Op1;
/// Alias: `OP_NOP2` is the pre-BIP-65 name for [`Opcode::OpCheckLockTimeVerify`].
pub const OP_NOP2: Opcode = Opcode::OpCheckLockTimeVerify;
/// Alias: `OP_NOP3` is the pre-BIP-112 name for [`Opcode::OpCheckSequenceVerify`].
pub const OP_NOP3: Opcode = Opcode::OpCheckSequenceVerify;

/// Maximum valid opcode byte value (`OP_NOP10` = `0xb9`).
pub const MAX_OPCODE: u8 = Opcode::OpNop10 as u8;

impl Opcode {
    /// Converts a raw byte to the corresponding `Opcode`, if one exists.
    ///
    /// Returns `None` for direct-push bytes (0x01..=0x4b) and for undefined
    /// bytes in the range 0xbb..=0xfe. Those ranges are handled separately by
    /// the script parser.
    pub fn from_u8(byte: u8) -> Option<Opcode> {
        // Direct data push opcodes (0x01..=0x4b) are not in the enum
        // They represent "push next N bytes" and are handled by the script parser
        match byte {
            0x00 => Some(Opcode::Op0),
            0x4c => Some(Opcode::OpPushData1),
            0x4d => Some(Opcode::OpPushData2),
            0x4e => Some(Opcode::OpPushData4),
            0x4f => Some(Opcode::Op1Negate),
            0x50 => Some(Opcode::OpReserved),
            0x51 => Some(Opcode::Op1),
            0x52 => Some(Opcode::Op2),
            0x53 => Some(Opcode::Op3),
            0x54 => Some(Opcode::Op4),
            0x55 => Some(Opcode::Op5),
            0x56 => Some(Opcode::Op6),
            0x57 => Some(Opcode::Op7),
            0x58 => Some(Opcode::Op8),
            0x59 => Some(Opcode::Op9),
            0x5a => Some(Opcode::Op10),
            0x5b => Some(Opcode::Op11),
            0x5c => Some(Opcode::Op12),
            0x5d => Some(Opcode::Op13),
            0x5e => Some(Opcode::Op14),
            0x5f => Some(Opcode::Op15),
            0x60 => Some(Opcode::Op16),
            0x61 => Some(Opcode::OpNop),
            0x62 => Some(Opcode::OpVer),
            0x63 => Some(Opcode::OpIf),
            0x64 => Some(Opcode::OpNotIf),
            0x65 => Some(Opcode::OpVerIf),
            0x66 => Some(Opcode::OpVerNotIf),
            0x67 => Some(Opcode::OpElse),
            0x68 => Some(Opcode::OpEndIf),
            0x69 => Some(Opcode::OpVerify),
            0x6a => Some(Opcode::OpReturn),
            0x6b => Some(Opcode::OpToAltStack),
            0x6c => Some(Opcode::OpFromAltStack),
            0x6d => Some(Opcode::Op2Drop),
            0x6e => Some(Opcode::Op2Dup),
            0x6f => Some(Opcode::Op3Dup),
            0x70 => Some(Opcode::Op2Over),
            0x71 => Some(Opcode::Op2Rot),
            0x72 => Some(Opcode::Op2Swap),
            0x73 => Some(Opcode::OpIfDup),
            0x74 => Some(Opcode::OpDepth),
            0x75 => Some(Opcode::OpDrop),
            0x76 => Some(Opcode::OpDup),
            0x77 => Some(Opcode::OpNip),
            0x78 => Some(Opcode::OpOver),
            0x79 => Some(Opcode::OpPick),
            0x7a => Some(Opcode::OpRoll),
            0x7b => Some(Opcode::OpRot),
            0x7c => Some(Opcode::OpSwap),
            0x7d => Some(Opcode::OpTuck),
            0x7e => Some(Opcode::OpCat),
            0x7f => Some(Opcode::OpSubStr),
            0x80 => Some(Opcode::OpLeft),
            0x81 => Some(Opcode::OpRight),
            0x82 => Some(Opcode::OpSize),
            0x83 => Some(Opcode::OpInvert),
            0x84 => Some(Opcode::OpAnd),
            0x85 => Some(Opcode::OpOr),
            0x86 => Some(Opcode::OpXor),
            0x87 => Some(Opcode::OpEqual),
            0x88 => Some(Opcode::OpEqualVerify),
            0x89 => Some(Opcode::OpReserved1),
            0x8a => Some(Opcode::OpReserved2),
            0x8b => Some(Opcode::Op1Add),
            0x8c => Some(Opcode::Op1Sub),
            0x8d => Some(Opcode::Op2Mul),
            0x8e => Some(Opcode::Op2Div),
            0x8f => Some(Opcode::OpNegate),
            0x90 => Some(Opcode::OpAbs),
            0x91 => Some(Opcode::OpNot),
            0x92 => Some(Opcode::Op0NotEqual),
            0x93 => Some(Opcode::OpAdd),
            0x94 => Some(Opcode::OpSub),
            0x95 => Some(Opcode::OpMul),
            0x96 => Some(Opcode::OpDiv),
            0x97 => Some(Opcode::OpMod),
            0x98 => Some(Opcode::OpLShift),
            0x99 => Some(Opcode::OpRShift),
            0x9a => Some(Opcode::OpBoolAnd),
            0x9b => Some(Opcode::OpBoolOr),
            0x9c => Some(Opcode::OpNumEqual),
            0x9d => Some(Opcode::OpNumEqualVerify),
            0x9e => Some(Opcode::OpNumNotEqual),
            0x9f => Some(Opcode::OpLessThan),
            0xa0 => Some(Opcode::OpGreaterThan),
            0xa1 => Some(Opcode::OpLessThanOrEqual),
            0xa2 => Some(Opcode::OpGreaterThanOrEqual),
            0xa3 => Some(Opcode::OpMin),
            0xa4 => Some(Opcode::OpMax),
            0xa5 => Some(Opcode::OpWithin),
            0xa6 => Some(Opcode::OpRipemd160),
            0xa7 => Some(Opcode::OpSha1),
            0xa8 => Some(Opcode::OpSha256),
            0xa9 => Some(Opcode::OpHash160),
            0xaa => Some(Opcode::OpHash256),
            0xab => Some(Opcode::OpCodeSeparator),
            0xac => Some(Opcode::OpCheckSig),
            0xad => Some(Opcode::OpCheckSigVerify),
            0xae => Some(Opcode::OpCheckMultiSig),
            0xaf => Some(Opcode::OpCheckMultiSigVerify),
            0xb0 => Some(Opcode::OpNop1),
            0xb1 => Some(Opcode::OpCheckLockTimeVerify),
            0xb2 => Some(Opcode::OpCheckSequenceVerify),
            0xb3 => Some(Opcode::OpNop4),
            0xb4 => Some(Opcode::OpNop5),
            0xb5 => Some(Opcode::OpNop6),
            0xb6 => Some(Opcode::OpNop7),
            0xb7 => Some(Opcode::OpNop8),
            0xb8 => Some(Opcode::OpNop9),
            0xb9 => Some(Opcode::OpNop10),
            0xba => Some(Opcode::OpCheckSigAdd),
            0xff => Some(Opcode::OpInvalidOpcode),
            // 0x01..=0x4b are direct push opcodes, handled by script parser
            // 0xbb..=0xfe are undefined but still valid bytes in script
            _ => None,
        }
    }

    /// Returns the human-readable name of this opcode (e.g. `"OP_DUP"`).
    ///
    /// Matches Bitcoin Core's `GetOpName()`.
    pub fn name(&self) -> &'static str {
        match self {
            Opcode::Op0 => "OP_0",
            Opcode::OpPushData1 => "OP_PUSHDATA1",
            Opcode::OpPushData2 => "OP_PUSHDATA2",
            Opcode::OpPushData4 => "OP_PUSHDATA4",
            Opcode::Op1Negate => "OP_1NEGATE",
            Opcode::OpReserved => "OP_RESERVED",
            Opcode::Op1 => "OP_1",
            Opcode::Op2 => "OP_2",
            Opcode::Op3 => "OP_3",
            Opcode::Op4 => "OP_4",
            Opcode::Op5 => "OP_5",
            Opcode::Op6 => "OP_6",
            Opcode::Op7 => "OP_7",
            Opcode::Op8 => "OP_8",
            Opcode::Op9 => "OP_9",
            Opcode::Op10 => "OP_10",
            Opcode::Op11 => "OP_11",
            Opcode::Op12 => "OP_12",
            Opcode::Op13 => "OP_13",
            Opcode::Op14 => "OP_14",
            Opcode::Op15 => "OP_15",
            Opcode::Op16 => "OP_16",
            Opcode::OpNop => "OP_NOP",
            Opcode::OpVer => "OP_VER",
            Opcode::OpIf => "OP_IF",
            Opcode::OpNotIf => "OP_NOTIF",
            Opcode::OpVerIf => "OP_VERIF",
            Opcode::OpVerNotIf => "OP_VERNOTIF",
            Opcode::OpElse => "OP_ELSE",
            Opcode::OpEndIf => "OP_ENDIF",
            Opcode::OpVerify => "OP_VERIFY",
            Opcode::OpReturn => "OP_RETURN",
            Opcode::OpToAltStack => "OP_TOALTSTACK",
            Opcode::OpFromAltStack => "OP_FROMALTSTACK",
            Opcode::Op2Drop => "OP_2DROP",
            Opcode::Op2Dup => "OP_2DUP",
            Opcode::Op3Dup => "OP_3DUP",
            Opcode::Op2Over => "OP_2OVER",
            Opcode::Op2Rot => "OP_2ROT",
            Opcode::Op2Swap => "OP_2SWAP",
            Opcode::OpIfDup => "OP_IFDUP",
            Opcode::OpDepth => "OP_DEPTH",
            Opcode::OpDrop => "OP_DROP",
            Opcode::OpDup => "OP_DUP",
            Opcode::OpNip => "OP_NIP",
            Opcode::OpOver => "OP_OVER",
            Opcode::OpPick => "OP_PICK",
            Opcode::OpRoll => "OP_ROLL",
            Opcode::OpRot => "OP_ROT",
            Opcode::OpSwap => "OP_SWAP",
            Opcode::OpTuck => "OP_TUCK",
            Opcode::OpCat => "OP_CAT",
            Opcode::OpSubStr => "OP_SUBSTR",
            Opcode::OpLeft => "OP_LEFT",
            Opcode::OpRight => "OP_RIGHT",
            Opcode::OpSize => "OP_SIZE",
            Opcode::OpInvert => "OP_INVERT",
            Opcode::OpAnd => "OP_AND",
            Opcode::OpOr => "OP_OR",
            Opcode::OpXor => "OP_XOR",
            Opcode::OpEqual => "OP_EQUAL",
            Opcode::OpEqualVerify => "OP_EQUALVERIFY",
            Opcode::OpReserved1 => "OP_RESERVED1",
            Opcode::OpReserved2 => "OP_RESERVED2",
            Opcode::Op1Add => "OP_1ADD",
            Opcode::Op1Sub => "OP_1SUB",
            Opcode::Op2Mul => "OP_2MUL",
            Opcode::Op2Div => "OP_2DIV",
            Opcode::OpNegate => "OP_NEGATE",
            Opcode::OpAbs => "OP_ABS",
            Opcode::OpNot => "OP_NOT",
            Opcode::Op0NotEqual => "OP_0NOTEQUAL",
            Opcode::OpAdd => "OP_ADD",
            Opcode::OpSub => "OP_SUB",
            Opcode::OpMul => "OP_MUL",
            Opcode::OpDiv => "OP_DIV",
            Opcode::OpMod => "OP_MOD",
            Opcode::OpLShift => "OP_LSHIFT",
            Opcode::OpRShift => "OP_RSHIFT",
            Opcode::OpBoolAnd => "OP_BOOLAND",
            Opcode::OpBoolOr => "OP_BOOLOR",
            Opcode::OpNumEqual => "OP_NUMEQUAL",
            Opcode::OpNumEqualVerify => "OP_NUMEQUALVERIFY",
            Opcode::OpNumNotEqual => "OP_NUMNOTEQUAL",
            Opcode::OpLessThan => "OP_LESSTHAN",
            Opcode::OpGreaterThan => "OP_GREATERTHAN",
            Opcode::OpLessThanOrEqual => "OP_LESSTHANOREQUAL",
            Opcode::OpGreaterThanOrEqual => "OP_GREATERTHANOREQUAL",
            Opcode::OpMin => "OP_MIN",
            Opcode::OpMax => "OP_MAX",
            Opcode::OpWithin => "OP_WITHIN",
            Opcode::OpRipemd160 => "OP_RIPEMD160",
            Opcode::OpSha1 => "OP_SHA1",
            Opcode::OpSha256 => "OP_SHA256",
            Opcode::OpHash160 => "OP_HASH160",
            Opcode::OpHash256 => "OP_HASH256",
            Opcode::OpCodeSeparator => "OP_CODESEPARATOR",
            Opcode::OpCheckSig => "OP_CHECKSIG",
            Opcode::OpCheckSigVerify => "OP_CHECKSIGVERIFY",
            Opcode::OpCheckMultiSig => "OP_CHECKMULTISIG",
            Opcode::OpCheckMultiSigVerify => "OP_CHECKMULTISIGVERIFY",
            Opcode::OpNop1 => "OP_NOP1",
            Opcode::OpCheckLockTimeVerify => "OP_CHECKLOCKTIMEVERIFY",
            Opcode::OpCheckSequenceVerify => "OP_CHECKSEQUENCEVERIFY",
            Opcode::OpNop4 => "OP_NOP4",
            Opcode::OpNop5 => "OP_NOP5",
            Opcode::OpNop6 => "OP_NOP6",
            Opcode::OpNop7 => "OP_NOP7",
            Opcode::OpNop8 => "OP_NOP8",
            Opcode::OpNop9 => "OP_NOP9",
            Opcode::OpNop10 => "OP_NOP10",
            Opcode::OpCheckSigAdd => "OP_CHECKSIGADD",
            Opcode::OpInvalidOpcode => "OP_INVALIDOPCODE",
        }
    }

    /// Returns `true` if this opcode is a data-push instruction.
    ///
    /// This includes `OP_0`, `OP_PUSHDATA1`..`OP_PUSHDATA4`, `OP_1NEGATE`,
    /// and the small-integer pushes `OP_1`..`OP_16`.
    pub fn is_push(&self) -> bool {
        (*self as u8) <= Opcode::Op16 as u8
    }

    /// Returns `true` if this opcode is disabled in all script versions.
    ///
    /// Disabled opcodes cause immediate script failure if encountered,
    /// even inside an unexecuted `OP_IF` branch.
    pub fn is_disabled(&self) -> bool {
        matches!(
            self,
            Opcode::OpCat
                | Opcode::OpSubStr
                | Opcode::OpLeft
                | Opcode::OpRight
                | Opcode::OpInvert
                | Opcode::OpAnd
                | Opcode::OpOr
                | Opcode::OpXor
                | Opcode::Op2Mul
                | Opcode::Op2Div
                | Opcode::OpMul
                | Opcode::OpDiv
                | Opcode::OpMod
                | Opcode::OpLShift
                | Opcode::OpRShift
        )
    }
}

impl std::fmt::Display for Opcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Parses a human-readable opcode name (e.g. `"OP_DUP"`) into the corresponding [`Opcode`].
///
/// Recognizes common aliases such as `"OP_FALSE"` for `OP_0`, `"OP_TRUE"` for `OP_1`,
/// and bare decimal digits `"0"`..`"16"` for the small-integer push opcodes.
/// Returns `None` for unrecognized names.
pub fn name_to_opcode(name: &str) -> Option<Opcode> {
    match name {
        "OP_0" | "OP_FALSE" | "0" => Some(Opcode::Op0),
        "OP_PUSHDATA1" => Some(Opcode::OpPushData1),
        "OP_PUSHDATA2" => Some(Opcode::OpPushData2),
        "OP_PUSHDATA4" => Some(Opcode::OpPushData4),
        "OP_1NEGATE" => Some(Opcode::Op1Negate),
        "OP_RESERVED" => Some(Opcode::OpReserved),
        "OP_1" | "OP_TRUE" | "1" => Some(Opcode::Op1),
        "OP_2" | "2" => Some(Opcode::Op2),
        "OP_3" | "3" => Some(Opcode::Op3),
        "OP_4" | "4" => Some(Opcode::Op4),
        "OP_5" | "5" => Some(Opcode::Op5),
        "OP_6" | "6" => Some(Opcode::Op6),
        "OP_7" | "7" => Some(Opcode::Op7),
        "OP_8" | "8" => Some(Opcode::Op8),
        "OP_9" | "9" => Some(Opcode::Op9),
        "OP_10" | "10" => Some(Opcode::Op10),
        "OP_11" | "11" => Some(Opcode::Op11),
        "OP_12" | "12" => Some(Opcode::Op12),
        "OP_13" | "13" => Some(Opcode::Op13),
        "OP_14" | "14" => Some(Opcode::Op14),
        "OP_15" | "15" => Some(Opcode::Op15),
        "OP_16" | "16" => Some(Opcode::Op16),
        "OP_NOP" => Some(Opcode::OpNop),
        "OP_IF" => Some(Opcode::OpIf),
        "OP_NOTIF" => Some(Opcode::OpNotIf),
        "OP_ELSE" => Some(Opcode::OpElse),
        "OP_ENDIF" => Some(Opcode::OpEndIf),
        "OP_VERIFY" => Some(Opcode::OpVerify),
        "OP_RETURN" => Some(Opcode::OpReturn),
        "OP_DUP" => Some(Opcode::OpDup),
        "OP_EQUAL" => Some(Opcode::OpEqual),
        "OP_EQUALVERIFY" => Some(Opcode::OpEqualVerify),
        "OP_HASH160" => Some(Opcode::OpHash160),
        "OP_HASH256" => Some(Opcode::OpHash256),
        "OP_CHECKSIG" => Some(Opcode::OpCheckSig),
        "OP_CHECKMULTISIG" => Some(Opcode::OpCheckMultiSig),
        "OP_CHECKLOCKTIMEVERIFY" | "OP_NOP2" => Some(Opcode::OpCheckLockTimeVerify),
        "OP_CHECKSEQUENCEVERIFY" | "OP_NOP3" => Some(Opcode::OpCheckSequenceVerify),
        "OP_CHECKSIGADD" => Some(Opcode::OpCheckSigAdd),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_values() {
        assert_eq!(Opcode::Op0 as u8, 0x00);
        assert_eq!(Opcode::OpDup as u8, 0x76);
        assert_eq!(Opcode::OpCheckSig as u8, 0xac);
        assert_eq!(Opcode::OpCheckSigAdd as u8, 0xba);
        assert_eq!(Opcode::OpInvalidOpcode as u8, 0xff);
    }

    #[test]
    fn test_from_u8() {
        assert_eq!(Opcode::from_u8(0x00), Some(Opcode::Op0));
        assert_eq!(Opcode::from_u8(0x76), Some(Opcode::OpDup));
        assert_eq!(Opcode::from_u8(0xba), Some(Opcode::OpCheckSigAdd));
        // Direct push bytes are not in the enum
        assert_eq!(Opcode::from_u8(0x01), None);
        assert_eq!(Opcode::from_u8(0x4b), None);
    }

    #[test]
    fn test_aliases() {
        assert_eq!(OP_FALSE as u8, Opcode::Op0 as u8);
        assert_eq!(OP_TRUE as u8, Opcode::Op1 as u8);
    }

    #[test]
    fn test_is_disabled() {
        assert!(Opcode::OpCat.is_disabled());
        assert!(Opcode::OpMul.is_disabled());
        assert!(!Opcode::OpAdd.is_disabled());
        assert!(!Opcode::OpDup.is_disabled());
    }
}
