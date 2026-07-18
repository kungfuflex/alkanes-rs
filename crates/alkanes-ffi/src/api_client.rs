use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

pub struct AlkanesApiClient {
    inner: crate::api_client_impl::AlkanesApiClientImpl,
}

#[repr(C)]
pub struct ApiResponse {
    pub ok: bool,
    pub json_data: *mut c_char,
    pub error: *mut c_char,
}

impl ApiResponse {
    fn success(json: String) -> Self {
        Self {
            ok: true,
            json_data: CString::new(json).unwrap().into_raw(),
            error: ptr::null_mut(),
        }
    }

    fn error(msg: String) -> Self {
        Self {
            ok: false,
            json_data: ptr::null_mut(),
            error: CString::new(msg).unwrap().into_raw(),
        }
    }
}

#[no_mangle]
pub extern "C" fn alkanes_api_client_new(base_url: *const c_char) -> *mut AlkanesApiClient {
    let base_url = unsafe {
        assert!(!base_url.is_null());
        CStr::from_ptr(base_url).to_string_lossy().into_owned()
    };

    let client = AlkanesApiClient {
        inner: crate::api_client_impl::AlkanesApiClientImpl::new(base_url),
    };

    Box::into_raw(Box::new(client))
}

#[no_mangle]
pub extern "C" fn alkanes_api_client_free(client: *mut AlkanesApiClient) {
    if !client.is_null() {
        unsafe {
            drop(Box::from_raw(client));
        }
    }
}

#[no_mangle]
pub extern "C" fn alkanes_api_response_free(response: *mut ApiResponse) {
    if !response.is_null() {
        unsafe {
            let resp = Box::from_raw(response);
            if !resp.json_data.is_null() {
                drop(CString::from_raw(resp.json_data));
            }
            if !resp.error.is_null() {
                drop(CString::from_raw(resp.error));
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn alkanes_get_address_balances(
    client: *mut AlkanesApiClient,
    address: *const c_char,
    include_outpoints: bool,
) -> *mut ApiResponse {
    let client = unsafe {
        assert!(!client.is_null());
        &*client
    };

    let address = unsafe {
        assert!(!address.is_null());
        CStr::from_ptr(address).to_string_lossy().into_owned()
    };

    match client.inner.get_address_balances(&address, include_outpoints) {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            Box::into_raw(Box::new(ApiResponse::success(json)))
        }
        Err(e) => Box::into_raw(Box::new(ApiResponse::error(e.to_string()))),
    }
}

#[no_mangle]
pub extern "C" fn alkanes_get_outpoint_balances(
    client: *mut AlkanesApiClient,
    outpoint: *const c_char,
) -> *mut ApiResponse {
    let client = unsafe {
        assert!(!client.is_null());
        &*client
    };

    let outpoint = unsafe {
        assert!(!outpoint.is_null());
        CStr::from_ptr(outpoint).to_string_lossy().into_owned()
    };

    match client.inner.get_outpoint_balances(&outpoint) {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            Box::into_raw(Box::new(ApiResponse::success(json)))
        }
        Err(e) => Box::into_raw(Box::new(ApiResponse::error(e.to_string()))),
    }
}

#[no_mangle]
pub extern "C" fn alkanes_get_holders(
    client: *mut AlkanesApiClient,
    alkane: *const c_char,
    page: i64,
    limit: i64,
) -> *mut ApiResponse {
    let client = unsafe {
        assert!(!client.is_null());
        &*client
    };

    let alkane = unsafe {
        assert!(!alkane.is_null());
        CStr::from_ptr(alkane).to_string_lossy().into_owned()
    };

    match client.inner.get_holders(&alkane, page, limit) {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            Box::into_raw(Box::new(ApiResponse::success(json)))
        }
        Err(e) => Box::into_raw(Box::new(ApiResponse::error(e.to_string()))),
    }
}

#[no_mangle]
pub extern "C" fn alkanes_get_keys(
    client: *mut AlkanesApiClient,
    alkane: *const c_char,
    prefix: *const c_char,
    limit: i64,
) -> *mut ApiResponse {
    let client = unsafe {
        assert!(!client.is_null());
        &*client
    };

    let alkane = unsafe {
        assert!(!alkane.is_null());
        CStr::from_ptr(alkane).to_string_lossy().into_owned()
    };

    let prefix = if prefix.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(prefix).to_string_lossy().into_owned() })
    };

    match client.inner.get_keys(&alkane, prefix, limit) {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            Box::into_raw(Box::new(ApiResponse::success(json)))
        }
        Err(e) => Box::into_raw(Box::new(ApiResponse::error(e.to_string()))),
    }
}

#[no_mangle]
pub extern "C" fn alkanes_get_trades(
    client: *mut AlkanesApiClient,
    pool: *const c_char,
    start_time: i64,
    end_time: i64,
    limit: i64,
) -> *mut ApiResponse {
    let client = unsafe {
        assert!(!client.is_null());
        &*client
    };

    let pool = unsafe {
        assert!(!pool.is_null());
        CStr::from_ptr(pool).to_string_lossy().into_owned()
    };

    let start = if start_time > 0 { Some(start_time) } else { None };
    let end = if end_time > 0 { Some(end_time) } else { None };

    match client.inner.get_trades(&pool, start, end, limit) {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            Box::into_raw(Box::new(ApiResponse::success(json)))
        }
        Err(e) => Box::into_raw(Box::new(ApiResponse::error(e.to_string()))),
    }
}

#[no_mangle]
pub extern "C" fn alkanes_get_candles(
    client: *mut AlkanesApiClient,
    pool: *const c_char,
    interval: *const c_char,
    start_time: i64,
    end_time: i64,
    limit: i64,
) -> *mut ApiResponse {
    let client = unsafe {
        assert!(!client.is_null());
        &*client
    };

    let pool = unsafe {
        assert!(!pool.is_null());
        CStr::from_ptr(pool).to_string_lossy().into_owned()
    };

    let interval = unsafe {
        assert!(!interval.is_null());
        CStr::from_ptr(interval).to_string_lossy().into_owned()
    };

    let start = if start_time > 0 { Some(start_time) } else { None };
    let end = if end_time > 0 { Some(end_time) } else { None };

    match client.inner.get_candles(&pool, &interval, start, end, limit) {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            Box::into_raw(Box::new(ApiResponse::success(json)))
        }
        Err(e) => Box::into_raw(Box::new(ApiResponse::error(e.to_string()))),
    }
}

#[no_mangle]
pub extern "C" fn alkanes_get_reserves(
    client: *mut AlkanesApiClient,
    pool: *const c_char,
) -> *mut ApiResponse {
    let client = unsafe {
        assert!(!client.is_null());
        &*client
    };

    let pool = unsafe {
        assert!(!pool.is_null());
        CStr::from_ptr(pool).to_string_lossy().into_owned()
    };

    match client.inner.get_reserves(&pool) {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            Box::into_raw(Box::new(ApiResponse::success(json)))
        }
        Err(e) => Box::into_raw(Box::new(ApiResponse::error(e.to_string()))),
    }
}

// Implementation module
mod api_client_impl {
    use super::*;

    pub struct AlkanesApiClientImpl {
        base_url: String,
        client: reqwest::blocking::Client,
    }

    impl AlkanesApiClientImpl {
        pub fn new(base_url: String) -> Self {
            Self {
                base_url,
                client: reqwest::blocking::Client::new(),
            }
        }

        fn post<T: serde::Serialize, R: for<'de> serde::Deserialize<'de>>(
            &self,
            endpoint: &str,
            body: &T,
        ) -> anyhow::Result<R> {
            let url = format!("{}/api/v1/{}", self.base_url, endpoint);
            let response = self.client.post(&url).json(body).send()?;
            
            if !response.status().is_success() {
                anyhow::bail!("API request failed: {}", response.status());
            }
            
            Ok(response.json()?)
        }

        pub fn get_address_balances(
            &self,
            address: &str,
            include_outpoints: bool,
        ) -> anyhow::Result<serde_json::Value> {
            self.post(
                "get-address-balances",
                &serde_json::json!({
                    "address": address,
                    "include_outpoints": include_outpoints,
                }),
            )
        }

        pub fn get_outpoint_balances(&self, outpoint: &str) -> anyhow::Result<serde_json::Value> {
            self.post(
                "get-outpoint-balances",
                &serde_json::json!({
                    "outpoint": outpoint,
                }),
            )
        }

        pub fn get_holders(
            &self,
            alkane: &str,
            page: i64,
            limit: i64,
        ) -> anyhow::Result<serde_json::Value> {
            self.post(
                "get-holders",
                &serde_json::json!({
                    "alkane": alkane,
                    "page": page,
                    "limit": limit,
                }),
            )
        }

        pub fn get_keys(
            &self,
            alkane: &str,
            prefix: Option<String>,
            limit: i64,
        ) -> anyhow::Result<serde_json::Value> {
            let mut body = serde_json::json!({
                "alkane": alkane,
                "limit": limit,
            });

            if let Some(p) = prefix {
                body["prefix"] = serde_json::Value::String(p);
            }

            self.post("get-keys", &body)
        }

        pub fn get_trades(
            &self,
            pool: &str,
            start_time: Option<i64>,
            end_time: Option<i64>,
            limit: i64,
        ) -> anyhow::Result<serde_json::Value> {
            let mut body = serde_json::json!({
                "pool": pool,
                "limit": limit,
            });

            if let Some(st) = start_time {
                body["start_time"] = serde_json::Value::from(st);
            }
            if let Some(et) = end_time {
                body["end_time"] = serde_json::Value::from(et);
            }

            self.post("get-trades", &body)
        }

        pub fn get_candles(
            &self,
            pool: &str,
            interval: &str,
            start_time: Option<i64>,
            end_time: Option<i64>,
            limit: i64,
        ) -> anyhow::Result<serde_json::Value> {
            let mut body = serde_json::json!({
                "pool": pool,
                "interval": interval,
                "limit": limit,
            });

            if let Some(st) = start_time {
                body["start_time"] = serde_json::Value::from(st);
            }
            if let Some(et) = end_time {
                body["end_time"] = serde_json::Value::from(et);
            }

            self.post("get-candles", &body)
        }

        pub fn get_reserves(&self, pool: &str) -> anyhow::Result<serde_json::Value> {
            self.post(
                "get-reserves",
                &serde_json::json!({
                    "pool": pool,
                }),
            )
        }
    }
}
