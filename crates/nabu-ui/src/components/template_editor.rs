use yew::prelude::*;
use crate::models::template::Template;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub template: Template,
    pub on_save: Callback<Template>,
}

#[function_component(TemplateEditor)]
pub fn template_editor(props: &Props) -> Html {
    let template = use_state(|| props.template.clone());

    let on_save = {
        let template = template.clone();
        props.on_save.reform(move |_| (*template).clone())
    };

    html! {
        <div class="template-editor">
            <input type="text" value={template.name.clone()} />
            <textarea value={template.body.clone()} />
            <button onclick={on_save}>{ "Save Template" }</button>
        </div>
    }
}
