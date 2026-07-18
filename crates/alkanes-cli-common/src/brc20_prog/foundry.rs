// Foundry JSON parsing utilities for BRC20-prog contract deployment

use crate::{AlkanesError, Result};
use serde::{Deserialize, Serialize};
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec, format};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec, format, fs, path::Path};

/// Foundry build artifact structure
/// This matches the output from `forge build --extra-output-files bin`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundryBuildArtifact {
    pub abi: Vec<serde_json::Value>,
    pub bytecode: BytecodeInfo,
    #[serde(rename = "deployedBytecode")]
    pub deployed_bytecode: Option<BytecodeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BytecodeInfo {
    pub object: String,
    #[serde(rename = "sourceMap")]
    pub source_map: Option<String>,
    #[serde(rename = "linkReferences")]
    pub link_references: Option<serde_json::Value>,
}

/// Parse Foundry build JSON from file path
#[cfg(feature = "std")]
pub fn parse_foundry_json<P: AsRef<Path>>(path: P) -> Result<FoundryBuildArtifact> {
    let content = fs::read_to_string(path.as_ref())
        .map_err(|e| AlkanesError::Other(format!("Failed to read file: {}", e)))?;
    
    parse_foundry_json_from_str(&content)
}

/// Parse Foundry build JSON from string
pub fn parse_foundry_json_from_str(json_str: &str) -> Result<FoundryBuildArtifact> {
    serde_json::from_str(json_str)
        .map_err(|e| AlkanesError::Other(format!("Failed to parse Foundry JSON: {}", e)))
}

/// Extract deployment bytecode from Foundry artifact
pub fn extract_deployment_bytecode(artifact: &FoundryBuildArtifact) -> Result<String> {
    let bytecode = &artifact.bytecode.object;
    
    // Ensure it has 0x prefix
    let bytecode = if bytecode.starts_with("0x") {
        bytecode.clone()
    } else {
        format!("0x{}", bytecode)
    };
    
    if bytecode.len() <= 2 {
        return Err(AlkanesError::Other("Empty bytecode in Foundry artifact".to_string()));
    }
    
    Ok(bytecode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_foundry_json() {
        let json = r#"{
            "abi": [],
            "bytecode": {
                "object": "0x608060405234801561001057600080fd5b50",
                "sourceMap": "",
                "linkReferences": {}
            }
        }"#;
        
        let artifact = parse_foundry_json_from_str(json).unwrap();
        let bytecode = extract_deployment_bytecode(&artifact).unwrap();
        
        assert_eq!(bytecode, "0x608060405234801561001057600080fd5b50");
    }

    #[test]
    fn test_parse_foundry_json_no_prefix() {
        let json = r#"{
            "abi": [],
            "bytecode": {
                "object": "608060405234801561001057600080fd5b50",
                "sourceMap": "",
                "linkReferences": {}
            }
        }"#;
        
        let artifact = parse_foundry_json_from_str(json).unwrap();
        let bytecode = extract_deployment_bytecode(&artifact).unwrap();
        
        assert_eq!(bytecode, "0x608060405234801561001057600080fd5b50");
    }
}
