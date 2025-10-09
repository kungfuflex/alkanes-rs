// Chadson v69.69: Systematic Task Completion
//
// Keystore components moved from slope-frontend.

use crate::state::AppState;
use bip39::{Language, Mnemonic};
use alkanes_web_sys::{
    keystore as alkanes_keystore,
    keystore_wallet::KeystoreWallet,
    wallet_provider::{BrowserWalletProvider, WalletConnector},
};
use leptos::{logging::log, *};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};
use std::rc::Rc;
use std::cell::RefCell;

#[component]
pub fn CreateKeystore(set_show_modal: WriteSignal<bool>) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState to be provided");
    let (mnemonic, set_mnemonic) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (confirm_password, set_confirm_password) = create_signal(String::new());
    let (copy_button_text, set_copy_button_text) = create_signal("Copy to Clipboard");
    let (is_creating, set_is_creating) = create_signal(false);

    create_effect(move |_| {
        let mut entropy = [0u8; 16];
        getrandom::getrandom(&mut entropy).expect("Failed to generate entropy");
        let new_mnemonic = Mnemonic::from_entropy(&entropy, Language::English)
            .expect("Failed to create mnemonic");
        set_mnemonic.set(new_mnemonic.to_string());
    });

    let passwords_match_and_are_valid = move || {
        !password.get().is_empty() && password.get() == confirm_password.get()
    };

    let create_and_download = move |_| {
        set_is_creating.set(true);
        let mnemonic_str = mnemonic.get_untracked();
        let password_str = password.get_untracked();
        let set_is_creating_clone = set_is_creating.clone();
        let state_clone = state.clone();

        spawn_local(async move {
            let promise = alkanes_keystore::encrypt_mnemonic(&mnemonic_str, &password_str);
            let result = wasm_bindgen_futures::JsFuture::from(promise).await;
            set_is_creating_clone.set(false);

            match result {
                Ok(keystore_val) => {
                    let keystore: alkanes_keystore::Keystore = match serde_wasm_bindgen::from_value(keystore_val) {
                        Ok(k) => k,
                        Err(e) => {
                            log!("Failed to deserialize keystore: {:?}", e);
                            return;
                        }
                    };
                    let keystore_json = match serde_json::to_string(&keystore) {
                        Ok(j) => j,
                        Err(e) => {
                            log!("Failed to serialize keystore: {:?}", e);
                            return;
                        }
                    };

                    // Save to localStorage
                    let timestamp = js_sys::Date::now() as u64;
                    let keystore_name = format!("keystore-{}.json", timestamp);
                    if let Some(window) = web_sys::window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            if let Err(e) = storage.set_item(&keystore_name, &keystore_json) {
                                log!("Failed to save keystore to localStorage: {:?}", e);
                            }
                            if let Err(e) = storage.set_item("latest-keystore-name", &keystore_name) {
                                log!("Failed to save latest keystore name to localStorage: {:?}", e);
                            }
                        }
                    }

                    // Download the keystore
                    let props = BlobPropertyBag::new();
                    props.set_type("application/json");
                    let blob = match Blob::new_with_str_sequence_and_options(
                        &js_sys::Array::of1(&JsValue::from_str(&keystore_json)),
                        &props,
                    ) {
                        Ok(b) => b,
                        Err(e) => {
                            log!("Failed to create blob: {:?}", e);
                            return;
                        }
                    };
                    let url = match Url::create_object_url_with_blob(&blob) {
                        Ok(u) => u,
                        Err(e) => {
                            log!("Failed to create object URL: {:?}", e);
                            return;
                        }
                    };

                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            if let Ok(a) = document.create_element("a") {
                                if let Ok(anchor) = a.dyn_into::<HtmlAnchorElement>() {
                                    anchor.set_href(&url);
                                    anchor.set_download(&keystore_name);
                                    if let Some(body) = document.body() {
                                        let _ = body.append_child(&anchor);
                                        anchor.click();
                                        let _ = body.remove_child(&anchor);
                                    }
                                }
                            }
                        }
                    }
                    let _ = Url::revoke_object_url(&url);

                    // Automatically log in
                    let connector = WalletConnector::new();
                    if let Some(wallet_info) = connector.get_wallet_info("keystore") {
                        let keystore_wallet = KeystoreWallet::new(wallet_info.clone(), keystore.clone(), Some(password_str));
                        match BrowserWalletProvider::connect_local(Box::new(keystore_wallet), "regtest".to_string()).await {
                            Ok(provider) => {
                                state_clone.keystore.set(Some(keystore));
                                state_clone.provider.set(Some(Rc::new(RefCell::new(provider))));
                                state_clone.set_is_wallet_locked.set(false);
                            },
                            Err(e) => {
                                log!("Failed to connect with keystore wallet: {:?}", e);
                                return;
                            }
                        }
                    }
                    set_show_modal.set(false);
                }
                Err(e) => {
                    log!("Failed to encrypt keystore: {:?}", e);
                }
            }
        });
    };

    view! {
        <div>
            <h2 class="text-2xl font-bold text-white mb-4">"Create Keystore"</h2>
            <p class="text-white mb-2">"Please save these 12 words in a safe place:"</p>
            <div class="bg-gray-700 p-4 rounded-lg mb-4">
                <p class="text-white font-mono text-lg">{move || mnemonic.get()}</p>
            </div>
            <button
                class="bg-blue-500 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded-lg mb-4 w-full"
                on:click=move |_| {
                    if let Some(clipboard) = web_sys::window().and_then(|w| Some(w.navigator().clipboard())) {
                        if !clipboard.is_undefined() {
                            let promise = clipboard.write_text(&mnemonic.get());
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
                                    log!("Failed to copy to clipboard: {:?}", e);
                                } else {
                                    log!("Copied to clipboard!");
                                    set_copy_button_text.set("Copied!");
                                    gloo_timers::future::TimeoutFuture::new(2_000).await;
                                    set_copy_button_text.set("Copy to Clipboard");
                                }
                            });
                        } else {
                            log!("Clipboard API is not available in this context.");
                        }
                    }
                }
            >
                {move || copy_button_text.get()}
            </button>
            <p class="text-white mb-2">"Create a password to encrypt your keystore:"</p>
            <input
                type="password"
                placeholder="Password"
                class="w-full bg-gray-700 text-white p-2 rounded-lg mb-2"
                on:input=move |ev| set_password.set(event_target_value(&ev))
            />
            <input
                type="password"
                placeholder="Confirm Password"
                class="w-full bg-gray-700 text-white p-2 rounded-lg mb-4"
                on:input=move |ev| set_confirm_password.set(event_target_value(&ev))
            />
            <button
                class="bg-green-500 text-white font-bold py-2 px-4 rounded-lg w-full"
                class:hover:bg-green-600=passwords_match_and_are_valid
                class:opacity-50=move || !passwords_match_and_are_valid() || is_creating.get()
                class:cursor-not-allowed=move || !passwords_match_and_are_valid() || is_creating.get()
                disabled=move || !passwords_match_and_are_valid() || is_creating.get()
                on:click=create_and_download
            >
                {move || if is_creating.get() { "Creating..." } else { "Create & Download Keystore" }}
            </button>
        </div>
    }
}

#[component]
pub fn ImportKeystore(set_show_modal: WriteSignal<bool>) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState to be provided");
    let (password, set_password) = create_signal(String::new());
    let (keystore_json, set_keystore_json) = create_signal(String::new());
    let file_input_ref = create_node_ref::<html::Input>();

    let on_file_change = move |ev: web_sys::Event| {
        let target = event_target::<web_sys::HtmlInputElement>(&ev);
        if let Some(file) = target.files().and_then(|list| list.get(0)) {
            if let Ok(reader) = web_sys::FileReader::new() {
                let reader_clone = reader.clone();
                let onload = wasm_bindgen::closure::Closure::wrap(Box::new(move |_| {
                    if let Ok(result) = reader_clone.result() {
                        if let Some(result_str) = result.as_string() {
                            set_keystore_json.set(result_str);
                        }
                    }
                }) as Box<dyn FnMut(web_sys::ProgressEvent)>);
                reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                if let Err(e) = reader.read_as_text(&file) {
                    log!("Failed to read file: {:?}", e);
                }
                onload.forget();
            }
        }
    };

    let import_and_unlock = move |_| {
        let keystore_json_str = keystore_json.get_untracked();
        let password_str = password.get_untracked();
        spawn_local(async move {
            let keystore: alkanes_keystore::Keystore = match serde_json::from_str(&keystore_json_str) {
                Ok(k) => k,
                Err(e) => {
                    log!("Failed to parse keystore JSON: {:?}", e);
                    return;
                }
            };

            let promise = keystore.decrypt_mnemonic(&password_str);
            let result = wasm_bindgen_futures::JsFuture::from(promise).await;

            match result {
                Ok(mnemonic_val) => {
                    if let Some(mnemonic_str) = mnemonic_val.as_string() {
                        if let Ok(mnemonic) = Mnemonic::from_phrase(&mnemonic_str, Language::English) {
                            let seed = bip39::Seed::new(&mnemonic, "");
                            let connector = WalletConnector::new();
                            if let Some(wallet_info) = connector.get_wallet_info("keystore") {
                                let mut keystore_from_json: alkanes_keystore::Keystore = serde_json::from_str(&keystore_json_str).unwrap();
                                keystore_from_json.seed = Some(seed);
                                let keystore_wallet = KeystoreWallet::new(wallet_info.clone(), keystore_from_json.clone(), Some(password_str));
                                match BrowserWalletProvider::connect_local(Box::new(keystore_wallet), "regtest".to_string()).await {
                                    Ok(provider) => {
                                        state.keystore.set(Some(keystore_from_json));
                                        state.provider.set(Some(Rc::new(RefCell::new(provider))));
                                        state.set_is_wallet_locked.set(false);
                                    },
                                    Err(e) => {
                                        log!("Failed to connect with keystore wallet: {:?}", e);
                                        return;
                                    }
                                }
                            }
                        } else {
                            log!("Failed to parse mnemonic from string");
                        }
                    } else {
                        log!("Failed to get mnemonic string from JsValue");
                    }
                    set_show_modal.set(false);
                }
                Err(e) => {
                    log!("Failed to decrypt keystore: {:?}", e);
                }
            }
        });
    };

    view! {
        <div>
            <h2 class="text-2xl font-bold text-white mb-4">"Import Keystore"</h2>
            <p class="text-white mb-2">"Select your keystore file:"</p>
            <label for="keystore-file-upload" class="w-full bg-gray-700 hover:bg-gray-600 text-white font-bold py-3 px-4 rounded-lg mb-4 cursor-pointer text-center block">
                "Choose Keystore File"
            </label>
            <input
                type="file"
                id="keystore-file-upload"
                class="hidden"
                node_ref=file_input_ref
                on:change=on_file_change
                accept=".json"
            />
            <p class="text-white mb-2">"Enter your password:"</p>
            <input
                type="password"
                placeholder="Password"
                class="w-full bg-gray-700 text-white p-2 rounded-lg mb-4"
                on:input=move |ev| set_password.set(event_target_value(&ev))
            />
            <button
                class="bg-green-500 text-white font-bold py-2 px-4 rounded-lg w-full"
                on:click=import_and_unlock
            >
                "Import & Unlock"
            </button>
        </div>
    }
}