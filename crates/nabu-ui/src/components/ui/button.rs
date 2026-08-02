//! Button primitives — 6 variants, 3 sizes, loading and icon modes.
//!
//! Accessibility: native `<button>` semantics, `aria-busy` while loading,
//! `disabled` handling, focus-visible ring from the design system.

use leptos::prelude::*;

/// Visual variants for [`Button`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonVariant {
    /// Solid accent (primary action).
    Primary,
    /// Default neutral button.
    #[default]
    Secondary,
    /// Borderless, low emphasis.
    Ghost,
    /// Outlined neutral button.
    Outline,
    /// Destructive action.
    Destructive,
    /// Square icon-only button.
    Icon,
}

/// Sizing for [`Button`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonSize {
    /// Small (caption).
    Sm,
    /// Default (body).
    #[default]
    Md,
    /// Large.
    Lg,
}

impl ButtonVariant {
    fn classes(self) -> &'static str {
        match self {
            ButtonVariant::Primary => "btn-primary",
            ButtonVariant::Secondary => "",
            ButtonVariant::Ghost => "btn-ghost",
            ButtonVariant::Outline => "btn-outline",
            ButtonVariant::Destructive => "btn-danger",
            ButtonVariant::Icon => "btn-ghost btn-icon",
        }
    }
}

impl ButtonSize {
    fn classes(self) -> &'static str {
        match self {
            ButtonSize::Sm => "btn-sm",
            ButtonSize::Md => "",
            ButtonSize::Lg => "btn-lg",
        }
    }
}

/// Builds the full class list for a button from variant + size + extra classes.
pub fn button_classes(variant: ButtonVariant, size: ButtonSize, extra: Option<&str>) -> String {
    let mut base = format!("btn {} {}", variant.classes(), size.classes());
    if let Some(extra) = extra {
        if !extra.trim().is_empty() {
            base.push(' ');
            base.push_str(extra.trim());
        }
    }
    base.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Core button. Supports primary/secondary/ghost/outline/destructive/icon
/// variants, sm/md/lg sizes, loading state and disabled state.
#[component]
pub fn Button(
    /// Visual variant.
    #[prop(optional)]
    variant: ButtonVariant,
    /// Size.
    #[prop(optional)]
    size: ButtonSize,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables interaction.
    #[prop(optional)]
    disabled: bool,
    /// Shows a spinner and disables the button.
    #[prop(optional)]
    loading: bool,
    /// Tooltip text.
    #[prop(optional)]
    title: Option<&'static str>,
    /// Click handler.
    #[prop(optional)]
    on_click: Option<Callback<web_sys::MouseEvent>>,
    /// Accessible name for icon-only buttons.
    #[prop(optional)]
    aria_label: Option<&'static str>,
    /// ARIA haspopup (for menu triggers).
    #[prop(optional)]
    aria_haspopup: Option<&'static str>,
    /// ARIA expanded (for menu triggers).
    #[prop(optional, into)]
    aria_expanded: Option<Signal<bool>>,
    children: ChildrenFn,
) -> impl IntoView {
    let classes = button_classes(variant, size, class);
    let is_disabled = move || disabled || loading;
    let on_click = on_click;
    let aria_expanded = aria_expanded;

    view! {
        <button
            class=classes
            disabled=move || is_disabled()
            aria-busy=move || loading
            title=title
            aria-label=aria_label
            aria-haspopup=aria_haspopup
            aria-expanded=move || aria_expanded.map(|s| s.get()).unwrap_or(false)
            on:click=move |ev| {
                if let Some(cb) = on_click.as_ref() {
                    cb.run(ev);
                }
            }
        >
            {move || if loading {
                view! { <span class="btn-spinner" aria-hidden="true"></span> }.into_any()
            } else {
                view! {}.into_any()
            }}
            {children()}
        </button>
    }
}

/// Convenience icon-only button (ghost + square).
#[component]
pub fn IconButton(
    /// Tooltip (also the accessible name for icon-only buttons).
    title: &'static str,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Disables interaction.
    #[prop(optional)]
    disabled: bool,
    /// Click handler.
    #[prop(optional)]
    on_click: Option<Callback<web_sys::MouseEvent>>,
    children: ChildrenFn,
) -> impl IntoView {
    view! {
        <Button
            variant=ButtonVariant::Icon
            class=class.unwrap_or("")
            title=title
            disabled=disabled
            on_click=on_click.unwrap_or_else(|| Callback::new(|_| {}))
            aria_label=title
        >
            {children()}
        </Button>
    }
}
