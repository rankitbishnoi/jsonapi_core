use leptos::prelude::*;

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn IncludesPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());
    let author = RwSignal::new(true);
    let comments_author = RwSignal::new(false);
    let tags = RwSignal::new(false);

    let fetch = Action::new_local(move |path: &String| {
        let path = path.clone();
        async move { client.get(&path).await.body }
    });
    Effect::new(move |_| {
        if let Some(b) = fetch.value().get() {
            body.set(b);
        }
    });

    let on_go = move |_| {
        let mut parts = Vec::new();
        if author.get() {
            parts.push("author");
        }
        if comments_author.get() {
            parts.push("comments.author");
        }
        if tags.get() {
            parts.push("tags");
        }
        let path = if parts.is_empty() {
            "/articles/art-01".to_string()
        } else {
            format!("/articles/art-01?include={}", parts.join(","))
        };
        fetch.dispatch_local(path);
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Includes (compound documents)"</h1>
        <p>"Transitive includes resolved N+1-safe on the server; watch "<code>"included"</code>" grow."</p>
        <label>
            <input type="checkbox" prop:checked=move || author.get()
                on:change=move |e| author.set(event_target_checked(&e))/>
            "author"
        </label>
        <label>
            <input type="checkbox" prop:checked=move || comments_author.get()
                on:change=move |e| comments_author.set(event_target_checked(&e))/>
            "comments.author (transitive)"
        </label>
        <label>
            <input type="checkbox" prop:checked=move || tags.get()
                on:change=move |e| tags.set(event_target_checked(&e))/>
            "tags"
        </label>
        <button on:click=on_go>"GET /articles/art-01?include=…"</button>
        <JsonView title="Response".to_string() body=response/>
    }
}
