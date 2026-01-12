use anyhow::Result;
use serde_json::json;

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Number,
    U128Be,
    U64Be,
    U32Be,
    U16Be,
    U8Be,
    String,
}

impl std::str::FromStr for OutputFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "number" => Ok(OutputFormat::Number),
            "u128be" => Ok(OutputFormat::U128Be),
            "u64be" => Ok(OutputFormat::U64Be),
            "u32be" => Ok(OutputFormat::U32Be),
            "u16be" => Ok(OutputFormat::U16Be),
            "u8be" => Ok(OutputFormat::U8Be),
            "string" => Ok(OutputFormat::String),
            _ => Err(anyhow::anyhow!(
                "Invalid format '{}'. Valid formats: number, u128be, u64be, u32be, u16be, u8be, string",
                s
            )),
        }
    }
}

impl OutputFormat {
    /// Parse data bytes according to format, returning JSON output
    pub fn parse(&self, data: &[u8]) -> Result<serde_json::Value> {
        match self {
            OutputFormat::Number => {
                if data.is_empty() {
                    anyhow::bail!("No data to parse (execution.data is empty)");
                }
                if data.len() > 16 {
                    anyhow::bail!(
                        "Data too large for number format (max 16 bytes for u128), got {} bytes\n  Raw hex: 0x{}",
                        data.len(),
                        hex::encode(data)
                    );
                }
                // Read as little-endian u128, padding with zeros if needed
                let mut bytes = [0u8; 16];
                bytes[..data.len()].copy_from_slice(data);
                let value = u128::from_le_bytes(bytes);
                Ok(json!({
                    "formatted_value": value.to_string(),
                    "format_type": "number",
                    "raw_hex": format!("0x{}", hex::encode(data)),
                    "byte_count": data.len()
                }))
            }

            OutputFormat::U128Be => {
                if data.len() != 16 {
                    anyhow::bail!(
                        "Expected exactly 16 bytes for u128be, got {} bytes\n  Raw hex: 0x{}",
                        data.len(),
                        hex::encode(data)
                    );
                }
                let value = u128::from_be_bytes(data.try_into().unwrap());
                Ok(json!({
                    "formatted_value": value.to_string(),
                    "format_type": "u128be",
                    "raw_hex": format!("0x{}", hex::encode(data)),
                    "byte_count": data.len()
                }))
            }

            OutputFormat::U64Be => {
                if data.len() != 8 {
                    anyhow::bail!(
                        "Expected exactly 8 bytes for u64be, got {} bytes\n  Raw hex: 0x{}",
                        data.len(),
                        hex::encode(data)
                    );
                }
                let value = u64::from_be_bytes(data.try_into().unwrap());
                Ok(json!({
                    "formatted_value": value,
                    "format_type": "u64be",
                    "raw_hex": format!("0x{}", hex::encode(data)),
                    "byte_count": data.len()
                }))
            }

            OutputFormat::U32Be => {
                if data.len() != 4 {
                    anyhow::bail!(
                        "Expected exactly 4 bytes for u32be, got {} bytes\n  Raw hex: 0x{}",
                        data.len(),
                        hex::encode(data)
                    );
                }
                let value = u32::from_be_bytes(data.try_into().unwrap());
                Ok(json!({
                    "formatted_value": value,
                    "format_type": "u32be",
                    "raw_hex": format!("0x{}", hex::encode(data)),
                    "byte_count": data.len()
                }))
            }

            OutputFormat::U16Be => {
                if data.len() != 2 {
                    anyhow::bail!(
                        "Expected exactly 2 bytes for u16be, got {} bytes\n  Raw hex: 0x{}",
                        data.len(),
                        hex::encode(data)
                    );
                }
                let value = u16::from_be_bytes(data.try_into().unwrap());
                Ok(json!({
                    "formatted_value": value,
                    "format_type": "u16be",
                    "raw_hex": format!("0x{}", hex::encode(data)),
                    "byte_count": data.len()
                }))
            }

            OutputFormat::U8Be => {
                if data.len() != 1 {
                    anyhow::bail!(
                        "Expected exactly 1 byte for u8be, got {} bytes\n  Raw hex: 0x{}",
                        data.len(),
                        hex::encode(data)
                    );
                }
                let value = data[0];
                Ok(json!({
                    "formatted_value": value,
                    "format_type": "u8be",
                    "raw_hex": format!("0x{}", hex::encode(data)),
                    "byte_count": data.len()
                }))
            }

            OutputFormat::String => {
                let value = std::str::from_utf8(data)
                    .map_err(|e| anyhow::anyhow!(
                        "Invalid UTF-8 data: {}\n  Raw hex: 0x{}",
                        e,
                        hex::encode(data)
                    ))?;
                Ok(json!({
                    "formatted_value": value,
                    "format_type": "string",
                    "raw_hex": format!("0x{}", hex::encode(data)),
                    "byte_count": data.len()
                }))
            }
        }
    }
}
