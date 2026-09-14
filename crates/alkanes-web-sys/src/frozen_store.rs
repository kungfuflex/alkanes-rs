//! IndexedDB-backed [`FrozenStore`] for the browser.
//!
//! The browser counterpart to `alkanes-cli-common`'s sqlite driver: a
//! freeze recorded in a web wallet must survive a page reload, or it
//! protects the coin only until the user hits refresh.
//!
//! `localStorage` (what [`crate::storage::WebStorage`] uses) would have
//! been less code, but it is synchronous, ~5 MB, and shared with every
//! other key the app writes. IndexedDB gives the frozen set its own object
//! store with its own lifetime, which matters for the same reason the
//! native driver refuses to live in the cache directory: clearing
//! app state for an unrelated reason must not quietly unfreeze a UTXO.
//!
//! Each record is stored as a JSON string keyed by the canonical
//! `txid:vout`, so the shape on disk matches [`FrozenRecord`] exactly.

use alkanes_cli_common::frozen_store::{normalize_outpoint, FrozenRecord, FrozenStore};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbObjectStore, IdbRequest, IdbTransactionMode};

/// Database and object-store names. Versioned so a future schema change
/// can migrate in `onupgradeneeded` rather than silently reading a store
/// that isn't there.
const DB_NAME: &str = "alkanes-wallet";
const DB_VERSION: u32 = 1;
const STORE_NAME: &str = "frozen_utxos";

/// Frozen set persisted in IndexedDB.
#[derive(Clone)]
pub struct IndexedDbFrozenStore {
    db: IdbDatabase,
}

impl core::fmt::Debug for IndexedDbFrozenStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IndexedDbFrozenStore").finish()
    }
}

fn js_err(context: &str, e: JsValue) -> anyhow::Error {
    anyhow!("{context}: {:?}", e)
}

/// Await an `IDBRequest`, resolving with its `result`.
///
/// IndexedDB is callback-based, so each request is wrapped in a Promise
/// whose handlers are installed before the request can settle.
async fn await_request(req: &IdbRequest, context: &str) -> Result<JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let on_success = Closure::once(Box::new(move |event: web_sys::Event| {
            let result = event
                .target()
                .and_then(|t| t.dyn_into::<IdbRequest>().ok())
                .and_then(|r| r.result().ok())
                .unwrap_or(JsValue::UNDEFINED);
            let _ = resolve.call1(&JsValue::NULL, &result);
        }) as Box<dyn FnOnce(web_sys::Event)>);

        let on_error = Closure::once(Box::new(move |event: web_sys::Event| {
            let err = event
                .target()
                .and_then(|t| t.dyn_into::<IdbRequest>().ok())
                .and_then(|r| r.error().ok().flatten())
                .map(JsValue::from)
                .unwrap_or_else(|| JsValue::from_str("IndexedDB request failed"));
            let _ = reject.call1(&JsValue::NULL, &err);
        }) as Box<dyn FnOnce(web_sys::Event)>);

        req.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        req.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        // The closures must outlive this scope; IndexedDB fires each at
        // most once, so leaking them is bounded by the request count and
        // is the standard wasm-bindgen pattern for one-shot handlers.
        on_success.forget();
        on_error.forget();
    });

    JsFuture::from(promise)
        .await
        .map_err(|e| js_err(context, e))
}

impl IndexedDbFrozenStore {
    /// Open (and if necessary create) the frozen-UTXO database.
    pub async fn open() -> Result<Self> {
        let window = web_sys::window().ok_or_else(|| anyhow!("no window object"))?;
        let factory = window
            .indexed_db()
            .map_err(|e| js_err("indexedDB unavailable", e))?
            .ok_or_else(|| anyhow!("indexedDB is not available in this context"))?;

        let open_req = factory
            .open_with_u32(DB_NAME, DB_VERSION)
            .map_err(|e| js_err("opening indexedDB", e))?;

        // Create the object store on first open / version bump.
        let upgrade = Closure::once(Box::new(move |event: web_sys::Event| {
            if let Some(req) = event
                .target()
                .and_then(|t| t.dyn_into::<IdbRequest>().ok())
            {
                if let Ok(result) = req.result() {
                    if let Ok(db) = result.dyn_into::<IdbDatabase>() {
                        if !db.object_store_names().contains(STORE_NAME) {
                            let _ = db.create_object_store(STORE_NAME);
                        }
                    }
                }
            }
        }) as Box<dyn FnOnce(web_sys::Event)>);
        open_req.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        upgrade.forget();

        let db_value = await_request(open_req.as_ref(), "opening frozen-UTXO database").await?;
        let db: IdbDatabase = db_value
            .dyn_into()
            .map_err(|_| anyhow!("indexedDB open did not yield a database"))?;

        Ok(Self { db })
    }

    fn store(&self, mode: IdbTransactionMode) -> Result<IdbObjectStore> {
        let tx = self
            .db
            .transaction_with_str_and_mode(STORE_NAME, mode)
            .map_err(|e| js_err("starting indexedDB transaction", e))?;
        tx.object_store(STORE_NAME)
            .map_err(|e| js_err("opening object store", e))
    }
}

#[async_trait(?Send)]
impl FrozenStore for IndexedDbFrozenStore {
    async fn freeze(&self, outpoint: &str, reason: Option<&str>) -> Result<()> {
        let key = normalize_outpoint(outpoint)?;
        let record = FrozenRecord {
            outpoint: key.clone(),
            reason: reason.map(|r| r.to_string()),
            // `Date.now()` in the browser; the shared `now_secs()` helper
            // returns 0 under wasm since there is no `SystemTime`.
            frozen_at: (js_sys::Date::now() / 1000.0) as u64,
        };
        let json = serde_json::to_string(&record)?;

        let store = self.store(IdbTransactionMode::Readwrite)?;
        let req = store
            .put_with_key(&JsValue::from_str(&json), &JsValue::from_str(&key))
            .map_err(|e| js_err("writing frozen record", e))?;
        await_request(&req, "writing frozen record").await?;
        Ok(())
    }

    async fn unfreeze(&self, outpoint: &str) -> Result<bool> {
        let key = normalize_outpoint(outpoint)?;
        // Read first so the caller can be told whether anything changed —
        // IndexedDB's delete succeeds regardless of whether the key existed.
        let existed = self.get(&key).await?.is_some();

        let store = self.store(IdbTransactionMode::Readwrite)?;
        let req = store
            .delete(&JsValue::from_str(&key))
            .map_err(|e| js_err("deleting frozen record", e))?;
        await_request(&req, "deleting frozen record").await?;
        Ok(existed)
    }

    async fn get(&self, outpoint: &str) -> Result<Option<FrozenRecord>> {
        let key = normalize_outpoint(outpoint)?;
        let store = self.store(IdbTransactionMode::Readonly)?;
        let req = store
            .get(&JsValue::from_str(&key))
            .map_err(|e| js_err("reading frozen record", e))?;
        let value = await_request(&req, "reading frozen record").await?;

        match value.as_string() {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    async fn list(&self) -> Result<Vec<FrozenRecord>> {
        let store = self.store(IdbTransactionMode::Readonly)?;
        let req = store
            .get_all()
            .map_err(|e| js_err("listing frozen records", e))?;
        let value = await_request(&req, "listing frozen records").await?;

        let array = js_sys::Array::from(&value);
        let mut records = Vec::with_capacity(array.length() as usize);
        for entry in array.iter() {
            if let Some(json) = entry.as_string() {
                records.push(serde_json::from_str(&json)?);
            }
        }
        Ok(records)
    }

    async fn clear(&self) -> Result<()> {
        let store = self.store(IdbTransactionMode::Readwrite)?;
        let req = store
            .clear()
            .map_err(|e| js_err("clearing frozen records", e))?;
        await_request(&req, "clearing frozen records").await?;
        Ok(())
    }
}
