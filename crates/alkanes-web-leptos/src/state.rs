// deezel-leptos/src/state.rs
// Chadson v69.69: Refactored global application state.
// This file establishes a shared context for managing application-wide state,
// such as wallet connection status and user information. It has been moved
// from slope-frontend to be a reusable component in deezel-leptos.

use leptos::*;
use alkanes_cli_common::traits::DeezelProvider;
use alkanes_cli_common::traits::{FeeRates};
use alkanes_cli_common::provider::{AllBalances, EnrichedUtxo};
use deezel_web::keystore::Keystore;
use gloo_storage::{LocalStorage, Storage};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use alkanes_cli_common::traits::TransactionInfo;

// The main application state, provided as a context to all children.
#[derive(Clone, Debug, PartialEq)]
pub enum RpcConnectionStatus {
    Connected,
    Connecting,
    Error(String),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConnectedAddressInfo {
    pub address: String,
    pub derivation_path: String,
    pub address_type: String,
}

#[derive(Clone)]
pub struct AppState {
    pub connected_address: RwSignal<Option<ConnectedAddressInfo>>,
    pub provider: RwSignal<Option<Rc<RefCell<dyn DeezelProvider>>>>,
    pub keystore: RwSignal<Option<Keystore>>,
    pub is_wallet_locked: ReadSignal<bool>,
    pub set_is_wallet_locked: WriteSignal<bool>,
    pub is_wallet_connected: RwSignal<bool>,
    pub show_unlock_modal: ReadSignal<bool>,
    pub set_show_unlock_modal: WriteSignal<bool>,
    pub show_receive_modal: ReadSignal<bool>,
    pub set_show_receive_modal: WriteSignal<bool>,
    pub address_type: RwSignal<String>,
    pub hd_path: RwSignal<String>,
    pub show_wrap_modal: ReadSignal<bool>,
    pub set_show_wrap_modal: WriteSignal<bool>,
    pub network: RwSignal<String>,
    pub custom_rpc_url: ReadSignal<String>,
    pub set_custom_rpc_url: WriteSignal<String>,
    pub custom_bech32_prefix: ReadSignal<String>,
    pub set_custom_bech32_prefix: WriteSignal<String>,
    pub custom_p2pkh_prefix: ReadSignal<String>,
    pub set_custom_p2pkh_prefix: WriteSignal<String>,
    pub custom_p2sh_prefix: ReadSignal<String>,
    pub set_custom_p2sh_prefix: WriteSignal<String>,
    pub block_height: ReadSignal<u64>,
    pub set_block_height: WriteSignal<u64>,
    pub refresh_trigger: RwSignal<u64>,
    pub is_refreshing: ReadSignal<bool>,
    pub set_is_refreshing: WriteSignal<bool>,
    pub fee_rates: ReadSignal<Option<FeeRates>>,
    pub set_fee_rates: WriteSignal<Option<FeeRates>>,
    pub show_qr_scanner_modal: ReadSignal<bool>,
    pub set_show_qr_scanner_modal: WriteSignal<bool>,
    pub scanned_qr_code: RwSignal<Option<String>>,
    pub show_address_selector_modal: ReadSignal<bool>,
    pub set_show_address_selector_modal: WriteSignal<bool>,
    pub show_preview_modal: ReadSignal<bool>,
    pub set_show_preview_modal: WriteSignal<bool>,
    pub transaction_analysis_data: RwSignal<Option<serde_json::Value>>,
    pub rpc_connection_status: RwSignal<RpcConnectionStatus>,
    pub show_transaction_confirmation_modal: ReadSignal<bool>,
    pub set_show_transaction_confirmation_modal: WriteSignal<bool>,
    pub psbt_to_confirm: RwSignal<Option<String>>,
    pub hd_paths: RwSignal<HashMap<String, String>>,
    pub address_data_cache: RwSignal<HashMap<String, (u64, Vec<TransactionInfo>)>>,
    pub dashboard_data_cache: RwSignal<HashMap<String, (AllBalances, Vec<EnrichedUtxo>)>>,
}

impl AppState {
    pub fn is_browser_wallet(&self) -> bool {
        self.provider.with_untracked(|p| {
            p.as_ref().map_or(false, |provider| {
                provider.borrow().provider_name().contains("browser")
            })
        })
    }
}

// Function to provide the AppState context to the application.
// This should be called once at the root of the component tree.
pub fn provide_app_state() {
    let connected_address = create_rw_signal(None);
    let provider = create_rw_signal(None);
    let keystore = create_rw_signal(None);
    let (is_wallet_locked, set_is_wallet_locked) = create_signal(false);
    let is_wallet_connected = create_rw_signal(false);
    let (show_unlock_modal, set_show_unlock_modal) = create_signal(false);
    let (show_receive_modal, set_show_receive_modal) = create_signal(false);
    let address_type = create_rw_signal("p2tr".to_string());
    let hd_path = create_rw_signal("m/86'/1'/0'/0/0".to_string());
    let (show_wrap_modal, set_show_wrap_modal) = create_signal(false);
    let initial_network = if let Some(window) = web_sys::window() {
        if window.location().protocol().unwrap_or_default() == "http:" {
            "regtest".to_string()
        } else {
            "mainnet".to_string()
        }
    } else {
        "mainnet".to_string()
    };
    let network = create_rw_signal(initial_network);
    let (custom_rpc_url, set_custom_rpc_url) = create_signal("".to_string());
    let (custom_bech32_prefix, set_custom_bech32_prefix) = create_signal("".to_string());
    let (custom_p2pkh_prefix, set_custom_p2pkh_prefix) = create_signal("".to_string());
    let (custom_p2sh_prefix, set_custom_p2sh_prefix) = create_signal("".to_string());
    let (block_height, set_block_height) = create_signal(0);
    let refresh_trigger = create_rw_signal(0);
    let (is_refreshing, set_is_refreshing) = create_signal(false);
    let (fee_rates, set_fee_rates) = create_signal(None);
    let (show_qr_scanner_modal, set_show_qr_scanner_modal) = create_signal(false);
    let scanned_qr_code = create_rw_signal(None);
    let (show_address_selector_modal, set_show_address_selector_modal) = create_signal(false);
    let (show_preview_modal, set_show_preview_modal) = create_signal(false);
    let transaction_analysis_data = create_rw_signal(None);
    let rpc_connection_status = create_rw_signal(RpcConnectionStatus::Connecting);
    
    let (show_transaction_confirmation_modal, set_show_transaction_confirmation_modal) = create_signal(false);
    let psbt_to_confirm = create_rw_signal(None);
    let stored_hd_paths: HashMap<String, String> = LocalStorage::get("hd_paths").unwrap_or_default();
    let hd_paths = create_rw_signal(stored_hd_paths);
    let address_data_cache = create_rw_signal(HashMap::new());
    let dashboard_data_cache = create_rw_signal(HashMap::new());


    let app_state = AppState {
        connected_address,
        provider,
        keystore,
        is_wallet_locked,
        set_is_wallet_locked,
        is_wallet_connected,
        show_unlock_modal,
        set_show_unlock_modal,
        show_receive_modal,
        set_show_receive_modal,
        address_type,
        hd_path,
        show_wrap_modal,
        set_show_wrap_modal,
        network,
        custom_rpc_url,
        set_custom_rpc_url,
        custom_bech32_prefix,
        set_custom_bech32_prefix,
        custom_p2pkh_prefix,
        set_custom_p2pkh_prefix,
        custom_p2sh_prefix,
        set_custom_p2sh_prefix,
        block_height,
        set_block_height,
        refresh_trigger,
        is_refreshing,
        set_is_refreshing,
        fee_rates,
        set_fee_rates,
        show_qr_scanner_modal,
        set_show_qr_scanner_modal,
        scanned_qr_code,
        show_address_selector_modal,
        set_show_address_selector_modal,
        show_preview_modal,
        set_show_preview_modal,
        transaction_analysis_data,
        rpc_connection_status,
        show_transaction_confirmation_modal,
        set_show_transaction_confirmation_modal,
        psbt_to_confirm,
        hd_paths,
        address_data_cache,
        dashboard_data_cache,
    };

    provide_context(app_state);
}