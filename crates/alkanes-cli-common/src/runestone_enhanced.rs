//! Enhanced Runestone decoder
//!
//! This module provides functionality for decoding Runestone transactions
//! and extracting protostone data from them. It supports all types of protostones
//! including DIESEL, Alkane contract calls, and Protorune token operations.
//!
//! The module provides two main functions:
//! - `decode_runestone`: Manually extracts and decodes Runestone data from a transaction
//! - `format_runestone`: Uses the ordinals crate to extract Runestones and convert them to Protostones

use anyhow::{anyhow, Context, Result};
use bitcoin::Transaction;
use bitcoin::blockdata::script::Instruction;
use bitcoin::blockdata::opcodes;
use log::{debug, trace};
use serde_json::{json, Value};
use ordinals::{Artifact, Runestone};
use crate::alkanes::protostone::Protostone;
use crate::alkanes::utils::decode_varint_list;
use hex;
use std::io::Cursor;



/// Magic number for Runestone protocol
pub const RUNESTONE_MAGIC_NUMBER: u8 = 13; // OP_PUSHNUM_13

/// Protocol tags for different protostone types
pub mod protocol_tags {
    /// DIESEL token operations
    pub const DIESEL: u128 = 1;
    
    /// Alkane contract calls
    pub const ALKANE: u128 = 2;
    
    /// Protorune token operations
    pub const PROTORUNE: u128 = 3;
    
    /// Alkane state operations
    pub const ALKANE_STATE: u128 = 4;
    
    /// Alkane event operations
    pub const ALKANE_EVENT: u128 = 5;
}

/// Operation types for Protorune token operations
pub mod protorune_operations {
    /// Mint operation
    pub const MINT: u8 = 1;
    
    /// Transfer operation
    pub const TRANSFER: u8 = 2;
    
    /// Burn operation
    pub const BURN: u8 = 3;
    
    /// Split operation
    pub const SPLIT: u8 = 4;
    
    /// Join operation
    pub const JOIN: u8 = 5;
}

/// Operation types for DIESEL token operations
pub mod diesel_operations {
    /// Mint operation (message [2, 0, 77])
    pub const MINT: [u8; 3] = [2, 0, 77];
}

/// Decode a Runestone from a transaction
///
/// This function manually extracts and decodes Runestone data from a transaction.
/// It searches for outputs with OP_RETURN followed by OP_PUSHNUM_13, then decodes
/// the payload to extract protocol data and protostone information.
///
/// # Arguments
///
/// * `tx` - The transaction to decode
///
/// # Returns
///
/// A JSON object containing the decoded Runestone data, or an error if no valid
/// Runestone was found in the transaction.
///
/// # Example
///
/// ```ignore
/// use bdk::bitcoin::Transaction;
/// use alkanes_cli_common::runestone_enhanced::decode_runestone;
/// use anyhow::Result;
///
/// fn example() -> Result<()> {
///     // let tx = get_transaction_from_somewhere();
///     // let runestone_data = decode_runestone(&tx)?;
///     // println!("{}", serde_json::to_string_pretty(&runestone_data)?);
///     Ok(())
/// }
/// ```
pub fn decode_runestone(tx: &Transaction) -> Result<Value> {
    debug!("Decoding Runestone from transaction {}", tx.compute_txid());
    
    // Search transaction outputs for Runestone
    for (vout, output) in tx.output.iter().enumerate() {
        let mut instructions = output.script_pubkey.instructions();
        
        // Check for OP_RETURN
        if instructions.next() != Some(Ok(Instruction::Op(opcodes::all::OP_RETURN))) {
            continue;
        }
        
        // Check for magic number (OP_PUSHNUM_13)
        if instructions.next() != Some(Ok(Instruction::Op(opcodes::all::OP_PUSHNUM_13))) {
            continue;
        }
        
        // Found a Runestone
        debug!("Found Runestone in output {vout}");
        
        // Extract payload from script
        let payload = extract_payload_from_instructions(instructions)?;
        
        // Decode the integers from the payload
        let integers = decode_integers(&payload)
            .context("Failed to decode integers from Runestone payload")?;
        
        // Extract protocol data (tag 13)
        let protocol_data = extract_protocol_data(&integers);
        
        // Create the base result
        let mut result = json!({
            "transaction_id": tx.compute_txid().to_string(),
            "output_index": vout,
            "protocol_data": protocol_data,
        });
        
        // Extract all tags and their values
        let all_tags = extract_all_tags(&integers);
        result["all_tags"] = all_tags;
        
        // Process protocol data if available
        if !protocol_data.is_empty() {
            // Extract protocol tag and message bytes
            let protocol_tag = protocol_data[0];
            let message_bytes: Vec<u8> = protocol_data.iter().skip(1).map(|&n| n as u8).collect();
            
            result["protocol_tag"] = json!(protocol_tag);
            result["message_bytes"] = json!(message_bytes);
            
            // Decode protostone based on protocol tag
            result["protostone"] = decode_protostone(protocol_tag, &message_bytes);
        }
        
        // Add raw integers for debugging
        result["raw_integers"] = json!(integers);
        
        return Ok(result);
    }
    
    Err(anyhow!("No Runestone found in transaction"))
}

/// Extract payload from script instructions
fn extract_payload_from_instructions<'a, I>(instructions: I) -> Result<Vec<u8>>
where
    I: Iterator<Item = std::result::Result<Instruction<'a>, bitcoin::blockdata::script::Error>>
{
    let mut payload = Vec::new();
    
    for result in instructions {
        match result {
            Ok(Instruction::PushBytes(push)) => {
                // Convert PushBytes to a slice before extending
                payload.extend_from_slice(push.as_bytes());
            }
            Ok(Instruction::Op(_)) => {
                return Err(anyhow!("Invalid opcode in Runestone payload"));
            }
            Err(_) => {
                return Err(anyhow!("Invalid script in Runestone payload"));
            }
        }
    }
    
    Ok(payload)
}

/// Extract protocol data (tag 13) from integers
fn extract_protocol_data(integers: &[u128]) -> Vec<u128> {
    let mut protocol_data = Vec::new();
    let mut i = 0;
    
    while i < integers.len() {
        let tag = integers[i];
        i += 1;
        
        // Tag 13 is the protocol tag
        if tag == RUNESTONE_MAGIC_NUMBER as u128 && i < integers.len() {
            protocol_data.push(integers[i]);
            i += 1;
        } else {
            // Skip other tags and their values
            if i < integers.len() {
                i += 1;
            }
        }
    }
    
    protocol_data
}

/// Extract all tags and their values from integers
fn extract_all_tags(integers: &[u128]) -> Value {
    let mut all_tags = json!({});
    let mut i = 0;
    
    while i < integers.len() {
        if i + 1 < integers.len() {
            let tag = integers[i];
            let value = integers[i + 1];
            
            // Add to the all_tags object
            if all_tags[tag.to_string()].is_null() {
                all_tags[tag.to_string()] = json!([value]);
            } else {
                all_tags[tag.to_string()].as_array_mut().unwrap().push(json!(value));
            }
            
            i += 2;
        } else {
            // Odd number of integers, skip the last one
            i += 1;
        }
    }
    
    all_tags
}

/// Decode protostone based on protocol tag
fn decode_protostone(protocol_tag: u128, message_bytes: &[u8]) -> Value {
    match protocol_tag {
        protocol_tags::DIESEL => decode_diesel_protostone(message_bytes),
        protocol_tags::ALKANE => decode_alkane_protostone(message_bytes),
        protocol_tags::PROTORUNE => decode_protorune_protostone(message_bytes),
        protocol_tags::ALKANE_STATE => decode_alkane_state_protostone(message_bytes),
        protocol_tags::ALKANE_EVENT => decode_alkane_event_protostone(message_bytes),
        _ => json!({
            "type": "Unknown",
            "protocol_tag": protocol_tag,
            "cellpack": message_bytes
        })
    }
}

/// Decode DIESEL protostone
fn decode_diesel_protostone(message_bytes: &[u8]) -> Value {
    // DIESEL token minting
    if message_bytes == diesel_operations::MINT {
        json!({
            "type": "DIESEL",
            "operation": "mint",
            "cellpack": {
                "message_type": message_bytes[0],
                "reserved": message_bytes[1],
                "action": "M" // ASCII 77 = 'M' for 'Mint'
            }
        })
    } else {
        json!({
            "type": "DIESEL",
            "operation": "unknown",
            "cellpack": message_bytes
        })
    }
}

/// Decode Alkane contract call protostone
fn decode_alkane_protostone(message_bytes: &[u8]) -> Value {
    let mut result = json!({
        "type": "Alkane",
        "operation": "contract_call",
        "cellpack": message_bytes
    });
    
    // Try to decode the cellpack structure
    if message_bytes.len() >= 2 {
        let call_type = message_bytes[0];
        let data = &message_bytes[1..];
        
        let call_type_name = match call_type {
            1 => "deploy",
            2 => "call",
            3 => "upgrade",
            _ => "unknown"
        };
        
        result["cellpack"] = json!({
            "call_type": call_type,
            "call_type_name": call_type_name,
            "data": data
        });
        
        // For contract calls (type 2), try to decode function selector and arguments
        if call_type == 2 && data.len() >= 4 {
            let function_selector = &data[0..4];
            let arguments = &data[4..];
            
            result["cellpack"]["function_selector"] = json!(hex::encode(function_selector));
            result["cellpack"]["arguments"] = json!(hex::encode(arguments));
        }
    }
    
    result
}

/// Decode Protorune token operation protostone
fn decode_protorune_protostone(message_bytes: &[u8]) -> Value {
    let mut result = json!({
        "type": "Protorune",
        "operation": "token_operation",
        "cellpack": message_bytes
    });
    
    // Try to decode the cellpack structure
    if message_bytes.len() >= 2 {
        let operation_type = message_bytes[0];
        let data = &message_bytes[1..];
        
        let operation_name = match operation_type {
            protorune_operations::MINT => "mint",
            protorune_operations::TRANSFER => "transfer",
            protorune_operations::BURN => "burn",
            protorune_operations::SPLIT => "split",
            protorune_operations::JOIN => "join",
            _ => "unknown"
        };
        
        result["cellpack"] = json!({
            "operation_type": operation_type,
            "operation_name": operation_name,
            "data": data
        });
        
        // For mint operations, try to decode token details
        if operation_type == protorune_operations::MINT && data.len() >= 3 {
            let token_id = data[0];
            let amount = data[1];
            let metadata = &data[2..];
            
            result["cellpack"]["token_details"] = json!({
                "token_id": token_id,
                "amount": amount,
                "metadata": metadata
            });
        }
        
        // For transfer operations, try to decode transfer details
        if operation_type == protorune_operations::TRANSFER && data.len() >= 3 {
            let token_id = data[0];
            let amount = data[1];
            let recipient = &data[2..];
            
            result["cellpack"]["transfer_details"] = json!({
                "token_id": token_id,
                "amount": amount,
                "recipient": hex::encode(recipient)
            });
        }
    }
    
    result
}

/// Decode Alkane state operation protostone
fn decode_alkane_state_protostone(message_bytes: &[u8]) -> Value {
    json!({
        "type": "AlkaneState",
        "operation": "state_operation",
        "cellpack": message_bytes
    })
}

/// Decode Alkane event operation protostone
fn decode_alkane_event_protostone(message_bytes: &[u8]) -> Value {
    json!({
        "type": "AlkaneEvent",
        "operation": "event_operation",
        "cellpack": message_bytes
    })
}

/// Decode integers from a payload
///
/// This function decodes a sequence of variable-length integers from a byte payload.
/// It processes the payload sequentially, extracting one integer at a time until
/// the entire payload has been consumed.
///
/// # Arguments
///
/// * `payload` - The byte payload to decode
///
/// # Returns
///
/// A vector of decoded integers, or an error if the payload is invalid.
///
/// # Errors
///
/// Returns an error if:
/// - The payload contains a truncated varint
/// - A varint is too large (exceeds 128 bits)
fn decode_integers(payload: &[u8]) -> Result<Vec<u128>> {
    let mut integers = Vec::new();
    let mut i = 0;
    
    while i < payload.len() {
        let (integer, length) = decode_varint(&payload[i..])
            .context(format!("Failed to decode varint at position {i}"))?;
        integers.push(integer);
        i += length;
    }
    
    Ok(integers)
}

/// Decode a variable-length integer
///
/// This function decodes a single variable-length integer from a byte slice using
/// the LEB128 encoding format. Each byte uses 7 bits for the value and 1 bit to
/// indicate if more bytes follow (1) or if this is the last byte (0).
///
/// # Arguments
///
/// * `bytes` - The byte slice to decode from
///
/// # Returns
///
/// A tuple containing the decoded integer and the number of bytes consumed,
/// or an error if the encoding is invalid.
///
/// # Errors
///
/// Returns an error if:
/// - The byte slice is empty or truncated
/// - The varint is too large (exceeds 128 bits)
///
/// # Algorithm
///
/// The LEB128 encoding uses the high bit (0x80) of each byte to indicate if more
/// bytes follow (1) or if this is the last byte (0). The remaining 7 bits contribute
/// to the value, with each successive byte adding 7 more bits of precision.
fn decode_varint(bytes: &[u8]) -> Result<(u128, usize)> {
    let mut result: u128 = 0;
    let mut shift = 0;
    let mut i = 0;
    
    loop {
        if i >= bytes.len() {
            return Err(anyhow!("Truncated varint"));
        }
        
        let byte = bytes[i];
        i += 1;
        
        result |= u128::from(byte & 0x7f) << shift;
        
        if byte & 0x80 == 0 {
            break;
        }
        
        shift += 7;
        
        if shift > 127 {
            return Err(anyhow!("Varint too large"));
        }
    }
    
    Ok((result, i))
}

/// Decode the message field of a Protostone from Vec<u8> to Vec<u128>
///
/// This function uses decode_varint_list to convert the message bytes
/// into a vector of u128 values, similar to how it's done in alkanes-rs.
///
/// # Arguments
///
/// * `message_bytes` - The message field from a Protostone as Vec<u8>
///
/// # Returns
///
/// A vector of u128 values decoded from the message bytes, or an error if decoding fails.
pub fn decode_protostone_message(message_bytes: &[u8]) -> Result<Vec<u128>> {
    if message_bytes.is_empty() {
        return Ok(Vec::new());
    }
    
    let mut cursor = Cursor::new(message_bytes.to_vec());
    decode_varint_list(&mut cursor)
        .context("Failed to decode protostone message as varint list")
}

/// Format a Runestone from a transaction using the ordinals crate
///
/// This function uses the ordinals crate to extract a Runestone from a transaction
/// and convert it to a vector of Protostones.
///
/// # Arguments
///
/// * `tx` - The transaction to extract the Runestone from
///
/// # Returns
///
/// A vector of Protostones, or an error if no valid Runestone was found in the transaction.
///
/// # Example
///
/// ```ignore
/// use bdk::bitcoin::Transaction;
/// use alkanes_cli_common::runestone_enhanced::format_runestone;
/// use anyhow::Result;
///
/// fn example() -> Result<()> {
///     // let tx = get_transaction_from_somewhere();
///     // let protostones = format_runestone(&tx)?;
///     // for protostone in protostones {
///     //     println!("{:?}", protostone);
///     // }
///     Ok(())
/// }
/// ```
pub fn format_runestone(tx: &Transaction) -> Result<Vec<Protostone>> {
    trace!("Formatting Runestone from transaction {}", tx.compute_txid());
    
    // Use the ordinals crate to decipher the Runestone
    let artifact = Runestone::decipher(tx)
        .ok_or_else(|| anyhow!("Failed to decipher Runestone"))
        .context("No Runestone found in transaction")?;
    
    // Extract the Runestone from the artifact
    match artifact {
        Artifact::Runestone(ref runestone) => {
            // Convert the Runestone to Protostones
            Protostone::from_runestone(runestone)
                .context("Failed to convert Runestone to Protostones")
        },
        _ => Err(anyhow!("Artifact is not a Runestone"))
    }
}

/// Extract address information from script pubkey
fn extract_address_from_script(script: &bitcoin::Script) -> Option<Value> {
    use bitcoin::Address;
    use bitcoin::Network;
    
    // Try to convert script to address
    if let Ok(address) = Address::from_script(script, Network::Bitcoin) {
        let script_type = if script.is_p2pkh() {
            "P2PKH"
        } else if script.is_p2sh() {
            "P2SH"
        } else if script.is_p2tr() {
            "P2TR"
        } else if script.is_witness_program() {
            "Witness"
        } else {
            "Unknown"
        };
        
        Some(json!({
            "address": address.to_string(),
            "script_type": script_type
        }))
    } else {
        None
    }
}

/// Format a Runestone from a transaction with decoded messages
///
/// This function extracts Protostones from a transaction and decodes their message fields
/// from Vec<u8> to Vec<u128> using decode_varint_list, providing a more detailed view
/// of the protostone data. It also includes comprehensive transaction information.
///
/// # Arguments
///
/// * `tx` - The transaction to extract the Runestone from
///
/// # Returns
///
/// A JSON value containing the protostones with decoded messages, or an error if no valid
/// Runestone was found in the transaction.
pub fn format_runestone_with_decoded_messages(tx: &Transaction) -> Result<Value> {
    let protostones = format_runestone(tx)?;
    
    // Build comprehensive transaction information
    let mut inputs = Vec::new();
    for (i, input) in tx.input.iter().enumerate() {
        inputs.push(json!({
            "index": i,
            "previous_output": {
                "txid": input.previous_output.txid.to_string(),
                "vout": input.previous_output.vout
            },
            "script_sig_size": input.script_sig.len(),
            "sequence": input.sequence.0,
            "witness_items": input.witness.len()
        }));
    }
    
    let mut outputs = Vec::new();
    for (i, output) in tx.output.iter().enumerate() {
        let mut output_info = json!({
            "index": i,
            "value": output.value,
            "script_pubkey": hex::encode(output.script_pubkey.as_bytes()),
            "script_pubkey_size": output.script_pubkey.len(),
            "script_type": "Unknown"
        });
        
        // Check if this is an OP_RETURN output
        if output.script_pubkey.is_op_return() {
            output_info["script_type"] = json!("OP_RETURN");
            
            // Extract OP_RETURN data
            let op_return_bytes = output.script_pubkey.as_bytes();
            if op_return_bytes.len() > 2 {
                let data_bytes = &op_return_bytes[2..]; // Skip OP_RETURN and length byte
                output_info["op_return_data"] = json!(hex::encode(data_bytes));
                output_info["op_return_size"] = json!(data_bytes.len());
            }
        } else {
            // Try to extract address information
            if let Some(address_info) = extract_address_from_script(&output.script_pubkey) {
                output_info["address"] = address_info["address"].clone();
                output_info["script_type"] = address_info["script_type"].clone();
            } else {
                // Determine script type without address
                if output.script_pubkey.is_p2pkh() {
                    output_info["script_type"] = json!("P2PKH");
                } else if output.script_pubkey.is_p2sh() {
                    output_info["script_type"] = json!("P2SH");
                } else if output.script_pubkey.is_p2tr() {
                    output_info["script_type"] = json!("P2TR");
                } else if output.script_pubkey.is_witness_program() {
                    output_info["script_type"] = json!("Witness");
                }
            }
        }
        
        outputs.push(output_info);
    }
    
    let mut result = json!({
        "transaction_id": tx.compute_txid().to_string(),
        "version": tx.version,
        "lock_time": tx.lock_time.to_consensus_u32(),
        "inputs": inputs,
        "outputs": outputs,
        "input_count": tx.input.len(),
        "output_count": tx.output.len(),
        "protostones": []
    });
    
    for protostone in protostones {
        let decoded_message = if !protostone.message.is_empty() {
            match decode_protostone_message(&protostone.message) {
                Ok(decoded) => Some(decoded),
                Err(e) => {
                    debug!("Failed to decode protostone message: {e}");
                    None
                }
            }
        } else {
            Some(Vec::new())
        };
        
        // Determine protocol name
        let protocol_name = match protostone.protocol_tag {
            1 => "ALKANES Metaprotocol",
            _ => "Unknown Protocol",
        };
        
        let mut protostone_json = json!({
            "protocol_tag": protostone.protocol_tag,
            "protocol_name": protocol_name,
            "message_bytes": protostone.message,
            "message_decoded": decoded_message,
            "burn": protostone.burn,
            "refund": protostone.refund,
            "pointer": protostone.pointer,
            "from": protostone.from,
            "edicts": protostone.edicts.iter().map(|edict| {
                let mut edict_json = json!({
                    "id": {
                        "block": edict.id.block,
                        "tx": edict.id.tx
                    },
                    "amount": edict.amount,
                    "output": edict.output
                });
                
                // Add destination output information if valid
                if (edict.output as usize) < tx.output.len() {
                    let dest_output = &tx.output[edict.output as usize];
                    edict_json["destination"] = json!({
                        "value": dest_output.value,
                        "script_type": if dest_output.script_pubkey.is_op_return() {
                            "OP_RETURN"
                        } else if dest_output.script_pubkey.is_p2pkh() {
                            "P2PKH"
                        } else if dest_output.script_pubkey.is_p2sh() {
                            "P2SH"
                        } else if dest_output.script_pubkey.is_p2tr() {
                            "P2TR"
                        } else if dest_output.script_pubkey.is_witness_program() {
                            "Witness"
                        } else {
                            "Unknown"
                        }
                    });
                    
                    if let Some(address_info) = extract_address_from_script(&dest_output.script_pubkey) {
                        edict_json["destination"]["address"] = address_info["address"].clone();
                    }
                }
                
                edict_json
            }).collect::<Vec<_>>()
        });
        
        // Add pointer destination information
        if let Some(pointer) = protostone.pointer {
            if (pointer as usize) < tx.output.len() {
                let pointer_output = &tx.output[pointer as usize];
                protostone_json["pointer_destination"] = json!({
                    "output_index": pointer,
                    "value": pointer_output.value,
                    "script_type": if pointer_output.script_pubkey.is_op_return() {
                        "OP_RETURN"
                    } else if pointer_output.script_pubkey.is_p2pkh() {
                        "P2PKH"
                    } else if pointer_output.script_pubkey.is_p2sh() {
                        "P2SH"
                    } else if pointer_output.script_pubkey.is_p2tr() {
                        "P2TR"
                    } else if pointer_output.script_pubkey.is_witness_program() {
                        "Witness"
                    } else {
                        "Unknown"
                    }
                });
                
                if let Some(address_info) = extract_address_from_script(&pointer_output.script_pubkey) {
                    protostone_json["pointer_destination"]["address"] = address_info["address"].clone();
                }
            }
        }
        
        // Add refund destination information
        if let Some(refund) = protostone.refund {
            if (refund as usize) < tx.output.len() {
                let refund_output = &tx.output[refund as usize];
                protostone_json["refund_destination"] = json!({
                    "output_index": refund,
                    "value": refund_output.value,
                    "script_type": if refund_output.script_pubkey.is_op_return() {
                        "OP_RETURN"
                    } else if refund_output.script_pubkey.is_p2pkh() {
                        "P2PKH"
                    } else if refund_output.script_pubkey.is_p2sh() {
                        "P2SH"
                    } else if refund_output.script_pubkey.is_p2tr() {
                        "P2TR"
                    } else if refund_output.script_pubkey.is_witness_program() {
                        "Witness"
                    } else {
                        "Unknown"
                    }
                });
                
                if let Some(address_info) = extract_address_from_script(&refund_output.script_pubkey) {
                    protostone_json["refund_destination"]["address"] = address_info["address"].clone();
                }
            }
        }
        
        result["protostones"].as_array_mut().unwrap().push(protostone_json);
    }
    
    Ok(result)
}

/// Print human-readable, styled runestone information (same as used in deezel runestone command)
pub fn print_human_readable_runestone(tx: &Transaction, result: &serde_json::Value) {
    println!("🔍 Transaction Analysis");
    println!("═══════════════════════");
    
    // Transaction basic info
    if let Some(txid) = result.get("transaction_id").and_then(|v| v.as_str()) {
        println!("📋 Transaction ID: {txid}");
    }
    println!("🔢 Version: {}", tx.version);
    println!("🔒 Lock Time: {}", tx.lock_time);
    
    // Transaction inputs
    println!("\n📥 Inputs ({}):", tx.input.len());
    for (i, input) in tx.input.iter().enumerate() {
        println!("  {}. 🔗 {}:{}", i + 1, input.previous_output.txid, input.previous_output.vout);
        if !input.witness.is_empty() {
            println!("     📝 Witness: {} items", input.witness.len());
        }
    }
    
    // Transaction outputs
    println!("\n📤 Outputs ({}):", tx.output.len());
    for (i, output) in tx.output.iter().enumerate() {
        println!("  {}. 💰 {} sats", i, output.value);
        
        // Check if this is an OP_RETURN output
        if output.script_pubkey.is_op_return() {
            println!("     📜 OP_RETURN script ({} bytes)", output.script_pubkey.len());
            // Show OP_RETURN data in hex
            let op_return_bytes = output.script_pubkey.as_bytes();
            if op_return_bytes.len() > 2 {
                let data_bytes = &op_return_bytes[2..]; // Skip OP_RETURN and length byte
                let hex_data = hex::encode(data_bytes);
                println!("     📄 Data: {hex_data}");
            }
        } else {
            // Try to extract address
            match extract_address_from_script(&output.script_pubkey) {
                Some(address_info) => {
                    println!("     🏠 {}: {}", address_info.get("script_type").and_then(|v| v.as_str()).unwrap_or("Unknown"), address_info.get("address").and_then(|v| v.as_str()).unwrap_or("Unknown"));
                }
                None => {
                    if output.script_pubkey.is_p2pkh() {
                        println!("     🏠 P2PKH (Legacy)");
                    } else if output.script_pubkey.is_p2sh() {
                        println!("     🏛️  P2SH (Script Hash)");
                    } else if output.script_pubkey.is_p2tr() {
                        println!("     🌳 P2TR (Taproot)");
                    } else if output.script_pubkey.is_witness_program() {
                        println!("     ⚡ Witness Program (SegWit)");
                    } else {
                        println!("     📋 Script ({} bytes)", output.script_pubkey.len());
                    }
                }
            }
        }
    }
    
    // Protostones information
    if let Some(protostones) = result.get("protostones").and_then(|v| v.as_array()) {
        if protostones.is_empty() {
            println!("\n🚫 No protostones found in this transaction");
        } else {
            println!("\n🪨 Protostones Found: {}", protostones.len());
            println!("═══════════════════════");
            
            for (i, protostone) in protostones.iter().enumerate() {
                println!("\n🪨 Protostone #{}", i + 1);
                println!("───────────────────");
                
                // Protocol tag
                if let Some(protocol_tag) = protostone.get("protocol_tag").and_then(|v| v.as_u64()) {
                    let protocol_name = match protocol_tag {
                        1 => "ALKANES Metaprotocol",
                        _ => "Unknown Protocol",
                    };
                    println!("🏷️  Protocol: {protocol_name} (tag: {protocol_tag})");
                }
                
                // Message information
                if let Some(message_bytes) = protostone.get("message_bytes").and_then(|v| v.as_array()) {
                    println!("📨 Message ({} bytes):", message_bytes.len());
                    
                    // Show raw bytes
                    let bytes_str = message_bytes.iter()
                        .filter_map(|v| v.as_u64())
                        .map(|n| format!("{n:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("   📄 Raw bytes: {bytes_str}");
                    
                    // Show decoded values
                    if let Some(message_decoded) = protostone.get("message_decoded").and_then(|v| v.as_array()) {
                        let decoded_str = message_decoded.iter()
                            .filter_map(|v| v.as_u64())
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        println!("   🔓 Decoded: [{decoded_str}]");
                        
                        // Special handling for DIESEL tokens
                        if let Some(protocol_tag) = protostone.get("protocol_tag").and_then(|v| v.as_u64()) {
                            if protocol_tag == 1 && message_decoded.len() >= 3 {
                                if let (Some(first), Some(second), Some(third)) = (
                                    message_decoded[0].as_u64(),
                                    message_decoded[1].as_u64(),
                                    message_decoded[2].as_u64()
                                ) {
                                    if first == 2 && second == 0 && third == 77 {
                                        println!("   🔥 DIESEL Token Mint Detected!");
                                        println!("   ⚡ Cellpack: [2, 0, 77] (Standard DIESEL mint)");
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Edicts with tree view
                if let Some(edicts) = protostone.get("edicts").and_then(|v| v.as_array()) {
                    if !edicts.is_empty() {
                        println!("📋 Token Transfers ({}):", edicts.len());
                        for (j, edict) in edicts.iter().enumerate() {
                            if let Some(edict_obj) = edict.as_object() {
                                let id_block = edict_obj.get("id").and_then(|v| v.get("block")).and_then(|v| v.as_u64()).unwrap_or(0);
                                let id_tx = edict_obj.get("id").and_then(|v| v.get("tx")).and_then(|v| v.as_u64()).unwrap_or(0);
                                let amount = edict_obj.get("amount").and_then(|v| v.as_u64()).unwrap_or(0);
                                let output_idx = edict_obj.get("output").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                
                                let tree_symbol = if j == edicts.len() - 1 { "└─" } else { "├─" };
                                println!("   {tree_symbol} 🪙 Token {id_block}:{id_tx}");
                                println!("   {}    💰 Amount: {} units", if j == edicts.len() - 1 { "  " } else { "│ " }, amount);
                                
                                // Show destination output details
                                if output_idx < tx.output.len() {
                                    let dest_output = &tx.output[output_idx];
                                    println!("   {}    🎯 → Output {}: {} sats",
                                        if j == edicts.len() - 1 { "  " } else { "│ " },
                                        output_idx, dest_output.value);
                                    
                                    if let Some(addr_info) = extract_address_from_script(&dest_output.script_pubkey) {
                                        println!("   {}       📍 {}",
                                            if j == edicts.len() - 1 { "  " } else { "│ " },
                                            addr_info.get("address").and_then(|v| v.as_str()).unwrap_or("Unknown"));
                                    }
                                } else {
                                    println!("   {}    ❌ → Invalid output {}",
                                        if j == edicts.len() - 1 { "  " } else { "│ " },
                                        output_idx);
                                }
                            }
                        }
                    }
                }
                
                // Pointer and refund with output details
                if let Some(pointer) = protostone.get("pointer").and_then(|v| v.as_u64()) {
                    let pointer_idx = pointer as usize;
                    println!("👉 Pointer: output {pointer}");
                    if pointer_idx < tx.output.len() {
                        let pointer_output = &tx.output[pointer_idx];
                        println!("   └─ 💰 {} sats", pointer_output.value);
                        if let Some(addr_info) = extract_address_from_script(&pointer_output.script_pubkey) {
                            println!("      📍 {}", addr_info.get("address").and_then(|v| v.as_str()).unwrap_or("Unknown"));
                        }
                    }
                }
                
                if let Some(refund) = protostone.get("refund").and_then(|v| v.as_u64()) {
                    let refund_idx = refund as usize;
                    println!("💸 Refund: output {refund}");
                    if refund_idx < tx.output.len() {
                        let refund_output = &tx.output[refund_idx];
                        println!("   └─ 💰 {} sats", refund_output.value);
                        if let Some(addr_info) = extract_address_from_script(&refund_output.script_pubkey) {
                            println!("      📍 {}", addr_info.get("address").and_then(|v| v.as_str()).unwrap_or("Unknown"));
                        }
                    }
                }
            }
        }
    }
    
    println!("\n✅ Analysis complete!");
}