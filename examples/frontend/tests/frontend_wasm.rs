#![cfg(target_arch = "wasm32")]

use jsonapi_core::{DocumentBuilder, Field};
use jsonapi_showcase_frontend::api::capture::{pretty_body, request_id};
use jsonapi_showcase_frontend::inspector::model::{ExchangeInit, Header};
use jsonapi_showcase_frontend::inspector::store::InspectorStore;
use jsonapi_showcase_resources::ArticlePatchResource;
use wasm_bindgen_test::*;

fn init(method: &str) -> ExchangeInit {
    let resp_headers = vec![Header {
        name: "x-request-id".into(),
        value: "req-1".into(),
    }];
    ExchangeInit {
        method: method.into(),
        url: "http://127.0.0.1:8080/articles".into(),
        req_headers: vec![Header {
            name: "accept".into(),
            value: "application/vnd.api+json".into(),
        }],
        req_body: None,
        status: 200,
        // Derive request_id the same way the client does — from the response
        // headers — so the store test is a genuine round-trip, not a field copy.
        request_id: request_id(&resp_headers),
        resp_headers,
        resp_body: Some("{}".into()),
        duration_ms: 3.0,
    }
}

#[wasm_bindgen_test]
fn store_records_and_selects_newest() {
    let owner = leptos::reactive::owner::Owner::new();
    owner.with(|| {
        let store = InspectorStore::new();
        let _first = store.record(init("GET"));
        let second = store.record(init("POST"));
        assert_eq!(store.entries().len(), 2);
        assert_eq!(store.entries()[0].method, "POST", "newest first");
        assert_eq!(store.selected_id(), Some(second));
        // request_id was extracted from the response headers by `request_id`,
        // then carried through record -> selected.
        assert_eq!(
            store.selected().unwrap().request_id.as_deref(),
            Some("req-1")
        );
    });
}

#[wasm_bindgen_test]
fn capture_helpers_work_in_wasm() {
    assert!(pretty_body(r#"{"data":1}"#).unwrap().contains('\n'));
    assert_eq!(pretty_body("   "), None);
    let headers = vec![Header {
        name: "X-Request-Id".into(),
        value: "abc".into(),
    }];
    assert_eq!(request_id(&headers).as_deref(), Some("abc"));
}

// The create/edit page dogfoods the derive to build PATCH bodies: a checked
// field is `Field::Set`, an unchecked field is `Field::Absent`. The derived
// `Serialize` must omit Absent members entirely so the wire document is a true
// partial update. This asserts that exact tri-state behaviour in wasm.
#[wasm_bindgen_test]
fn typed_patch_omits_absent_fields() {
    let patch = ArticlePatchResource {
        id: "art-01".into(),
        title: Field::Set("Edited".into()),
        body: Field::Absent,
    };
    let doc = DocumentBuilder::single(patch).build();
    let json = serde_json::to_value(&doc).expect("serialize typed patch");
    let attrs = &json["data"]["attributes"];
    assert_eq!(json["data"]["type"], "articles");
    assert_eq!(json["data"]["id"], "art-01");
    assert_eq!(attrs["title"], "Edited", "Set member is present");
    assert!(
        attrs.get("body").is_none(),
        "Absent member must be omitted, got {attrs:?}"
    );
}
