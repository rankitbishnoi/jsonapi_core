use leptos::prelude::*;
use serde_json::json;

use crate::api::ApiClient;
use crate::components::JsonView;

const ATOMIC_MEDIA: &str = "application/vnd.api+json; ext=\"https://jsonapi.org/ext/atomic\"";

#[component]
pub fn AtomicPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());
    let author_name = RwSignal::new("Grace Hopper".to_string());
    let author_email = RwSignal::new("grace@example.com".to_string());
    let title = RwSignal::new("Atomic-authored article".to_string());

    let run = Action::new_local(move |payload: &String| {
        let payload = payload.clone();
        async move {
            client
                .send_raw(
                    "POST",
                    "/operations",
                    ATOMIC_MEDIA,
                    ATOMIC_MEDIA,
                    Some(payload),
                )
                .await
                .body
        }
    });
    Effect::new(move |_| {
        if let Some(b) = run.value().get() {
            body.set(b);
        }
    });

    let compose = move |_| {
        let doc = json!({
            "atomic:operations": [
                {
                    "op": "add",
                    "data": {
                        "type": "authors",
                        "lid": "new-author",
                        "attributes": { "name": author_name.get(), "email": author_email.get() }
                    }
                },
                {
                    "op": "add",
                    "data": {
                        "type": "articles",
                        "attributes": { "title": title.get(), "body": "Created atomically." },
                        "relationships": {
                            "author": { "data": { "type": "authors", "lid": "new-author" } }
                        }
                    }
                }
            ]
        });
        run.dispatch_local(doc.to_string());
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Atomic Operations"</h1>
        <p>"One transactional request: create an author, then an article that references it by "<code>"lid"</code>". All-or-nothing; the server resolves the lid to the new author id."</p>
        <label>
            "author name"
            <input prop:value=move || author_name.get() on:input=move |e| author_name.set(event_target_value(&e))/>
        </label>
        <label>
            "author email"
            <input prop:value=move || author_email.get() on:input=move |e| author_email.set(event_target_value(&e))/>
        </label>
        <label>
            "article title"
            <input prop:value=move || title.get() on:input=move |e| title.set(event_target_value(&e))/>
        </label>
        <button on:click=compose>"POST /operations (2 ops, lid ref)"</button>
        <JsonView title="Response".to_string() body=response/>
    }
}
