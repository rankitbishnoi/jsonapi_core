use leptos::prelude::*;
use serde_json::json;

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn RelationshipsPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());
    let article = RwSignal::new("art-01".to_string());
    let new_author = RwSignal::new("a2".to_string());
    let tag = RwSignal::new("t-rust".to_string());

    let call = Action::new_local(move |arg: &(String, String, Option<String>)| {
        let (method, path, payload) = arg.clone();
        async move { client.send(&method, &path, payload).await.body }
    });
    Effect::new(move |_| {
        if let Some(b) = call.value().get() {
            body.set(b);
        }
    });

    // Encode the free-text article id before putting it in the URL path.
    let article_seg = move || {
        js_sys::encode_uri_component(&article.get())
            .as_string()
            .unwrap_or_default()
    };

    let get_tags = move |_| {
        let path = format!("/articles/{}/relationships/tags", article_seg());
        call.dispatch_local(("GET".into(), path, None));
    };
    let patch_author = move |_| {
        let path = format!("/articles/{}/relationships/author", article_seg());
        let doc = json!({ "data": { "type": "authors", "id": new_author.get() } });
        call.dispatch_local(("PATCH".into(), path, Some(doc.to_string())));
    };
    let add_tag = move |_| {
        let path = format!("/articles/{}/relationships/tags", article_seg());
        let doc = json!({ "data": [{ "type": "tags", "id": tag.get() }] });
        call.dispatch_local(("POST".into(), path, Some(doc.to_string())));
    };
    let remove_tag = move |_| {
        let path = format!("/articles/{}/relationships/tags", article_seg());
        let doc = json!({ "data": [{ "type": "tags", "id": tag.get() }] });
        call.dispatch_local(("DELETE".into(), path, Some(doc.to_string())));
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Relationships"</h1>
        <label>
            "article id"
            <input prop:value=move || article.get() on:input=move |e| article.set(event_target_value(&e))/>
        </label>

        <h3>"To-one: author"</h3>
        <label>
            "new author id"
            <input prop:value=move || new_author.get() on:input=move |e| new_author.set(event_target_value(&e))/>
        </label>
        <button on:click=patch_author>"PATCH …/relationships/author"</button>

        <h3>"To-many: tags"</h3>
        <label>
            "tag id"
            <input prop:value=move || tag.get() on:input=move |e| tag.set(event_target_value(&e))/>
        </label>
        <button on:click=get_tags>"GET tags"</button>" "
        <button on:click=add_tag>"POST tag"</button>" "
        <button on:click=remove_tag>"DELETE tag"</button>

        <JsonView title="Response".to_string() body=response/>
    }
}
