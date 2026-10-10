//! Fallback for unknown routes and invalid ids.

use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <h1>"Not found"</h1>
        <p>"The page you are looking for does not exist."</p>
        <A href="/books">"Back to books"</A>
    }
}
