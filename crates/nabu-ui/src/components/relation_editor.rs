use yew::prelude::*;
use uuid::Uuid;
use crate::models::graph::RelationType;
use nabu_core::models::knowledge_object::KnowledgeObject;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub object: KnowledgeObject,
    // Add additional necessary props like existing relations, etc.
}

#[function_component(RelationEditor)]
pub fn relation_editor(props: &Props) -> Html {
    let selected_relation = use_state(|| RelationType::RelatedTo);

    html! {
        <div class="relation-editor">
            <h3>{ format!("Relations for {}", props.object.metadata.title.clone().unwrap_or_default()) }</h3>
            <div class="add-relation">
                <input type="text" placeholder="Search entities..." />
                <select>
                    // Render relation types dynamically
                </select>
                <button>{ "Add Relation" }</button>
            </div>
            <div class="existing-relations">
                // Render existing relations
            </div>
        </div>
    }
}
