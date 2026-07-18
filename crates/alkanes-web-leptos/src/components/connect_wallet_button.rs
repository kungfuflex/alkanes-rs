// Chadson v69.69: Systematic Task Completion
//
// This file defines the ConnectWalletButton component.
// It provides a button that handles wallet connection, unlocking, and displays the connected address.

use leptos::*;
use leptos_router::A;
use crate::state::AppState;
use crate::components::wallet_modal::WalletModal;

#[component]
pub fn ConnectWalletButton() -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let (show_wallet_modal, set_show_wallet_modal) = create_signal(false);
    let set_show_unlock_modal = app_state.set_show_unlock_modal;

    let connect_wallet = move |_| {
        set_show_wallet_modal.set(true);
    };

    let unlock_wallet = move |_| {
        set_show_unlock_modal.set(true);
    };

    view! {
        <div class="wallet-container">
            {move || {
                if app_state.is_wallet_locked.get() {
                    view! {
                        <button
                            on:click=unlock_wallet
                            class="bg-yellow-600 hover:bg-yellow-700 text-white font-bold py-2 px-4 rounded-lg"
                        >
                            "Unlock Wallet"
                        </button>
                    }.into_view()
                } else {
                    match app_state.connected_address.get() {
                        Some(info) => view! {
                            <A href="/wallet">
                                <button
                                    class="bg-gray-700 text-green-300 font-mono py-2 px-4 rounded-lg"
                                >
                                    {
                                        let addr_str = info.address;
                                        if addr_str.len() > 10 {
                                            format!("{}...{}", &addr_str[..6], &addr_str[addr_str.len()-4..])
                                        } else {
                                            addr_str
                                        }
                                    }
                                </button>
                            </A>
                        }.into_view(),
                        None => view! {
                            <button
                                on:click=connect_wallet
                                class="bg-green-900 hover:bg-green-800 text-white font-bold py-2 px-4 rounded-lg"
                            >
                                "Connect Wallet"
                            </button>
                        }.into_view(),
                    }
                }
            }}
            {move || show_wallet_modal.get().then(|| view! { <WalletModal set_show_modal=set_show_wallet_modal /> })}
        </div>
    }
}