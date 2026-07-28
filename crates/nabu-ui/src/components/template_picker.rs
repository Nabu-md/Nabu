use yew::prelude::*;
use crate::models::template::Template;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub templates: Vec<Template>,
    pub on_select: Callback<Template>,
}

#[function_component(TemplatePicker)]
pub fn template_picker(props: &Props) -> Html {
    let search = use_state(|| String::new());

    let on_input = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            search.set(input.value());
        })
    };

    html! {
        <div class="template-picker">
            <input type="text" placeholder="Search templates..." oninput={on_input} />
            <div class="template-list">
                { for props.templates.iter().filter(|t| t.name.contains(&*search)).map(|t| {
                    let template = t.clone();
                    let on_click = props.on_select.reform(move |_| template.clone());
                    html! {
                        <button onclick={on_click}>
                            { &t.name }
                        </button>
                    }
                })}
            </div>
        </div>
    }
}
