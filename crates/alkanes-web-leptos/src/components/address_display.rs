// Chadson v69.69: Systematic Task Completion
//
// This file defines the AddressDisplay component.
// It is responsible for displaying the connected address and providing a copy button.

use leptos::*;
use crate::state::AppState;

#[component]
pub fn AddressDisplay() -> impl IntoView {
    let app_state = use_context::<AppState>().expect("AppState to be provided");
    let connected_address = app_state.connected_address;

    let copy_address = move |_| {
        if let Some(info) = connected_address.get_untracked() {
            let clipboard = web_sys::window().unwrap().navigator().clipboard();
            let _ = clipboard.write_text(&info.address);
        }
    };

    view! {
        <div class="flex justify-between items-start gap-4">
            <p class="text-white font-mono break-all flex-grow">{move || connected_address.get().map(|info| info.address).unwrap_or_default()}</p>
            <div class="flex flex-col gap-2 flex-shrink-0">
                <button on:click=copy_address class="bg-gray-700 hover:bg-gray-600 text-white p-2 rounded-lg">
                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                    </svg>
                </button>
            </div>
        </div>
    }
}