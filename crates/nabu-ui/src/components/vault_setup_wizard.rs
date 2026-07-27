use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn VaultSetupWizard<F>(on_vault_selected: F) -> impl IntoView
where
    F: Fn(String) + Copy + 'static,
{
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (is_loading, set_is_loading) = signal(false);

    let handle_select_vault = move |_| {
        set_is_loading.set(true);
        set_error_msg.set(None);
        spawn_local(async move {
            let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let res = crate::ipc::tauri_invoke("select_vault_dialog", empty_args).await;
            set_is_loading.set(false);
            if let Ok(path_val) = serde_wasm_bindgen::from_value::<Option<String>>(res) {
                if let Some(path) = path_val {
                    if !path.trim().is_empty() {
                        on_vault_selected(path);
                        return;
                    }
                }
            } else {
                set_error_msg.set(Some("Failed to select vault directory.".to_string()));
            }
        });
    };

    let handle_create_vault = move |_| {
        set_is_loading.set(true);
        set_error_msg.set(None);
        spawn_local(async move {
            let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let res = crate::ipc::tauri_invoke("create_vault_dialog", empty_args).await;
            set_is_loading.set(false);
            if let Ok(path_val) = serde_wasm_bindgen::from_value::<Option<String>>(res) {
                if let Some(path) = path_val {
                    if !path.trim().is_empty() {
                        on_vault_selected(path);
                        return;
                    }
                }
            } else {
                set_error_msg.set(Some("Failed to create vault directory.".to_string()));
            }
        });
    };

    view! {
        <div class="flex h-screen w-screen items-center justify-center bg-gray-950 text-gray-100 p-6 select-none">
            <div class="max-w-md w-full bg-gray-900 border border-gray-800 rounded-xl p-8 shadow-2xl space-y-6">
                <div class="text-center space-y-2">
                    <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-blue-600/20 text-blue-400 text-3xl mb-2">
                        "📖"
                    </div>
                    <h1 class="text-2xl font-bold tracking-tight text-white">"Welcome to Nabu"</h1>
                    <p class="text-sm text-gray-400">
                        "Select or create a markdown directory to initialize your knowledge vault."
                    </p>
                </div>

                {move || error_msg.get().map(|err| view! {
                    <div class="p-3 bg-red-900/40 border border-red-700/60 rounded-lg text-xs text-red-300">
                        {err}
                    </div>
                })}

                <div class="space-y-4 pt-2">
                    <button
                        class="w-full flex items-center justify-between p-4 bg-gray-800 hover:bg-gray-750 border border-gray-700/70 hover:border-blue-500/50 rounded-xl transition-all group cursor-pointer"
                        on:click=handle_select_vault
                        disabled=move || is_loading.get()
                    >
                        <div class="flex items-center space-x-3 text-left">
                            <span class="text-2xl">"📂"</span>
                            <div>
                                <div class="text-sm font-semibold text-white group-hover:text-blue-400">"Select Existing Vault"</div>
                                <div class="text-xs text-gray-400">"Open an existing folder with notes"</div>
                            </div>
                        </div>
                        <span class="text-gray-500 group-hover:text-blue-400">"→"</span>
                    </button>

                    <button
                        class="w-full flex items-center justify-between p-4 bg-gray-800 hover:bg-gray-750 border border-gray-700/70 hover:border-blue-500/50 rounded-xl transition-all group cursor-pointer"
                        on:click=handle_create_vault
                        disabled=move || is_loading.get()
                    >
                        <div class="flex items-center space-x-3 text-left">
                            <span class="text-2xl">"✨"</span>
                            <div>
                                <div class="text-sm font-semibold text-white group-hover:text-blue-400">"Create New Vault"</div>
                                <div class="text-xs text-gray-400">"Initialize a new directory for Nabu"</div>
                            </div>
                        </div>
                        <span class="text-gray-500 group-hover:text-blue-400">"→"</span>
                    </button>
                </div>

                {move || if is_loading.get() {
                    view! {
                        <div class="flex items-center justify-center space-x-2 text-xs text-blue-400 pt-2">
                            <div class="w-4 h-4 border-2 border-blue-400 border-t-transparent rounded-full animate-spin"></div>
                            <span>"Opening system dialog..."</span>
                        </div>
                    }.into_any()
                } else {
                    view! {}.into_any()
                }}
            </div>
        </div>
    }
}
