//! `/members/new` and `/members/:id/edit`: the same form for both.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_params_map};
use library_shared::{Member, MemberInput};

use crate::api::{self, ApiRequestError, ApiResult};
use crate::components::error_message::ErrorMessage;
use crate::components::field_error::FieldError;
use crate::pages::not_found::NotFound;
use crate::url::parse_id;

#[component]
pub fn MemberNew() -> impl IntoView {
    view! {
        <h1>"Add member"</h1>
        <MemberForm
            initial=MemberInput { name: String::new(), email: String::new() }
            save=|input| async move { api::members::create(&input).await.map(|_| ()) }
        />
    }
}

#[component]
pub fn MemberEdit() -> impl IntoView {
    let params = use_params_map();
    let Some(id) = parse_id(params.read_untracked().get("id").as_deref()) else {
        return view! { <NotFound/> }.into_any();
    };
    let member = LocalResource::new(move || async move { api::members::get(id).await });

    view! {
        <h1>"Edit member"</h1>
        <Suspense fallback=|| view! { <p aria-busy="true">"Loading…"</p> }>
            {move || {
                member
                    .get()
                    .map(|result| match result {
                        Err(e) if e.status == 404 => view! { <NotFound/> }.into_any(),
                        Err(e) => view! { <ErrorMessage error=e/> }.into_any(),
                        Ok(Member { name, email, .. }) => {
                            view! {
                                <MemberForm
                                    initial=MemberInput { name, email }
                                    save=move |input| async move {
                                        api::members::update(id, &input).await.map(|_| ())
                                    }
                                />
                            }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
    .into_any()
}

/// Same shape as the book form: `save` does the API call, errors come back
/// as `ApiRequestError`, and success returns to the list.
#[component]
fn MemberForm<F, Fut>(initial: MemberInput, save: F) -> impl IntoView
where
    F: Fn(MemberInput) -> Fut + Clone + 'static,
    Fut: std::future::Future<Output = ApiResult<()>> + 'static,
{
    let navigate = use_navigate();
    let name = RwSignal::new(initial.name);
    let email = RwSignal::new(initial.email);
    let error = RwSignal::new(None::<ApiRequestError>);
    let busy = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        error.set(None);
        busy.set(true);
        let input = MemberInput {
            name: name.get_untracked(),
            email: email.get_untracked(),
        };
        let (save, navigate) = (save.clone(), navigate.clone());
        spawn_local(async move {
            match save(input).await {
                Ok(()) => navigate("/members", Default::default()),
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    let field =
        move |name: &'static str| Signal::derive(move || error.get().and_then(|e| e.field(name)));
    let invalid = move |name: &'static str| field(name).get().is_some().then_some("true");

    view! {
        <form on:submit=on_submit>
            <label>
                "Name"
                <input
                    aria-invalid=move || invalid("name")
                    prop:value=move || name.get()
                    on:input=move |ev| name.set(event_target_value(&ev))
                />
                <FieldError message=field("name")/>
            </label>
            <label>
                "Email"
                <input
                    type="email"
                    aria-invalid=move || invalid("email")
                    prop:value=move || email.get()
                    on:input=move |ev| email.set(event_target_value(&ev))
                />
                <FieldError message=field("email")/>
            </label>
            {move || {
                error
                    .get()
                    .filter(|e| e.fields.is_empty())
                    .map(|e| view! { <ErrorMessage error=e/> })
            }}
            <div class="actions">
                <button type="submit" aria-busy=move || busy.get().then_some("true") disabled=move || busy.get()>
                    "Save"
                </button>
                <a href="/members" role="button" class="secondary outline">
                    "Cancel"
                </a>
            </div>
        </form>
    }
}
