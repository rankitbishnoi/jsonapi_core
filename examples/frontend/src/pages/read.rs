use leptos::prelude::*;

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn ReadPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let last = RwSignal::new(String::new());

    let load =
        Action::new_local(move |_: &()| async move { client.get("/articles/art-01").await.body });

    Effect::new(move |_| {
        if let Some(body) = load.value().get() {
            last.set(body);
        }
    });

    let body_signal: Signal<String> = last.into();

    view! {
        <h1>"Read"</h1>
        <button on:click=move |_| { load.dispatch_local(()); }>"GET /articles/art-01"</button>
        <JsonView title="Response".to_string() body=body_signal/>
    }
}
