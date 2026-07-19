use alkanes_support::id::AlkaneId;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Record {
    pub target: AlkaneId,
    pub opcode: u128,
    pub height: u32,
    pub gas_used: u64,
}

static RECORDS: Mutex<Vec<Record>> = Mutex::new(Vec::new());

pub fn record(target: AlkaneId, opcode: u128, height: u32, gas_used: u64) {
    RECORDS.lock().unwrap().push(Record {
        target,
        opcode,
        height,
        gas_used,
    });
}

pub fn clear() {
    RECORDS.lock().unwrap().clear();
}

pub fn snapshot() -> Vec<Record> {
    RECORDS.lock().unwrap().clone()
}
