#![cfg(target_arch = "wasm32")]

// deezel-leptos/src/lib.rs
// Chadson v69.69: Crate root for deezel-leptos.
// This file defines the public API of the deezel-leptos crate,
// exporting the reusable state management and UI components.

pub mod state;
pub mod components {
    pub mod wallet_modal;
    pub mod unlock_modal;
    pub mod address_selector_modal;
    pub mod balance_view;
    pub mod utxo_list;
    pub mod network_selector;
    pub mod address_type_selector;
    pub mod address_display;
    pub mod connect_wallet_button;
    pub mod keystore;
}
