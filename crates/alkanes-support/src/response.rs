use crate::parcel::AlkaneTransferParcel;
use crate::storage::StorageMap;
use anyhow::Result;
use metashrew_support::utils::consume_to_end;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CallResponse {
    pub alkanes: AlkaneTransferParcel,
    pub data: Vec<u8>,
}

impl CallResponse {
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<CallResponse> {
        let parcel = AlkaneTransferParcel::parse(cursor)?;
        Ok(CallResponse {
            alkanes: parcel,
            data: consume_to_end(cursor)?,
        })
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut list = Vec::<Vec<u128>>::new();
        list.push(vec![self.alkanes.0.len() as u128]);
        self.alkanes
            .0
            .iter()
            .for_each(|v| list.push(vec![v.id.block, v.id.tx, v.value]));
        let mut result: Vec<u8> = list
            .into_iter()
            .flatten()
            .map(|v| (&v.to_le_bytes()).to_vec())
            .flatten()
            .collect();
        result.extend(&self.data);
        result
    }
    pub fn forward(incoming_alkanes: &AlkaneTransferParcel) -> CallResponse {
        let mut response = CallResponse::default();
        response.alkanes = incoming_alkanes.clone();
        response
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendedCallResponse {
    pub alkanes: AlkaneTransferParcel,
    pub storage: StorageMap,
    pub data: Vec<u8>,
}

impl Into<ExtendedCallResponse> for CallResponse {
    fn into(self) -> ExtendedCallResponse {
        ExtendedCallResponse {
            alkanes: self.alkanes,
            storage: StorageMap::default(),
            data: self.data,
        }
    }
}

impl Into<CallResponse> for ExtendedCallResponse {
    fn into(self) -> CallResponse {
        CallResponse {
            alkanes: self.alkanes,
            data: self.data,
        }
    }
}

impl ExtendedCallResponse {
    pub fn serialize(&self) -> Vec<u8> {
        let mut list = Vec::<Vec<u128>>::new();
        list.push(vec![self.alkanes.0.len() as u128]);
        self.alkanes
            .0
            .iter()
            .for_each(|v| list.push(vec![v.id.block, v.id.tx, v.value]));
        let mut result: Vec<u8> = list
            .into_iter()
            .flatten()
            .map(|v| (&v.to_le_bytes()).to_vec())
            .flatten()
            .collect();
        result.extend(&self.storage.serialize());
        result.extend(&self.data);
        result
    }
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Self> {
        let alkanes = AlkaneTransferParcel::parse(cursor)?;
        let storage = StorageMap::parse(cursor)?;
        let data = consume_to_end(cursor)?;
        Ok(Self {
            alkanes,
            storage,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use anyhow::Result;

    use crate::response::ExtendedCallResponse;
    #[test]
    pub fn test_serialize_deserialize() -> Result<()> {
        let mut response = ExtendedCallResponse::default();
        response.data.push(1);
        let serialized = response.serialize();
        let mut c = Cursor::new(serialized);
        let parsed = ExtendedCallResponse::parse(&mut c)?;
        assert_eq!(parsed, response);
        Ok(())
    }
}
