// Chadson v69.69: Systematic Task Completion
//
// This file defines the NetworkSelector component.
// It allows the user to switch between different Bitcoin networks.

use leptos::*;
use crate::state::AppState;

#[component]
pub fn NetworkSelector() -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let network = app_state.network;
    let is_browser_wallet = create_memo(move |_| app_state.is_browser_wallet());

    let on_network_change = move |ev: ev::Event| {
        let value = event_target_value(&ev);
        network.set(value);
    };

    view! {
        <select
            class="bg-gray-700 text-white p-2 rounded-lg text-sm w-full md:w-auto"
            on:change=on_network_change
            prop:value=move || network.get()
            disabled=is_browser_wallet
        >
            <option value="mainnet" selected=move || network.get() == "mainnet">"Mainnet"</option>
            <option value="testnet" selected=move || network.get() == "testnet">"Testnet"</option>
            <option value="signet" selected=move || network.get() == "signet">"Signet"</option>
            <option value="regtest" selected=move || network.get() == "regtest">"Regtest"</option>
            <option value="custom" selected=move || network.get() == "custom">"Custom"</option>
        </select>
    }
}