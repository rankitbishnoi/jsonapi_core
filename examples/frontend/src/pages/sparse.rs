use leptos::prelude::*;

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn SparsePage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());
    let title = RwSignal::new(true);
    let body_field = RwSignal::new(false);
    let author_name = RwSignal::new(true);

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
        let mut article_fields = Vec::new();
        if title.get() {
            article_fields.push("title");
        }
        if body_field.get() {
            article_fields.push("body");
        }
        let mut path = String::from("/articles/art-01?include=author");
        if !article_fields.is_empty() {
            path.push_str(&format!("&fields[articles]={}", article_fields.join(",")));
        }
        if author_name.get() {
            path.push_str("&fields[authors]=name");
        }
        fetch.dispatch_local(path);
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Sparse fieldsets"</h1>
        <p>"Toggle which attributes each type returns; the payload shrinks accordingly."</p>
        <p class="caption">"fields[articles]"</p>
        <label>
            <input type="checkbox" prop:checked=move || title.get()
                on:change=move |e| title.set(event_target_checked(&e))/>
            "title"
        </label>
        <label>
            <input type="checkbox" prop:checked=move || body_field.get()
                on:change=move |e| body_field.set(event_target_checked(&e))/>
            "body"
        </label>
        <p class="caption">"fields[authors]"</p>
        <label>
            <input type="checkbox" prop:checked=move || author_name.get()
                on:change=move |e| author_name.set(event_target_checked(&e))/>
            "name"
        </label>
        <button on:click=on_go>"GET with fields[…]"</button>
        <JsonView title="Response".to_string() body=response/>
    }
}
