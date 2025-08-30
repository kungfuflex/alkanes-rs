use crate::adapters::RocksDBAdapter;
use anyhow::Result;
use jsonrpsee::core::server::rpc_module::RpcModule;
use jsonrpsee::types::ErrorObjectOwned;
use alkanes::VIEW_FUNCTIONS;
use crate::adapters::RocksDBAdapter;
use crate::shred_host;
use serde_json::Value;

pub async fn handle_request(
    storage: RocksDBAdapter,
    request: &str,
) -> Result<String, ErrorObjectOwned> {
    let mut module = RpcModule::new(storage);
    module
        .register_async_method("metashrew_view", |params, storage| async move {
            let mut params = params.sequence();
            let name: String = params.next().unwrap();
            let inputs: Vec<String> = params.next().unwrap();
            println!("metashrew_view called with name: {} and inputs: {:?}", name, inputs);
            shred_host::set_storage_adapter(storage.clone());
            let func = match VIEW_FUNCTIONS.get(name.as_str()) {
                Some(f) => f,
                None => {
                    return Err(ErrorObjectOwned::owned(
                        -32601,
                        "Function not found",
                        None::<()>,
                    ))
                }
            };
            let input_bytes: Vec<u8> = inputs.into_iter().flat_map(|s| s.into_bytes()).collect();
            let result = match func(&input_bytes) {
                Ok(r) => r,
                Err(e) => return Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
            };
            String::from_utf8(result)
                .map_err(|e| ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>))
        })
        .unwrap();

    let (id, result) = module.raw_json_request(request).await;
    let result = result.result.unwrap_or(Value::Null);
    Ok(serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .unwrap())
}