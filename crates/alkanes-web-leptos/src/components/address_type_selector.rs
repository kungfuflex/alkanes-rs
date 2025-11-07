// Chadson v69.69: Systematic Task Completion
//
// This file defines the AddressTypeSelector component.
// It allows the user to switch between different Bitcoin address types.

use leptos::*;
use crate::state::AppState;

#[component]
pub fn AddressTypeSelector() -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let address_type = app_state.address_type;
    let is_browser_wallet = create_memo(move |_| app_state.is_browser_wallet());

    view! {
        <div class="flex-1">
            <label class="text-gray-400 block mb-1">"Address Type"</label>
            <select
                class="bg-gray-700 text-white p-2 rounded-lg w-full"
                on:change=move |ev| {
                    let new_addr_type = event_target_value(&ev);
                    address_type.set(new_addr_type);
                }
                prop:value=move || address_type.get()
                disabled=is_browser_wallet
            >
                <option value="p2tr">"Taproot (P2TR)"</option>
                <option value="p2wpkh">"Native Segwit (P2WPKH)"</option>
                <option value="p2sh-p2wpkh">"Nested Segwit (P2SH-P2WPKH)"</option>
                <option value="p2pkh">"Legacy (P2PKH)"</option>
            </select>
        </div>
    }
}