// ═══════════════════════════════════════════════════════════════════════
// Alkanes GPU WASM Interpreter — Compute Shader
// ═══════════════════════════════════════════════════════════════════════
//
// A WebAssembly bytecode interpreter running on GPU compute shaders.
// Each workgroup thread processes one alkanes contract message.
//
// Limitations vs full wasmi:
//   - Fixed memory size (no memory.grow beyond initial)
//   - No floating point (f32/f64 ops eject to CPU)
//   - External calls (call/delegatecall) eject to CPU
//   - Preloaded K/V context only — cache miss = ejection
//
// ═══════════════════════════════════════════════════════════════════════

// ── Constants ────────────────────────────────────────────────────────

const MAX_THREADS: u32 = 64u;
const WASM_PAGE_SIZE: u32 = 65536u;
// Memory: 16 pages = 1MB per thread, stored as u32 words
const WASM_MEMORY_PAGES: u32 = 16u;
const WASM_MEMORY_BYTES: u32 = 1048576u; // 16 * 65536
const WASM_MEMORY_WORDS: u32 = 262144u;  // WASM_MEMORY_BYTES / 4

const STACK_SIZE: u32 = 512u;
const LOCALS_SIZE: u32 = 256u;
const MAX_CALL_FRAMES: u32 = 64u;
const MAX_LABELS: u32 = 128u;

// Max bytecode size per contract (256 KB in u32 words)
const MAX_BYTECODE_WORDS: u32 = 65536u;

// Function table: maps function index to code offset + local count
// Stored after bytecode in input buffer
// Each entry: [code_offset: u32, local_count: u32]
const MAX_FUNCTIONS: u32 = 256u;
const FUNC_ENTRY_U32S: u32 = 2u;

// K/V limits
const MAX_KV_PAIRS: u32 = 1024u;
const MAX_KEY_BYTES: u32 = 256u;
const MAX_VALUE_BYTES: u32 = 1024u;
const MAX_KEY_WORDS: u32 = 64u;   // 256/4
const MAX_VALUE_WORDS: u32 = 256u; // 1024/4

// Return data
const MAX_RETURN_WORDS: u32 = 64u; // 256 bytes

// Ejection codes
const EJECTION_NONE: u32 = 0u;
const EJECTION_STORAGE_OVERFLOW: u32 = 1u;
const EJECTION_MEMORY_CONSTRAINT: u32 = 2u;
const EJECTION_KV_OVERFLOW: u32 = 3u;
const EJECTION_CALLDATA_OVERFLOW: u32 = 4u;
const EJECTION_EXTCALL: u32 = 5u;
const EJECTION_FUEL_EXHAUSTED: u32 = 6u;
const EJECTION_TRAP: u32 = 7u;
const EJECTION_UNSUPPORTED: u32 = 8u;

// Host function IDs (must match alkanes VM import order)
const HOST_ABORT: u32 = 0u;
const HOST_LOAD_STORAGE: u32 = 1u;
const HOST_REQUEST_STORAGE: u32 = 2u;
const HOST_LOG: u32 = 3u;
const HOST_BALANCE: u32 = 4u;
const HOST_REQUEST_CONTEXT: u32 = 5u;
const HOST_LOAD_CONTEXT: u32 = 6u;
const HOST_SEQUENCE: u32 = 7u;
const HOST_FUEL: u32 = 8u;
const HOST_HEIGHT: u32 = 9u;
const HOST_RETURNDATACOPY: u32 = 10u;
const HOST_REQUEST_TRANSACTION: u32 = 11u;
const HOST_LOAD_TRANSACTION: u32 = 12u;
const HOST_REQUEST_BLOCK: u32 = 13u;
const HOST_LOAD_BLOCK: u32 = 14u;
const HOST_CALL: u32 = 15u;
const HOST_DELEGATECALL: u32 = 16u;
const HOST_STATICCALL: u32 = 17u;

// WASM opcodes
const OP_UNREACHABLE: u32 = 0x00u;
const OP_NOP: u32 = 0x01u;
const OP_BLOCK: u32 = 0x02u;
const OP_LOOP: u32 = 0x03u;
const OP_IF: u32 = 0x04u;
const OP_ELSE: u32 = 0x05u;
const OP_END: u32 = 0x0Bu;
const OP_BR: u32 = 0x0Cu;
const OP_BR_IF: u32 = 0x0Du;
const OP_RETURN: u32 = 0x0Fu;
const OP_CALL: u32 = 0x10u;
const OP_CALL_INDIRECT: u32 = 0x11u;
const OP_DROP: u32 = 0x1Au;
const OP_SELECT: u32 = 0x1Bu;
const OP_LOCAL_GET: u32 = 0x20u;
const OP_LOCAL_SET: u32 = 0x21u;
const OP_LOCAL_TEE: u32 = 0x22u;
const OP_GLOBAL_GET: u32 = 0x23u;
const OP_GLOBAL_SET: u32 = 0x24u;
const OP_I32_LOAD: u32 = 0x28u;
const OP_I64_LOAD: u32 = 0x29u;
const OP_F32_LOAD: u32 = 0x2Au;
const OP_F64_LOAD: u32 = 0x2Bu;
const OP_I32_LOAD8_S: u32 = 0x2Cu;
const OP_I32_LOAD8_U: u32 = 0x2Du;
const OP_I32_LOAD16_S: u32 = 0x2Eu;
const OP_I32_LOAD16_U: u32 = 0x2Fu;
const OP_I64_LOAD8_S: u32 = 0x30u;
const OP_I64_LOAD8_U: u32 = 0x31u;
const OP_I64_LOAD16_S: u32 = 0x32u;
const OP_I64_LOAD16_U: u32 = 0x33u;
const OP_I64_LOAD32_S: u32 = 0x34u;
const OP_I64_LOAD32_U: u32 = 0x35u;
const OP_I32_STORE: u32 = 0x36u;
const OP_I64_STORE: u32 = 0x37u;
const OP_F32_STORE: u32 = 0x38u;
const OP_F64_STORE: u32 = 0x39u;
const OP_I32_STORE8: u32 = 0x3Au;
const OP_I32_STORE16: u32 = 0x3Bu;
const OP_I64_STORE8: u32 = 0x3Cu;
const OP_I64_STORE16: u32 = 0x3Du;
const OP_I64_STORE32: u32 = 0x3Eu;
const OP_MEMORY_SIZE: u32 = 0x3Fu;
const OP_MEMORY_GROW: u32 = 0x40u;
const OP_I32_CONST: u32 = 0x41u;
const OP_I64_CONST: u32 = 0x42u;
const OP_I32_EQZ: u32 = 0x45u;
const OP_I32_EQ: u32 = 0x46u;
const OP_I32_NE: u32 = 0x47u;
const OP_I32_LT_S: u32 = 0x48u;
const OP_I32_LT_U: u32 = 0x49u;
const OP_I32_GT_S: u32 = 0x4Au;
const OP_I32_GT_U: u32 = 0x4Bu;
const OP_I32_LE_S: u32 = 0x4Cu;
const OP_I32_LE_U: u32 = 0x4Du;
const OP_I32_GE_S: u32 = 0x4Eu;
const OP_I32_GE_U: u32 = 0x4Fu;
const OP_I64_EQZ: u32 = 0x50u;
const OP_I64_EQ: u32 = 0x51u;
const OP_I64_NE: u32 = 0x52u;
const OP_I64_LT_S: u32 = 0x53u;
const OP_I64_LT_U: u32 = 0x54u;
const OP_I64_GT_S: u32 = 0x55u;
const OP_I64_GT_U: u32 = 0x56u;
const OP_I64_LE_S: u32 = 0x57u;
const OP_I64_LE_U: u32 = 0x58u;
const OP_I64_GE_S: u32 = 0x59u;
const OP_I64_GE_U: u32 = 0x5Au;
const OP_I32_CLZ: u32 = 0x67u;
const OP_I32_CTZ: u32 = 0x68u;
const OP_I32_POPCNT: u32 = 0x69u;
const OP_I32_ADD: u32 = 0x6Au;
const OP_I32_SUB: u32 = 0x6Bu;
const OP_I32_MUL: u32 = 0x6Cu;
const OP_I32_DIV_S: u32 = 0x6Du;
const OP_I32_DIV_U: u32 = 0x6Eu;
const OP_I32_REM_S: u32 = 0x6Fu;
const OP_I32_REM_U: u32 = 0x70u;
const OP_I32_AND: u32 = 0x71u;
const OP_I32_OR: u32 = 0x72u;
const OP_I32_XOR: u32 = 0x73u;
const OP_I32_SHL: u32 = 0x74u;
const OP_I32_SHR_S: u32 = 0x75u;
const OP_I32_SHR_U: u32 = 0x76u;
const OP_I32_ROTL: u32 = 0x77u;
const OP_I32_ROTR: u32 = 0x78u;
const OP_I64_CLZ: u32 = 0x79u;
const OP_I64_CTZ: u32 = 0x7Au;
const OP_I64_POPCNT: u32 = 0x7Bu;
const OP_I64_ADD: u32 = 0x7Cu;
const OP_I64_SUB: u32 = 0x7Du;
const OP_I64_MUL: u32 = 0x7Eu;
const OP_I64_DIV_S: u32 = 0x7Fu;
const OP_I64_DIV_U: u32 = 0x80u;
const OP_I64_REM_S: u32 = 0x81u;
const OP_I64_REM_U: u32 = 0x82u;
const OP_I64_AND: u32 = 0x83u;
const OP_I64_OR: u32 = 0x84u;
const OP_I64_XOR: u32 = 0x85u;
const OP_I64_SHL: u32 = 0x86u;
const OP_I64_SHR_S: u32 = 0x87u;
const OP_I64_SHR_U: u32 = 0x88u;
const OP_I64_ROTL: u32 = 0x89u;
const OP_I64_ROTR: u32 = 0x8Au;
const OP_I32_WRAP_I64: u32 = 0xA7u;
const OP_I64_EXTEND_I32_S: u32 = 0xACu;
const OP_I64_EXTEND_I32_U: u32 = 0xADu;
const OP_I32_EXTEND8_S: u32 = 0xC0u;
const OP_I32_EXTEND16_S: u32 = 0xC1u;
const OP_I64_EXTEND8_S: u32 = 0xC2u;
const OP_I64_EXTEND16_S: u32 = 0xC3u;
const OP_I64_EXTEND32_S: u32 = 0xC4u;

// ── Buffers ──────────────────────────────────────────────────────────

// Input: shard header + bytecode + messages + kv context
@group(0) @binding(0)
var<storage, read> input_data: array<u32>;

// Output: per-thread results + kv writes
@group(0) @binding(1)
var<storage, read_write> output_data: array<u32>;

// Per-thread WASM memory (large — separate binding)
@group(0) @binding(2)
var<storage, read_write> wasm_memory: array<u32>;

// Per-thread execution state (stack, locals, call frames)
@group(0) @binding(3)
var<storage, read_write> thread_state: array<u32>;

// ── Input layout helpers ─────────────────────────────────────────────
// Input buffer layout:
//   [0]:  message_count
//   [1]:  kv_count
//   [2]:  block_height
//   [3]:  base_fuel_lo
//   [4]:  base_fuel_hi
//   [5]:  bytecode_len (in bytes)
//   [6]:  import_count (number of imported functions)
//   [7]:  entry_pc (byte offset of entry function in bytecode)
//   [8]:  func_count (number of entries in function table)
//   [9]:  func_table_offset (u32 offset from start of input_data to function table)
//   [10..]: bytecode (packed as u32)
//   Then: function table (func_count * 2 u32s: code_offset, local_count)
//   Then: kv pairs

const HEADER_SIZE: u32 = 10u;

fn get_message_count() -> u32 { return input_data[0]; }
fn get_kv_count() -> u32 { return input_data[1]; }
fn get_block_height() -> u32 { return input_data[2]; }
fn get_base_fuel_lo() -> u32 { return input_data[3]; }
fn get_base_fuel_hi() -> u32 { return input_data[4]; }
fn get_bytecode_len() -> u32 { return input_data[5]; }
fn get_import_count() -> u32 { return input_data[6]; }
fn get_entry_pc() -> u32 { return input_data[7]; }
fn get_func_count() -> u32 { return input_data[8]; }
fn get_func_table_offset() -> u32 { return input_data[9]; }

fn bytecode_base() -> u32 { return HEADER_SIZE; }

// Look up a function's code offset and local count from the function table
fn get_func_entry(func_idx: u32) -> vec2<u32> {
    let import_count = get_import_count();
    let internal_idx = func_idx - import_count;
    let table_base = get_func_table_offset();
    let entry_base = table_base + internal_idx * FUNC_ENTRY_U32S;
    return vec2<u32>(
        input_data[entry_base + 0u],  // code_offset (byte offset in bytecode)
        input_data[entry_base + 1u],  // local_count
    );
}

// Read a byte from bytecode at byte offset `pos`
fn read_bytecode_byte(pos: u32) -> u32 {
    let base = bytecode_base();
    let word_idx = base + (pos >> 2u);
    let byte_idx = pos & 3u;
    return (input_data[word_idx] >> (byte_idx * 8u)) & 0xFFu;
}

// ── Thread state layout ─────────────────────────────────────────────
// Per thread in thread_state buffer:
//   stack:       STACK_SIZE u32s (value stack, pairs for i64: lo, hi)
//   locals:      LOCALS_SIZE u32s
//   call_frames: MAX_CALL_FRAMES * 4 u32s (return_pc, locals_base, stack_base, func_idx)
//   labels:      MAX_LABELS * 3 u32s (target_pc, stack_depth, is_loop)
//   scalars:     pc, sp, fp, lp, fuel_lo, fuel_hi, ejected, ejection_reason,
//                import_count, entry_pc, mem_pages, result_len

const CALL_FRAME_SIZE: u32 = 4u;
const LABEL_SIZE: u32 = 3u;
const SCALAR_COUNT: u32 = 12u;
const THREAD_STATE_SIZE: u32 = STACK_SIZE + LOCALS_SIZE
    + MAX_CALL_FRAMES * CALL_FRAME_SIZE
    + MAX_LABELS * LABEL_SIZE
    + SCALAR_COUNT;

fn ts_base(tid: u32) -> u32 { return tid * THREAD_STATE_SIZE; }
fn ts_stack_base(tid: u32) -> u32 { return ts_base(tid); }
fn ts_locals_base(tid: u32) -> u32 { return ts_base(tid) + STACK_SIZE; }
fn ts_frames_base(tid: u32) -> u32 { return ts_base(tid) + STACK_SIZE + LOCALS_SIZE; }
fn ts_labels_base(tid: u32) -> u32 { return ts_frames_base(tid) + MAX_CALL_FRAMES * CALL_FRAME_SIZE; }
fn ts_scalars_base(tid: u32) -> u32 { return ts_labels_base(tid) + MAX_LABELS * LABEL_SIZE; }

// Scalar accessors
fn get_pc(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 0u]; }
fn set_pc(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 0u] = v; }
fn get_sp(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 1u]; }
fn set_sp(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 1u] = v; }
fn get_fp(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 2u]; }
fn set_fp(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 2u] = v; }
fn get_lp(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 3u]; }
fn set_lp(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 3u] = v; }
fn get_fuel_lo(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 4u]; }
fn set_fuel_lo(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 4u] = v; }
fn get_fuel_hi(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 5u]; }
fn set_fuel_hi(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 5u] = v; }
fn get_ejected(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 6u]; }
fn set_ejected(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 6u] = v; }
fn get_ejection_reason(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 7u]; }
fn set_ejection_reason(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 7u] = v; }
fn get_mem_pages(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 10u]; }
fn set_mem_pages(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 10u] = v; }

// ── Stack operations ─────────────────────────────────────────────────

fn push_i32(tid: u32, val: u32) {
    let sp = get_sp(tid);
    if sp >= STACK_SIZE {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_TRAP);
        return;
    }
    thread_state[ts_stack_base(tid) + sp] = val;
    set_sp(tid, sp + 1u);
}

fn pop_i32(tid: u32) -> u32 {
    let sp = get_sp(tid);
    if sp == 0u {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_TRAP);
        return 0u;
    }
    let new_sp = sp - 1u;
    set_sp(tid, new_sp);
    return thread_state[ts_stack_base(tid) + new_sp];
}

fn peek_i32(tid: u32) -> u32 {
    let sp = get_sp(tid);
    if sp == 0u { return 0u; }
    return thread_state[ts_stack_base(tid) + sp - 1u];
}

// ── WASM memory operations ──────────────────────────────────────────

fn mem_base(tid: u32) -> u32 { return tid * WASM_MEMORY_WORDS; }

fn wasm_load_byte(tid: u32, addr: u32) -> u32 {
    if addr >= get_mem_pages(tid) * WASM_PAGE_SIZE {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_MEMORY_CONSTRAINT);
        return 0u;
    }
    let base = mem_base(tid);
    let word = wasm_memory[base + (addr >> 2u)];
    return (word >> ((addr & 3u) * 8u)) & 0xFFu;
}

fn wasm_store_byte(tid: u32, addr: u32, val: u32) {
    if addr >= get_mem_pages(tid) * WASM_PAGE_SIZE {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_MEMORY_CONSTRAINT);
        return;
    }
    let base = mem_base(tid);
    let word_idx = base + (addr >> 2u);
    let shift = (addr & 3u) * 8u;
    let mask = ~(0xFFu << shift);
    wasm_memory[word_idx] = (wasm_memory[word_idx] & mask) | ((val & 0xFFu) << shift);
}

fn wasm_load_u32(tid: u32, addr: u32) -> u32 {
    // Byte-by-byte for unaligned access support
    let b0 = wasm_load_byte(tid, addr);
    let b1 = wasm_load_byte(tid, addr + 1u);
    let b2 = wasm_load_byte(tid, addr + 2u);
    let b3 = wasm_load_byte(tid, addr + 3u);
    return b0 | (b1 << 8u) | (b2 << 16u) | (b3 << 24u);
}

fn wasm_store_u32(tid: u32, addr: u32, val: u32) {
    wasm_store_byte(tid, addr, val & 0xFFu);
    wasm_store_byte(tid, addr + 1u, (val >> 8u) & 0xFFu);
    wasm_store_byte(tid, addr + 2u, (val >> 16u) & 0xFFu);
    wasm_store_byte(tid, addr + 3u, (val >> 24u) & 0xFFu);
}

// ── LEB128 decoder ───────────────────────────────────────────────────

fn read_leb128_u32(tid: u32) -> u32 {
    var result: u32 = 0u;
    var shift: u32 = 0u;
    var pc = get_pc(tid);
    for (var i: u32 = 0u; i < 5u; i = i + 1u) {
        let byte = read_bytecode_byte(pc);
        pc = pc + 1u;
        result = result | ((byte & 0x7Fu) << shift);
        if (byte & 0x80u) == 0u {
            set_pc(tid, pc);
            return result;
        }
        shift = shift + 7u;
    }
    set_pc(tid, pc);
    return result;
}

fn read_leb128_i32(tid: u32) -> i32 {
    var result: u32 = 0u;
    var shift: u32 = 0u;
    var pc = get_pc(tid);
    var byte: u32 = 0u;
    for (var i: u32 = 0u; i < 5u; i = i + 1u) {
        byte = read_bytecode_byte(pc);
        pc = pc + 1u;
        result = result | ((byte & 0x7Fu) << shift);
        shift = shift + 7u;
        if (byte & 0x80u) == 0u {
            break;
        }
    }
    set_pc(tid, pc);
    // Sign extend
    if shift < 32u && (byte & 0x40u) != 0u {
        result = result | (0xFFFFFFFFu << shift);
    }
    return bitcast<i32>(result);
}

// ── Fuel management ──────────────────────────────────────────────────

fn consume_fuel(tid: u32, amount: u32) -> bool {
    let lo = get_fuel_lo(tid);
    let hi = get_fuel_hi(tid);
    if hi == 0u && lo < amount {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_FUEL_EXHAUSTED);
        return false;
    }
    // 64-bit subtract: fuel -= amount
    let new_lo = lo - amount;
    var new_hi = hi;
    if new_lo > lo { // underflow in low word
        new_hi = new_hi - 1u;
    }
    set_fuel_lo(tid, new_lo);
    set_fuel_hi(tid, new_hi);
    return true;
}

// ── Label stack (for block/loop/if control flow) ─────────────────────

fn push_label(tid: u32, target_pc: u32, stack_depth: u32, is_loop: u32) {
    let lp = get_lp(tid);
    if lp >= MAX_LABELS {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_TRAP);
        return;
    }
    let base = ts_labels_base(tid) + lp * LABEL_SIZE;
    thread_state[base + 0u] = target_pc;
    thread_state[base + 1u] = stack_depth;
    thread_state[base + 2u] = is_loop;
    set_lp(tid, lp + 1u);
}

fn pop_label(tid: u32) {
    let lp = get_lp(tid);
    if lp > 0u {
        set_lp(tid, lp - 1u);
    }
}

fn get_label(tid: u32, depth: u32) -> vec3<u32> {
    let lp = get_lp(tid);
    if depth >= lp {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_TRAP);
        return vec3<u32>(0u, 0u, 0u);
    }
    let idx = lp - 1u - depth;
    let base = ts_labels_base(tid) + idx * LABEL_SIZE;
    return vec3<u32>(
        thread_state[base + 0u],  // target_pc
        thread_state[base + 1u],  // stack_depth
        thread_state[base + 2u],  // is_loop
    );
}

// ── Call frame management ────────────────────────────────────────────

fn push_frame(tid: u32, return_pc: u32, locals_base: u32, stack_base: u32, func_idx: u32) {
    let fp = get_fp(tid);
    if fp >= MAX_CALL_FRAMES {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_TRAP);
        return;
    }
    let base = ts_frames_base(tid) + fp * CALL_FRAME_SIZE;
    thread_state[base + 0u] = return_pc;
    thread_state[base + 1u] = locals_base;
    thread_state[base + 2u] = stack_base;
    thread_state[base + 3u] = func_idx;
    set_fp(tid, fp + 1u);
}

fn pop_frame(tid: u32) -> vec4<u32> {
    let fp = get_fp(tid);
    if fp == 0u {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_TRAP);
        return vec4<u32>(0u);
    }
    let new_fp = fp - 1u;
    set_fp(tid, new_fp);
    let base = ts_frames_base(tid) + new_fp * CALL_FRAME_SIZE;
    return vec4<u32>(
        thread_state[base + 0u],  // return_pc
        thread_state[base + 1u],  // locals_base
        thread_state[base + 2u],  // stack_base
        thread_state[base + 3u],  // func_idx
    );
}

// ── Local variable access ────────────────────────────────────────────

fn get_local(tid: u32, idx: u32) -> u32 {
    return thread_state[ts_locals_base(tid) + idx];
}

fn set_local(tid: u32, idx: u32, val: u32) {
    thread_state[ts_locals_base(tid) + idx] = val;
}

// ── K/V Storage lookup ───────────────────────────────────────────────
// Preloaded K/V pairs sit after the bytecode and messages in the input buffer.
// Layout per pair: [key_len: u32, value_len: u32, pad: u32, pad: u32,
//                   key: MAX_KEY_WORDS u32s, value: MAX_VALUE_WORDS u32s]
const KV_PAIR_U32S: u32 = 4u + MAX_KEY_WORDS + MAX_VALUE_WORDS; // 4 + 64 + 256 = 324

// Per-thread K/V write tracking in thread_state (after scalars)
// We store a write count and up to 32 writes per thread
const MAX_KV_WRITES_PER_THREAD: u32 = 32u;
// Each write: key_len(1) + key(MAX_KEY_WORDS=64) + value_len(1) + value(MAX_VALUE_WORDS=256) = 322
const KV_WRITE_ENTRY_U32S: u32 = 322u;

// Temporary storage for the last requested storage pointer (per thread)
// We use a region in WASM memory at a known high address for this
const STORAGE_TEMP_ADDR: u32 = 1040384u; // near end of 1MB, 8KB reserved
const STORAGE_TEMP_SIZE: u32 = 4096u;

fn get_kv_base() -> u32 {
    // K/V pairs start after the function table in input buffer
    let func_table_end = get_func_table_offset() + get_func_count() * FUNC_ENTRY_U32S;
    return func_table_end;
}

// Search preloaded K/V pairs for a key. Returns value offset in input_data, or 0xFFFFFFFF if not found.
fn kv_lookup(key_ptr: u32, key_len: u32, tid: u32) -> u32 {
    let kv_count = get_kv_count();
    let kv_base = get_kv_base();

    for (var i: u32 = 0u; i < kv_count; i = i + 1u) {
        let pair_base = kv_base + i * KV_PAIR_U32S;
        let stored_key_len = input_data[pair_base + 0u];

        if stored_key_len != key_len {
            continue;
        }

        // Compare key bytes
        var match_found = true;
        let key_word_count = (key_len + 3u) >> 2u;
        let stored_key_base = pair_base + 4u; // after header
        for (var w: u32 = 0u; w < key_word_count; w = w + 1u) {
            // Read key from WASM memory
            let mem_word = wasm_load_u32(tid, key_ptr + w * 4u);
            let stored_word = input_data[stored_key_base + w];
            if mem_word != stored_word {
                match_found = false;
                break;
            }
        }

        if match_found {
            // Return offset to the value section of this pair
            return pair_base;
        }
    }

    return 0xFFFFFFFFu; // not found
}

// Copy value from a K/V pair in input_data to a WASM memory address
fn kv_copy_value_to_memory(tid: u32, pair_base: u32, dest_addr: u32) -> u32 {
    let value_len = input_data[pair_base + 1u]; // value_len at offset 1
    let value_base = pair_base + 4u + MAX_KEY_WORDS; // after header + key

    let word_count = (value_len + 3u) >> 2u;
    for (var w: u32 = 0u; w < word_count; w = w + 1u) {
        wasm_store_u32(tid, dest_addr + w * 4u, input_data[value_base + w]);
    }
    return value_len;
}

// ── Per-thread K/V write buffer ──────────────────────────────────────
// Stored in the output buffer after the per-message results.
// Layout: output_data[KV_WRITES_BASE + tid * ...]

fn get_kv_write_count(tid: u32) -> u32 {
    // Stored as the kv_write_count field in the result header
    let base = tid * RESULT_U32S;
    return output_data[base + 6u];
}

fn set_kv_write_count(tid: u32, count: u32) {
    let base = tid * RESULT_U32S;
    output_data[base + 6u] = count;
}

// ── Host function dispatch ───────────────────────────────────────────

// Temporary per-thread storage for request_storage result
// We reuse a scalar slot to track the last found pair_base
fn get_last_kv_pair(tid: u32) -> u32 { return thread_state[ts_scalars_base(tid) + 11u]; }
fn set_last_kv_pair(tid: u32, v: u32) { thread_state[ts_scalars_base(tid) + 11u] = v; }

fn dispatch_host_function(tid: u32, func_id: u32) {
    switch func_id {
        case HOST_ABORT: {
            set_ejected(tid, 1u);
            set_ejection_reason(tid, EJECTION_TRAP);
        }

        case HOST_REQUEST_STORAGE: {
            // Args on stack: key_ptr, key_len (pushed by WASM caller)
            // Returns: length of value (i32)
            if !consume_fuel(tid, 1u) { return; }
            let key_len = pop_i32(tid);
            let key_ptr = pop_i32(tid);
            let pair_base = kv_lookup(key_ptr, key_len, tid);
            if pair_base == 0xFFFFFFFFu {
                // Key not in preloaded context — eject to CPU
                set_ejected(tid, 1u);
                set_ejection_reason(tid, EJECTION_STORAGE_OVERFLOW);
                return;
            }
            set_last_kv_pair(tid, pair_base);
            let value_len = input_data[pair_base + 1u];
            if !consume_fuel(tid, value_len) { return; } // fuel per load byte
            push_i32(tid, value_len);
        }

        case HOST_LOAD_STORAGE: {
            // Copies the last requested value into WASM memory
            // Args: dest_ptr (where to write in WASM memory)
            if !consume_fuel(tid, 2u) { return; }
            let dest_ptr = pop_i32(tid);
            let pair_base = get_last_kv_pair(tid);
            if pair_base == 0xFFFFFFFFu {
                set_ejected(tid, 1u);
                set_ejection_reason(tid, EJECTION_STORAGE_OVERFLOW);
                return;
            }
            _ = kv_copy_value_to_memory(tid, pair_base, dest_ptr);
        }

        case HOST_SEQUENCE: {
            if !consume_fuel(tid, 5u) { return; }
            // Return sequence as 0 for GPU execution (placeholder)
            push_i32(tid, 0u);
        }

        case HOST_FUEL: {
            if !consume_fuel(tid, 5u) { return; }
            push_i32(tid, get_fuel_lo(tid));
        }

        case HOST_HEIGHT: {
            if !consume_fuel(tid, 10u) { return; }
            push_i32(tid, get_block_height());
        }

        case HOST_REQUEST_CONTEXT: {
            // Returns length of serialized context
            if !consume_fuel(tid, 1u) { return; }
            // Context not fully supported on GPU yet — eject
            set_ejected(tid, 1u);
            set_ejection_reason(tid, EJECTION_UNSUPPORTED);
        }

        case HOST_LOAD_CONTEXT: {
            if !consume_fuel(tid, 2u) { return; }
            set_ejected(tid, 1u);
            set_ejection_reason(tid, EJECTION_UNSUPPORTED);
        }

        case HOST_BALANCE: {
            if !consume_fuel(tid, 10u) { return; }
            // Balance queries not preloaded yet — eject
            set_ejected(tid, 1u);
            set_ejection_reason(tid, EJECTION_UNSUPPORTED);
        }

        case HOST_RETURNDATACOPY: {
            if !consume_fuel(tid, 1u) { return; }
            // No return data from previous calls on GPU
            push_i32(tid, 0u); // 0 bytes
        }

        case HOST_REQUEST_TRANSACTION, HOST_LOAD_TRANSACTION,
             HOST_REQUEST_BLOCK, HOST_LOAD_BLOCK: {
            // Transaction/block data too large for GPU — eject
            set_ejected(tid, 1u);
            set_ejection_reason(tid, EJECTION_UNSUPPORTED);
        }

        case HOST_CALL, HOST_DELEGATECALL, HOST_STATICCALL: {
            // External calls cannot be emulated on GPU — eject
            set_ejected(tid, 1u);
            set_ejection_reason(tid, EJECTION_EXTCALL);
        }

        case HOST_LOG: {
            // Ignore logs on GPU, just consume the args
            _ = pop_i32(tid); // length
            _ = pop_i32(tid); // ptr
            if !consume_fuel(tid, 1u) { return; }
        }

        default: {
            // Unhandled host function — eject to CPU
            set_ejected(tid, 1u);
            set_ejection_reason(tid, EJECTION_UNSUPPORTED);
        }
    }
}

// ── Skip block body (for untaken if/else branches) ───────────────────

fn skip_to_end_or_else(tid: u32) {
    var depth: u32 = 1u;
    let bytecode_len = get_bytecode_len();
    var pc = get_pc(tid);
    while pc < bytecode_len && depth > 0u {
        let op = read_bytecode_byte(pc);
        pc = pc + 1u;
        switch op {
            case OP_BLOCK, OP_LOOP, OP_IF: {
                // Skip block type byte
                pc = pc + 1u;
                depth = depth + 1u;
            }
            case OP_END: {
                depth = depth - 1u;
            }
            case OP_ELSE: {
                if depth == 1u {
                    // Found matching else
                    set_pc(tid, pc);
                    return;
                }
            }
            // Skip LEB128 immediates for opcodes that have them
            case OP_BR, OP_BR_IF, OP_CALL, OP_LOCAL_GET,
                 OP_LOCAL_SET, OP_LOCAL_TEE, OP_GLOBAL_GET,
                 OP_GLOBAL_SET, OP_I32_CONST, OP_I64_CONST,
                 OP_CALL_INDIRECT, OP_MEMORY_SIZE, OP_MEMORY_GROW: {
                // Skip LEB128 encoded immediate(s)
                while pc < bytecode_len {
                    let b = read_bytecode_byte(pc);
                    pc = pc + 1u;
                    if (b & 0x80u) == 0u { break; }
                }
            }
            // Load/store have two LEB128 immediates (align + offset)
            case OP_I32_LOAD, OP_I64_LOAD, OP_I32_STORE, OP_I64_STORE,
                 OP_I32_LOAD8_S, OP_I32_LOAD8_U, OP_I32_LOAD16_S,
                 OP_I32_LOAD16_U, OP_I32_STORE8, OP_I32_STORE16,
                 OP_I64_LOAD8_S, OP_I64_LOAD8_U, OP_I64_LOAD16_S,
                 OP_I64_LOAD16_U, OP_I64_LOAD32_S, OP_I64_LOAD32_U,
                 OP_I64_STORE8, OP_I64_STORE16, OP_I64_STORE32,
                 OP_F32_LOAD, OP_F64_LOAD, OP_F32_STORE, OP_F64_STORE: {
                // Skip two LEB128s (align, offset)
                for (var j: u32 = 0u; j < 2u; j = j + 1u) {
                    while pc < bytecode_len {
                        let b = read_bytecode_byte(pc);
                        pc = pc + 1u;
                        if (b & 0x80u) == 0u { break; }
                    }
                }
            }
            default: {
                // No immediate — just continue
            }
        }
    }
    set_pc(tid, pc);
}

// ── Main interpreter loop ────────────────────────────────────────────

fn interpret(tid: u32) {
    let bytecode_len = get_bytecode_len();
    let import_count = get_import_count();

    // Main interpreter loop — runs until end of bytecode, ejection, or return
    var max_iterations: u32 = 1000000u; // safety limit
    while get_pc(tid) < bytecode_len && get_ejected(tid) == 0u && max_iterations > 0u {
        max_iterations = max_iterations - 1u;

        let pc_before = get_pc(tid);
        let opcode = read_bytecode_byte(pc_before);
        set_pc(tid, pc_before + 1u);

        // Consume 1 fuel per instruction
        if !consume_fuel(tid, 1u) { return; }

        switch opcode {
            // ── NOP ──
            case OP_NOP: { }

            // ── UNREACHABLE ──
            case OP_UNREACHABLE: {
                set_ejected(tid, 1u);
                set_ejection_reason(tid, EJECTION_TRAP);
            }

            // ── BLOCK ──
            case OP_BLOCK: {
                let _block_type = read_bytecode_byte(get_pc(tid));
                set_pc(tid, get_pc(tid) + 1u);
                // Push label: target = END (will be patched by skip), stack depth = current
                push_label(tid, 0u, get_sp(tid), 0u);
            }

            // ── LOOP ──
            case OP_LOOP: {
                let _block_type = read_bytecode_byte(get_pc(tid));
                set_pc(tid, get_pc(tid) + 1u);
                // Push label: target = current pc (loop head), is_loop = 1
                push_label(tid, get_pc(tid), get_sp(tid), 1u);
            }

            // ── IF ──
            case OP_IF: {
                let _block_type = read_bytecode_byte(get_pc(tid));
                set_pc(tid, get_pc(tid) + 1u);
                let cond = pop_i32(tid);
                push_label(tid, 0u, get_sp(tid), 0u);
                if cond == 0u {
                    // Skip to else or end
                    skip_to_end_or_else(tid);
                }
            }

            // ── ELSE ──
            case OP_ELSE: {
                // We're in the true branch and hit else — skip to end
                pop_label(tid);
                push_label(tid, 0u, get_sp(tid), 0u);
                // Skip to matching end
                var depth: u32 = 1u;
                var pc = get_pc(tid);
                while pc < bytecode_len && depth > 0u {
                    let op = read_bytecode_byte(pc);
                    pc = pc + 1u;
                    if op == OP_BLOCK || op == OP_LOOP || op == OP_IF {
                        pc = pc + 1u; // skip block type
                        depth = depth + 1u;
                    } else if op == OP_END {
                        depth = depth - 1u;
                    }
                }
                set_pc(tid, pc);
                pop_label(tid);
            }

            // ── END ──
            case OP_END: {
                let lp = get_lp(tid);
                if lp > 0u {
                    pop_label(tid);
                } else {
                    // End of function — return
                    let fp = get_fp(tid);
                    if fp == 0u {
                        // Top-level end — execution complete
                        return;
                    }
                    let frame = pop_frame(tid);
                    set_pc(tid, frame.x);  // return_pc
                }
            }

            // ── BR ──
            case OP_BR: {
                let depth = read_leb128_u32(tid);
                let label = get_label(tid, depth);
                // Unwind label stack
                let target_lp = get_lp(tid) - depth - 1u;
                set_lp(tid, target_lp);
                set_sp(tid, label.y); // restore stack depth
                if label.z == 1u {
                    // Loop: branch to loop head
                    set_pc(tid, label.x);
                    push_label(tid, label.x, label.y, 1u); // re-push loop label
                }
                // Block: continue after end (already past it via pc)
            }

            // ── BR_IF ──
            case OP_BR_IF: {
                let depth = read_leb128_u32(tid);
                let cond = pop_i32(tid);
                if cond != 0u {
                    let label = get_label(tid, depth);
                    let target_lp = get_lp(tid) - depth - 1u;
                    set_lp(tid, target_lp);
                    set_sp(tid, label.y);
                    if label.z == 1u {
                        set_pc(tid, label.x);
                        push_label(tid, label.x, label.y, 1u);
                    }
                }
            }

            // ── RETURN ──
            case OP_RETURN: {
                let fp = get_fp(tid);
                if fp == 0u {
                    return; // top-level return
                }
                let frame = pop_frame(tid);
                set_pc(tid, frame.x);
                // Reset label pointer to before this call
                // (simplified — proper impl would track label depth per frame)
            }

            // ── CALL ──
            case OP_CALL: {
                let func_idx = read_leb128_u32(tid);
                if func_idx < import_count {
                    // Host function call
                    dispatch_host_function(tid, func_idx);
                } else {
                    // Internal function call via function table
                    let func_entry = get_func_entry(func_idx);
                    let code_offset = func_entry.x;

                    if code_offset == 0u && func_idx != get_import_count() {
                        set_ejected(tid, 1u);
                        set_ejection_reason(tid, EJECTION_TRAP);
                    } else {
                        // Save current frame
                        push_frame(tid, get_pc(tid), 0u, get_sp(tid), func_idx);
                        // Jump to callee code
                        set_pc(tid, code_offset);
                        // Push a label for the function body
                        push_label(tid, 0u, get_sp(tid), 0u);
                    }
                }
            }

            // ── DROP ──
            case OP_DROP: {
                _ =pop_i32(tid);
            }

            // ── SELECT ──
            case OP_SELECT: {
                let cond = pop_i32(tid);
                let val2 = pop_i32(tid);
                let val1 = pop_i32(tid);
                if cond != 0u { push_i32(tid, val1); }
                else { push_i32(tid, val2); }
            }

            // ── LOCAL.GET ──
            case OP_LOCAL_GET: {
                let idx = read_leb128_u32(tid);
                push_i32(tid, get_local(tid, idx));
            }

            // ── LOCAL.SET ──
            case OP_LOCAL_SET: {
                let idx = read_leb128_u32(tid);
                set_local(tid, idx, pop_i32(tid));
            }

            // ── LOCAL.TEE ──
            case OP_LOCAL_TEE: {
                let idx = read_leb128_u32(tid);
                let val = peek_i32(tid);
                set_local(tid, idx, val);
            }

            // ── I32.CONST ──
            case OP_I32_CONST: {
                let val = read_leb128_i32(tid);
                push_i32(tid, bitcast<u32>(val));
            }

            // ── I32 LOAD ──
            case OP_I32_LOAD: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let base = pop_i32(tid);
                let addr = base + offset;
                if !consume_fuel(tid, 2u) { return; }
                let val = wasm_load_u32(tid, addr);
                push_i32(tid, val);
            }

            // ── I32 STORE ──
            case OP_I32_STORE: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let val = pop_i32(tid);
                let base = pop_i32(tid);
                let addr = base + offset;
                if !consume_fuel(tid, 2u) { return; }
                wasm_store_u32(tid, addr, val);
            }

            // ── I32 LOAD8_U ──
            case OP_I32_LOAD8_U: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let base = pop_i32(tid);
                let val = wasm_load_byte(tid, base + offset);
                push_i32(tid, val);
            }

            // ── I32 LOAD8_S ──
            case OP_I32_LOAD8_S: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let base = pop_i32(tid);
                var val = wasm_load_byte(tid, base + offset);
                if (val & 0x80u) != 0u { val = val | 0xFFFFFF00u; }
                push_i32(tid, val);
            }

            // ── I32 STORE8 ──
            case OP_I32_STORE8: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let val = pop_i32(tid);
                let base = pop_i32(tid);
                wasm_store_byte(tid, base + offset, val);
            }

            // ── MEMORY.SIZE ──
            case OP_MEMORY_SIZE: {
                _ =read_leb128_u32(tid); // memory index (always 0)
                push_i32(tid, get_mem_pages(tid));
            }

            // ── MEMORY.GROW ──
            case OP_MEMORY_GROW: {
                _ =read_leb128_u32(tid); // memory index
                let pages = pop_i32(tid);
                let current = get_mem_pages(tid);
                let new_pages = current + pages;
                if new_pages > WASM_MEMORY_PAGES {
                    push_i32(tid, 0xFFFFFFFFu); // -1 = failure
                } else {
                    set_mem_pages(tid, new_pages);
                    push_i32(tid, current);
                }
            }

            // ── I32 arithmetic ──
            case OP_I32_ADD: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a + b); }
            case OP_I32_SUB: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a - b); }
            case OP_I32_MUL: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a * b); }
            case OP_I32_DIV_U: {
                let b = pop_i32(tid); let a = pop_i32(tid);
                if b == 0u { set_ejected(tid, 1u); set_ejection_reason(tid, EJECTION_TRAP); }
                else { push_i32(tid, a / b); }
            }
            case OP_I32_DIV_S: {
                let b = bitcast<i32>(pop_i32(tid)); let a = bitcast<i32>(pop_i32(tid));
                if b == 0i { set_ejected(tid, 1u); set_ejection_reason(tid, EJECTION_TRAP); }
                else { push_i32(tid, bitcast<u32>(a / b)); }
            }
            case OP_I32_REM_U: {
                let b = pop_i32(tid); let a = pop_i32(tid);
                if b == 0u { set_ejected(tid, 1u); set_ejection_reason(tid, EJECTION_TRAP); }
                else { push_i32(tid, a % b); }
            }
            case OP_I32_REM_S: {
                let b = bitcast<i32>(pop_i32(tid)); let a = bitcast<i32>(pop_i32(tid));
                if b == 0i { set_ejected(tid, 1u); set_ejection_reason(tid, EJECTION_TRAP); }
                else { push_i32(tid, bitcast<u32>(a % b)); }
            }

            // ── I32 bitwise ──
            case OP_I32_AND: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a & b); }
            case OP_I32_OR:  { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a | b); }
            case OP_I32_XOR: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a ^ b); }
            case OP_I32_SHL: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a << (b & 31u)); }
            case OP_I32_SHR_U: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, a >> (b & 31u)); }
            case OP_I32_SHR_S: {
                let b = pop_i32(tid); let a = bitcast<i32>(pop_i32(tid));
                push_i32(tid, bitcast<u32>(a >> (b & 31u)));
            }
            case OP_I32_ROTL: {
                let b = pop_i32(tid) & 31u; let a = pop_i32(tid);
                push_i32(tid, (a << b) | (a >> (32u - b)));
            }
            case OP_I32_ROTR: {
                let b = pop_i32(tid) & 31u; let a = pop_i32(tid);
                push_i32(tid, (a >> b) | (a << (32u - b)));
            }

            // ── I32 comparison ──
            case OP_I32_EQZ: { let a = pop_i32(tid); push_i32(tid, select(0u, 1u, a == 0u)); }
            case OP_I32_EQ:  { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, select(0u, 1u, a == b)); }
            case OP_I32_NE:  { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, select(0u, 1u, a != b)); }
            case OP_I32_LT_U: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, select(0u, 1u, a < b)); }
            case OP_I32_GT_U: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, select(0u, 1u, a > b)); }
            case OP_I32_LE_U: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, select(0u, 1u, a <= b)); }
            case OP_I32_GE_U: { let b = pop_i32(tid); let a = pop_i32(tid); push_i32(tid, select(0u, 1u, a >= b)); }
            case OP_I32_LT_S: {
                let b = bitcast<i32>(pop_i32(tid)); let a = bitcast<i32>(pop_i32(tid));
                push_i32(tid, select(0u, 1u, a < b));
            }
            case OP_I32_GT_S: {
                let b = bitcast<i32>(pop_i32(tid)); let a = bitcast<i32>(pop_i32(tid));
                push_i32(tid, select(0u, 1u, a > b));
            }
            case OP_I32_LE_S: {
                let b = bitcast<i32>(pop_i32(tid)); let a = bitcast<i32>(pop_i32(tid));
                push_i32(tid, select(0u, 1u, a <= b));
            }
            case OP_I32_GE_S: {
                let b = bitcast<i32>(pop_i32(tid)); let a = bitcast<i32>(pop_i32(tid));
                push_i32(tid, select(0u, 1u, a >= b));
            }

            // ── I32 misc ──
            case OP_I32_CLZ: { let a = pop_i32(tid); push_i32(tid, countLeadingZeros(a)); }
            case OP_I32_CTZ: { let a = pop_i32(tid); push_i32(tid, countTrailingZeros(a)); }
            case OP_I32_POPCNT: { let a = pop_i32(tid); push_i32(tid, countOneBits(a)); }

            // ── Conversions ──
            case OP_I32_WRAP_I64: {
                // For now i64 is not fully supported — just pass through low word
                // This works if the stack only has i32 values
                // (proper i64 requires dual-word stack entries)
            }
            case OP_I32_EXTEND8_S: {
                var a = pop_i32(tid);
                if (a & 0x80u) != 0u { a = a | 0xFFFFFF00u; }
                push_i32(tid, a);
            }
            case OP_I32_EXTEND16_S: {
                var a = pop_i32(tid);
                if (a & 0x8000u) != 0u { a = a | 0xFFFF0000u; }
                push_i32(tid, a);
            }

            // ── I64 operations — eject for now (Phase 3 TODO) ──
            case OP_I64_CONST: {
                // Read and discard the i64 LEB128 immediate
                _ =read_leb128_i32(tid); // low bits
                // i64 not fully supported yet — eject
                set_ejected(tid, 1u);
                set_ejection_reason(tid, EJECTION_UNSUPPORTED);
            }
            case OP_I64_ADD, OP_I64_SUB, OP_I64_MUL, OP_I64_DIV_S, OP_I64_DIV_U,
                 OP_I64_REM_S, OP_I64_REM_U, OP_I64_AND, OP_I64_OR, OP_I64_XOR,
                 OP_I64_SHL, OP_I64_SHR_S, OP_I64_SHR_U, OP_I64_ROTL, OP_I64_ROTR,
                 OP_I64_CLZ, OP_I64_CTZ, OP_I64_POPCNT,
                 OP_I64_EQZ, OP_I64_EQ, OP_I64_NE,
                 OP_I64_LT_S, OP_I64_LT_U, OP_I64_GT_S, OP_I64_GT_U,
                 OP_I64_LE_S, OP_I64_LE_U, OP_I64_GE_S, OP_I64_GE_U,
                 OP_I64_LOAD, OP_I64_STORE,
                 OP_I64_LOAD8_S, OP_I64_LOAD8_U, OP_I64_LOAD16_S, OP_I64_LOAD16_U,
                 OP_I64_LOAD32_S, OP_I64_LOAD32_U, OP_I64_STORE8, OP_I64_STORE16,
                 OP_I64_STORE32, OP_I64_EXTEND_I32_S, OP_I64_EXTEND_I32_U,
                 OP_I64_EXTEND8_S, OP_I64_EXTEND16_S, OP_I64_EXTEND32_S: {
                set_ejected(tid, 1u);
                set_ejection_reason(tid, EJECTION_UNSUPPORTED);
            }

            // ── Floating point — always eject ──
            case OP_F32_LOAD, OP_F64_LOAD, OP_F32_STORE, OP_F64_STORE: {
                set_ejected(tid, 1u);
                set_ejection_reason(tid, EJECTION_UNSUPPORTED);
            }

            // ── I32 LOAD16 variants ──
            case OP_I32_LOAD16_U: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let base = pop_i32(tid);
                let addr = base + offset;
                let b0 = wasm_load_byte(tid, addr);
                let b1 = wasm_load_byte(tid, addr + 1u);
                push_i32(tid, b0 | (b1 << 8u));
            }
            case OP_I32_LOAD16_S: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let base = pop_i32(tid);
                let addr = base + offset;
                let b0 = wasm_load_byte(tid, addr);
                let b1 = wasm_load_byte(tid, addr + 1u);
                var val = b0 | (b1 << 8u);
                if (val & 0x8000u) != 0u { val = val | 0xFFFF0000u; }
                push_i32(tid, val);
            }

            // ── I32 STORE16 ──
            case OP_I32_STORE16: {
                let _align = read_leb128_u32(tid);
                let offset = read_leb128_u32(tid);
                let val = pop_i32(tid);
                let base = pop_i32(tid);
                let addr = base + offset;
                wasm_store_byte(tid, addr, val & 0xFFu);
                wasm_store_byte(tid, addr + 1u, (val >> 8u) & 0xFFu);
            }

            // ── Unknown opcode — eject ──
            default: {
                set_ejected(tid, 1u);
                set_ejection_reason(tid, EJECTION_UNSUPPORTED);
            }
        }
    }

    if max_iterations == 0u {
        set_ejected(tid, 1u);
        set_ejection_reason(tid, EJECTION_FUEL_EXHAUSTED);
    }
}

// ── Output layout ────────────────────────────────────────────────────
// Per-thread result: 8 u32 header + 64 u32 return data = 72 u32s
const RESULT_U32S: u32 = 72u;

fn write_result(tid: u32) {
    let base = tid * RESULT_U32S;
    let ejected = get_ejected(tid);

    output_data[base + 0u] = select(1u, 0u, ejected != 0u);  // success
    output_data[base + 1u] = ejected;                          // ejected
    output_data[base + 2u] = get_ejection_reason(tid);         // ejection_reason
    output_data[base + 3u] = get_fuel_lo(tid);                 // gas_used_lo (remaining fuel)
    output_data[base + 4u] = get_fuel_hi(tid);                 // gas_used_hi
    output_data[base + 5u] = 0u;                               // return_data_len
    output_data[base + 6u] = 0u;                               // kv_write_count
    output_data[base + 7u] = 0u;                               // _pad
}

// ── Compute entry point ──────────────────────────────────────────────

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tid = global_id.x;
    let message_count = get_message_count();

    if tid >= message_count {
        return;
    }

    // Initialize thread state
    set_pc(tid, get_entry_pc());
    set_sp(tid, 0u);
    set_fp(tid, 0u);
    set_lp(tid, 0u);
    set_fuel_lo(tid, get_base_fuel_lo());
    set_fuel_hi(tid, get_base_fuel_hi());
    set_ejected(tid, 0u);
    set_ejection_reason(tid, EJECTION_NONE);
    set_mem_pages(tid, WASM_MEMORY_PAGES);

    // Run the WASM interpreter
    interpret(tid);

    // Write results
    write_result(tid);
}
