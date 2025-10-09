// Chadson v69.69: Systematic Task Completion
//
// This file defines the WalletModal component.
// It has been moved from slope-frontend to deezel-leptos.

use leptos::{*, logging::log};
use alkanes_cli_common::WalletProvider;
use crate::state::AppState;
use crate::state::ConnectedAddressInfo;
use alkanes_web_sys::wallet_provider::{WalletConnector, BrowserWalletProvider, WalletInfo, WalletBackend};
use crate::components::keystore::{CreateKeystore, ImportKeystore};
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq)]
enum ModalView {
    Main,
    Keystore,
    CreateKeystore,
    ImportKeystore,
}

#[component]
pub fn WalletModal(set_show_modal: WriteSignal<bool>) -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let (current_view, set_current_view) = create_signal(ModalView::Main);

    // Resource to get all supported wallets and check their availability.
    let wallets_with_availability = create_resource(
        || (),
        |_| async {
            let connector = WalletConnector::new();
            let supported_wallets = WalletConnector::get_supported_wallets();
            let mut availability_checks = Vec::new();

            for wallet_info in supported_wallets {
                if wallet_info.name == "Keystore" {
                    continue;
                }
                let info_clone = wallet_info.clone();
                let connector_clone = connector.clone();
                availability_checks.push(async move {
                    let injected_wallet = connector_clone.create_injected_wallet(info_clone.clone()).ok();
                    let is_available = if let Some(iw) = injected_wallet {
                        iw.is_available().await
                    } else {
                        Ok(false)
                    };
                    (info_clone, is_available)
                });
            }
            futures::future::join_all(availability_checks).await
        }
    );

    let connect_wallet_app_state = app_state.clone();
    let connect_wallet = Callback::new(move |wallet_info: WalletInfo| {
        let app_state = connect_wallet_app_state.clone();
        let set_show_modal = set_show_modal.clone();
        spawn_local(async move {
            log!("Attempting to connect to: {}", &wallet_info.name);
            match BrowserWalletProvider::connect(wallet_info, "mainnet".to_string()).await {
                Ok(provider) => {
                    match provider.get_address().await {
                        Ok(address) => {
                            log!("Successfully connected. Address: {}", address);
                            let info = ConnectedAddressInfo {
                                address,
                                derivation_path: "".to_string(),
                                address_type: "".to_string(),
                            };
                            app_state.provider.set(Some(Rc::new(RefCell::new(provider))));
                            app_state.connected_address.set(Some(info));
                            app_state.is_wallet_connected.set(true);
                            set_show_modal.set(false);
                        },
                        Err(e) => log!("Error getting address after connecting: {:?}", e),
                    }
                },
                Err(e) => log!("Error connecting to wallet: {:?}", e),
            }
        });
    });

    let main_view = view! {
        <div class="flex justify-between items-center mb-6">
            <h2 class="text-2xl font-bold text-white">"Connect a Wallet"</h2>
        </div>

        <div class="modal-content overflow-y-auto max-h-[70vh]">
            <div class="mb-4">
                <button on:click=move |_| set_current_view.set(ModalView::Keystore) class="w-full bg-blue-500 hover:bg-blue-600 text-white font-bold py-3 px-4 rounded-lg">"Keystore"</button>
            </div>
            <div class="relative my-4">
                <div class="absolute inset-0 flex items-center" aria-hidden="true">
                    <div class="w-full border-t border-gray-600"></div>
                </div>
                <div class="relative flex justify-center">
                    <span class="bg-gray-800 px-2 text-sm text-gray-400">"or connect with"</span>
                </div>
            </div>
            <Transition fallback=move || view! { <p>"Detecting wallets..."</p> }>
                {move || {
                    wallets_with_availability.get().map(|wallets| {
                        if wallets.is_empty() {
                            view! { <p class="text-white text-center">"Loading wallet list..."</p> }.into_view()
                        } else {
                            let wallets = wallets.clone();
                            view! {
                                <ul class="mt-2 gap-4 grid grid-cols-1">
                                    {wallets.into_iter().map(|(wallet, is_available)| {
                                        let is_available = is_available.unwrap_or(false);
                                        let wallet_clone = wallet.clone();
                                        let connect_disabled = !is_available;
                                        view! {
                                            <li>
                                                <button
                                                    on:click=move |_| {
                                                        if !connect_disabled {
                                                            connect_wallet.call(wallet_clone.clone())
                                                        }
                                                    }
                                                    class="w-full bg-gray-700 text-white font-bold py-3 px-4 rounded-lg flex items-center justify-between transition-colors duration-200"
                                                    class:hover:bg-gray-600={!connect_disabled}
                                                    class:opacity-50=connect_disabled
                                                    class:cursor-not-allowed=connect_disabled
                                                    disabled=connect_disabled
                                                >
                                                    <div class="flex items-center gap-4">
                                                        <img src=wallet.icon.clone() alt=&wallet.name class="w-8 h-8"/>
                                                        <span class="text-lg font-semibold">{&wallet.name}</span>
                                                    </div>
                                                    {if !is_available {
                                                        view! { <span class="text-xs text-gray-400">"Not Detected"</span> }.into_view()
                                                    } else {
                                                        view! {
                                                            <svg class="h-5 w-5 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                                                            </svg>
                                                        }.into_view()
                                                    }}
                                                </button>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }.into_view()
                        }
                    })
                }}
            </Transition>
        </div>
    };
    
    let keystore_view = view! {
        <div class="flex justify-between items-center mb-6">
            <h2 class="text-2xl font-bold text-white">"Keystore"</h2>
            <button on:click=move |_| set_show_modal.set(false) class="text-white">
                <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
            </button>
        </div>
        <div class="modal-content">
            <button on:click=move |_| set_current_view.set(ModalView::CreateKeystore) class="w-full bg-blue-500 hover:bg-blue-600 text-white font-bold py-3 px-4 rounded-lg mb-2">"Create Keystore"</button>
            <button on:click=move |_| set_current_view.set(ModalView::ImportKeystore) class="w-full bg-gray-700 hover:bg-gray-600 text-white font-bold py-3 px-4 rounded-lg">"Import Keystore"</button>
        </div>
    };

    view! {
        <div class="fixed inset-0 bg-gray-900 bg-opacity-75 z-50 flex items-center justify-center">
            <div class="bg-gray-800 p-8 rounded-2xl shadow-2xl w-full max-w-md relative">
                <button on:click=move |_| set_show_modal.set(false) class="absolute top-4 right-4 text-gray-400 hover:text-white">
                    <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
                {move || match current_view.get() {
                    ModalView::Main => main_view.clone().into_view(),
                    ModalView::Keystore => keystore_view.clone().into_view(),
                    ModalView::CreateKeystore => view! { <CreateKeystore set_show_modal=set_show_modal/> }.into_view(),
                    ModalView::ImportKeystore => view! { <ImportKeystore set_show_modal=set_show_modal/> }.into_view(),
                }}
            </div>
        </div>
    }
}