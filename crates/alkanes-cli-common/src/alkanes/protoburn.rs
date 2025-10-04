use crate::{DeezelError, Result};
use alloc::{
    collections::BTreeMap,
    vec::Vec,
};
use core::ops::Deref;
use crate::alkanes::balance_sheet::{ProtoruneRuneId};

#[derive(Clone, Debug)]
pub struct Protoburn {
    pub tag: Option<u128>,
    pub pointer: Option<u32>,
    pub from: Option<Vec<u32>>,
}

pub trait Protoburns<T>: Deref<Target = [T]> {
    fn construct_burncycle(&self) -> Result<BurnCycle> {
        let length = u32::try_from(self.len()).map_err(|_| DeezelError::Other("Failed to convert length".into()))?;
        Ok(BurnCycle::new(length))
    }
}

impl Protoburns<Protoburn> for Vec<Protoburn> {}

pub struct BurnCycle {
    max: u32,
    cycles: BTreeMap<ProtoruneRuneId, i32>,
}

impl BurnCycle {
    pub fn new(max: u32) -> Self {
        BurnCycle {
            max,
            cycles: BTreeMap::<ProtoruneRuneId, i32>::new(),
        }
    }
    pub fn next(&mut self, rune: &ProtoruneRuneId) -> Result<i32> {
        if !self.cycles.contains_key(rune) {
            self.cycles.insert(rune.clone(), 0);
        }
        let cycles = self.cycles.clone();
        let cycle = cycles.get(rune).ok_or(DeezelError::Other("no value found".into()))?;
        self.cycles
            .insert(rune.clone(), (cycle.clone() + 1) % (self.max as i32));
        Ok(cycle.clone())
    }
    pub fn peek(&mut self, rune: &ProtoruneRuneId) -> Result<i32> {
        if !self.cycles.contains_key(rune) {
            self.cycles.insert(rune.clone(), 0);
        }
        Ok(self
            .cycles
            .get(rune)
            .ok_or(DeezelError::Other("value not found".into()))?
            .clone())
    }
}