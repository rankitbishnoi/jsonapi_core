use leptos::prelude::*;

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn SortFilterPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());
    let sort = RwSignal::new("-created_at".to_string());
    let author = RwSignal::new(String::new());

    let fetch = Action::new_local(move |path: &String| {
        let path = path.clone();
        async move { client.get(&path).await.body }
    });
    Effect::new(move |_| {
        if let Some(b) = fetch.value().get() {
            body.set(b);
        }
    });

    let encode = |v: String| js_sys::encode_uri_component(&v).as_string().unwrap_or(v);
    let on_go = move |_| {
        let mut path = format!("/articles?sort={}", encode(sort.get()));
        let a = author.get();
        if !a.is_empty() {
            path.push_str(&format!("&filter[author]={}", encode(a)));
        }
        fetch.dispatch_local(path);
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Sort & filter"</h1>
        <p>"Whitelisted sort keys and a "<code>"filter[author]"</code>" equality filter."</p>
        <label>
            "sort (e.g. -created_at,title)"
            <input prop:value=move || sort.get() on:input=move |e| sort.set(event_target_value(&e))/>
        </label>
        <label>
            "filter[author] (author id, e.g. a1 — blank to omit)"
            <input prop:value=move || author.get() on:input=move |e| author.set(event_target_value(&e))/>
        </label>
        <button on:click=on_go>"GET /articles?sort=…&filter[author]=…"</button>
        <JsonView title="Response".to_string() body=response/>
    }
}
