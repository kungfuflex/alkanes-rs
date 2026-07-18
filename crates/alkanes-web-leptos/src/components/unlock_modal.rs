// Chadson v69.69: Systematic Task Completion
//
// This file defines the UnlockModal component, which is responsible for
// prompting the user for a password to unlock their existing keystore.
// It has been moved from slope-frontend to be a reusable component.

use leptos::*;
use wasm_bindgen_futures::spawn_local;
use gloo_storage::{LocalStorage, Storage};
use crate::state::AppState;
use deezel_web::keystore::Keystore;
use wasm_bindgen_futures::JsFuture;
use bip39::{Mnemonic, Seed};

#[component]
pub fn UnlockModal(
    show: ReadSignal<bool>,
    set_show: WriteSignal<bool>,
) -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let (password, set_password) = create_signal(String::new());
    let (error, set_error) = create_signal(None::<String>);

    let unlock_wallet = move |_| {
        let pass = password.get_untracked();
        if pass.is_empty() {
            set_error.set(Some("Password cannot be empty.".to_string()));
            return;
        }
        set_error.set(None);

        spawn_local(async move {
            if let Ok(keystore_name) = LocalStorage::get::<String>("latest-keystore-name") {
                match LocalStorage::get::<String>(&keystore_name) {
                    Ok(keystore_json) => {
                        match serde_json::from_str::<Keystore>(&keystore_json) {
                            Ok(mut keystore) => {
                                let promise = keystore.decrypt_mnemonic(&pass);
                                let future = JsFuture::from(promise);
                                match future.await {
                                    Ok(mnemonic_js) => {
                                        if let Some(mnemonic_str) = mnemonic_js.as_string() {
                                            if let Ok(mnemonic) = Mnemonic::from_phrase(&mnemonic_str, bip39::Language::English) {
                                                let seed = Seed::new(&mnemonic, "");
                                                keystore.seed = Some(seed);
                                                app_state.keystore.set(Some(keystore));
                                                app_state.set_is_wallet_locked.set(false);
                                                app_state.is_wallet_connected.set(true);
                                                set_show.set(false);
                                            } else {
                                                set_error.set(Some("Failed to parse mnemonic.".to_string()));
                                            }
                                        } else {
                                            set_error.set(Some("Failed to get mnemonic string from JS value.".to_string()));
                                        }
                                    }
                                    Err(_) => {
                                        set_error.set(Some("Invalid password.".to_string()));
                                    }
                                }
                            }
                            Err(_) => {
                                set_error.set(Some("Could not parse keystore data.".to_string()));
                            }
                        }
                    }
                    Err(_) => {
                        set_error.set(Some("Could not find keystore in storage.".to_string()));
                    }
                }
            } else {
                set_error.set(Some("Could not find keystore in storage.".to_string()));
            }
        });
    };

    let disconnect_wallet = move |_| {
        if let Ok(keystore_name) = LocalStorage::get::<String>("latest-keystore-name") {
            LocalStorage::delete(&keystore_name);
        }
        LocalStorage::delete("latest-keystore-name");
        app_state.connected_address.set(None);
        app_state.set_is_wallet_locked.set(false); // Reset lock state
        app_state.keystore.set(None);
        app_state.is_wallet_connected.set(false);
        set_show.set(false);
    };

    view! {
        <Show when=move || show.get() fallback=|| view! {}>
            <div class="fixed inset-0 bg-gray-900 bg-opacity-75 z-40 flex justify-center items-center">
                <div class="bg-gray-800 border border-gray-700 rounded-lg p-8 shadow-xl w-full max-w-md">
                    <h2 class="text-2xl font-bold text-white mb-4">"Unlock Wallet"</h2>
                    <p class="text-gray-400 mb-6">"Your wallet is locked. Please enter your password to continue."</p>

                    <input
                        type="password"
                        placeholder="Password"
                        class="w-full bg-gray-900 border border-gray-700 rounded-md p-3 text-white mb-4"
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                        prop:value=password
                    />

                    {move || error.get().map(|e| view! { <p class="text-red-500 mb-4">{e}</p> })}

                    <div class="flex justify-between items-center">
                        <button
                            on:click=disconnect_wallet
                            class="text-gray-400 hover:text-white"
                        >
                            "Disconnect"
                        </button>
                        <button
                            on:click=unlock_wallet
                            class="bg-green-600 hover:bg-green-700 text-white font-bold py-2 px-6 rounded-lg"
                        >
                            "Unlock"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}