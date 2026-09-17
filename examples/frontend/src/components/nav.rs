use leptos::prelude::*;
use leptos_router::components::A;

const LINKS: &[(&str, &str)] = &[
    ("/", "Read"),
    ("/pagination", "Pagination"),
    ("/includes", "Includes"),
    ("/sparse", "Sparse fields"),
    ("/sort-filter", "Sort & filter"),
    ("/create-edit", "Create / edit"),
    ("/relationships", "Relationships"),
    ("/errors", "Errors"),
    ("/atomic", "Atomic ops"),
];

#[component]
pub fn Nav() -> impl IntoView {
    view! {
        <nav class="nav">
            <strong>"JSON:API Showcase"</strong>
            {LINKS.iter().map(|(href, label)| view! {
                <A href=*href>{*label}</A>
            }).collect_view()}
        </nav>
    }
}
