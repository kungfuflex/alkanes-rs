use alkanes_support::response::CallResponse;
use anyhow::Result;

// Re-export the MessageDispatch derive macro
pub use alkanes_macros::MessageDispatch;

/// Trait for dispatching messages based on opcodes
pub trait MessageDispatch<T>: Sized {
    /// Convert from an opcode and inputs to a message enum variant
    fn from_opcode(opcode: u128, inputs: Vec<u128>) -> Result<Self>;

    /// Dispatch the message to the appropriate method on the responder
    fn dispatch(&self, responder: &T) -> Result<CallResponse>;

    /// Export ABI metadata for the message enum
    fn export_abi() -> Vec<u8>;
}
