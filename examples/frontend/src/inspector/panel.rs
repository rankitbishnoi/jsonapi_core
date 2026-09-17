use leptos::prelude::*;

use super::model::{Exchange, Header};
use super::store::InspectorStore;

fn status_class(status: u16) -> &'static str {
    match status / 100 {
        2 => "status-2",
        4 => "status-4",
        5 => "status-5",
        _ => "",
    }
}

fn headers_block(title: &str, headers: &[Header]) -> String {
    let mut out = format!("{title}\n");
    for h in headers {
        out.push_str(&format!("  {}: {}\n", h.name, h.value));
    }
    out
}

#[component]
pub fn InspectorPanel() -> impl IntoView {
    let store = use_context::<InspectorStore>().expect("InspectorStore in context");

    let rows = move || {
        let selected_id = store.selected_id();
        store
            .entries()
            .into_iter()
            .map(move |ex| {
                let selected = selected_id == Some(ex.id);
                let cls = if selected {
                    "inspector-row selected"
                } else {
                    "inspector-row"
                };
                let sc = status_class(ex.status);
                let path = ex.url.clone();
                let id = ex.id;
                view! {
                    <button type="button" class=cls on:click=move |_| store.select(id)>
                        <span>{ex.method.clone()}</span>
                        <span class=sc>{ex.status}</span>
                        <span>{format!("{:.0}ms", ex.duration_ms)}</span>
                        <span>{path}</span>
                    </button>
                }
            })
            .collect_view()
    };

    let detail = move || match store.selected() {
        None => view! { <pre>"Fire a demo action to capture a request."</pre> }.into_any(),
        Some(ex) => detail_view(ex).into_any(),
    };

    view! {
        <aside class="inspector">
            <div class="inspector-list">{rows}</div>
            <div class="inspector-detail">{detail}</div>
        </aside>
    }
}

fn detail_view(ex: Exchange) -> impl IntoView {
    let req_id = ex.request_id.clone().unwrap_or_else(|| "—".into());
    let req = format!(
        "{} {}\n\n{}{}",
        ex.method,
        ex.url,
        headers_block("request headers:", &ex.req_headers),
        ex.req_body
            .as_deref()
            .map(|b| format!("\nrequest body:\n{b}"))
            .unwrap_or_default(),
    );
    let resp = format!(
        "status: {}   duration: {:.1}ms   x-request-id: {}\n\n{}{}",
        ex.status,
        ex.duration_ms,
        req_id,
        headers_block("response headers:", &ex.resp_headers),
        ex.resp_body
            .as_deref()
            .map(|b| format!("\nresponse body:\n{b}"))
            .unwrap_or_default(),
    );
    view! {
        <pre>{req}</pre>
        <pre>{resp}</pre>
    }
}
