use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::api::ApiClient;
use crate::components::Nav;
use crate::inspector::{InspectorPanel, InspectorStore};
use crate::pages::{
    CreateEditPage, IncludesPage, PaginationPage, ReadPage, SortFilterPage, SparsePage,
};

#[component]
pub fn App() -> impl IntoView {
    let store = InspectorStore::new();
    provide_context(store);
    provide_context(ApiClient::new(store));

    view! {
        <Router>
            <div class="app">
                <Nav/>
                <main class="demo">
                    <Routes fallback=|| view! { <p>"Not found"</p> }>
                        <Route path=path!("/") view=ReadPage/>
                        <Route path=path!("/pagination") view=PaginationPage/>
                        <Route path=path!("/includes") view=IncludesPage/>
                        <Route path=path!("/sparse") view=SparsePage/>
                        <Route path=path!("/sort-filter") view=SortFilterPage/>
                        <Route path=path!("/create-edit") view=CreateEditPage/>
                    </Routes>
                </main>
                <InspectorPanel/>
            </div>
        </Router>
    }
}
