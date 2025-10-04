// Chadson v69.69: Systematic Task Completion
//
// This file defines the AddressSelectorModal component, which allows users
// to select a specific address derived from their keystore.
// It has been moved from slope-frontend to be a reusable component.

use leptos::*;
use wasm_bindgen::JsCast;
use crate::state::{AppState, ConnectedAddressInfo};
use deezel_common::traits::KeystoreAddress;
use deezel_common::DeezelError;

const ADDRESS_FETCH_BATCH_SIZE: u32 = 20;

#[component]
fn AddressRow(addr: KeystoreAddress, on_select: Callback<KeystoreAddress>) -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let (addr_signal, _) = create_signal(addr);

    let balance = create_resource(
        move || addr_signal.get(),
        move |addr| {
            let provider = app_state.provider.get_untracked().unwrap();
            async move {
                match provider.borrow().get_utxos(false, Some(vec![addr.address])).await {
                    Ok(utxos) => Some(utxos.iter().map(|(_, u)| u.amount).sum::<u64>()),
                    Err(_) => None,
                }
            }
        },
    );

   view! {
       <button
           on:click=move |_| on_select.call(addr_signal.get_untracked())
           class="w-full text-left p-3 mb-2 rounded-lg hover:bg-gray-700 transition-colors"
       >
           <div class="flex justify-between items-center">
               <div>
                   <p class="font-mono text-white break-all">{move || addr_signal.get().address}</p>
                   <p class="text-sm text-gray-500">{move || addr_signal.get().derivation_path}</p>
               </div>
               <div class="text-right">
                   <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                       {move || match balance.get() {
                           Some(Some(bal)) => view! { <p>{format!("{:.8} BTC", bal as f64 / 100_000_000.0)}</p> }.into_view(),
                           _ => view! { <p>"-"</p> }.into_view(),
                       }}
                   </Suspense>
               </div>
           </div>
       </button>
   }
}

#[component]
pub fn AddressSelectorModal(
    show: ReadSignal<bool>,
    set_show: WriteSignal<bool>,
) -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let app_state_for_select = app_state.clone();
    let (addresses, set_addresses) = create_signal(Vec::<KeystoreAddress>::new());
    let (next_index, set_next_index) = create_signal(0);
    let (is_loading, set_is_loading) = create_signal(false);
    let scroll_container_ref = create_node_ref::<html::Div>();

    let fetch_addresses_action = create_action(move |&index: &u32| {
        let app_state = app_state.clone();
        async move {
            set_is_loading.set(true);
            let result = if let Some(provider) = app_state.provider.get_untracked() {
                match provider.borrow().get_master_public_key().await {
                    Ok(Some(xpub)) => {
                        let network_str = app_state.network.get_untracked();
                        let address_type = app_state.address_type.get_untracked();
                        let network_params = deezel_common::network::NetworkParams::from_network_str(&network_str).unwrap();
                        
                        provider.borrow().derive_addresses(
                            &xpub,
                            &network_params,
                            &[&address_type],
                            index,
                            ADDRESS_FETCH_BATCH_SIZE
                        ).await
                    },
                    _ => Err(DeezelError::Other("Wallet does not support address derivation or no master public key found.".to_string())),
                }
            } else {
                Err(DeezelError::Other("Provider not available".to_string()))
            };
            set_is_loading.set(false);
            result
        }
    });

    create_effect(move |_| {
        if show.get() && addresses.get().is_empty() {
            fetch_addresses_action.dispatch(0);
        }
    });

    create_effect(move |_| {
        if let Some(Ok(new_addresses)) = fetch_addresses_action.value().get() {
            set_addresses.update(|a| a.extend(new_addresses));
            set_next_index.update(|i| *i += ADDRESS_FETCH_BATCH_SIZE);
        }
    });

    let on_address_select = Callback::new(move |addr: KeystoreAddress| {
        let info = ConnectedAddressInfo {
            address: addr.address,
            derivation_path: addr.derivation_path,
            address_type: addr.script_type.to_string(),
        };
        app_state_for_select.connected_address.set(Some(info));
        set_show.set(false);
    });

    create_effect(move |_| {
        if let Some(container) = scroll_container_ref.get() {
            let container_clone = container.clone();
            let fetch_more = move |_e: web_sys::Event| {
                if !is_loading.get() {
                    let scroll_top = container_clone.scroll_top();
                    let scroll_height = container_clone.scroll_height();
                    let client_height = container_clone.client_height();
                    if scroll_height - scroll_top - client_height < 200 { // 200px threshold
                        fetch_addresses_action.dispatch(next_index.get());
                    }
                }
            };
            let event_listener = wasm_bindgen::closure::Closure::wrap(Box::new(fetch_more) as Box<dyn FnMut(_)>);
            let _ = container.add_event_listener_with_callback("scroll", event_listener.as_ref().unchecked_ref());
            event_listener.forget();
        }
    });

     view! {
        <Show when=move || show.get() fallback=|| view! {}>
            <div class="fixed inset-0 bg-black bg-opacity-75 flex justify-center items-center z-50" on:click=move |_| set_show.set(false)>
                <div class="bg-gray-800 border border-gray-700 rounded-lg shadow-xl p-8 w-full max-w-2xl flex flex-col" on:click=|ev| ev.stop_propagation()>
                    <div class="flex justify-between items-center mb-6">
                        <h2 class="text-2xl font-bold text-white">"Select Address"</h2>
                        <button on:click=move |_| set_show.set(false) class="text-gray-400 hover:text-white">
                            <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>

                    <div node_ref=scroll_container_ref class="flex-grow overflow-y-auto custom-scrollbar pr-4 -mr-4" style="max-height: 60vh;">
                        <For
                            each=move || addresses.get()
                            key=|addr| addr.address.clone()
                            children=move |addr| {
                                view! {
                                    <AddressRow addr=addr on_select=on_address_select />
                                }
                            }
                        />
                    </div>

                    <div class="mt-6 pt-4 border-t border-gray-700 text-center">
                        <Show when=move || is_loading.get()>
                            <p>"Loading..."</p>
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
    }
}