use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Parsed alkanes.toml manifest.
#[derive(Debug, Deserialize)]
pub struct AlkanesManifest {
    pub contract: ContractSection,
    #[serde(default)]
    pub opcodes: HashMap<String, u64>,
    #[serde(default)]
    pub views: HashMap<String, toml::Value>,
    #[serde(default)]
    pub imports: HashMap<String, HashMap<String, u64>>,
}

#[derive(Debug, Deserialize)]
pub struct ContractSection {
    pub name: String,
}

impl AlkanesManifest {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("failed to read {}: {}", path.display(), e))?;
        Self::from_str(&content)
    }

    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(|e| anyhow!("failed to parse alkanes.toml: {}", e))
    }

    pub fn get_opcode(&self, wit_name: &str) -> Result<u128> {
        self.opcodes
            .get(wit_name)
            .map(|&v| v as u128)
            .ok_or_else(|| anyhow!("no opcode mapping for method '{}' in alkanes.toml", wit_name))
    }

    pub fn is_view(&self, wit_name: &str) -> bool {
        self.views.contains_key(wit_name)
    }

    pub fn get_import_opcode(&self, interface_name: &str, method_name: &str) -> Result<u128> {
        let interface = self
            .imports
            .get(interface_name)
            .ok_or_else(|| anyhow!("no import section for '{}' in alkanes.toml", interface_name))?;
        interface.get(method_name).map(|&v| v as u128).ok_or_else(|| {
            anyhow!(
                "no opcode for '{}.{}' in alkanes.toml",
                interface_name,
                method_name
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let toml = r#"
[contract]
name = "OwnedToken"

[opcodes]
initialize = 0
initialize-with-name-symbol = 1
mint = 77
burn = 88
get-name = 99
get-symbol = 100
get-total-supply = 101
get-data = 1000

[views]
get-name = true
get-symbol = true
get-total-supply = true
get-data = true

[imports.token-ref]
get-name = 99
mint = 77
"#;
        let manifest = AlkanesManifest::from_str(toml).unwrap();
        assert_eq!(manifest.contract.name, "OwnedToken");
        assert_eq!(manifest.get_opcode("initialize").unwrap(), 0);
        assert_eq!(manifest.get_opcode("mint").unwrap(), 77);
        assert!(manifest.is_view("get-name"));
        assert!(!manifest.is_view("mint"));
        assert_eq!(
            manifest.get_import_opcode("token-ref", "get-name").unwrap(),
            99
        );
    }
}
