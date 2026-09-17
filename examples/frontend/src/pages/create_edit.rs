use leptos::prelude::*;

use jsonapi_core::{DocumentBuilder, Field, Relationship};
use jsonapi_showcase_resources::{ArticlePatchResource, NewArticleResource};

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn CreateEditPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());

    let new_title = RwSignal::new("A brand new article".to_string());
    let new_body = RwSignal::new("Body text.".to_string());
    let new_author = RwSignal::new("a1".to_string());

    let patch_id = RwSignal::new("art-01".to_string());
    let send_title = RwSignal::new(true);
    let patch_title = RwSignal::new("Edited title".to_string());
    let send_body = RwSignal::new(false);
    let patch_body = RwSignal::new(String::new());

    let post = Action::new_local(move |payload: &String| {
        let payload = payload.clone();
        async move { client.send("POST", "/articles", Some(payload)).await.body }
    });
    let patch = Action::new_local(move |arg: &(String, String)| {
        let (path, payload) = arg.clone();
        async move { client.send("PATCH", &path, Some(payload)).await.body }
    });
    Effect::new(move |_| {
        if let Some(b) = post.value().get() {
            body.set(b);
        }
    });
    Effect::new(move |_| {
        if let Some(b) = patch.value().get() {
            body.set(b);
        }
    });

    let on_create = move |_| {
        // Dogfood the typed write resource: `#[derive(JsonApi)]` serializes it
        // into a proper JSON:API document (type + attributes + relationships).
        let new = NewArticleResource {
            id: None,
            title: new_title.get(),
            body: new_body.get(),
            author: Relationship::to_one_id("authors", new_author.get()),
        };
        let doc = DocumentBuilder::single(new).build();
        let payload = serde_json::to_string(&doc).unwrap_or_default();
        post.dispatch_local(payload);
    };

    let on_patch = move |_| {
        // Field<T> tri-state: Set when the box is checked, Absent otherwise. The
        // derive omits Absent members from the serialized `attributes`, emits
        // `null` for Null, and the value for Set — exactly PATCH semantics.
        let patch_res = ArticlePatchResource {
            id: patch_id.get(),
            title: if send_title.get() {
                Field::Set(patch_title.get())
            } else {
                Field::Absent
            },
            body: if send_body.get() {
                Field::Set(patch_body.get())
            } else {
                Field::Absent
            },
        };
        // Raw id in the body; percent-encoded when placed in the URL path.
        let id_seg = js_sys::encode_uri_component(&patch_res.id)
            .as_string()
            .unwrap_or_default();
        let doc = DocumentBuilder::single(patch_res).build();
        let payload = serde_json::to_string(&doc).unwrap_or_default();
        patch.dispatch_local((format!("/articles/{id_seg}"), payload));
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Create / edit"</h1>

        <h3>"Create (POST /articles → 201 + Location)"</h3>
        <label>
            "title"
            <input prop:value=move || new_title.get() on:input=move |e| new_title.set(event_target_value(&e))/>
        </label>
        <label>
            "body"
            <input prop:value=move || new_body.get() on:input=move |e| new_body.set(event_target_value(&e))/>
        </label>
        <label>
            "author id"
            <input prop:value=move || new_author.get() on:input=move |e| new_author.set(event_target_value(&e))/>
        </label>
        <button on:click=on_create>"POST /articles"</button>

        <h3>"Edit (PATCH — Field<T> tri-state)"</h3>
        <p>"Only checked fields are sent; unchecked fields stay absent (unchanged)."</p>
        <label>
            "article id"
            <input prop:value=move || patch_id.get() on:input=move |e| patch_id.set(event_target_value(&e))/>
        </label>
        <label>
            <input type="checkbox" prop:checked=move || send_title.get()
                on:change=move |e| send_title.set(event_target_checked(&e))/>
            "send title"
        </label>
        <label>
            "title value"
            <input prop:value=move || patch_title.get() on:input=move |e| patch_title.set(event_target_value(&e))/>
        </label>
        <label>
            <input type="checkbox" prop:checked=move || send_body.get()
                on:change=move |e| send_body.set(event_target_checked(&e))/>
            "send body"
        </label>
        <label>
            "body value"
            <input prop:value=move || patch_body.get() on:input=move |e| patch_body.set(event_target_value(&e))/>
        </label>
        <button on:click=on_patch>"PATCH /articles/:id"</button>

        <JsonView title="Response".to_string() body=response/>
    }
}
