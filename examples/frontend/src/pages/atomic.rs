use std::collections::BTreeMap;

use leptos::prelude::*;

use jsonapi_core::atomic::{AtomicOperation, AtomicRequest, OperationTarget};
use jsonapi_core::{
    Identity, PrimaryData, RelationshipData, Resource, ResourceIdentifier, ResourceRelationship,
};

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
        // Dogfood the typed atomic-ops model: build `AtomicRequest` from typed
        // operations (the `op` discriminant is compile-checked), pre-flight it
        // with `validate_lid_refs`, then serialize.
        let author = Resource {
            r#type: "authors".into(),
            id: None,
            lid: Some("new-author".into()),
            attributes: serde_json::json!({
                "name": author_name.get(),
                "email": author_email.get(),
            }),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        };

        let mut article_rels = BTreeMap::new();
        article_rels.insert(
            "author".into(),
            ResourceRelationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
                r#type: "authors".into(),
                identity: Identity::Lid("new-author".into()),
                meta: None,
            }))),
        );
        let article = Resource {
            r#type: "articles".into(),
            id: None,
            lid: None,
            attributes: serde_json::json!({
                "title": title.get(),
                "body": "Created atomically.",
            }),
            relationships: article_rels,
            links: None,
            meta: None,
        };

        let req = AtomicRequest {
            operations: vec![
                AtomicOperation::Add {
                    target: OperationTarget::default(),
                    data: PrimaryData::Single(Box::new(author)),
                },
                AtomicOperation::Add {
                    target: OperationTarget::default(),
                    data: PrimaryData::Single(Box::new(article)),
                },
            ],
        };

        // Client-side pre-flight: reject inconsistent lid references before the
        // network round-trip (no exchange is recorded when this fails).
        if let Err(e) = req.validate_lid_refs() {
            body.set(format!("client-side lid validation failed: {e}"));
            return;
        }
        let payload = serde_json::to_string(&req).unwrap_or_default();
        run.dispatch_local(payload);
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
