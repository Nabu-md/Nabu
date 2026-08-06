//! Button primitives — 6 variants, 3 sizes, loading and icon modes.

use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonVariant {
    Primary,
    #[default]
    Secondary,
    Ghost,
    Outline,
    Destructive,
    Icon,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonSize {
    Sm,
    #[default]
    Md,
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

#[component]
pub fn Button(
    #[props(optional)] variant: ButtonVariant,
    #[props(optional)] size: ButtonSize,
    #[props(optional)] class: Option<&'static str>,
    #[props(optional)] disabled: bool,
    #[props(optional)] loading: bool,
    #[props(optional)] title: Option<&'static str>,
    #[props(optional)] on_click: Option<EventHandler<MouseEvent>>,
    #[props(optional)] aria_label: Option<&'static str>,
    #[props(optional)] aria_haspopup: Option<&'static str>,
    #[props(optional)] aria_expanded: Option<Signal<bool>>,
    children: Element,
) -> Element {
    let classes = button_classes(variant, size, class);
    let is_disabled = disabled || loading;
    let on_click_cb = on_click;
    rsx! {
        button {
            class: classes,
            disabled: is_disabled,
            "aria-busy": loading,
            title: title,
            "aria-label": aria_label,
            "aria-haspopup": aria_haspopup,
            "aria-expanded": if let Some(sig) = aria_expanded {
                "{*sig.read()}"
            } else {
                ""
            },
            onclick: move |ev: MouseEvent| {
                if !is_disabled {
                    if let Some(cb) = on_click_cb.as_ref() {
                        cb.call(ev);
                    }
                }
            },
            if loading {
                span { class: "btn-spinner", "aria-hidden": "true" }
            }
            {children}
        }
    }
}

#[component]
pub fn IconButton(
    title: &'static str,
    #[props(optional)] class: Option<&'static str>,
    #[props(optional)] disabled: bool,
    #[props(optional)] on_click: Option<EventHandler<MouseEvent>>,
    children: Element,
) -> Element {
    rsx! {
        Button {
            variant: ButtonVariant::Icon,
            class: class.unwrap_or(""),
            title: title,
            disabled: disabled,
            on_click: on_click,
            aria_label: Some(title),
            {children}
        }
    }
}
