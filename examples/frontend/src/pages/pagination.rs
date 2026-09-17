use leptos::prelude::*;

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn PaginationPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());

    let fetch = Action::new_local(move |path: &String| {
        let path = path.clone();
        async move { client.get(&path).await.body }
    });
    Effect::new(move |_| {
        if let Some(b) = fetch.value().get() {
            body.set(b);
        }
    });

    let number = RwSignal::new(1u32);
    let size = RwSignal::new(3u32);
    let offset = RwSignal::new(0u32);
    let limit = RwSignal::new(3u32);
    let cursor_size = RwSignal::new(3u32);
    let after = RwSignal::new(String::new());

    let go_number = move |_| {
        fetch.dispatch_local(format!(
            "/articles?page[number]={}&page[size]={}",
            number.get(),
            size.get()
        ));
    };
    let go_offset = move |_| {
        fetch.dispatch_local(format!(
            "/articles/offset?page[offset]={}&page[limit]={}",
            offset.get(),
            limit.get()
        ));
    };
    let go_cursor = move |_| {
        let mut path = format!("/articles/cursor?page[size]={}", cursor_size.get());
        let after_val = after.get();
        if !after_val.is_empty() {
            let encoded = js_sys::encode_uri_component(&after_val)
                .as_string()
                .unwrap_or(after_val);
            path.push_str(&format!("&page[after]={encoded}"));
        }
        fetch.dispatch_local(path);
    };

    let response: Signal<String> = body.into();
    view! {
        <h1>"Pagination"</h1>
        <p>"Three strategies; watch the "<code>"links"</code>" object in each response."</p>

        <h3>"Page-number ("<code>"/articles"</code>")"</h3>
        <label>
            "page[number]"
            <input type="number" min="1" prop:value=move || number.get().to_string()
                on:input=move |e| number.set(event_target_value(&e).parse().unwrap_or(1))/>
        </label>
        <label>
            "page[size]"
            <input type="number" min="1" prop:value=move || size.get().to_string()
                on:input=move |e| size.set(event_target_value(&e).parse().unwrap_or(3))/>
        </label>
        <button on:click=go_number>"Fetch page-number"</button>

        <h3>"Offset ("<code>"/articles/offset"</code>")"</h3>
        <label>
            "page[offset]"
            <input type="number" min="0" prop:value=move || offset.get().to_string()
                on:input=move |e| offset.set(event_target_value(&e).parse().unwrap_or(0))/>
        </label>
        <label>
            "page[limit]"
            <input type="number" min="1" prop:value=move || limit.get().to_string()
                on:input=move |e| limit.set(event_target_value(&e).parse().unwrap_or(3))/>
        </label>
        <button on:click=go_offset>"Fetch offset"</button>

        <h3>"Cursor ("<code>"/articles/cursor"</code>", ethanresnick profile)"</h3>
        <label>
            "page[size]"
            <input type="number" min="1" prop:value=move || cursor_size.get().to_string()
                on:input=move |e| cursor_size.set(event_target_value(&e).parse().unwrap_or(3))/>
        </label>
        <label>
            "page[after] (blank for first page; e.g. art-03)"
            <input prop:value=move || after.get()
                on:input=move |e| after.set(event_target_value(&e))/>
        </label>
        <button on:click=go_cursor>"Fetch cursor"</button>

        <JsonView title="Response".to_string() body=response/>
    }
}
