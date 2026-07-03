use anyhow::{anyhow, Result};
use metashrew_support::utils::{consume_exact, consume_sized_int};
use std::collections::BTreeMap;
use std::io::Cursor;

/// Bytes left to read from `cursor`.
fn remaining(cursor: &Cursor<Vec<u8>>) -> u64 {
    (cursor.get_ref().len() as u64).saturating_sub(cursor.position())
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct StorageMap(pub BTreeMap<Vec<u8>, Vec<u8>>);

impl FromIterator<(Vec<u8>, Vec<u8>)> for StorageMap {
    fn from_iter<I: IntoIterator<Item = (Vec<u8>, Vec<u8>)>>(iter: I) -> Self {
        Self(BTreeMap::<Vec<u8>, Vec<u8>>::from_iter(iter))
    }
}

impl StorageMap {
    pub fn parse(cursor: &mut Cursor<Vec<u8>>) -> Result<StorageMap> {
        let mut pairs = Vec::<(Vec<u8>, Vec<u8>)>::new();
        let len = consume_sized_int::<u32>(cursor)? as u64;

        // INVARIANT: never allocate more than the input actually contains. Each
        // entry needs at least 8 bytes (two u32 length prefixes), so a claimed
        // entry count larger than remaining/8 is malformed — reject before the
        // loop to bound both the iteration count and the total allocation.
        if len > remaining(cursor) / 8 {
            return Err(anyhow!(
                "StorageMap: entry count {} exceeds remaining input",
                len
            ));
        }

        for _i in 0..len {
            let key_length: u64 = consume_sized_int::<u32>(cursor)?.into();
            if key_length > remaining(cursor) {
                return Err(anyhow!(
                    "StorageMap: key length {} exceeds remaining input",
                    key_length
                ));
            }
            let key: Vec<u8> = consume_exact(cursor, key_length as usize)?;
            let value_length: u64 = consume_sized_int::<u32>(cursor)?.into();
            if value_length > remaining(cursor) {
                return Err(anyhow!(
                    "StorageMap: value length {} exceeds remaining input",
                    value_length
                ));
            }
            let value: Vec<u8> = consume_exact(cursor, value_length as usize)?;
            pairs.push((key, value));
        }

        Ok(StorageMap::from_iter(pairs.into_iter()))
    }
    pub fn get<T: AsRef<[u8]>>(&self, k: T) -> Option<&Vec<u8>> {
        self.0.get(k.as_ref())
    }
    pub fn get_mut<T: AsRef<[u8]>>(&mut self, k: T) -> Option<&mut Vec<u8>> {
        self.0.get_mut(k.as_ref())
    }
    pub fn set<KT: AsRef<[u8]>, VT: AsRef<[u8]>>(&mut self, k: KT, v: VT) {
        self.0.insert(k.as_ref().to_vec(), v.as_ref().to_vec());
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::<u8>::new();
        let size = self.0.len() as u32;
        buffer.extend(&(size).to_le_bytes());
        if size > 0 {
            for (k, v) in self.0.iter() {
                buffer.extend(&(k.len() as u32).to_le_bytes());
                buffer.extend(k);
                buffer.extend(&(v.len() as u32).to_le_bytes());
                buffer.extend(v);
            }
        }
        buffer
    }
}
