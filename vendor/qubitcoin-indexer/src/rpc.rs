//! RPC method registration for secondary indexers.
//!
//! Registers 4 RPC methods:
//! - `secondaryview`   — call a view function on an indexer
//! - `secondaryheight` — get an indexer's current tip height
//! - `secondaryhash`   — get the SHA-256 of an indexer's WASM binary
//! - `secondaryroot`   — get the SMT state root (if enabled)

use crate::IndexerManager;
use std::sync::Arc;

/// Register all secondary indexer RPC methods on the provided registry.
///
/// The registry type is kept generic via a closure-based approach so this
/// crate doesn't depend on `qubitcoin-rpc`.
pub fn register_indexer_rpcs<F>(register: &mut F, manager: Arc<IndexerManager>)
where
    F: FnMut(&str, Box<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync>),
{
    // metashrew_height [] — alias for alkanes-jsonrpc compatibility
    {
        let mgr = manager.clone();
        register(
            "metashrew_height",
            Box::new(move |_params: &serde_json::Value| {
                match mgr.get_indexer("alkanes") {
                    Some(inst) => {
                        let h = inst.tip_height.load(std::sync::atomic::Ordering::Relaxed);
                        serde_json::json!(h.to_string())
                    }
                    None => serde_json::json!({
                        "error": "alkanes indexer not found"
                    }),
                }
            }),
        );
    }

    // metashrew_view [view_fn, input_hex] — alias for alkanes-jsonrpc compatibility
    {
        let mgr = manager.clone();
        register(
            "metashrew_view",
            Box::new(move |params: &serde_json::Value| {
                let view_fn = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
                let input_hex = params.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let input_bytes = if input_hex.is_empty() || input_hex == "0x" {
                    Vec::new()
                } else {
                    let hex = input_hex.strip_prefix("0x").unwrap_or(input_hex);
                    match hex_decode(hex) {
                        Ok(b) => b,
                        Err(e) => return serde_json::json!({"error": format!("bad hex: {}", e)}),
                    }
                };
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(
                        mgr.call_view_async("alkanes", view_fn, input_bytes),
                    )
                });
                match result {
                    Ok(data) => {
                        let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
                        serde_json::json!(format!("0x{}", hex))
                    }
                    Err(e) => serde_json::json!({"error": e}),
                }
            }),
        );
    }

    // secondaryheight ["label"]
    {
        let mgr = manager.clone();
        register(
            "secondaryheight",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => {
                        return serde_json::json!({
                            "error": "missing label parameter"
                        })
                    }
                };
                match mgr.get_indexer(label) {
                    Some(inst) => {
                        serde_json::json!(inst.tip_height.load(std::sync::atomic::Ordering::Relaxed))
                    }
                    None => serde_json::json!({
                        "error": format!("indexer '{}' not found", label)
                    }),
                }
            }),
        );
    }

    // secondaryhash ["label"]
    {
        let mgr = manager.clone();
        register(
            "secondaryhash",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => {
                        return serde_json::json!({
                            "error": "missing label parameter"
                        })
                    }
                };
                match mgr.get_indexer(label) {
                    Some(inst) => {
                        let hex: String =
                            inst.wasm_hash.iter().map(|b| format!("{:02x}", b)).collect();
                        serde_json::json!(hex)
                    }
                    None => serde_json::json!({
                        "error": format!("indexer '{}' not found", label)
                    }),
                }
            }),
        );
    }

    // secondaryroot ["label"]
    {
        let mgr = manager.clone();
        register(
            "secondaryroot",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => {
                        return serde_json::json!({
                            "error": "missing label parameter"
                        })
                    }
                };
                match mgr.get_indexer(label) {
                    Some(inst) => {
                        if !inst.smt_enabled {
                            return serde_json::json!({
                                "error": "SMT not enabled for this indexer"
                            });
                        }
                        let height =
                            inst.tip_height.load(std::sync::atomic::Ordering::Relaxed);
                        let root_key = crate::smt::smt_root_key(height);
                        match inst.storage.get(&root_key) {
                            Some(root) => {
                                let hex: String =
                                    root.iter().map(|b| format!("{:02x}", b)).collect();
                                serde_json::json!(format!("0x{}", hex))
                            }
                            None => serde_json::json!(
                                "0x0000000000000000000000000000000000000000000000000000000000000000"
                            ),
                        }
                    }
                    None => serde_json::json!({
                        "error": format!("indexer '{}' not found", label)
                    }),
                }
            }),
        );
    }

    // secondaryview ["label", "view_fn", "input_hex", "latest"|height]
    {
        let mgr = manager.clone();
        register(
            "secondaryview",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => {
                        return serde_json::json!({
                            "error": "missing label parameter"
                        })
                    }
                };
                let view_fn = match params.get(1).and_then(|v| v.as_str()) {
                    Some(f) => f,
                    None => {
                        return serde_json::json!({
                            "error": "missing view_fn parameter"
                        })
                    }
                };
                let input_hex = params
                    .get(2)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Decode hex input.
                let input_bytes = match hex_decode(input_hex) {
                    Ok(b) => b,
                    Err(e) => {
                        return serde_json::json!({
                            "error": format!("invalid hex input: {}", e)
                        })
                    }
                };

                // Use async view with fuel-based cooperative yielding.
                // Since RPC handlers are sync, use tokio::task::block_in_place
                // to call the async view without blocking the runtime.
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(
                        mgr.call_view_async(label, view_fn, input_bytes),
                    )
                });
                match result {
                    Ok(result) => {
                        let hex: String =
                            result.iter().map(|b| format!("{:02x}", b)).collect();
                        serde_json::json!(format!("0x{}", hex))
                    }
                    Err(e) => serde_json::json!({
                        "error": e
                    }),
                }
            }),
        );
    }
}

/// Simple hex decoder.
/// Register admin RPC methods for indexer lifecycle management.
///
/// All methods require authentication (Admin tier).
pub fn register_indexer_admin_rpcs<F>(register: &mut F, manager: Arc<parking_lot::RwLock<IndexerManager>>)
where
    F: FnMut(&str, Box<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync>),
{
    // indexerpause [label]
    {
        let mgr = manager.clone();
        register(
            "indexerpause",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label parameter"}),
                };
                let mgr = mgr.read();
                match mgr.pause(label) {
                    Ok(info) => serde_json::json!({
                        "label": info.label,
                        "db_path": info.db_path.display().to_string(),
                        "tip_height": info.tip_height,
                    }),
                    Err(e) => serde_json::json!({"error": e}),
                }
            }),
        );
    }

    // indexerresume [label]
    {
        let mgr = manager.clone();
        register(
            "indexerresume",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label parameter"}),
                };
                let mgr = mgr.read();
                match mgr.resume(label) {
                    Ok(()) => serde_json::json!({"label": label, "resumed": true}),
                    Err(e) => serde_json::json!({"error": e}),
                }
            }),
        );
    }

    // indexerrollback [label, height]
    {
        let mgr = manager.clone();
        register(
            "indexerrollback",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label parameter"}),
                };
                let height = match params.get(1).and_then(|v| v.as_u64()) {
                    Some(h) => h as u32,
                    None => return serde_json::json!({"error": "missing height parameter"}),
                };
                let mgr = mgr.read();
                match mgr.rollback_indexer(label, height) {
                    Ok(deleted) => serde_json::json!({
                        "label": label,
                        "new_height": height,
                        "deleted_entries": deleted,
                    }),
                    Err(e) => serde_json::json!({"error": e}),
                }
            }),
        );
    }

    // indexerload [label, wasm_path, {options}]
    {
        let mgr = manager.clone();
        register(
            "indexerload",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label parameter"}),
                };
                let wasm_path = match params.get(1).and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => return serde_json::json!({"error": "missing wasm_path parameter"}),
                };
                let opts = params.get(2);
                let cfg = crate::config::IndexerConfig {
                    label: label.to_string(),
                    wasm_path: std::path::PathBuf::from(wasm_path),
                    smt_enabled: opts
                        .and_then(|o| o.get("smt"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    start_height: opts
                        .and_then(|o| o.get("start_height"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    layer: match opts
                        .and_then(|o| o.get("layer"))
                        .and_then(|v| v.as_str())
                    {
                        Some("tertiary") => crate::config::IndexerLayer::Tertiary,
                        _ => crate::config::IndexerLayer::Secondary,
                    },
                    depends_on: opts
                        .and_then(|o| o.get("depends_on"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                };
                let mut mgr = mgr.write();
                match mgr.load(label, std::path::Path::new(wasm_path), cfg) {
                    Ok(hash) => serde_json::json!({"label": label, "wasm_hash": hash, "loaded": true}),
                    Err(e) => serde_json::json!({"error": e}),
                }
            }),
        );
    }

    // indexerunload [label]
    {
        let mgr = manager.clone();
        register(
            "indexerunload",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label parameter"}),
                };
                let mut mgr = mgr.write();
                match mgr.unload(label) {
                    Ok(()) => serde_json::json!({"label": label, "unloaded": true}),
                    Err(e) => serde_json::json!({"error": e}),
                }
            }),
        );
    }

    // indexerstatus []
    {
        let mgr = manager.clone();
        register(
            "indexerstatus",
            Box::new(move |_params: &serde_json::Value| {
                let mgr = mgr.read();
                let statuses: Vec<serde_json::Value> = mgr
                    .status()
                    .into_iter()
                    .map(|s| {
                        serde_json::json!({
                            "label": s.label,
                            "height": s.height,
                            "paused": s.paused,
                            "wasm_hash": s.wasm_hash,
                            "layer": format!("{:?}", s.layer),
                            "db_path": s.db_path.display().to_string(),
                            "smt_enabled": s.smt_enabled,
                            "start_height": s.start_height,
                            "depends_on": s.depends_on,
                        })
                    })
                    .collect();
                serde_json::json!(statuses)
            }),
        );
    }
}

/// Register KV access RPC methods for secondary/tertiary indexers.
///
/// Read methods are Public, write methods require Admin + paused state.
pub fn register_indexer_kv_rpcs<F>(register: &mut F, manager: Arc<IndexerManager>)
where
    F: FnMut(&str, Box<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync>),
{
    // secondarykvget [label, hex_key] — raw KV read
    {
        let mgr = manager.clone();
        register(
            "secondarykvget",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!(null),
                };
                let hex_key = match params.get(1).and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return serde_json::json!(null),
                };
                let key = match hex_decode(hex_key) {
                    Ok(k) => k,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                match mgr.get_indexer(label) {
                    Some(inst) => match inst.storage.get(&key) {
                        Some(val) => serde_json::json!(hex_encode(&val)),
                        None => serde_json::json!(null),
                    },
                    None => serde_json::json!({"error": format!("indexer '{}' not found", label)}),
                }
            }),
        );
    }

    // secondarykvgetlatest [label, hex_key] — reorg-aware latest read
    {
        let mgr = manager.clone();
        register(
            "secondarykvgetlatest",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!(null),
                };
                let hex_key = match params.get(1).and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return serde_json::json!(null),
                };
                let key = match hex_decode(hex_key) {
                    Ok(k) => k,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                match mgr.get_indexer(label) {
                    Some(inst) => match inst.storage.get_latest_canonical(&key) {
                        Some(val) => serde_json::json!(hex_encode(&val)),
                        None => serde_json::json!(null),
                    },
                    None => serde_json::json!({"error": format!("indexer '{}' not found", label)}),
                }
            }),
        );
    }

    // secondarykvlen [label, hex_key] — length of append list
    {
        let mgr = manager.clone();
        register(
            "secondarykvlen",
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!(null),
                };
                let hex_key = match params.get(1).and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return serde_json::json!(null),
                };
                let key = match hex_decode(hex_key) {
                    Ok(k) => k,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                match mgr.get_indexer(label) {
                    Some(inst) => serde_json::json!(inst.storage.get_length(&key)),
                    None => serde_json::json!({"error": format!("indexer '{}' not found", label)}),
                }
            }),
        );
    }

    // Tertiary aliases (same implementation, semantic distinction)
    for method_name in &["tertiarykvget", "tertiarykvgetlatest", "tertiarykvlen"] {
        let mgr = manager.clone();
        let is_latest = method_name.contains("latest");
        let is_len = method_name.contains("len");
        let name = method_name.to_string();
        register(
            &name,
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!(null),
                };
                let hex_key = match params.get(1).and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return serde_json::json!(null),
                };
                let key = match hex_decode(hex_key) {
                    Ok(k) => k,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                match mgr.get_indexer(label) {
                    Some(inst) => {
                        if is_len {
                            serde_json::json!(inst.storage.get_length(&key))
                        } else if is_latest {
                            match inst.storage.get_latest_canonical(&key) {
                                Some(val) => serde_json::json!(hex_encode(&val)),
                                None => serde_json::json!(null),
                            }
                        } else {
                            match inst.storage.get(&key) {
                                Some(val) => serde_json::json!(hex_encode(&val)),
                                None => serde_json::json!(null),
                            }
                        }
                    }
                    None => serde_json::json!({"error": format!("indexer '{}' not found", label)}),
                }
            }),
        );
    }
}

/// Register admin KV write RPC methods (require auth + paused indexer).
pub fn register_indexer_kv_write_rpcs<F>(register: &mut F, manager: Arc<IndexerManager>)
where
    F: FnMut(&str, Box<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync>),
{
    // secondarykvput [label, hex_key, hex_value]
    for prefix in &["secondary", "tertiary"] {
        let mgr = manager.clone();
        let method = format!("{}kvput", prefix);
        register(
            &method,
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label"}),
                };
                let hex_key = match params.get(1).and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return serde_json::json!({"error": "missing hex_key"}),
                };
                let hex_val = match params.get(2).and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => return serde_json::json!({"error": "missing hex_value"}),
                };
                let key = match hex_decode(hex_key) {
                    Ok(k) => k,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                let val = match hex_decode(hex_val) {
                    Ok(v) => v,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                match mgr.get_indexer(label) {
                    Some(inst) => {
                        if !inst.paused.load(std::sync::atomic::Ordering::Relaxed) {
                            return serde_json::json!({"error": "indexer must be paused for KV writes"});
                        }
                        match inst.storage.put(&key, &val) {
                            Ok(()) => serde_json::json!({"ok": true}),
                            Err(e) => serde_json::json!({"error": e}),
                        }
                    }
                    None => serde_json::json!({"error": format!("indexer '{}' not found", label)}),
                }
            }),
        );

        let mgr2 = manager.clone();
        let del_method = format!("{}kvdelete", prefix);
        register(
            &del_method,
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label"}),
                };
                let hex_key = match params.get(1).and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return serde_json::json!({"error": "missing hex_key"}),
                };
                let key = match hex_decode(hex_key) {
                    Ok(k) => k,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                match mgr2.get_indexer(label) {
                    Some(inst) => {
                        if !inst.paused.load(std::sync::atomic::Ordering::Relaxed) {
                            return serde_json::json!({"error": "indexer must be paused for KV deletes"});
                        }
                        match inst.storage.delete_batch(&[key]) {
                            Ok(()) => serde_json::json!({"ok": true}),
                            Err(e) => serde_json::json!({"error": e}),
                        }
                    }
                    None => serde_json::json!({"error": format!("indexer '{}' not found", label)}),
                }
            }),
        );

        let mgr3 = manager.clone();
        let append_method = format!("{}kvappend", prefix);
        register(
            &append_method,
            Box::new(move |params: &serde_json::Value| {
                let label = match params.get(0).and_then(|v| v.as_str()) {
                    Some(l) => l,
                    None => return serde_json::json!({"error": "missing label"}),
                };
                let hex_key = match params.get(1).and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return serde_json::json!({"error": "missing hex_key"}),
                };
                let hex_val = match params.get(2).and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => return serde_json::json!({"error": "missing hex_value"}),
                };
                let height = match params.get(3).and_then(|v| v.as_u64()) {
                    Some(h) => h as u32,
                    None => return serde_json::json!({"error": "missing height"}),
                };
                let key = match hex_decode(hex_key) {
                    Ok(k) => k,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                let val = match hex_decode(hex_val) {
                    Ok(v) => v,
                    Err(e) => return serde_json::json!({"error": e}),
                };
                match mgr3.get_indexer(label) {
                    Some(inst) => {
                        if !inst.paused.load(std::sync::atomic::Ordering::Relaxed) {
                            return serde_json::json!({"error": "indexer must be paused for KV appends"});
                        }
                        match inst.storage.append(&key, &val, height) {
                            Ok(()) => {
                                let new_len = inst.storage.get_length(&key);
                                serde_json::json!({"ok": true, "new_length": new_len})
                            }
                            Err(e) => serde_json::json!({"error": e}),
                        }
                    }
                    None => serde_json::json!({"error": format!("indexer '{}' not found", label)}),
                }
            }),
        );
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    if hex.is_empty() {
        return Ok(Vec::new());
    }
    if hex.len() % 2 != 0 {
        return Err("odd-length hex string".into());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| format!("{}", e)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config, IndexerManager, IndexerMode};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn setup_manager() -> (Arc<IndexerManager>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, crate::build_test_wasm()).unwrap();

        let configs = vec![config::IndexerConfig {
            label: "testrpc".to_string(),
            wasm_path,
            smt_enabled: false,
            start_height: 0,
            layer: config::IndexerLayer::Secondary,
            depends_on: vec![],
        }];

        let datadir = PathBuf::from(dir.path());
        let mgr = Arc::new(
            IndexerManager::new(configs, &datadir, IndexerMode::Synchronous).unwrap(),
        );
        (mgr, dir)
    }

    fn collect_rpcs(
        mgr: Arc<IndexerManager>,
    ) -> HashMap<String, Box<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync>>
    {
        let mut rpcs: HashMap<
            String,
            Box<dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync>,
        > = HashMap::new();
        register_indexer_rpcs(
            &mut |name: &str,
                  handler: Box<
                dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync,
            >| {
                rpcs.insert(name.to_string(), handler);
            },
            mgr,
        );
        rpcs
    }

    #[test]
    fn test_rpcs_registered() {
        let (mgr, _dir) = setup_manager();
        let rpcs = collect_rpcs(mgr);
        assert!(rpcs.contains_key("secondaryheight"));
        assert!(rpcs.contains_key("secondaryhash"));
        assert!(rpcs.contains_key("secondaryroot"));
        assert!(rpcs.contains_key("secondaryview"));
    }

    #[test]
    fn test_secondaryheight() {
        let (mgr, _dir) = setup_manager();
        mgr.on_block_connected(42, b"block");
        let rpcs = collect_rpcs(mgr);

        let handler = rpcs.get("secondaryheight").unwrap();
        let result = handler(&serde_json::json!(["testrpc"]));
        assert_eq!(result, serde_json::json!(42));
    }

    #[test]
    fn test_secondaryheight_not_found() {
        let (mgr, _dir) = setup_manager();
        let rpcs = collect_rpcs(mgr);

        let handler = rpcs.get("secondaryheight").unwrap();
        let result = handler(&serde_json::json!(["nonexistent"]));
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_secondaryheight_missing_param() {
        let (mgr, _dir) = setup_manager();
        let rpcs = collect_rpcs(mgr);

        let handler = rpcs.get("secondaryheight").unwrap();
        let result = handler(&serde_json::json!([]));
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_secondaryhash() {
        let (mgr, _dir) = setup_manager();
        let rpcs = collect_rpcs(mgr);

        let handler = rpcs.get("secondaryhash").unwrap();
        let result = handler(&serde_json::json!(["testrpc"]));
        // Should be a 64-char hex string.
        let hash = result.as_str().unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_secondaryroot_not_enabled() {
        let (mgr, _dir) = setup_manager();
        let rpcs = collect_rpcs(mgr);

        let handler = rpcs.get("secondaryroot").unwrap();
        let result = handler(&serde_json::json!(["testrpc"]));
        assert!(result.get("error").is_some());
        assert!(
            result["error"]
                .as_str()
                .unwrap()
                .contains("not enabled")
        );
    }

    #[test]
    fn test_hex_decode_basic() {
        assert_eq!(hex_decode("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn test_hex_decode_0x_prefix() {
        assert_eq!(hex_decode("0xab").unwrap(), vec![0xab]);
    }

    #[test]
    fn test_hex_decode_empty() {
        assert_eq!(hex_decode("").unwrap(), Vec::<u8>::new());
        assert_eq!(hex_decode("0x").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_hex_decode_odd_length() {
        assert!(hex_decode("abc").is_err());
    }

    #[test]
    fn test_hex_decode_invalid_chars() {
        assert!(hex_decode("zzzz").is_err());
    }
}
