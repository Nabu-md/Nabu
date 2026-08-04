//! Menu system — dropdown, context, command, overflow.
//!
//! Menus render inside `.menu` / `.menu-item` chrome. Keyboard support:
//! Escape closes; menu items are focusable buttons (Tab / Enter / Space).

use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;

/// Menu item. Use inside [`DropdownMenu`], [`ContextMenu`] or [`CommandMenu`].
#[component]
pub fn MenuItem(
    /// Label text.
    label: String,
    /// Optional icon rendered before the label (replaces leading emoji).
    #[prop(optional)]
    icon: Option<Icon>,
    /// Optional hint / shortcut shown on the right.
    #[prop(optional)]
    hint: Option<String>,
    /// Danger styling.
    #[prop(optional)]
    danger: bool,
    /// Disables the item.
    #[prop(optional)]
    disabled: bool,
    /// Called when the item is activated.
    #[prop(optional)]
    on_select: Option<Callback<()>>,
) -> impl IntoView {
    let extra = if danger { " menu-item-danger" } else { "" };
    view! {
        <button
            type="button"
            role="menuitem"
            class=format!("menu-item{extra}")
            disabled=disabled
            on:click=move |_| {
                if let Some(cb) = on_select.as_ref() {
                    cb.run(());
                }
            }
        >
            {icon.map(|ic| view! { <span class="menu-item-icon" aria-hidden="true">{render_icon_view(ic)}</span> }.into_any())}
            <span class="flex-1 text-left">{label}</span>
            {hint.map(|h| view! { <span class="text-xs text-gray-500">{h}</span> }.into_any())}
        </button>
    }
}

/// Horizontal divider between menu items.
#[component]
pub fn MenuSeparator() -> impl IntoView {
    view! { <div class="menu-separator" role="separator"></div> }
}

/// Dropdown menu — a trigger button that opens a floating menu.
#[component]
pub fn DropdownMenu(
    /// Trigger button label.
    trigger: String,
    /// Extra trigger utility classes.
    #[prop(optional)]
    trigger_class: Option<&'static str>,
    /// Extra menu utility classes.
    #[prop(optional)]
    menu_class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let (open, set_open) = signal(false);
    let menu_extra = menu_class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div
            class="relative inline-block"
            on:keydown=move |ev| {
                if ev.key() == "Escape" {
                    set_open.set(false);
                }
            }
        >
            <Button
                variant=ButtonVariant::Ghost
                class=trigger_class.unwrap_or("")
                aria_haspopup="menu"
                aria_expanded=open
                on_click=Callback::new(move |_| set_open.update(|o| *o = !*o))
            >
                {trigger.clone()}
                <span class="text-xs text-gray-500" aria-hidden="true">{render_icon_view(Icon::ChevronDown)}</span>
            </Button>
            {move || if open.get() {
                view! {
                    <div
                        class=format!("menu absolute right-0 top-full mt-1{menu_extra}")
                        role="menu"
                        on:click=move |_| set_open.set(false)
                    >
                        {children()}
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}

/// Overflow menu — a "⋯" dropdown with no visible trigger text.
#[component]
pub fn OverflowMenu(
    /// Extra menu utility classes.
    #[prop(optional)]
    menu_class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    view! {
        <DropdownMenu trigger="⋯".to_string() trigger_class="btn-icon" menu_class=menu_class.unwrap_or("")>
            {children()}
        </DropdownMenu>
    }
}

/// One command entry in a [`CommandMenu`].
#[derive(Clone, PartialEq)]
pub struct CommandItem {
    pub id: String,
    pub label: String,
    pub hint: Option<String>,
}

impl CommandItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Command menu / palette — a searchable list of commands.
/// Opens with `open.set(true)`; closes on selection or Escape.
#[component]
pub fn CommandMenu(
    /// Two-way bound open state.
    open: RwSignal<bool>,
    /// Commands to search.
    items: Vec<CommandItem>,
    /// Called with the selected command id.
    #[prop(optional)]
    on_select: Option<Callback<String>>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (active_index, set_active_index) = signal(0usize);

    let items = items;
    let filtered = Memo::new(move |_| {
        let q = query.get().to_lowercase();
        if q.is_empty() {
            items.clone()
        } else {
            items
                .iter()
                .filter(|i| i.label.to_lowercase().contains(&q))
                .cloned()
                .collect()
        }
    });

    let on_select = on_select;
    let choose = Callback::new(move |id: String| {
        open.set(false);
        set_query.set(String::new());
        if let Some(cb) = on_select.as_ref() {
            cb.run(id);
        }
    });
    let close = Callback::new(move |_| {
        open.set(false);
        set_query.set(String::new());
    });
    let close_key = close.clone();
    let choose_enter = choose.clone();

    view! {
        {move || if open.get() {
            let list = filtered.get();
            view! {
                <div class="dialog-overlay" role="presentation" on:click=move |_| close.run(())>
                    <div
                        class="menu w-80 max-w-full"
                        role="dialog"
                        aria-modal="true"
                        aria-label="Command menu"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <input
                            type="text"
                            class="input mb-1"
                            placeholder="Type a command…"
                            prop:value=query
                            on:input=move |ev| {
                                set_query.set(event_target_value(&ev));
                                set_active_index.set(0);
                            }
                            on:keydown=move |ev| {
                                let key = ev.key();
                                if key == "Escape" {
                                    close_key.run(());
                                } else if key == "ArrowDown" {
                                    ev.prevent_default();
                                    set_active_index.update(|i| {
                                        let len = filtered.get().len();
                                        *i = if len == 0 { 0 } else { (*i + 1) % len };
                                    });
                                } else if key == "ArrowUp" {
                                    ev.prevent_default();
                                    set_active_index.update(|i| {
                                        let len = filtered.get().len();
                                        *i = if len == 0 { 0 } else { (*i + len - 1) % len };
                                    });
                                } else if key == "Enter" {
                                    ev.prevent_default();
                                    let list = filtered.get();
                                    if let Some(item) = list.get(active_index.get()) {
                                        choose_enter.run(item.id.clone());
                                    }
                                }
                            }
                        />
                        <div class="max-h-64 overflow-y-auto">
                            {if list.is_empty() {
                                view! {
                                    <div class="menu-item text-gray-500 cursor-default">
                                        "No matching commands"
                                    </div>
                                }.into_any()
                            } else {
                                list.into_iter().enumerate().map(|(i, item)| {
                                    let choose_item = choose.clone();
                                    let id = item.id.clone();
                                    let label = item.label.clone();
                                    let hint = item.hint.clone();
                                    view! {
                                        <button
                                            type="button"
                                            role="option"
                                            aria-selected=move || i == active_index.get()
                                            class=move || format!("menu-item w-full text-left{}", if i == active_index.get() { " menu-item-active" } else { "" })
                                            on:mouseenter=move |_| set_active_index.set(i)
                                            on:click=move |_| choose_item.run(id.clone())
                                        >
                                            <span class="flex-1">{label}</span>
                                            {hint.map(|h| view! { <span class="text-xs text-gray-500">{h}</span> }.into_any())}
                                        </button>
                                    }
                                }).collect_view().into_any()
                            }}
                        </div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}

/// Context menu — right-click on the trigger opens a floating menu at the cursor.
#[component]
pub fn ContextMenu(
    /// The right-click target content.
    children: ChildrenFn,
    /// Items rendered inside the floating menu (e.g. [`MenuItem`]s).
    menu_items: ChildrenFn,
) -> impl IntoView {
    let (open, set_open) = signal(false);
    let (pos, set_pos) = signal((0.0, 0.0));
    view! {
        <div
            class="contents"
            on:contextmenu=move |ev: web_sys::MouseEvent| {
                ev.prevent_default();
                set_pos.set((ev.client_x() as f64, ev.client_y() as f64));
                set_open.set(true);
            }
        >
            {children()}
        </div>
        {move || if open.get() {
            view! {
                <div
                    class="fixed inset-0 z-40"
                    on:click=move |_| set_open.set(false)
                    on:contextmenu=move |ev: web_sys::MouseEvent| {
                        ev.prevent_default();
                        set_open.set(false);
                    }
                ></div>
                <div
                    class="menu fixed z-50"
                    role="menu"
                    style=move || format!("left: {}px; top: {}px;", pos.get().0, pos.get().1)
                    on:click=move |_| set_open.set(false)
                >
                    {menu_items()}
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}
