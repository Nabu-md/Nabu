use leptos::prelude::*;

#[component]
pub fn PdfViewer(#[prop(into)] pdf_path: String) -> impl IntoView {
    view! {
        <div class="pdf-viewer-container" style="width: 100%; height: 100vh;">
            <iframe 
                src=format!("file://{}", pdf_path)
                class="pdf-viewer-frame"
                style="width: 100%; height: 100%; border: none;"
                title="PDF Viewer"
            />
        </div>
    }
}
