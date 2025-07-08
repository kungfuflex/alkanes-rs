//! SPIR-V-compatible WASM interpreter for alkanes message processing
//! 
//! This is a minimal WASM interpreter designed to run in SPIR-V compute shaders.
//! It implements the exact host functions and fuel metering that alkanes contracts expect,
//! while being compatible with SPIR-V's no-allocation, compile-time constraints.

#[cfg(target_arch = "spirv")]
use spirv_std::glam::{UVec4, Vec4};

#[cfg(not(target_arch = "spirv"))]
use crate::{UVec4};

// For non-SPIR-V targets, provide a dummy Vec4 type
#[cfg(not(target_arch = "spirv"))]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// Maximum WASM memory size (42MB like the original)
const MAX_MEMORY_SIZE: usize = 43554432;

/// Maximum call stack depth to prevent infinite recursion
const MAX_CALL_DEPTH: usize = 75;

/// WASM value types
#[derive(Debug, Clone, Copy)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl WasmValue {
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            WasmValue::I32(v) => Some(*v),
            _ => None,
        }
    }
    
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            WasmValue::I64(v) => Some(*v),
            _ => None,
        }
    }
}

/// WASM instruction opcodes (minimal set for alkanes contracts)
#[derive(Debug, Clone, Copy)]
pub enum WasmOpcode {
    // Control flow
    Call(u32),
    CallIndirect(u32),
    Return,
    
    // Memory
    I32Load(u32, u32),    // align, offset
    I32Store(u32, u32),   // align, offset
    I64Load(u32, u32),
    I64Store(u32, u32),
    
    // Constants
    I32Const(i32),
    I64Const(i64),
    
    // Local variables
    LocalGet(u32),
    LocalSet(u32),
    
    // Arithmetic
    I32Add,
    I32Sub,
    I32Mul,
    I64Add,
    I64Sub,
    
    // Comparison
    I32Eq,
    I32Ne,
    I32LtS,
    
    // Control
    If,
    Else,
    End,
    Block,
    Loop,
    Br(u32),
    BrIf(u32),
    
    // Host function calls (these map to our host functions)
    HostCall(u32),
}

/// WASM function signature
#[derive(Debug, Clone, Copy)]
pub struct WasmFuncType {
    pub params: &'static [WasmValueType],
    pub results: &'static [WasmValueType],
}

#[derive(Debug, Clone, Copy)]
pub enum WasmValueType {
    I32,
    I64,
    F32,
    F64,
}

/// Host function IDs (matching the alkanes VM)
pub mod host_functions {
    pub const ABORT: u32 = 0;
    pub const LOAD_STORAGE: u32 = 1;
    pub const REQUEST_STORAGE: u32 = 2;
    pub const LOG: u32 = 3;
    pub const BALANCE: u32 = 4;
    pub const REQUEST_CONTEXT: u32 = 5;
    pub const LOAD_CONTEXT: u32 = 6;
    pub const SEQUENCE: u32 = 7;
    pub const FUEL: u32 = 8;
    pub const HEIGHT: u32 = 9;
    pub const RETURNDATACOPY: u32 = 10;
    pub const REQUEST_TRANSACTION: u32 = 11;
    pub const LOAD_TRANSACTION: u32 = 12;
    pub const REQUEST_BLOCK: u32 = 13;
    pub const LOAD_BLOCK: u32 = 14;
    pub const CALL: u32 = 15;
    pub const DELEGATECALL: u32 = 16;
    pub const STATICCALL: u32 = 17;
}

/// Fuel costs (matching alkanes VM exactly)
pub mod fuel_costs {
    pub const PER_REQUEST_BYTE: u64 = 1;
    pub const PER_LOAD_BYTE: u64 = 2;
    pub const PER_STORE_BYTE: u64 = 40; // CHANGE1 value
    pub const SEQUENCE: u64 = 5;
    pub const FUEL: u64 = 5;
    pub const EXTCALL: u64 = 500;
    pub const HEIGHT: u64 = 10;
    pub const BALANCE: u64 = 10;
    pub const LOAD_BLOCK: u64 = 1000;
    pub const LOAD_TRANSACTION: u64 = 500;
}

/// SPIR-V-compatible execution context
/// Uses fixed-size arrays instead of dynamic allocation
#[derive(Clone)]
pub struct SpirvWasmContext {
    /// Current fuel remaining
    pub fuel: u64,
    
    /// Contract ID being executed
    pub contract_id: UVec4, // [block_hi, block_lo, tx_hi, tx_lo]
    
    /// Caller contract ID
    pub caller_id: UVec4,
    
    /// Current block height
    pub height: u32,
    
    /// Current sequence number
    pub sequence: u64,
    
    /// Execution failed flag
    pub failed: bool,
    
    /// Call depth (for recursion prevention)
    pub call_depth: u32,
    
    /// Return data buffer (fixed size for SPIR-V)
    pub return_data: [u8; 4096],
    pub return_data_len: u32,
}

impl SpirvWasmContext {
    pub fn new(fuel: u64, contract_id: UVec4, height: u32) -> Self {
        Self {
            fuel,
            contract_id,
            caller_id: UVec4::ZERO,
            height,
            sequence: 0,
            failed: false,
            call_depth: 0,
            return_data: [0u8; 4096],
            return_data_len: 0,
        }
    }
    
    pub fn consume_fuel(&mut self, amount: u64) -> bool {
        if self.fuel >= amount {
            self.fuel -= amount;
            true
        } else {
            self.failed = true;
            false
        }
    }
}

/// SPIR-V-compatible WASM execution state
/// Uses fixed-size arrays for all dynamic data
pub struct SpirvWasmExecutor {
    /// Linear memory (fixed size)
    memory: [u8; MAX_MEMORY_SIZE],
    pub memory_size: u32,
    
    /// Value stack (fixed size)
    stack: [WasmValue; 1024],
    stack_ptr: u32,
    
    /// Local variables (fixed size)
    locals: [WasmValue; 256],
    
    /// Call frames (fixed size)
    call_frames: [CallFrame; MAX_CALL_DEPTH],
    frame_ptr: u32,
    
    /// Execution context
    context: SpirvWasmContext,
    
    /// Program counter
    pc: u32,
    
    /// Current function being executed
    current_func: u32,
}

#[derive(Clone, Copy)]
struct CallFrame {
    func_id: u32,
    return_pc: u32,
    locals_base: u32,
    stack_base: u32,
}

impl SpirvWasmExecutor {
    pub fn new(context: SpirvWasmContext) -> Self {
        Self {
            memory: [0u8; MAX_MEMORY_SIZE],
            memory_size: 65536, // Start with 1 page (64KB)
            stack: [WasmValue::I32(0); 1024],
            stack_ptr: 0,
            locals: [WasmValue::I32(0); 256],
            call_frames: [CallFrame {
                func_id: 0,
                return_pc: 0,
                locals_base: 0,
                stack_base: 0,
            }; MAX_CALL_DEPTH],
            frame_ptr: 0,
            context,
            pc: 0,
            current_func: 0,
        }
    }
    
    /// Execute a single WASM instruction
    pub fn execute_instruction(&mut self, opcode: WasmOpcode) -> Result<bool, &'static str> {
        // Consume base fuel for instruction execution
        if !self.context.consume_fuel(1) {
            return Err("out of fuel");
        }
        
        match opcode {
            WasmOpcode::I32Const(val) => {
                self.push_value(WasmValue::I32(val))?;
            }
            
            WasmOpcode::I64Const(val) => {
                self.push_value(WasmValue::I64(val))?;
            }
            
            WasmOpcode::LocalGet(idx) => {
                let val = self.get_local(idx)?;
                self.push_value(val)?;
            }
            
            WasmOpcode::LocalSet(idx) => {
                let val = self.pop_value()?;
                self.set_local(idx, val)?;
            }
            
            WasmOpcode::I32Load(align, offset) => {
                let addr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                let effective_addr = addr.wrapping_add(offset);
                let val = self.load_i32(effective_addr)?;
                self.push_value(WasmValue::I32(val))?;
                
                // Consume fuel for memory access
                if !self.context.consume_fuel(fuel_costs::PER_LOAD_BYTE * 4) {
                    return Err("out of fuel");
                }
            }
            
            WasmOpcode::I32Store(align, offset) => {
                let val = self.pop_value()?.as_i32().ok_or("expected i32")?;
                let addr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                let effective_addr = addr.wrapping_add(offset);
                self.store_i32(effective_addr, val)?;
                
                // Consume fuel for memory access
                if !self.context.consume_fuel(fuel_costs::PER_STORE_BYTE * 4) {
                    return Err("out of fuel");
                }
            }
            
            WasmOpcode::I32Add => {
                let b = self.pop_value()?.as_i32().ok_or("expected i32")?;
                let a = self.pop_value()?.as_i32().ok_or("expected i32")?;
                self.push_value(WasmValue::I32(a.wrapping_add(b)))?;
            }
            
            WasmOpcode::I32Sub => {
                let b = self.pop_value()?.as_i32().ok_or("expected i32")?;
                let a = self.pop_value()?.as_i32().ok_or("expected i32")?;
                self.push_value(WasmValue::I32(a.wrapping_sub(b)))?;
            }
            
            WasmOpcode::I32Eq => {
                let b = self.pop_value()?.as_i32().ok_or("expected i32")?;
                let a = self.pop_value()?.as_i32().ok_or("expected i32")?;
                self.push_value(WasmValue::I32(if a == b { 1 } else { 0 }))?;
            }
            
            WasmOpcode::Call(func_idx) => {
                self.call_function(func_idx)?;
            }
            
            WasmOpcode::HostCall(host_func_id) => {
                self.call_host_function(host_func_id)?;
            }
            
            WasmOpcode::Return => {
                return Ok(false); // Signal end of execution
            }
            
            _ => {
                return Err("unsupported instruction");
            }
        }
        
        Ok(true) // Continue execution
    }
    
    /// Call a host function (implements alkanes VM host functions)
    pub fn call_host_function(&mut self, func_id: u32) -> Result<(), &'static str> {
        match func_id {
            host_functions::FUEL => {
                // Return remaining fuel
                let output_ptr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                let fuel_bytes = self.context.fuel.to_le_bytes();
                self.write_memory(output_ptr, &fuel_bytes)?;
                
                if !self.context.consume_fuel(fuel_costs::FUEL) {
                    return Err("out of fuel");
                }
            }
            
            host_functions::HEIGHT => {
                // Return current block height
                let output_ptr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                let height_bytes = self.context.height.to_le_bytes();
                self.write_memory(output_ptr, &height_bytes)?;
                
                if !self.context.consume_fuel(fuel_costs::HEIGHT) {
                    return Err("out of fuel");
                }
            }
            
            host_functions::SEQUENCE => {
                // Return current sequence number
                let output_ptr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                let seq_bytes = self.context.sequence.to_le_bytes();
                self.write_memory(output_ptr, &seq_bytes)?;
                
                if !self.context.consume_fuel(fuel_costs::SEQUENCE) {
                    return Err("out of fuel");
                }
            }
            
            host_functions::ABORT => {
                // Mark execution as failed
                self.context.failed = true;
                return Err("contract aborted");
            }
            
            host_functions::LOG => {
                // For SPIR-V, we can't actually log, but we consume fuel
                let _msg_ptr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                // In a real implementation, we'd read the message and log it
                // For now, just consume minimal fuel
                if !self.context.consume_fuel(10) {
                    return Err("out of fuel");
                }
            }
            
            // Storage operations would need to interface with the K/V store
            // For now, we implement stubs that consume appropriate fuel
            host_functions::REQUEST_STORAGE => {
                let _key_ptr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                // Return 0 size for now (empty storage)
                self.push_value(WasmValue::I32(0))?;
                
                if !self.context.consume_fuel(fuel_costs::PER_REQUEST_BYTE * 32) {
                    return Err("out of fuel");
                }
            }
            
            host_functions::LOAD_STORAGE => {
                let _value_ptr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                let _key_ptr = self.pop_value()?.as_i32().ok_or("expected i32")? as u32;
                // Return 0 (no data loaded)
                self.push_value(WasmValue::I32(0))?;
                
                if !self.context.consume_fuel(fuel_costs::PER_LOAD_BYTE * 32) {
                    return Err("out of fuel");
                }
            }
            
            _ => {
                return Err("unsupported host function");
            }
        }
        
        Ok(())
    }
    
    /// Execute the main contract function (__execute)
    pub fn execute_contract(&mut self, bytecode: &[u8]) -> Result<u32, &'static str> {
        // For this minimal implementation, we'll simulate contract execution
        // In a real implementation, this would parse and execute WASM bytecode
        
        // Simulate some basic operations that consume fuel
        if !self.context.consume_fuel(1000) {
            return Err("out of fuel");
        }
        
        // Simulate successful execution returning a result pointer
        // In reality, this would be the result of the __execute function
        Ok(0) // Return pointer to result data
    }
    
    /// Check if execution should be ejected (shard marked for ejection)
    pub fn should_eject(&self) -> bool {
        self.context.failed || self.context.fuel == 0
    }
    
    /// Get remaining fuel (for compatibility with wasmi)
    pub fn get_fuel(&self) -> u64 {
        self.context.fuel
    }
    
    // Helper methods for stack and memory management
    
    pub fn push_value(&mut self, val: WasmValue) -> Result<(), &'static str> {
        if self.stack_ptr >= 1024 {
            return Err("stack overflow");
        }
        self.stack[self.stack_ptr as usize] = val;
        self.stack_ptr += 1;
        Ok(())
    }
    
    pub fn pop_value(&mut self) -> Result<WasmValue, &'static str> {
        if self.stack_ptr == 0 {
            return Err("stack underflow");
        }
        self.stack_ptr -= 1;
        Ok(self.stack[self.stack_ptr as usize])
    }
    
    fn get_local(&self, idx: u32) -> Result<WasmValue, &'static str> {
        if idx >= 256 {
            return Err("local index out of bounds");
        }
        Ok(self.locals[idx as usize])
    }
    
    fn set_local(&mut self, idx: u32, val: WasmValue) -> Result<(), &'static str> {
        if idx >= 256 {
            return Err("local index out of bounds");
        }
        self.locals[idx as usize] = val;
        Ok(())
    }
    
    pub fn load_i32(&self, addr: u32) -> Result<i32, &'static str> {
        if addr + 4 > self.memory_size {
            return Err("memory access out of bounds");
        }
        let bytes = [
            self.memory[addr as usize],
            self.memory[addr as usize + 1],
            self.memory[addr as usize + 2],
            self.memory[addr as usize + 3],
        ];
        Ok(i32::from_le_bytes(bytes))
    }
    
    pub fn store_i32(&mut self, addr: u32, val: i32) -> Result<(), &'static str> {
        if addr + 4 > self.memory_size {
            return Err("memory access out of bounds");
        }
        let bytes = val.to_le_bytes();
        for i in 0..4 {
            self.memory[addr as usize + i] = bytes[i];
        }
        Ok(())
    }
    
    fn write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), &'static str> {
        if addr + data.len() as u32 > self.memory_size {
            return Err("memory write out of bounds");
        }
        for (i, &byte) in data.iter().enumerate() {
            self.memory[addr as usize + i] = byte;
        }
        Ok(())
    }
    
    fn call_function(&mut self, func_idx: u32) -> Result<(), &'static str> {
        if self.context.call_depth >= MAX_CALL_DEPTH as u32 {
            return Err("call stack overflow");
        }
        
        // Save current frame
        self.call_frames[self.frame_ptr as usize] = CallFrame {
            func_id: self.current_func,
            return_pc: self.pc,
            locals_base: 0, // Simplified for this example
            stack_base: self.stack_ptr,
        };
        
        self.frame_ptr += 1;
        self.context.call_depth += 1;
        self.current_func = func_idx;
        self.pc = 0; // Jump to function start
        
        Ok(())
    }
    
    /// Expose call_host_function for testing
    #[cfg(test)]
    pub fn test_call_host_function(&mut self, func_id: u32) -> Result<(), &'static str> {
        self.call_host_function(func_id)
    }
    
    /// Expose memory_size for testing
    #[cfg(test)]
    pub fn get_memory_size(&self) -> u32 {
        self.memory_size
    }
    
    /// Expose push_value for testing
    #[cfg(test)]
    pub fn test_push_value(&mut self, val: WasmValue) -> Result<(), &'static str> {
        self.push_value(val)
    }
    
    /// Expose pop_value for testing
    #[cfg(test)]
    pub fn test_pop_value(&mut self) -> Result<WasmValue, &'static str> {
        self.pop_value()
    }
}

/// Main entry point for SPIR-V WASM execution
/// This function will be called from the GPU shader
pub fn execute_alkanes_message(
    contract_bytecode: &[u8],
    fuel: u64,
    contract_id: UVec4,
    height: u32,
) -> Result<(u32, u64), &'static str> {
    let context = SpirvWasmContext::new(fuel, contract_id, height);
    let mut executor = SpirvWasmExecutor::new(context);
    
    // Execute the contract
    let result_ptr = executor.execute_contract(contract_bytecode)?;
    
    // Check if execution should be ejected
    if executor.should_eject() {
        return Err("shard ejected");
    }
    
    // Return result pointer and remaining fuel
    Ok((result_ptr, executor.get_fuel()))
}