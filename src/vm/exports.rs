use super::{AlkanesInstance, AlkanesState};
use alkanes_support::{
    parcel::AlkaneTransferParcel, response::ExtendedCallResponse, storage::StorageMap,
};
use anyhow::{anyhow, Result};
use metashrew_support::utils::{consume_exact, consume_sized_int, consume_to_end};
use wasmi::*;

pub struct AlkanesExportsImpl(());
impl AlkanesExportsImpl {
    pub fn _get_export(vm: &mut AlkanesInstance, name: &str) -> Result<Func> {
        let instance: &mut Instance = &mut vm.instance;
        let store: &mut Store<AlkanesState> = &mut vm.store;
        Ok(instance.get_func(store, name).ok_or("").map_err(|_| {
            anyhow!(format!(
                "{} not found -- is this WASM built with the ALKANES SDK?",
                name
            ))
        })?)
    }
    pub fn _get_result(vm: &mut AlkanesInstance, result: &[Val; 1]) -> Result<Vec<u8>> {
        vm.read_arraybuffer(
            result[0]
                .i32()
                .ok_or("")
                .map_err(|_| anyhow!("result is not an i32"))?,
        )
    }

    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<ExtendedCallResponse> {
        let alkanes = AlkaneTransferParcel::parse(cursor)?;
        let storage = {
            let mut pairs = Vec::<(Vec<u8>, Vec<u8>)>::new();
            let len = consume_sized_int::<u32>(cursor)? as u64;
            if len > 0 {
                for _i in 0..len {
                    let key_length: usize = consume_sized_int::<u32>(cursor)?.try_into()?;
                    let key: Vec<u8> = consume_exact(cursor, key_length)?;
                    let value_length: usize = consume_sized_int::<u32>(cursor)?.try_into()?;
                    let value: Vec<u8> = consume_exact(cursor, value_length)?;
                    pairs.push((key, value));
                }
            }
            StorageMap::from_iter(pairs.into_iter())
        };
        let data = consume_to_end(cursor)?;
        Ok(ExtendedCallResponse {
            alkanes,
            storage,
            data,
        })
    }
    pub fn execute(vm: &mut AlkanesInstance) -> Result<ExtendedCallResponse> {
        let mut result = [Val::I32(0)];
        let func = Self::_get_export(vm, "__execute")?;
        func.call(&mut vm.store, &[], &mut result)?;
        println!("fuel left: {}", vm.store.get_fuel()?);
        let response = ExtendedCallResponse::parse(&mut std::io::Cursor::new(Self::_get_result(
            vm, &result,
        )?))?;
        Ok(response)
    }

    pub fn call_meta(vm: &mut AlkanesInstance) -> Result<Vec<u8>> {
        let mut result = [Val::I32(0)];
        let func = Self::_get_export(vm, "__meta")?;
        func.call(&mut vm.store, &[], &mut result)?;
        Self::_get_result(vm, &result)
    }
}
