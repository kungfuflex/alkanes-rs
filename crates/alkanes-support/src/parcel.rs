use crate::id::AlkaneId;
use anyhow::Result;
use metashrew_support::utils::consume_sized_int;
use metashrew_support::{byte_view::ByteView, index_pointer::KeyValuePointer};
use protorune_support::balance_sheet::CachedBalanceSheet;
use protorune_support::{balance_sheet::BalanceSheet, rune_transfer::RuneTransfer};

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AlkaneTransfer {
    pub id: AlkaneId,
    pub value: u128,
}

impl From<Vec<RuneTransfer>> for AlkaneTransferParcel {
    fn from(v: Vec<RuneTransfer>) -> AlkaneTransferParcel {
        AlkaneTransferParcel(
            v.into_iter()
                .map(|incoming| AlkaneTransfer {
                    id: incoming.id.into(),
                    value: incoming.value,
                })
                .collect(),
        )
    }
}

impl Into<RuneTransfer> for AlkaneTransfer {
    fn into(self) -> RuneTransfer {
        RuneTransfer {
            id: self.id.into(),
            value: self.value,
        }
    }
}

impl Into<Vec<RuneTransfer>> for AlkaneTransferParcel {
    fn into(self) -> Vec<RuneTransfer> {
        self.0.into_iter().map(|v| v.into()).collect()
    }
}

impl<P: KeyValuePointer + Clone> TryInto<BalanceSheet<P>> for AlkaneTransferParcel {
    type Error = anyhow::Error;
    fn try_into(self) -> Result<BalanceSheet<P>> {
        <AlkaneTransferParcel as Into<Vec<RuneTransfer>>>::into(self).try_into()
    }
}

impl TryInto<CachedBalanceSheet> for AlkaneTransferParcel {
    type Error = anyhow::Error;
    fn try_into(self) -> Result<CachedBalanceSheet> {
        <AlkaneTransferParcel as Into<Vec<RuneTransfer>>>::into(self).try_into()
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AlkaneTransferParcel(pub Vec<AlkaneTransfer>);

impl AlkaneTransferParcel {
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<AlkaneTransferParcel> {
        let mut result = AlkaneTransferParcel::default();
        let len = consume_sized_int::<u128>(cursor)?;
        for _i in 0..len {
            result.0.push(AlkaneTransfer::parse(cursor)?);
        }
        Ok(result)
    }
    pub fn pay(&mut self, transfer: AlkaneTransfer) {
        self.0.push(transfer);
    }
    pub fn to_vec(&self) -> Vec<u128> {
        let len = self.0.len();
        let mut buffer: Vec<u128> = Vec::<u128>::with_capacity(len * 3 + 1);
        buffer.push(len as u128);
        for v in self.0.clone() {
            let transfer_list: Vec<u128> = v.into();
            buffer.extend(&transfer_list);
        }
        buffer
    }
    pub fn serialize(&self) -> Vec<u8> {
        let v = self
            .to_vec()
            .into_iter()
            .map(|v| (v.to_bytes()))
            .flatten()
            .collect::<Vec<u8>>();
        v
    }
}

impl Into<Vec<u128>> for AlkaneTransfer {
    fn into(self) -> Vec<u128> {
        let mut buffer = Vec::<u128>::with_capacity(3);
        let id_ints: Vec<u128> = self.id.into();
        buffer.extend(&id_ints);
        buffer.push(self.value);
        buffer
    }
}

impl AlkaneTransfer {
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<AlkaneTransfer> {
        let id = AlkaneId::parse(cursor)?;
        let value = consume_sized_int::<u128>(cursor)?;
        Ok(AlkaneTransfer { id, value })
    }
    pub fn to_vec(&self) -> Vec<u128> {
        <AlkaneTransfer as Into<Vec<u128>>>::into(self.clone())
    }
}
