//! # Pretty-printing for transaction and runestone analysis
//!
//! This module provides functions to display transaction and runestone
//! analysis results in a human-readable format for the CLI.

use alkanes_cli_common::alkanes::types::{
    AlkaneMetadata, AlkanesInspectResult, FuzzingResults, ReadyToSignRevealTx, ReadyToSignTx,
};
/// Pretty-prints the analysis for a transaction that is ready to be signed.
pub fn pretty_print_ready_to_sign(state: &ReadyToSignTx) {
    println!("Transaction Preview:");
    state.analysis.pretty_print();

    if let Some(inspection_result) = &state.inspection_result {
        pretty_print_inspection_preview(inspection_result);
    }
}

/// Pretty-prints the analysis for a reveal transaction that is ready to be signed.
pub fn pretty_print_reveal_analysis(state: &ReadyToSignRevealTx) {
    println!("Transaction Preview:");
    state.analysis.pretty_print();

    if let Some(inspection_result) = &state.inspection_result {
        pretty_print_inspection_preview(inspection_result);
    }
}

/// Pretty-prints a preview of the inspection/simulation results.
fn pretty_print_inspection_preview(result: &AlkanesInspectResult) {
    println!("\nSimulation Preview:");
    if let Some(metadata) = &result.metadata {
        println!("  Contract: {}", metadata.name);
        if let Some(fuzzing_results) = &result.fuzzing_results {
            if let Some(opcode_result) = fuzzing_results.opcode_results.first() {
                if let Some(method) = metadata
                    .methods
                    .iter()
                    .find(|m| m.opcode == opcode_result.opcode)
                {
                    println!("  Method: {}", method.name);
                }
            }
        }
    }
    if let Some(fuzzing_results) = &result.fuzzing_results {
        pretty_print_fuzzing_results(fuzzing_results, &result.alkane_id).unwrap();
    } else if let Some(metadata) = &result.metadata {
        pretty_print_metadata(metadata);
    }
}

/// Pretty-prints the result of an alkane inspection, mimicking the output of deezel-old.
pub fn pretty_print_inspection_result(result: &AlkanesInspectResult) -> anyhow::Result<()> {
    if let Some(codehash) = &result.codehash {
        println!("=== WASM CODEHASH ===");
        println!("📦 WASM size: {} bytes", result.bytecode_length);
        println!("🔐 SHA3 (Keccak256): 0x{codehash}");
        println!("=====================");
    }

    if let Some(metadata) = &result.metadata {
        pretty_print_metadata(metadata);
    } else if let Some(error) = &result.metadata_error {
        println!("=== ALKANE METADATA ===");
        println!("Note: Failed to extract metadata from __meta export");
        println!("Error: {error}");
        println!("========================");
    }

    if let Some(disassembly) = &result.disassembly {
        println!("=== WASM DISASSEMBLY (WAT) ===");
        println!("{disassembly}");
        println!("==============================");
    }

    if let Some(fuzzing_results) = &result.fuzzing_results {
        pretty_print_fuzzing_results(fuzzing_results, &result.alkane_id)?;
    }

    Ok(())
}

/// Pretty-prints the metadata of an alkane contract.
fn pretty_print_metadata(metadata: &AlkaneMetadata) {
    println!("=== ALKANE METADATA ===");
    println!("📦 Contract: {}", metadata.name);
    println!("🏷️  Version: {}", metadata.version);

    if let Some(desc) = &metadata.description {
        println!("📝 Description: {desc}");
    }

    if metadata.methods.is_empty() {
        println!("⚠️  No methods found");
    } else {
        println!("🔧 Methods ({}):", metadata.methods.len());

        let mut sorted_methods = metadata.methods.clone();
        sorted_methods.sort_by_key(|m| m.opcode);

        for (i, method) in sorted_methods.iter().enumerate() {
            let is_last = i == sorted_methods.len() - 1;
            let prefix = if is_last { "└──" } else { "├──" };

            let params_str = if method.params.is_empty() {
                "()".to_string()
            } else {
                format!("({})", method.params.join(", "))
            };

            println!("   {} 🎯 {} {}", prefix, method.name, params_str);

            let detail_prefix = if is_last { "      " } else { "   │  " };
            println!("{}├─ 🔢 Opcode: {}", detail_prefix, method.opcode);
            println!("{}└─ 📤 Returns: {}", detail_prefix, method.returns);

            if !is_last {
                println!("   │");
            }
        }
    }

    println!("========================");
}

/// Pretty-prints the results of fuzzing analysis.
fn pretty_print_fuzzing_results(
    fuzzing_results: &FuzzingResults,
    alkane_id: &alkanes_cli_common::alkanes::types::AlkaneId,
) -> anyhow::Result<()> {
    println!("\n=== FUZZING ANALYSIS ===");
    println!("Alkane: {}:{}", alkane_id.block, alkane_id.tx);
    println!();
    println!(
        "📊 Total opcodes tested: {}",
        fuzzing_results.total_opcodes_tested
    );
    if fuzzing_results.opcodes_filtered_out > 0 {
        println!(
            "🔍 Opcodes filtered out (undefined behavior): {}",
            fuzzing_results.opcodes_filtered_out
        );
    }
    println!(
        "✅ Successful executions: {}",
        fuzzing_results.successful_executions
    );
    println!("❌ Failed executions: {}", fuzzing_results.failed_executions);
    println!(
        "🎯 Implemented opcodes: {} total",
        fuzzing_results.implemented_opcodes.len()
    );

    if !fuzzing_results.implemented_opcodes.is_empty() {
        println!();
        println!("🔍 Implemented Opcodes:");
        let ranges = compress_opcode_ranges(&fuzzing_results.implemented_opcodes);
        println!("   📋 Opcodes: {ranges}");

        println!();
        println!("📊 Detailed Results for Implemented Opcodes:");
        for result in &fuzzing_results.opcode_results {
            let status = if result.success { "✅" } else { "❌" };
            println!(
                "   {} Opcode {}: return={:?}, time={:?}μs",
                status, result.opcode, result.return_value, result.execution_time_micros
            );

            let decoded_data = decode_data_bytevector(&result.return_data);
            println!("      📦 Data: {decoded_data}");

            if !result.host_calls.is_empty() {
                println!("      🔧 Host Calls ({}):", result.host_calls.len());
                for (i, call) in result.host_calls.iter().enumerate() {
                    let call_prefix = if i == result.host_calls.len() - 1 {
                        "└─"
                    } else {
                        "├─"
                    };
                    println!(
                        "         {} {}: {} -> {}",
                        call_prefix,
                        call.function_name,
                        call.parameters.join(", "),
                        call.result
                    );
                }
            }

            if let Some(error) = &result.error {
                println!("      ⚠️  Error: {error}");
            }
        }
    }

    println!("========================");
    Ok(())
}

/// Decodes a byte vector for display, checking for common patterns.
fn decode_data_bytevector(data: &[u8]) -> String {
    if data.is_empty() {
        return "Empty (0 bytes)".to_string();
    }

    let hex_part = if data.len() <= 32 {
        format!("Hex: {}", hex::encode(data))
    } else {
        format!(
            "Hex: {} (first 32 bytes of {})",
            hex::encode(&data[..32]),
            data.len()
        )
    };

    if data.len() >= 4 && data[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
        let message_bytes = &data[4..];
        if let Ok(utf8_string) = String::from_utf8(message_bytes.to_vec()) {
            let clean_string = utf8_string.trim_matches('\0').trim();
            if !clean_string.is_empty() && clean_string.is_ascii() {
                return format!("{hex_part} | Solidity Error: \"{clean_string}\"");
            }
        }
        return format!("{hex_part} | Solidity Error");
    }

    if let Ok(utf8_string) = String::from_utf8(data.to_vec()) {
        let clean_string = utf8_string.trim_matches('\0').trim();
        if !clean_string.is_empty() && clean_string.is_ascii() && clean_string.len() > 3 {
            return format!("{hex_part} | UTF-8: \"{clean_string}\"");
        }
    }

    if data.len() == 16 {
        let value = u128::from_le_bytes(data.try_into().unwrap_or([0; 16]));
        return format!("{hex_part} | u128: {value}");
    } else if data.len() == 8 {
        let value = u64::from_le_bytes(data.try_into().unwrap_or([0; 8]));
        return format!("{hex_part} | u64: {value}");
    } else if data.len() == 4 {
        let value = u32::from_le_bytes(data.try_into().unwrap_or([0; 4]));
        return format!("{hex_part} | u32: {value}");
    }

    hex_part
}

/// Compresses a list of opcodes into readable ranges.
fn compress_opcode_ranges(opcodes: &[u128]) -> String {
    if opcodes.is_empty() {
        return String::new();
    }

    let mut sorted_opcodes = opcodes.to_vec();
    sorted_opcodes.sort_unstable();

    let mut ranges = Vec::new();
    let mut start = sorted_opcodes[0];
    let mut end = sorted_opcodes[0];

    for &opcode in sorted_opcodes.iter().skip(1) {
        if opcode == end + 1 {
            end = opcode;
        } else {
            if start == end {
                ranges.push(start.to_string());
            } else {
                ranges.push(format!("{start}-{end}"));
            }
            start = opcode;
            end = opcode;
        }
    }

    if start == end {
        ranges.push(start.to_string());
    } else {
        ranges.push(format!("{start}-{end}"));
    }

    ranges.join(", ")
}

pub fn pretty_print_blockchain_info(info: &serde_json::Value) -> anyhow::Result<()> {
    println!("Blockchain Info:");
    if let Some(obj) = info.as_object() {
        for (key, value) in obj {
            println!("  {}: {}", key, value);
        }
    }
    Ok(())
}

pub fn pretty_print_network_info(info: &serde_json::Value) -> anyhow::Result<()> {
    println!("Network Info:");
    if let Some(obj) = info.as_object() {
        for (key, value) in obj {
            println!("  {}: {}", key, value);
        }
    }
    Ok(())
}