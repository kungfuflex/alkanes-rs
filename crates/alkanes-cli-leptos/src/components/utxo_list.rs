// Chadson v69.69: Systematic Task Completion
//
// This file defines the UtxoList component.
// It is responsible for displaying a paginated list of UTXOs.

use leptos::*;
use deezel_common::provider::EnrichedUtxo;

#[component]
pub fn UtxoList(utxos: Signal<Option<Vec<EnrichedUtxo>>>) -> impl IntoView {
    let (current_page, set_current_page) = create_signal(1);
    let items_per_page = 10;

    view! {
        <div class="bg-gray-800 rounded-lg p-6">
            <h2 class="text-2xl font-bold text-white mb-4">"Unspent Transaction Outputs (UTXOs)"</h2>
            <div class="h-96 overflow-y-auto custom-scrollbar">
                <table class="min-w-full">
                    <thead class="sticky top-0 bg-gray-800">
                        <tr class="border-b border-gray-700">
                            <th class="text-left text-sm font-semibold text-gray-400 p-2">"Output"</th>
                            <th class="text-left text-sm font-semibold text-gray-400 p-2">"Amount (sats)"</th>
                            <th class="text-left text-sm font-semibold text-gray-400 p-2">"Assets"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            utxos.get().map(|utxo_data| {
                                let page = current_page.get();
                                let start = (page - 1) * items_per_page;
                                let end = start + items_per_page;
                                let paginated_utxos = utxo_data.get(start..end).unwrap_or(&[]).to_vec();

                                if paginated_utxos.is_empty() {
                                    view! { <tr><td colspan="3" class="text-center text-gray-500 py-4">"No UTXOs found."</td></tr> }.into_view()
                                } else {
                                    paginated_utxos.into_iter().map(|utxo| view! {
                                        <tr class="border-b border-gray-700 hover:bg-gray-700/50">
                                            <td class="p-2 text-white font-mono text-sm">{format!("{}...:{}", &utxo.utxo_info.txid[..8], utxo.utxo_info.vout)}</td>
                                            <td class="p-2 text-white font-mono">{utxo.utxo_info.amount.to_string()}</td>
                                            <td class="p-2 text-white">
                                                {if utxo.assets.is_empty() {
                                                    view! { <span class="text-gray-500">"-"</span> }.into_view()
                                                } else {
                                                    utxo.assets.into_iter().map(|asset| view! { <div>{asset.name}</div> }).collect_view()
                                                }}
                                            </td>
                                        </tr>
                                    }).collect_view()
                                }
                            }).unwrap_or_else(|| view! { <tr><td colspan="3" class="text-center text-gray-500 py-4">"Loading..."</td></tr> }.into_view())
                        }}
                    </tbody>
                </table>
            </div>
            <div class="flex justify-between items-center mt-4">
                <button
                    on:click=move |_| set_current_page.update(|p| if *p > 1 { *p -= 1 })
                    class="bg-gray-700 hover:bg-gray-600 text-white font-bold py-2 px-4 rounded-lg disabled:opacity-50"
                    disabled=move || current_page.get() == 1
                >
                    "Previous"
                </button>
                <span class="text-white">{move || format!("Page {}", current_page.get())}</span>
                <button
                    on:click=move |_| set_current_page.update(|p| *p += 1)
                    class="bg-gray-700 hover:bg-gray-600 text-white font-bold py-2 px-4 rounded-lg disabled:opacity-50"
                    disabled=move || utxos.with(|u| u.as_ref().map_or(true, |u| current_page.get() * items_per_page >= u.len()))
                >
                    "Next"
                </button>
            </div>
        </div>
    }
}