//! FIREVECTOR — the emission-weighting stack machine.
//!
//! A weightmap is a `Vec<u128>`: a postfix (RPN) program over unsigned 128-bit
//! integers. The indexer runs it once per candidate item in a block and gets
//! back that item's weight. Emission is split by weight instead of by headcount.
//!
//! Two properties are load-bearing and everything here is shaped around them:
//!
//! 1. **TOTALITY.** After [`validate`] accepts a program, [`eval`] cannot fault
//!    — no panic, no `Err`, no unbounded work, for any item. This is not
//!    stylistic. At mainnet h=953281 a malformed protostone made
//!    `_get_number_diesel_mints` `?` out, so it never returned a count, so every
//!    DIESEL mint spun on a failed extcall until per-tx fuel ran out and an
//!    indexer wedged at tip 953280. This machine has vastly more input surface
//!    than that counter did. So: validate once per block, then execute
//!    infallibly. All arithmetic saturates, division by zero yields zero, every
//!    field load is total, and the step budget is checked on every step.
//!
//! 2. **DETERMINISM.** Integer only. No floats, no randomness, no clock, no
//!    memory, no host reads, no backward branches. The instruction set is not
//!    Turing complete because there is nothing to jump with.

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Maximum operand-stack depth. Enforced statically by [`validate`], so [`eval`]
/// never needs to bounds-check a push.
pub const MAX_STACK: usize = 64;

/// Hard ceiling on a program's length in `u128` words, immediates included.
pub const MAX_PROGRAM_WORDS: usize = 4096;

/// Hard ceiling on the per-item step budget a header may request.
pub const MAX_STEPS_LIMIT: u32 = 65_536;

// ---------------------------------------------------------------------------
// Opcodes
// ---------------------------------------------------------------------------

// Constant.
pub const OP_PUSH: u128 = 1;

// Fact loads — what is true about the item being weighed.
pub const OP_TARGET_BLOCK: u128 = 16;
pub const OP_TARGET_TX: u128 = 17;
pub const OP_OPCODE: u128 = 18;
pub const OP_INPUT: u128 = 19;
pub const OP_STATUS: u128 = 20;
pub const OP_IN_AMOUNT: u128 = 21;
pub const OP_OUT_AMOUNT: u128 = 22;
pub const OP_TXINDEX: u128 = 23;
pub const OP_PSTONE_INDEX: u128 = 24;
pub const OP_HEIGHT: u128 = 25;

// Arithmetic — saturating; division by zero yields zero.
pub const OP_ADD: u128 = 32;
pub const OP_SUB: u128 = 33;
pub const OP_MUL: u128 = 34;
pub const OP_DIV: u128 = 35;
pub const OP_MIN: u128 = 36;
pub const OP_MAX: u128 = 37;
pub const OP_SQRT: u128 = 38;
pub const OP_MULDIV: u128 = 39;
pub const OP_SHR: u128 = 40;
pub const OP_SHL: u128 = 41;

// Comparison — each yields 1 or 0.
pub const OP_EQ: u128 = 48;
pub const OP_NE: u128 = 49;
pub const OP_LT: u128 = 50;
pub const OP_GT: u128 = 51;
pub const OP_LE: u128 = 52;
pub const OP_GE: u128 = 53;

// Logic.
pub const OP_AND: u128 = 64;
pub const OP_OR: u128 = 65;
pub const OP_NOT: u128 = 66;

// Stack.
pub const OP_DUP: u128 = 80;
pub const OP_SWAP: u128 = 81;
pub const OP_DROP: u128 = 82;
pub const OP_SELECT: u128 = 83;

/// Static shape of one instruction: how many immediate words follow it, how many
/// operands it pops, how many it pushes.
///
/// Returns `None` for an unknown opcode, which is what makes an unknown opcode a
/// validation failure rather than a runtime one.
pub fn shape(op: u128) -> Option<(usize, usize, usize)> {
    let s = match op {
        OP_PUSH => (1, 0, 1),

        OP_TARGET_BLOCK | OP_TARGET_TX | OP_OPCODE | OP_STATUS | OP_TXINDEX | OP_PSTONE_INDEX
        | OP_HEIGHT => (0, 0, 1),
        OP_INPUT => (1, 0, 1),
        OP_IN_AMOUNT | OP_OUT_AMOUNT => (2, 0, 1),

        OP_ADD | OP_SUB | OP_MUL | OP_DIV | OP_MIN | OP_MAX => (0, 2, 1),
        OP_SQRT => (0, 1, 1),
        OP_MULDIV => (0, 3, 1),
        OP_SHR | OP_SHL => (1, 1, 1),

        OP_EQ | OP_NE | OP_LT | OP_GT | OP_LE | OP_GE => (0, 2, 1),

        OP_AND | OP_OR => (0, 2, 1),
        OP_NOT => (0, 1, 1),

        OP_DUP => (0, 1, 2),
        OP_SWAP => (0, 2, 2),
        OP_DROP => (0, 1, 0),
        OP_SELECT => (0, 3, 1),

        _ => return None,
    };
    Some(s)
}

/// Human-readable mnemonic, for the disassembler.
pub fn mnemonic(op: u128) -> Option<&'static str> {
    Some(match op {
        OP_PUSH => "PUSH",
        OP_TARGET_BLOCK => "TARGET_BLOCK",
        OP_TARGET_TX => "TARGET_TX",
        OP_OPCODE => "OPCODE",
        OP_INPUT => "INPUT",
        OP_STATUS => "STATUS",
        OP_IN_AMOUNT => "IN_AMOUNT",
        OP_OUT_AMOUNT => "OUT_AMOUNT",
        OP_TXINDEX => "TXINDEX",
        OP_PSTONE_INDEX => "PSTONE_INDEX",
        OP_HEIGHT => "HEIGHT",
        OP_ADD => "ADD",
        OP_SUB => "SUB",
        OP_MUL => "MUL",
        OP_DIV => "DIV",
        OP_MIN => "MIN",
        OP_MAX => "MAX",
        OP_SQRT => "SQRT",
        OP_MULDIV => "MULDIV",
        OP_SHR => "SHR",
        OP_SHL => "SHL",
        OP_EQ => "EQ",
        OP_NE => "NE",
        OP_LT => "LT",
        OP_GT => "GT",
        OP_LE => "LE",
        OP_GE => "GE",
        OP_AND => "AND",
        OP_OR => "OR",
        OP_NOT => "NOT",
        OP_DUP => "DUP",
        OP_SWAP => "SWAP",
        OP_DROP => "DROP",
        OP_SELECT => "SELECT",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Item — the facts a program can see
// ---------------------------------------------------------------------------

/// Where a rule's items come from, which determines which fact loads are legal.
///
/// `PrevBlock` items are drawn from the previous block's committed execution
/// traces, so success and outgoing amounts are known. `SameBlock` items are
/// parsed from the current block before anything has executed, so they are not.
/// Using [`OP_STATUS`] or [`OP_OUT_AMOUNT`] in a `SameBlock` rule is a
/// validation error rather than a silent zero — a vector that reads a fact that
/// cannot exist is a bug in the vector, and governance should learn that before
/// the block, not after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    PrevBlock,
    SameBlock,
}

/// One weighable unit of activity: a single protocol-tag-1 protostone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub target_block: u128,
    pub target_tx: u128,
    pub opcode: u128,
    /// Cellpack inputs after the opcode.
    ///
    /// Note that calldata is zero-padded to 15-byte chunks on the wire, so
    /// trailing zeros here may be padding rather than arguments — arity is not
    /// recoverable. [`OP_INPUT`] therefore treats out-of-range and padding
    /// identically, both yielding 0.
    pub inputs: Vec<u128>,
    /// 1 on success, 0 on revert. Always 0 for `SameBlock` items.
    pub status: u128,
    /// `(alkane_block, alkane_tx, amount)` that arrived.
    pub incoming: Vec<(u128, u128, u128)>,
    /// `(alkane_block, alkane_tx, amount)` that left. Empty for `SameBlock`.
    pub outgoing: Vec<(u128, u128, u128)>,
    pub txindex: u128,
    pub pstone_index: u128,
    pub height: u128,
}

impl Item {
    /// Total units of `block:tx` that arrived, saturating across transfers.
    pub fn in_amount(&self, block: u128, tx: u128) -> u128 {
        sum_amount(&self.incoming, block, tx)
    }

    /// Total units of `block:tx` that left, saturating across transfers.
    pub fn out_amount(&self, block: u128, tx: u128) -> u128 {
        sum_amount(&self.outgoing, block, tx)
    }

    /// `inputs[i]`, or 0 when absent. Never panics.
    pub fn input(&self, i: u128) -> u128 {
        if i > usize::MAX as u128 {
            return 0;
        }
        self.inputs.get(i as usize).copied().unwrap_or(0)
    }
}

fn sum_amount(v: &[(u128, u128, u128)], block: u128, tx: u128) -> u128 {
    let mut total: u128 = 0;
    for (b, t, amt) in v {
        if *b == block && *t == tx {
            total = total.saturating_add(*amt);
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidateError {
    Empty,
    TooLong { words: usize },
    UnknownOpcode { at: usize, op: u128 },
    TruncatedImmediate { at: usize, op: u128 },
    StackUnderflow { at: usize, op: u128 },
    StackOverflow { at: usize, op: u128, depth: usize },
    /// The program left something other than exactly one value on the stack.
    BadResidue { depth: usize },
    /// A fact load that cannot be answered for this rule's [`Source`].
    FactUnavailable { at: usize, op: u128, source: Source },
    StepBudget { requested: u32 },
}

impl core::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ValidateError::Empty => write!(f, "empty program"),
            ValidateError::TooLong { words } => {
                write!(f, "program is {} words, limit {}", words, MAX_PROGRAM_WORDS)
            }
            ValidateError::UnknownOpcode { at, op } => {
                write!(f, "unknown opcode {} at word {}", op, at)
            }
            ValidateError::TruncatedImmediate { at, op } => write!(
                f,
                "opcode {} at word {} wants immediates past end of program",
                op, at
            ),
            ValidateError::StackUnderflow { at, op } => {
                write!(f, "opcode {} at word {} underflows the stack", op, at)
            }
            ValidateError::StackOverflow { at, op, depth } => write!(
                f,
                "opcode {} at word {} takes stack depth to {}, limit {}",
                op, at, depth, MAX_STACK
            ),
            ValidateError::BadResidue { depth } => write!(
                f,
                "program must leave exactly 1 value on the stack, left {}",
                depth
            ),
            ValidateError::FactUnavailable { at, op, source } => write!(
                f,
                "opcode {} at word {} reads a fact unavailable to a {:?} rule",
                op, at, source
            ),
            ValidateError::StepBudget { requested } => {
                write!(f, "step budget {} exceeds limit {}", requested, MAX_STEPS_LIMIT)
            }
        }
    }
}

/// Statically check a program, once, before any item is evaluated.
///
/// Because the instruction set has no branches, stack depth is exact rather than
/// approximate: a program that validates cannot underflow, overflow, or read an
/// immediate past the end at runtime. That is what lets [`eval`] be infallible.
///
/// Returns the exact number of instructions, which is also the step count
/// [`eval`] will charge.
pub fn validate(program: &[u128], source: Source) -> Result<u32, ValidateError> {
    if program.is_empty() {
        return Err(ValidateError::Empty);
    }
    if program.len() > MAX_PROGRAM_WORDS {
        return Err(ValidateError::TooLong {
            words: program.len(),
        });
    }

    let mut depth: usize = 0;
    let mut steps: u32 = 0;
    let mut i: usize = 0;

    while i < program.len() {
        let op = program[i];
        let (imm, pops, pushes) = shape(op).ok_or(ValidateError::UnknownOpcode { at: i, op })?;

        if source == Source::SameBlock && matches!(op, OP_STATUS | OP_OUT_AMOUNT) {
            return Err(ValidateError::FactUnavailable { at: i, op, source });
        }

        if i + imm >= program.len() {
            return Err(ValidateError::TruncatedImmediate { at: i, op });
        }

        if depth < pops {
            return Err(ValidateError::StackUnderflow { at: i, op });
        }
        depth = depth - pops + pushes;
        if depth > MAX_STACK {
            return Err(ValidateError::StackOverflow { at: i, op, depth });
        }

        steps = steps.saturating_add(1);
        i += 1 + imm;
    }

    if depth != 1 {
        return Err(ValidateError::BadResidue { depth });
    }
    Ok(steps)
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Run a validated program against one item and return its weight.
///
/// Infallible by construction. `budget` caps the number of instructions
/// executed; exhausting it yields weight 0 rather than an error, so a pathological
/// program degrades to "nothing qualifies" instead of stalling a block.
///
/// Passing a program that did not come from [`validate`] is a caller bug, not a
/// crash: this function still cannot panic, it simply stops and returns 0 the
/// moment it meets something it cannot execute.
pub fn eval(program: &[u128], item: &Item, budget: u32) -> u128 {
    let mut stack: [u128; MAX_STACK] = [0; MAX_STACK];
    let mut sp: usize = 0;
    let mut steps: u32 = 0;
    let mut i: usize = 0;

    macro_rules! pop {
        () => {
            if sp == 0 {
                return 0;
            } else {
                sp -= 1;
                stack[sp]
            }
        };
    }
    macro_rules! push {
        ($v:expr) => {
            if sp >= MAX_STACK {
                return 0;
            } else {
                stack[sp] = $v;
                sp += 1;
            }
        };
    }
    macro_rules! imm {
        ($n:expr) => {
            match program.get(i + $n) {
                Some(v) => *v,
                None => return 0,
            }
        };
    }

    while i < program.len() {
        if steps >= budget {
            return 0;
        }
        steps += 1;

        let op = program[i];
        let imm_count = match shape(op) {
            Some((imm, _, _)) => imm,
            None => return 0,
        };

        match op {
            OP_PUSH => {
                let v = imm!(1);
                push!(v);
            }

            OP_TARGET_BLOCK => push!(item.target_block),
            OP_TARGET_TX => push!(item.target_tx),
            OP_OPCODE => push!(item.opcode),
            OP_INPUT => {
                let idx = imm!(1);
                push!(item.input(idx));
            }
            OP_STATUS => push!(item.status),
            OP_IN_AMOUNT => {
                let b = imm!(1);
                let t = imm!(2);
                push!(item.in_amount(b, t));
            }
            OP_OUT_AMOUNT => {
                let b = imm!(1);
                let t = imm!(2);
                push!(item.out_amount(b, t));
            }
            OP_TXINDEX => push!(item.txindex),
            OP_PSTONE_INDEX => push!(item.pstone_index),
            OP_HEIGHT => push!(item.height),

            OP_ADD => {
                let b = pop!();
                let a = pop!();
                push!(a.saturating_add(b));
            }
            OP_SUB => {
                let b = pop!();
                let a = pop!();
                push!(a.saturating_sub(b));
            }
            OP_MUL => {
                let b = pop!();
                let a = pop!();
                push!(a.saturating_mul(b));
            }
            OP_DIV => {
                let b = pop!();
                let a = pop!();
                push!(if b == 0 { 0 } else { a / b });
            }
            OP_MIN => {
                let b = pop!();
                let a = pop!();
                push!(core::cmp::min(a, b));
            }
            OP_MAX => {
                let b = pop!();
                let a = pop!();
                push!(core::cmp::max(a, b));
            }
            OP_SQRT => {
                let a = pop!();
                push!(isqrt(a));
            }
            OP_MULDIV => {
                let c = pop!();
                let b = pop!();
                let a = pop!();
                push!(mul_div_floor(a, b, c));
            }
            OP_SHR => {
                let n = imm!(1);
                let a = pop!();
                push!(if n >= 128 { 0 } else { a >> (n as u32) });
            }
            OP_SHL => {
                let n = imm!(1);
                let a = pop!();
                push!(if n >= 128 {
                    0
                } else {
                    a.checked_shl(n as u32).unwrap_or(0)
                });
            }

            OP_EQ => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a == b));
            }
            OP_NE => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a != b));
            }
            OP_LT => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a < b));
            }
            OP_GT => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a > b));
            }
            OP_LE => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a <= b));
            }
            OP_GE => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a >= b));
            }

            OP_AND => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a != 0 && b != 0));
            }
            OP_OR => {
                let b = pop!();
                let a = pop!();
                push!(bool_word(a != 0 || b != 0));
            }
            OP_NOT => {
                let a = pop!();
                push!(bool_word(a == 0));
            }

            OP_DUP => {
                let a = pop!();
                push!(a);
                push!(a);
            }
            OP_SWAP => {
                let b = pop!();
                let a = pop!();
                push!(b);
                push!(a);
            }
            OP_DROP => {
                let _ = pop!();
            }
            OP_SELECT => {
                let b = pop!();
                let a = pop!();
                let c = pop!();
                push!(if c != 0 { a } else { b });
            }

            _ => return 0,
        }

        i += 1 + imm_count;
    }

    if sp == 0 {
        0
    } else {
        stack[sp - 1]
    }
}

#[inline]
fn bool_word(b: bool) -> u128 {
    if b {
        1
    } else {
        0
    }
}

/// Integer square root, floor. Newton's method from a bit-length seed.
///
/// Guarantees `isqrt(n)^2 <= n < (isqrt(n)+1)^2` for all `n`, including
/// `u128::MAX` where the naive `(x+1)^2` check would overflow.
pub fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    // Seed at 2^ceil(bits/2), which is >= sqrt(n), so the iteration descends.
    let bits = 128 - n.leading_zeros();
    let mut x: u128 = 1u128 << ((bits + 1) / 2);
    loop {
        // y = (x + n/x) / 2, computed without overflowing.
        let y = x / 2 + (n / x) / 2 + ((x & 1) + ((n / x) & 1)) / 2;
        if y >= x {
            break;
        }
        x = y;
    }
    // Descend to the true floor; the seed guarantees at most a couple of steps.
    while x > 0 && x > n / x {
        x -= 1;
    }
    x
}

/// `a * b / c`, floored, with a full 256-bit intermediate so large operands do
/// not saturate prematurely. Division by zero yields 0.
///
/// Fast path is a plain `checked_mul`. The fallback is schoolbook long division
/// over a 256-bit intermediate held as a `(hi, lo)` pair — bounded at 256
/// iterations, so it is constant-cost, not data-dependent.
pub fn mul_div_floor(a: u128, b: u128, c: u128) -> u128 {
    if c == 0 {
        return 0;
    }
    if let Some(p) = a.checked_mul(b) {
        return p / c;
    }

    let (hi, lo) = mul_full(a, b);

    // Long division of the 256-bit value (hi, lo) by c, MSB first.
    let mut rem: u128 = 0;
    let mut quo_hi: u128 = 0;
    let mut quo_lo: u128 = 0;

    for bit in (0..256).rev() {
        let cur = if bit >= 128 {
            (hi >> (bit - 128)) & 1
        } else {
            (lo >> bit) & 1
        };

        // rem = rem*2 + cur. If rem*2 would overflow, rem is already >= 2^127,
        // hence >= c, so the subtraction below is guaranteed and we can carry
        // the overflow bit out instead of losing it.
        let overflow = rem >> 127 != 0;
        rem = (rem << 1) | cur;

        if overflow || rem >= c {
            rem = rem.wrapping_sub(c);
            if bit >= 128 {
                quo_hi |= 1u128 << (bit - 128);
            } else {
                quo_lo |= 1u128 << bit;
            }
        }
    }

    // The true quotient exceeds u128; saturate rather than wrap.
    if quo_hi != 0 {
        return u128::MAX;
    }
    quo_lo
}

/// Full 256-bit product of two `u128`s as `(hi, lo)`.
fn mul_full(a: u128, b: u128) -> (u128, u128) {
    const MASK: u128 = u64::MAX as u128;
    let (a_lo, a_hi) = (a & MASK, a >> 64);
    let (b_lo, b_hi) = (b & MASK, b >> 64);

    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;

    // Accumulate the two middle terms with explicit carry tracking.
    let mid = (ll >> 64) + (lh & MASK) + (hl & MASK);
    let lo = (ll & MASK) | (mid & MASK) << 64;
    let hi = hh + (lh >> 64) + (hl >> 64) + (mid >> 64);
    (hi, lo)
}

/// Convenience: validate then evaluate, returning `None` if the program is
/// invalid. Intended for the `simulate` view, not the indexing hot path — in the
/// hot path you validate once per block and then call [`eval`] per item.
pub fn validate_and_eval(program: &[u128], item: &Item, source: Source) -> Option<u128> {
    let steps = validate(program, source).ok()?;
    Some(eval(program, item, steps.max(1)))
}
