//! Parser for calldata and alkanes transfer parameters
//! 
//! Format: [block,tx,inputs...]:[block:tx:value]:[block:tx:value]
//! Example: [4,302206,101]:[2:0:4000000]:[2:1:400000]

use crate::{AlkanesError, Result};
use alkanes_support::{cellpack::Cellpack, id::AlkaneId, parcel::{AlkaneTransfer, AlkaneTransferParcel}};

/// Parse the params string into Cellpack (calldata) and AlkaneTransferParcel (alkanes)
/// Format: [block,tx,inputs...]:[block:tx:value]:[block:tx:value]
pub fn parse_params(params: &str) -> Result<(Cellpack, AlkaneTransferParcel)> {
    let parts: Vec<&str> = params.split(':').collect();
    if parts.is_empty() {
        return Err(AlkanesError::InvalidParameters(
            "Empty params string".to_string(),
        ));
    }

    // Parse first part as Cellpack (calldata)
    let cellpack = parse_cellpack(parts[0])?;

    // Parse remaining parts as AlkaneTransferParcel (alkanes)
    let mut transfers = Vec::new();
    for part in parts.iter().skip(1) {
        transfers.push(parse_alkane_transfer(part)?);
    }
    let parcel = AlkaneTransferParcel(transfers);

    Ok((cellpack, parcel))
}

/// Parse a cellpack from format: [block,tx,input1,input2,...]
/// Example: [4,302206,101]
fn parse_cellpack(s: &str) -> Result<Cellpack> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return Err(AlkanesError::InvalidParameters(format!(
            "Invalid cellpack format, expected [block,tx,inputs...], got: {}",
            s
        )));
    }

    let inner = &s[1..s.len() - 1];
    let values: Result<Vec<u128>> = inner
        .split(',')
        .map(|v| {
            v.trim().parse::<u128>().map_err(|_| {
                AlkanesError::InvalidParameters(format!("Invalid u128 value: {}", v))
            })
        })
        .collect();

    let values = values?;
    if values.len() < 2 {
        return Err(AlkanesError::InvalidParameters(
            "Cellpack must have at least block and tx".to_string(),
        ));
    }

    let target = AlkaneId {
        block: values[0],
        tx: values[1],
    };
    let inputs = values[2..].to_vec();

    Ok(Cellpack { target, inputs })
}

/// Parse an alkane transfer from format: [block:tx:value]
/// Example: [2:0:4000000]
fn parse_alkane_transfer(s: &str) -> Result<AlkaneTransfer> {
    let s = s.trim();
    
    // Handle both [block:tx:value] and block:tx:value formats
    let inner = if s.starts_with('[') && s.ends_with(']') {
        &s[1..s.len() - 1]
    } else {
        s
    };

    let parts: Vec<&str> = inner.split(':').collect();
    if parts.len() != 3 {
        return Err(AlkanesError::InvalidParameters(format!(
            "Invalid alkane transfer format, expected block:tx:value, got: {}",
            s
        )));
    }

    let block = parts[0].trim().parse::<u128>().map_err(|_| {
        AlkanesError::InvalidParameters(format!("Invalid block value: {}", parts[0]))
    })?;
    let tx = parts[1].trim().parse::<u128>().map_err(|_| {
        AlkanesError::InvalidParameters(format!("Invalid tx value: {}", parts[1]))
    })?;
    let value = parts[2].trim().parse::<u128>().map_err(|_| {
        AlkanesError::InvalidParameters(format!("Invalid value: {}", parts[2]))
    })?;

    Ok(AlkaneTransfer {
        id: AlkaneId { block, tx },
        value,
    })
}

/// Parse an outpoint from format: txid:vout
pub fn parse_outpoint(s: &str) -> Result<(String, u32)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(AlkanesError::InvalidParameters(format!(
            "Invalid outpoint format, expected txid:vout, got: {}",
            s
        )));
    }

    let txid = parts[0].to_string();
    let vout = parts[1].parse::<u32>().map_err(|_| {
        AlkanesError::InvalidParameters(format!("Invalid vout value: {}", parts[1]))
    })?;

    Ok((txid, vout))
}

/// Parse an alkane ID from format: block:tx
pub fn parse_alkane_id(s: &str) -> Result<AlkaneId> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(AlkanesError::InvalidParameters(format!(
            "Invalid alkane ID format, expected block:tx, got: {}",
            s
        )));
    }

    let block = parts[0].parse::<u128>().map_err(|_| {
        AlkanesError::InvalidParameters(format!("Invalid block value: {}", parts[0]))
    })?;
    let tx = parts[1].parse::<u128>().map_err(|_| {
        AlkanesError::InvalidParameters(format!("Invalid tx value: {}", parts[1]))
    })?;

    Ok(AlkaneId { block, tx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cellpack() {
        let result = parse_cellpack("[4,302206,101]").unwrap();
        assert_eq!(result.target.block, 4);
        assert_eq!(result.target.tx, 302206);
        assert_eq!(result.inputs, vec![101]);
    }

    #[test]
    fn test_parse_alkane_transfer() {
        let result = parse_alkane_transfer("[2:0:4000000]").unwrap();
        assert_eq!(result.id.block, 2);
        assert_eq!(result.id.tx, 0);
        assert_eq!(result.value, 4000000);
    }

    #[test]
    fn test_parse_params() {
        let (cellpack, parcel) = parse_params("[4,302206,101]:[2:0:4000000]:[2:1:400000]").unwrap();
        assert_eq!(cellpack.target.block, 4);
        assert_eq!(cellpack.target.tx, 302206);
        assert_eq!(cellpack.inputs, vec![101]);
        assert_eq!(parcel.0.len(), 2);
        assert_eq!(parcel.0[0].value, 4000000);
        assert_eq!(parcel.0[1].value, 400000);
    }

    #[test]
    fn test_parse_outpoint() {
        let (txid, vout) = parse_outpoint("abc123:0").unwrap();
        assert_eq!(txid, "abc123");
        assert_eq!(vout, 0);
    }

    #[test]
    fn test_parse_alkane_id() {
        let id = parse_alkane_id("4:302206").unwrap();
        assert_eq!(id.block, 4);
        assert_eq!(id.tx, 302206);
    }
}
