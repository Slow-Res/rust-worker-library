//! `/login`: username/password form.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_query_map};
use library_shared::LoginRequest;

use crate::api::{self, ApiRequestError};
use crate::auth::use_auth;
use crate::components::error_message::ErrorMessage;
use crate::components::field_error::FieldError;
use crate::url::safe_next;

#[component]
pub fn Login() -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();
    let query = use_query_map();

    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<ApiRequestError>);
    let busy = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        // Stop the browser from doing a full-page form submission.
        ev.prevent_default();
        busy.set(true);
        error.set(None);
        let request = LoginRequest {
            username: username.get_untracked(),
            password: password.get_untracked(),
        };
        let next = safe_next(query.read_untracked().get("next").as_deref());
        let navigate = navigate.clone();
        spawn_local(async move {
            let result = api::auth::login(&request).await;
            // Reset `busy` before navigating: after `navigate` this page's
            // reactive owner may already be disposed.
            busy.set(false);
            match result {
                Ok(user) => {
                    auth.user.set(Some(user));
                    navigate(&next, Default::default());
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let field =
        move |name: &'static str| Signal::derive(move || error.get().and_then(|e| e.field(name)));
    let invalid = move |name: &'static str| field(name).get().is_some().then_some("true");

    view! {
        <h1>"Log in"</h1>
        // `novalidate` turns off the browser's own validation bubbles so the
        // server's field errors (shown by <FieldError>) are what users see.
        <form novalidate on:submit=on_submit>
            <label>
                "Username"
                <input
                    autocomplete="username"
                    aria-invalid=move || invalid("username")
                    prop:value=move || username.get()
                    on:input=move |ev| username.set(event_target_value(&ev))
                />
                <FieldError message=field("username")/>
            </label>
            <label>
                "Password"
                <input
                    type="password"
                    autocomplete="current-password"
                    aria-invalid=move || invalid("password")
                    prop:value=move || password.get()
                    on:input=move |ev| password.set(event_target_value(&ev))
                />
                <FieldError message=field("password")/>
            </label>
            {move || {
                // Field errors are shown inline; only show the banner otherwise.
                error
                    .get()
                    .filter(|e| e.fields.is_empty())
                    .map(|e| view! { <ErrorMessage error=e/> })
            }}
            <button type="submit" aria-busy=move || busy.get().then_some("true") disabled=move || busy.get()>
                "Log in"
            </button>
        </form>
    }
}
