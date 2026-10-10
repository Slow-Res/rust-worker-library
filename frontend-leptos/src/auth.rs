//! Current-user state shared with the whole app through Leptos context.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::Outlet;
use leptos_router::hooks::use_location;
use library_shared::CurrentUser;

use crate::api;
use crate::url::build_url;

/// Who is logged in. `RwSignal` is a reactive cell: reading it with `.get()`
/// inside a view closure re-renders that part of the page when it changes.
/// It is `Copy`, so it can be moved into any closure without cloning.
#[derive(Clone, Copy)]
pub struct AuthContext {
    pub user: RwSignal<Option<CurrentUser>>,
    /// False until the startup `/api/auth/me` check finishes, so protected
    /// pages do not flash a redirect to `/login` on refresh.
    pub ready: RwSignal<bool>,
}

/// Reads the [`AuthContext`] provided by [`AuthProvider`].
pub fn use_auth() -> AuthContext {
    // `expect_context` panics if no provider is above us; `App` always mounts
    // one, so this is a startup invariant rather than a runtime failure.
    expect_context::<AuthContext>()
}

/// Provides [`AuthContext`] to its children (Leptos context = typed
/// dependency injection down the component tree) and loads the session.
#[component]
pub fn AuthProvider(children: Children) -> impl IntoView {
    let auth = AuthContext {
        user: RwSignal::new(None),
        ready: RwSignal::new(false),
    };
    provide_context(auth);

    // Any 401 from the API means the session is gone.
    api::set_unauthorized_handler(move || auth.user.set(None));

    // `spawn_local` runs a (non-`Send`) future on the browser's event loop.
    spawn_local(async move {
        // A 401 here just means "not logged in", which is not an error.
        auth.user.set(api::auth::me().await.ok());
        auth.ready.set(true);
    });

    children()
}

/// Layout route for protected pages: renders the matched child route via
/// `<Outlet/>` when logged in, otherwise redirects to `/login?next=<here>`.
#[component]
pub fn RequireAuth() -> impl IntoView {
    let auth = use_auth();
    let location = use_location();

    view! {
        {move || {
            if !auth.ready.get() {
                view! { <p aria-busy="true">"Loading…"</p> }.into_any()
            } else if auth.user.get().is_some() {
                view! { <Outlet/> }.into_any()
            } else {
                let here = format!("{}{}", location.pathname.get(), location.search.get());
                let to = build_url("/login", &[("next", &here)]);
                view! { <leptos_router::components::Redirect path=to/> }.into_any()
            }
        }}
    }
}
