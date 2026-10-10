//! Validation message shown under an input.

use leptos::prelude::*;

/// `message` is a closure so the text updates reactively as the form's error
/// state changes; renders nothing when there is no message.
#[component]
pub fn FieldError(#[prop(into)] message: Signal<Option<String>>) -> impl IntoView {
    view! {
        {move || {
            message
                .get()
                .map(|text| {
                    view! { <small role="alert" class="error">{text}</small> }
                })
        }}
    }
}
