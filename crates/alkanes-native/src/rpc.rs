//
// Chadson's Journal
//
// GENESIS: The initial implementation of the native RPC handler.
//
// REFACTOR 1: The `handle_request` function signature was changed to accept a
// `Arc<dyn metashrew_core::native_host::StorageAdapter>` to solve a trait bound
// error. This was incorrect.
//
// REFACTOR 2: Reverting the signature of `handle_request` back to using the
// concrete type `Arc<RocksDBAdapter>`. The conversion to a trait object will be
// handled inside the function before passing it to the `RpcModule`. This is
// necessary because the calling context in `rockshrew-mono` provides the
// concrete type.
//
//
// Chadson's Journal
//
// ... (previous journal entries condensed)
//
// REFACTOR 3: The core issue is creating the `RpcModule` with the correct
// type. It requires a trait object `Arc<dyn StorageAdapter>`. The fix is to
// create this trait object from the concrete `Arc<RocksDBAdapter>` *before*
// initializing the module. This allows `jsonrpsee` to correctly manage the
// context, and the closure will receive the correct type, which can then be
// passed to `set_storage_adapter`.
//
use anyhow::Result;
use jsonrpsee::types::ErrorObjectOwned;
use metashrew_core::native_host::StorageAdapter;
use std::sync::Arc;

pub async fn handle_request<T: StorageAdapter>(
    storage: T,
    request: &str,
) -> Result<String, ErrorObjectOwned> {
    // TODO: This is dead code and needs to be removed. The RPC logic is handled
    // by rockshrew-mono. This is just here to make the compiler happy.
    let _ = storage;
    let _ = request;
    Ok("".to_string())
}