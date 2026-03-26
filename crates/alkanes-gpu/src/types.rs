//! GPU buffer data structures.
//!
//! All structs are #[repr(C)] and derive bytemuck traits so they can be
//! copied directly into wgpu storage buffers. Field layout must match
//! the corresponding WGSL struct definitions in shader.wgsl exactly.

use bytemuck::{Pod, Zeroable};

// ── Constants ────────────────────────────────────────────────────────

pub const MAX_SHARD_MESSAGES: usize = 64;
pub const MAX_CALLDATA_SIZE: usize = 256;
pub const MAX_KV_PAIRS: usize = 1024;
pub const MAX_KEY_SIZE: usize = 256;
pub const MAX_VALUE_SIZE: usize = 1024;
pub const MAX_RETURN_DATA_SIZE: usize = 256;

// Ejection reason codes
pub const EJECTION_NONE: u32 = 0;
pub const EJECTION_STORAGE_OVERFLOW: u32 = 1;
pub const EJECTION_MEMORY_CONSTRAINT: u32 = 2;
pub const EJECTION_KV_OVERFLOW: u32 = 3;
pub const EJECTION_CALLDATA_OVERFLOW: u32 = 4;
pub const EJECTION_EXTCALL: u32 = 5;
pub const EJECTION_FUEL_EXHAUSTED: u32 = 6;

// ── Input structures ─────────────────────────────────────────────────

/// Header for a shard dispatch. Sits at the start of the input buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShardHeader {
    pub message_count: u32,
    pub kv_count: u32,
    pub block_height: u32,
    pub base_fuel: u32, // fuel per message (low 32 bits)
    pub base_fuel_hi: u32, // fuel per message (high 32 bits)
    pub _pad: [u32; 3],
}

/// One message to execute on GPU.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMessageInput {
    pub txid: [u8; 32],
    pub tx_index: u32,
    pub vout: u32,
    pub target_block: u32,
    pub target_tx: u32,
    pub calldata_len: u32,
    pub _pad: [u32; 3],
    pub calldata: [u8; MAX_CALLDATA_SIZE],
}

/// One preloaded key-value pair for the shard context.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuKvPair {
    pub key_len: u32,
    pub value_len: u32,
    pub _pad: [u32; 2],
    pub key: [u8; MAX_KEY_SIZE],
    pub value: [u8; MAX_VALUE_SIZE],
}

// ── Output structures ────────────────────────────────────────────────

/// Per-message result written by the shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMessageResult {
    pub success: u32,
    pub ejected: u32,
    pub ejection_reason: u32,
    pub gas_used_lo: u32,
    pub gas_used_hi: u32,
    pub return_data_len: u32,
    pub kv_write_count: u32,
    pub _pad: u32,
    pub return_data: [u8; MAX_RETURN_DATA_SIZE],
}

/// K/V write produced by a message during execution.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuKvWrite {
    pub message_index: u32,
    pub key_len: u32,
    pub value_len: u32,
    pub _pad: u32,
    pub key: [u8; MAX_KEY_SIZE],
    pub value: [u8; MAX_VALUE_SIZE],
}
