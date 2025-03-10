# Active Context

## Current Focus: Message Dispatch Framework

We're currently working on improving the message dispatch framework for Alkanes contracts. This framework provides a unified way to develop contracts and expose their ABIs.

### Recent Changes

We've enhanced the `MessageDispatch` derive macro to automatically generate:

1. Method dispatch logic based on opcodes
2. Parameter extraction and validation
3. JSON ABI generation with contract name, methods, opcodes, and parameter types

The framework now uses `serde_json` for proper JSON serialization instead of manual string construction, making it more robust and maintainable.

### Key Components

- **MessageDispatch trait**: Defines the interface for dispatching messages to contracts
- **Derive macro**: Automatically implements the trait for enums with method and opcode attributes
- **ABI generation**: Exposes contract methods, opcodes, and parameter types in a standardized JSON format

### Implementation Details

The `MessageDispatch` derive macro:
- Extracts method names and opcodes from enum variants
- Generates match arms for opcode-based dispatch
- Creates parameter extraction and validation logic
- Builds a JSON representation of the contract ABI

### Next Steps

1. Ensure the contract name is correctly extracted and included in the ABI
2. Add comprehensive tests for the ABI generation
3. Document the framework for developers
4. Consider adding support for return type information in the ABI

### Active Decisions

- Using `serde_json` for JSON serialization instead of manual string construction
- Keeping parameter types simple (currently just "u128") for the initial implementation
- Using runtime type information to extract the concrete contract name