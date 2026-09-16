use leptos::prelude::*;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <div class="app">
            <nav class="nav"><strong>"JSON:API Showcase"</strong></nav>
            <main class="demo"><h1>"Showcase"</h1><p>"Scaffold online."</p></main>
            <aside class="inspector"><div class="inspector-detail">"Inspector"</div></aside>
        </div>
    }
}
