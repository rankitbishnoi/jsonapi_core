use leptos::prelude::*;

use crate::api::ApiClient;
use crate::components::JsonView;

#[component]
pub fn ReadPage() -> impl IntoView {
    let client = use_context::<ApiClient>().expect("ApiClient in context");
    let body = RwSignal::new(String::new());
    let titles = RwSignal::new(Vec::<String>::new());

    let get_one =
        Action::new_local(move |_: &()| async move { client.get("/articles/art-01").await.body });
    let get_list = Action::new_local(move |_: &()| async move {
        client
            .list_article_titles("/articles?page[number]=1&page[size]=5")
            .await
    });

    Effect::new(move |_| {
        if let Some(b) = get_one.value().get() {
            body.set(b);
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(t)) = get_list.value().get() {
            titles.set(t);
        }
    });

    let response: Signal<String> = body.into();
    view! {
        <h1>"Read"</h1>
        <p>"Single resource with self links, and a typed list parsed via jsonapi_core."</p>
        <button on:click=move |_| { get_one.dispatch_local(()); }>"GET /articles/art-01"</button>
        " "
        <button on:click=move |_| { get_list.dispatch_local(()); }>"GET /articles (typed)"</button>
        <JsonView title="Single response".to_string() body=response/>
        <p class="caption">"Parsed titles (Document<ArticleResource> into_many)"</p>
        <ul>
            {move || titles.get().into_iter().map(|t| view! { <li>{t}</li> }).collect_view()}
        </ul>
    }
}
