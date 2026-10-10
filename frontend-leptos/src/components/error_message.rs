//! General (non-field) error banner.

use leptos::prelude::*;

use crate::api::ApiRequestError;

/// Shows `error.message` from the API (or a transport error).
#[component]
pub fn ErrorMessage(#[prop(into)] error: ApiRequestError) -> impl IntoView {
    view! {
        <p role="alert" class="error">{error.message}</p>
    }
}
