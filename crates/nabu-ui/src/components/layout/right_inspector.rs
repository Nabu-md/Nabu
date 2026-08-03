//! # Right Inspector — connected knowledge for the active note
//!
//! Phase 13.1: the inspector's Tags / Backlinks / Outgoing / Mentions tabs are
//! now driven by the backend `note_links` command for the currently active
//! note (workspace `active_path`). Backlinks and mentions render context
//! snippets with the matched span highlighted, and quick actions jump to the
//! source note, open external URLs, copy `[[wikilinks]]`, convert plain-text
//! mentions into links, and ignore unwanted mention suggestions.

use crate::components::graph_view::MentionSnippet;
use crate::components::ui::feedback::{use_toast, LoadingBlock, SpinnerSize};
use crate::components::ui::info::EmptyState;
use crate::components::ui::nav::{TabDef, Tabs};
use crate::components::workspace::{open_tab, use_workspace};
use crate::models::graph::NoteLinks;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// Fetches `note_links` for a path via IPC.
async fn fetch_links(path: String) -> Option<NoteLinks> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path })).ok()?;
    let res = crate::ipc::tauri_invoke("note_links", args).await;
    serde_wasm_bindgen::from_value::<NoteLinks>(res).ok()
}

#[component]
pub fn RightInspector() -> impl IntoView {
    let ws = use_workspace();
    let toasts = use_toast();
    let active_tab = RwSignal::new("🏷️".to_string());

    let (links, set_links) = signal(None::<NoteLinks>);
    let (version, set_version) = signal(0u32);

    // Reload links whenever the active note (or the action counter) changes.
    // The version token guards against a stale response landing after a newer
    // selection (rapid tab switching).
    Effect::new(move |_| {
        let path = ws.active_path.get();
        let v = version.get();
        if let Some(path) = path {
            let p = path.clone();
            set_links.set(None);
            spawn_local(async move {
                if let Some(l) = fetch_links(p).await {
                    if v == version.get_untracked() {
                        set_links.set(Some(l));
                    }
                }
            });
        } else {
            set_links.set(None);
        }
    });

    // ── Actions ─────────────────────────────────────────────────────
    let open_note = move |path: String| open_tab(ws, &path);

    let link_mention_action = move |path: String, title: String| {
        let toasts = toasts;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                "path": path,
                "title": title,
            }))
            .unwrap();
            let res = crate::ipc::tauri_invoke("link_mention", args).await;
            if serde_wasm_bindgen::from_value::<String>(res).is_ok() {
                toasts.success("Linked", format!("[[{title}]] added to the note"));
                // Tell the editor this file changed on disk so it reloads
                // instead of autosaving the stale buffer over the new link.
                crate::components::workspace::bump_content_version(ws, &path);
                set_version.update(|v| *v += 1);
            } else {
                toasts.error("Link mention", "Could not convert that mention into a link");
            }
        });
    };

    let ignore_mention_action = move |title: String| {
        let toasts = toasts;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "title": title })).unwrap();
            let res = crate::ipc::tauri_invoke("mention_ignore", args).await;
            if serde_wasm_bindgen::from_value::<()>(res).is_ok() {
                toasts.info("Ignored", format!("{title} will no longer be suggested"));
                set_version.update(|v| *v += 1);
            } else {
                toasts.error("Ignore mention", "Could not save the ignore preference");
            }
        });
    };

    let copy_wikilink = move |title: String| {
        let toasts = toasts;
        spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let cb = window.navigator().clipboard();
                let _ = cb.write_text(&format!("[[{title}]]"));
                toasts.success("Copied", format!("[[{title}]]"));
            }
        });
    };

    let open_external = move |url: String| {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(&url, "_blank");
        }
    };

    let tabs = vec![
        TabDef::new("🏷️", "Tags"),
        TabDef::new("🔗", "Backlinks"),
        TabDef::new("➡️", "Outgoing"),
        TabDef::new("💬", "Mentions"),
    ];

    // ── Per-tab render ───────────────────────────────────────────────
    let tags_tab = move || {
        let links = links.get();
        let Some(links) = links else {
            return view! {
                <div class="p-4"><LoadingBlock label="Loading…" size=SpinnerSize::Sm /></div>
            }.into_any();
        };
        if links.tags.is_empty() {
            return view! {
                <EmptyState
                    icon="🏷️"
                    title="No tags yet".to_string()
                    description="Tags in this note's frontmatter will appear here.".to_string()
                ></EmptyState>
            }.into_any();
        }
        view! {
            <div class="flex flex-wrap gap-1.5 p-3">
                {links.tags.into_iter().map(|t| {
                    view! {
                        <span class="inline-flex items-center gap-1 rounded-full bg-gray-800 border border-gray-700 px-2.5 py-1 text-xs text-gray-200">
                            "#" {t}
                        </span>
                    }
                }).collect_view()}
            </div>
        }.into_any()
    };

    let backlinks_tab = move || {
        let links = links.get();
        let Some(links) = links else {
            return view! {
                <div class="p-4"><LoadingBlock label="Loading…" size=SpinnerSize::Sm /></div>
            }.into_any();
        };
        if links.backlinks.is_empty() {
            return view! {
                <EmptyState
                    icon="🔗"
                    title="No backlinks yet".to_string()
                    description="Other notes that link to this one will appear here.".to_string()
                ></EmptyState>
            }.into_any();
        }
        view! {
            <div class="divide-y divide-gray-800/60">
                {links.backlinks.into_iter().map(|b| {
                    let path = b.path.clone();
                    let title = b.title.clone();
                    let folder = b.folder.clone();
                    let count = b.count;
                    let snippet = b.snippet.clone();
                    let s = b.match_start;
                    let e = b.match_end;
                    view! {
                        <div class="px-3 py-2">
                            <button class="w-full text-left" on:click=move |_| open_note(path.clone())>
                                <div class="flex items-center justify-between gap-2">
                                    <span class="text-xs font-medium text-blue-300 truncate">{title}</span>
                                    {if count > 1 {
                                        view! { <span class="text-[10px] text-gray-500 shrink-0">{format!("×{count}")}</span> }.into_any()
                                    } else {
                                        view! {}.into_any()
                                    }}
                                </div>
                                <div class="text-[10px] text-gray-600 truncate">
                                    {if folder.is_empty() { "vault root".to_string() } else { folder }}
                                </div>
                            </button>
                            <div class="text-[11px] text-gray-500 mt-1 leading-snug">
                                <MentionSnippet snippet=snippet match_start=s match_end=e />
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>
        }.into_any()
    };

    let outgoing_tab = move || {
        let links = links.get();
        let Some(links) = links else {
            return view! {
                <div class="p-4"><LoadingBlock label="Loading…" size=SpinnerSize::Sm /></div>
            }.into_any();
        };
        if links.outgoing.is_empty() {
            return view! {
                <EmptyState
                    icon="➡️"
                    title="No outgoing links".to_string()
                    description="Links you write in this note will appear here.".to_string()
                ></EmptyState>
            }.into_any();
        }
        view! {
            <div class="divide-y divide-gray-800/60">
                {links.outgoing.into_iter().map(|o| {
                    let kind = o.kind.clone();
                    let target = o.target.clone();
                    let count = o.count;
                    view! {
                        <div class="px-3 py-2 flex items-center justify-between gap-2">
                            <div class="min-w-0">
                                <div class="text-xs text-gray-300 truncate">{target.clone()}</div>
                                <div class="text-[10px] text-gray-500">
                                    {match kind.as_str() {
                                        "internal" => "note",
                                        "broken" => "broken link",
                                        _ => "external URL",
                                    }}
                                    {if count > 1 { format!(" · ×{count}") } else { String::new() }}
                                </div>
                            </div>
                            <div class="flex gap-1 shrink-0">
                                {if let Some(path) = o.path.clone() {
                                    view! {
                                        <button class="btn btn-xs btn-ghost" on:click=move |_| open_note(path.clone())>"Open"</button>
                                    }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}
                                {if kind == "external" {
                                    view! {
                                        <button class="btn btn-xs btn-ghost" on:click=move |_| open_external(o.target.clone())>"↗"</button>
                                    }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}
                                <button class="btn btn-xs btn-ghost" on:click=move |_| copy_wikilink(target.clone())>"⧉"</button>
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>
        }.into_any()
    };

    let mentions_tab = move || {
        let links = links.get();
        let Some(links) = links else {
            return view! {
                <div class="p-4"><LoadingBlock label="Loading…" size=SpinnerSize::Sm /></div>
            }.into_any();
        };
        if links.mentions.is_empty() {
            return view! {
                <EmptyState
                    icon="💬"
                    title="No unlinked mentions".to_string()
                    description="Other notes referenced by plain text will appear here — link them with one click.".to_string()
                ></EmptyState>
            }.into_any();
        }
        view! {
            <div class="divide-y divide-gray-800/60">
                {links.mentions.into_iter().map(|m| {
                    let title = m.title.clone();
                    let title_link = title.clone();
                    let title_ignore = title.clone();
                    let snippet = m.snippet.clone();
                    let s = m.match_start;
                    let e = m.match_end;
                    let note_path = ws.active_path.get().unwrap_or_default();
                    view! {
                        <div class="px-3 py-2">
                            <div class="flex items-center justify-between gap-2">
                                <span class="text-xs font-medium text-gray-300 truncate">{title}</span>
                                <div class="flex gap-1 shrink-0">
                                    <button class="btn btn-xs btn-primary" on:click=move |_| link_mention_action(note_path.clone(), title_link.clone())>"Link"</button>
                                    <button class="btn btn-xs btn-ghost" on:click=move |_| ignore_mention_action(title_ignore.clone())>"Ignore"</button>
                                </div>
                            </div>
                            <div class="text-[11px] text-gray-500 mt-1 leading-snug">
                                <MentionSnippet snippet=snippet match_start=s match_end=e />
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>
        }.into_any()
    };

    view! {
        <div class="right-inspector w-64 border-l border-gray-700 bg-gray-900 h-screen flex flex-col transition-[width] duration-slow ease-standard">
            <div class="flex border-b border-gray-700">
                <Tabs tabs=tabs active=active_tab />
            </div>
            <div class="flex-1 overflow-y-auto text-gray-300 text-sm">
                {move || match active_tab.get().as_str() {
                    "🏷️" => tags_tab(),
                    "🔗" => backlinks_tab(),
                    "➡️" => outgoing_tab(),
                    "💬" => mentions_tab(),
                    _ => view! {}.into_any(),
                }}
            </div>
        </div>
    }
}
