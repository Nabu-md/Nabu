use crate::components::ui::card::{Card, CardVariant};
use crate::components::ui::feedback::{Alert, Spinner, SpinnerSize, ToastKind};
use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn VaultSetupWizard<F>(on_vault_selected: F) -> impl IntoView
where
    F: Fn(String) + Copy + Send + Sync + 'static,
{
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (is_loading, set_is_loading) = signal(false);

    let handle_select_vault = Callback::new(move |_| {
        if is_loading.get() {
            return;
        }
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
    });

    let handle_create_vault = Callback::new(move |_| {
        if is_loading.get() {
            return;
        }
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
    });

    view! {
        <div class="flex h-screen w-screen items-center justify-center bg-gray-950 text-gray-100 p-6 select-none">
            <div class="max-w-md w-full bg-gray-900 border border-gray-800 rounded-xl p-8 shadow-2xl space-y-6">
                <div class="text-center space-y-2">
                    <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-blue-600/20 text-blue-400 text-3xl mb-2">
                        {render_icon_view(Icon::BookOpen)}
                    </div>
                    <h1 class="text-2xl font-bold tracking-tight text-white">"Welcome to Nabu"</h1>
                    <p class="text-sm text-gray-400">
                        "Select or create a markdown directory to initialize your knowledge vault."
                    </p>
                </div>

                {move || error_msg.get().map(|err| view! {
                    <Alert kind=ToastKind::Error message=err />
                })}

                <div class="space-y-4 pt-2">
                    <Card variant=CardVariant::Interactive class="w-full" on_click=handle_select_vault>
                        <div class="flex items-center space-x-3 text-left w-full">
                            <span class="text-2xl">{render_icon_view(Icon::FolderOpen)}</span>
                            <div class="flex-1">
                                <div class="text-sm font-semibold text-white">"Select Existing Vault"</div>
                                <div class="text-xs text-gray-400">"Open an existing folder with notes"</div>
                            </div>
                            <span class="text-gray-500">{render_icon_view(Icon::ExternalLink)}</span>
                        </div>
                    </Card>

                    <Card variant=CardVariant::Interactive class="w-full" on_click=handle_create_vault>
                        <div class="flex items-center space-x-3 text-left w-full">
                            <span class="text-2xl">{render_icon_view(Icon::Sparkles)}</span>
                            <div class="flex-1">
                                <div class="text-sm font-semibold text-white">"Create New Vault"</div>
                                <div class="text-xs text-gray-400">"Initialize a new directory for Nabu"</div>
                            </div>
                            <span class="text-gray-500">{render_icon_view(Icon::ExternalLink)}</span>
                        </div>
                    </Card>
                </div>

                {move || if is_loading.get() {
                    view! {
                        <div class="flex items-center justify-center space-x-2 text-xs text-blue-400 pt-2">
                            <Spinner size=SpinnerSize::Sm />
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
