use anyhow::Result;
use metashrew_support::utils::consume_sized_int;
use protorune_support::balance_sheet::ProtoruneRuneId;
use std::hash::{Hash, Hasher};

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AlkaneId {
    pub block: u128,
    pub tx: u128,
}

impl Into<Vec<u128>> for AlkaneId {
    fn into(self) -> Vec<u128> {
        (&[self.block, self.tx]).to_vec()
    }
}

impl TryFrom<Vec<u8>> for AlkaneId {
    type Error = anyhow::Error;
    fn try_from(v: Vec<u8>) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(v);
        let block = consume_sized_int::<u128>(&mut cursor)?;
        let tx = consume_sized_int::<u128>(&mut cursor)?;
        Ok(Self::new(block, tx))
    }
}

impl From<ProtoruneRuneId> for AlkaneId {
    fn from(id: ProtoruneRuneId) -> AlkaneId {
        AlkaneId {
            block: id.block,
            tx: id.tx,
        }
    }
}

impl Into<ProtoruneRuneId> for AlkaneId {
    fn into(self) -> ProtoruneRuneId {
        ProtoruneRuneId {
            block: self.block,
            tx: self.tx,
        }
    }
}

impl AlkaneId {
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<AlkaneId> {
        let (block, tx) = (
            consume_sized_int::<u128>(cursor)?,
            consume_sized_int::<u128>(cursor)?,
        );
        Ok(AlkaneId { block, tx })
    }
    pub fn new(block: u128, tx: u128) -> AlkaneId {
        AlkaneId { block, tx }
    }
    pub fn is_created(&self, next_sequence: u128) -> bool {
        self.block == 2 && self.tx < next_sequence || self.block == 4
    }
    pub fn is_create(&self) -> bool {
        self.block == 1 && self.tx == 0
    }
    pub fn is_deployment(&self) -> bool {
        if self.block == 1 || self.block == 3 || self.block == 5 || self.block == 6 {
            true
        } else {
            false
        }
    }
    pub fn reserved(&self) -> Option<u128> {
        if self.block == 3 {
            Some(self.tx)
        } else {
            None
        }
    }
    pub fn factory(&self) -> Option<AlkaneId> {
        if self.block == 5 {
            Some(AlkaneId {
                block: 2,
                tx: self.tx,
            })
        } else if self.block == 6 {
            Some(AlkaneId {
                block: 4,
                tx: self.tx,
            })
        } else {
            None
        }
    }
}

impl From<AlkaneId> for Vec<u8> {
    fn from(rune_id: AlkaneId) -> Self {
        let mut bytes = Vec::<u8>::with_capacity(32);
        bytes.extend(&rune_id.block.to_le_bytes());
        bytes.extend(&rune_id.tx.to_le_bytes());
        bytes
    }
}

impl Into<AlkaneId> for [u128; 2] {
    fn into(self) -> AlkaneId {
        AlkaneId {
            block: self[0],
            tx: self[1],
        }
    }
}

impl From<&AlkaneId> for Vec<u8> {
    fn from(rune_id: &AlkaneId) -> Self {
        let mut bytes = Vec::<u8>::with_capacity(32);
        bytes.extend(&rune_id.block.to_le_bytes());
        bytes.extend(&rune_id.tx.to_le_bytes());
        bytes
    }
}

// Implement Hash trait for AlkaneId
impl Hash for AlkaneId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.block.hash(state);
        self.tx.hash(state);
    }
}
