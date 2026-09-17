use leptos::prelude::*;

/// Render a JSON string (already pretty-printed or raw) in a code block.
#[component]
pub fn JsonView(#[prop(into)] title: String, body: Signal<String>) -> impl IntoView {
    view! {
        <p class="caption">{title}</p>
        <pre>{move || body.get()}</pre>
    }
}
