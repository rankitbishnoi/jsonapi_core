use leptos::prelude::*;
use serde_json::json;

use crate::api::ApiClient;
use crate::components::JsonView;

const MEDIA: &str = "application/vnd.api+json";

/// A deliberately-malformed call: caller-chosen method/path/headers/body used to
/// provoke a specific JSON:API error status.
#[derive(Clone)]
struct RawCall {
    method: String,
    path: String,
    accept: String,
    content_type: String,
    body: Option<String>,
}

#[component]
pub fn ErrorsPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());

    let raw = Action::new_local(move |arg: &RawCall| {
        let arg = arg.clone();
        async move {
            client
                .send_raw(
                    &arg.method,
                    &arg.path,
                    &arg.accept,
                    &arg.content_type,
                    arg.body,
                )
                .await
                .body
        }
    });
    Effect::new(move |_| {
        if let Some(b) = raw.value().get() {
            body.set(b);
        }
    });

    let unsupported_media = move |_| {
        raw.dispatch_local(RawCall {
            method: "POST".into(),
            path: "/articles".into(),
            accept: MEDIA.into(),
            content_type: "application/json".into(),
            body: Some(json!({"data":{"type":"articles"}}).to_string()),
        });
    };
    let not_acceptable = move |_| {
        raw.dispatch_local(RawCall {
            method: "GET".into(),
            path: "/articles/art-01".into(),
            accept: "text/html".into(),
            content_type: MEDIA.into(),
            body: None,
        });
    };
    let not_found = move |_| {
        raw.dispatch_local(RawCall {
            method: "GET".into(),
            path: "/articles/does-not-exist".into(),
            accept: MEDIA.into(),
            content_type: MEDIA.into(),
            body: None,
        });
    };
    let unprocessable = move |_| {
        let doc = json!({
            "data": {
                "type": "articles",
                "attributes": { "title": "", "body": "" },
                "relationships": { "author": { "data": { "type": "authors", "id": "a1" } } }
            }
        });
        raw.dispatch_local(RawCall {
            method: "POST".into(),
            path: "/articles".into(),
            accept: MEDIA.into(),
            content_type: MEDIA.into(),
            body: Some(doc.to_string()),
        });
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Error model"</h1>
        <p>"Each button provokes a specific error; inspect the JSON:API "<code>"errors"</code>" array, pointers, and status."</p>
        <button on:click=unsupported_media>"415 Unsupported Media Type"</button>" "
        <button on:click=not_acceptable>"406 Not Acceptable"</button>" "
        <button on:click=not_found>"404 Not Found"</button>" "
        <button on:click=unprocessable>"422 (multi-error aggregation)"</button>
        <JsonView title="Response".to_string() body=response/>
    }
}
