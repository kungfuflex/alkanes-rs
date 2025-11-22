//! Parsing logic for alkanes commands
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec, format};
#[cfg(feature = "std")]
use std::{string::{String, ToString}, vec::Vec, format};
use anyhow::{anyhow, Context, Result};
use super::types::{InputRequirement, OutputTarget, ProtostoneEdict, ProtostoneSpec, BitcoinTransfer};
use alkanes_support::cellpack::Cellpack;

/// Parse input requirements from string format
pub fn parse_input_requirements(input_str: &str) -> Result<Vec<InputRequirement>> {
    let mut requirements = Vec::new();
    
    for part in input_str.split(',') {
        let trimmed = part.trim();
        
        if trimmed.starts_with("B:") {
            // Bitcoin requirement: B:amount or B:amount:vN
            let parts: Vec<&str> = trimmed.split(':').collect();
            
            if parts.len() == 2 {
                // Simple format: B:amount
                let amount = parts[1].parse::<u64>()
                    .context("Invalid Bitcoin amount in input requirement")?;
                requirements.push(InputRequirement::Bitcoin { amount });
            } else if parts.len() == 3 {
                // Output assignment format: B:amount:vN
                let amount = parts[1].parse::<u64>()
                    .context("Invalid Bitcoin amount in B:amount:vN requirement")?;
                let target = parse_output_target(parts[2])
                    .context("Invalid output target in B:amount:vN requirement")?;
                requirements.push(InputRequirement::BitcoinOutput { amount, target });
            } else {
                return Err(anyhow!("Invalid Bitcoin requirement format. Expected 'B:amount' or 'B:amount:vN'"));
            }
        } else {
            // Alkanes requirement: block:tx:amount
            let parts: Vec<&str> = trimmed.split(':').collect();
            if parts.len() != 3 {
                return Err(anyhow!("Invalid alkanes input requirement format. Expected 'block:tx:amount'"));
            }
            
            let block = parts[0].parse::<u64>()
                .context("Invalid block number in alkanes requirement")?;
            let tx = parts[1].parse::<u64>()
                .context("Invalid tx number in alkanes requirement")?;
            let amount = parts[2].parse::<u64>()
                .context("Invalid amount in alkanes requirement")?;
            
            requirements.push(InputRequirement::Alkanes { block, tx, amount });
        }
    }
    
    Ok(requirements)
}

/// Parse protostone specifications from complex string format
pub fn parse_protostones(protostones_str: &str) -> Result<Vec<ProtostoneSpec>> {
    // Split by comma, but ignore commas inside [] brackets (cellpacks)
    let protostone_parts = split_respecting_brackets(protostones_str, ',')?;
    
    let mut protostones = Vec::new();
    
    for part in &protostone_parts {
        let spec = parse_single_protostone(part)?;
        protostones.push(spec);
    }
    
    Ok(protostones)
}

/// Parse a single protostone specification with flexible component ordering
/// 
/// Format: Components can appear in any order, separated by colons:
/// - Bracketed components: [cellpack] or [edict]
/// - Non-bracketed components: pointer, refund_pointer, or B:amount:target
/// 
/// Rules:
/// - First non-bracketed non-B value = pointer
/// - Second non-bracketed non-B value = refund_pointer
/// - If refund_pointer omitted, it equals pointer
/// - If both omitted, defaults to v0
/// - Bracketed components are classified by content:
///   - Cellpack: only comma-separated numbers
///   - Edict: contains colons (block:tx:amount:target)
///
/// Examples:
/// - [3,100]:v0:v1:[2:1:100:v0]
/// - v0:v1:[2:1:100:v0]:[3,100]
/// - [2:1:100:v0]:[3,100]:v0:v1
/// - [3,100]:v0:[2:1:100:v0]
fn parse_single_protostone(spec_str: &str) -> Result<ProtostoneSpec> {
    let parts = split_respecting_brackets(spec_str, ':')?;
    
    let mut cellpack = None;
    let mut edicts = Vec::new();
    let mut bitcoin_transfer = None;
    let mut non_bracketed_targets = Vec::new();
    let mut bracketed_parts = Vec::new();
    
    // Step 1: Separate bracketed from non-bracketed parts
    for part in parts {
        let trimmed = part.trim();
        
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Bracketed component (cellpack or edict)
            bracketed_parts.push(trimmed[1..trimmed.len() - 1].to_string());
        } else if trimmed.starts_with("B:") {
            // Bitcoin transfer (special case)
            bitcoin_transfer = Some(parse_bitcoin_transfer(trimmed)?);
        } else if !trimmed.is_empty() {
            // Non-bracketed target (pointer or refund)
            non_bracketed_targets.push(trimmed.to_string());
        }
    }
    
    // Step 2: Parse pointer and refund_pointer from non-bracketed targets
    let pointer = if non_bracketed_targets.is_empty() {
        // No targets specified, default to v0
        Some(OutputTarget::Output(0))
    } else {
        Some(parse_output_target(&non_bracketed_targets[0])?)
    };
    
    let refund_pointer = if non_bracketed_targets.len() >= 2 {
        // Explicit refund pointer
        Some(parse_output_target(&non_bracketed_targets[1])?)
    } else {
        // Refund pointer equals pointer
        pointer.clone()
    };
    
    // Step 3: Classify and parse bracketed components
    for content in bracketed_parts {
        if is_cellpack_format(&content) {
            // This is a cellpack (comma-separated numbers)
            if cellpack.is_some() {
                return Err(anyhow!("Multiple cellpacks found in protostone specification"));
            }
            cellpack = Some(parse_cellpack(&content)?);
        } else {
            // This is an edict (contains colons)
            edicts.push(parse_edict(&content)?);
        }
    }
    
    log::debug!("Parsed protostone: pointer={:?}, refund={:?}, cellpack={}, edicts={}", 
                pointer, refund_pointer, cellpack.is_some(), edicts.len());
    
    Ok(ProtostoneSpec {
        cellpack,
        edicts,
        bitcoin_transfer,
        pointer,
        refund: refund_pointer,
    })
}

/// Determine if a bracketed content is a cellpack format
/// Cellpack: only comma-separated numbers (may have commas)
/// Edict: contains colons (block:tx:amount:target format)
fn is_cellpack_format(content: &str) -> bool {
    // If it contains a colon, it's an edict
    if content.contains(':') {
        return false;
    }
    
    // If it only contains numbers and commas, it's a cellpack
    // Check if all parts are valid u128 numbers
    for part in content.split(',') {
        if part.trim().parse::<u128>().is_err() {
            return false;
        }
    }
    
    true
}

/// Parse cellpack from string format
fn parse_cellpack(cellpack_str: &str) -> Result<Cellpack> {
    // Parse comma-separated numbers into Vec<u128>
    let mut values = Vec::new();
    
    for part in cellpack_str.split(',') {
        let trimmed = part.trim();
        let value = trimmed.parse::<u128>()
            .with_context(|| format!("Invalid u128 value in cellpack: {trimmed}"))?;
        values.push(value);
    }
    
    // Convert Vec<u128> to Cellpack using TryFrom
    // The first two values become target (block, tx), remaining values become inputs
    Cellpack::try_from(values)
        .with_context(|| "Failed to create Cellpack from values (need at least 2 values for target)")
}

/// Parse Bitcoin transfer specification
fn parse_bitcoin_transfer(transfer_str: &str) -> Result<BitcoinTransfer> {
    // Format: B:amount:target
    let parts: Vec<&str> = transfer_str.split(':').collect();
    if parts.len() != 3 {
        return Err(anyhow!("Invalid Bitcoin transfer format. Expected 'B:amount:target'"));
    }
    
    let amount = parts[1].parse::<u64>()
        .context("Invalid amount in Bitcoin transfer")?;
    let target = parse_output_target(parts[2])?;
    
    Ok(BitcoinTransfer { amount, target })
}

/// Parse edict specification
fn parse_edict(edict_str: &str) -> Result<ProtostoneEdict> {
    // Handle both formats:
    // 1. Simple format: block:tx:amount:target
    // 2. Bracketed format: [block:tx:amount:output] (where output becomes target)
    
    let trimmed = edict_str.trim();
    
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        // Bracketed format: [block:tx:amount:output]
        let content = &trimmed[1..trimmed.len()-1];
        let parts: Vec<&str> = content.split(':').collect();
        if parts.len() != 4 {
            return Err(anyhow!("Invalid bracketed edict format. Expected '[block:tx:amount:output]'"));
        }
        
        let block = parts[0].parse::<u64>()
            .context("Invalid block number in bracketed edict")?;
        let tx = parts[1].parse::<u64>()
            .context("Invalid tx number in bracketed edict")?;
        let amount = parts[2].parse::<u64>()
            .context("Invalid amount in bracketed edict")?;
        let target = parse_output_target(parts[3])?;
        
        Ok(ProtostoneEdict {
            alkane_id: super::types::AlkaneId { block, tx },
            amount,
            target,
        })
    } else {
        // Simple format: block:tx:amount:target
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() < 4 {
            return Err(anyhow!("Invalid edict format. Expected 'block:tx:amount:target' or '[block:tx:amount:output]'"));
        }
        
        let block = parts[0].parse::<u64>()
            .context("Invalid block number in edict")?;
        let tx = parts[1].parse::<u64>()
            .context("Invalid tx number in edict")?;
        let amount = parts[2].parse::<u64>()
            .context("Invalid amount in edict")?;
        let target = parse_output_target(parts[3])?;
        
        Ok(ProtostoneEdict {
            alkane_id: super::types::AlkaneId { block, tx },
            amount,
            target,
        })
    }
}

/// Parse output target (vN, pN, or split)
fn parse_output_target(target_str: &str) -> Result<OutputTarget> {
    let trimmed = target_str.trim();
    
    if trimmed == "split" {
        Ok(OutputTarget::Split)
    } else if let Some(index_str) = trimmed.strip_prefix('v') {
        let index = index_str.parse::<u32>()
            .context("Invalid output index in target")?;
        Ok(OutputTarget::Output(index))
    } else if let Some(index_str) = trimmed.strip_prefix('p') {
        let index = index_str.parse::<u32>()
            .context("Invalid protostone index in target")?;
        Ok(OutputTarget::Protostone(index))
    } else {
        Err(anyhow!("Invalid output target format. Expected 'vN', 'pN', or 'split'"))
    }
}

/// Split string by delimiter while respecting bracket nesting
fn split_respecting_brackets(input: &str, delimiter: char) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0;
    
    for ch in input.chars() {
        match ch {
            '[' => {
                bracket_depth += 1;
                current.push(ch);
            },
            ']' => {
                bracket_depth -= 1;
                current.push(ch);
                if bracket_depth < 0 {
                    return Err(anyhow!("Unmatched closing bracket"));
                }
            },
            c if c == delimiter && bracket_depth == 0 => {
                if !current.trim().is_empty() {
                    parts.push(current.trim().to_string());
                }
                current.clear();
            },
            _ => {
                current.push(ch);
            }
        }
    }
    
    if bracket_depth != 0 {
        return Err(anyhow!("Unmatched opening bracket"));
    }
    
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    
    Ok(parts)
}
