use anyhow::Result;
use metashrew_support::utils::{consume_exact, consume_sized_int};
use std::collections::HashMap;
use std::io::Cursor;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct StorageMap(pub HashMap<Vec<u8>, Vec<u8>>);

impl FromIterator<(Vec<u8>, Vec<u8>)> for StorageMap {
    fn from_iter<I: IntoIterator<Item = (Vec<u8>, Vec<u8>)>>(iter: I) -> Self {
        Self(HashMap::<Vec<u8>, Vec<u8>>::from_iter(iter))
    }
}

impl StorageMap {
    pub fn parse(cursor: &mut Cursor<Vec<u8>>) -> Result<StorageMap> {
        let mut pairs = Vec::<(Vec<u8>, Vec<u8>)>::new();
        let len = consume_sized_int::<u32>(cursor)? as u64;

        for _i in 0..len {
            let key_length: usize = consume_sized_int::<u32>(cursor)?.try_into()?;
            let key: Vec<u8> = consume_exact(cursor, key_length)?;
            let value_length: usize = consume_sized_int::<u32>(cursor)?.try_into()?;
            let value: Vec<u8> = consume_exact(cursor, value_length)?;
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
