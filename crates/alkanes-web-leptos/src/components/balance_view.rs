// Chadson v69.69: Systematic Task Completion
//
// This file defines the BalanceView component.
// It is responsible for displaying BTC and other asset balances.

use leptos::*;
use alkanes_cli_common::provider::AllBalances;

#[component]
pub fn BalanceView(balances: Signal<Option<AllBalances>>) -> impl IntoView {
    let (show_all_assets, set_show_all_assets) = create_signal(false);

    view! {
        <div class="bg-gray-800 rounded-lg p-6">
            <h2 class="text-2xl font-bold text-white mb-4">"Balances"</h2>
            <div class="space-y-4">
                <div>
                    <p class="text-sm text-gray-400">"BTC Balance"</p>
                    <p class="text-2xl text-white">
                        {move || {
                            balances.get().map(|b| {
                                format!("{:.8}", b.btc.confirmed as f64 / 100_000_000.0)
                            }).unwrap_or_else(|| "0.00000000".to_string())
                        }}
                        " BTC"
                    </p>
                </div>
                <div class="border-t border-gray-700 pt-4">
                    <h3 class="text-lg font-semibold text-white mb-2">"Other Assets"</h3>
                    {move || {
                        balances.get().map(|b| {
                            let balances_clone = b.other.clone();
                            let total_assets = balances_clone.len();
                            let assets_to_show = if show_all_assets.get() { total_assets } else { 5 };
                            let assets_view = balances_clone.into_iter().take(assets_to_show).map(|asset| view! {
                                <div class="flex justify-between items-center">
                                    <span class="text-white">{asset.name}</span>
                                    <span class="font-mono text-green-400">{asset.balance.to_string()}</span>
                                </div>
                            }).collect_view();

                            view! {
                                {assets_view}
                                {if total_assets > 5 {
                                    view! {
                                        <button on:click=move |_| set_show_all_assets.update(|v| *v = !*v) class="text-green-500 mt-2">
                                            {if show_all_assets.get() { "Show Less" } else { "More..." }}
                                        </button>
                                    }.into_view()
                                } else {
                                    view! {}.into_view()
                                }}
                            }
                        }).unwrap_or_else(|| (view! { <p>"Loading..."</p> }).into_view().into())
                    }}
                </div>
            </div>
        </div>
    }
}