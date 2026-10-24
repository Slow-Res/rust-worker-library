//! Current-user state shared with the whole app through Leptos context.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::NavigateOptions;
use leptos_router::components::Outlet;
use leptos_router::hooks::{use_location, use_navigate};
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
    /// True while a `RequireAuth` guard is mounted (protected page showing).
    pub guard_mounted: RwSignal<bool>,
    /// Set by log out when the guard is mounted, so the guard lets the user
    /// disappear without redirecting to `/login`; the guard resets it.
    pub logging_out: RwSignal<bool>,
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
        guard_mounted: RwSignal::new(false),
        logging_out: RwSignal::new(false),
    };
    provide_context(auth);

    // Any 401 from the API means the session is gone: forget the user and, like
    // the React app, go to `/login?next=<here>` (also from public pages).
    // A mounted `RequireAuth` guard reacts to the cleared user by redirecting
    // itself, so navigating here as well would redirect twice.
    let navigate = use_navigate();
    let location = use_location();
    api::set_unauthorized_handler(move || {
        auth.user.set(None);
        if auth.guard_mounted.get_untracked() {
            return;
        }
        let pathname = location.pathname.get_untracked();
        if pathname.starts_with("/login") {
            return;
        }
        let to = login_redirect_target(&pathname, &location.search.get_untracked());
        navigate(
            &to,
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });

    // `spawn_local` runs a (non-`Send`) future on the browser's event loop.
    spawn_local(async move {
        // A 401 here just means "not logged in", which is not an error.
        auth.user.set(api::auth::me().await.ok());
        auth.ready.set(true);
    });

    children()
}

/// Where to send an unauthenticated visitor: `/login?next=<path+query>`.
/// Never builds `next` from a login URL (that is how redirect loops grow).
/// Leptos' `search` has no leading `?`; it is added back when non-empty.
pub fn login_redirect_target(pathname: &str, search: &str) -> String {
    if pathname.starts_with("/login") {
        return "/login".to_string();
    }
    let here = if search.is_empty() {
        pathname.to_string()
    } else {
        format!("{pathname}?{search}")
    };
    build_url("/login", &[("next", &here)])
}

/// Layout route for protected pages: renders the matched child route via
/// `<Outlet/>` when logged in, otherwise redirects to `/login?next=<here>`.
#[component]
pub fn RequireAuth() -> impl IntoView {
    let auth = use_auth();
    let location = use_location();

    auth.guard_mounted.set(true);
    on_cleanup(move || {
        auth.guard_mounted.set(false);
        auth.logging_out.set(false);
    });

    view! {
        // This closure re-runs whenever `auth.ready` / `auth.user` change.
        // The location is read with `get_untracked` on purpose: a tracked read
        // would subscribe us to the URL, so after the redirect moved us to
        // `/login?next=...` we would re-run, build `next` from the login URL
        // and redirect again, forever. Untracked, we capture "where the user
        // was" exactly once, at the moment they lost access.
        {move || {
            if !auth.ready.get() {
                view! { <p aria-busy="true">"Loading…"</p> }.into_any()
            } else if auth.user.get().is_some() {
                view! { <Outlet/> }.into_any()
            } else if auth.logging_out.get_untracked() {
                // Deliberate log out: the page is about to be replaced, so
                // show nothing instead of redirecting to the login form.
                auth.logging_out.set(false);
                ().into_any()
            } else {
                let to = login_redirect_target(
                    &location.pathname.get_untracked(),
                    &location.search.get_untracked(),
                );
                view! { <leptos_router::components::Redirect path=to/> }.into_any()
            }
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_path_and_query_in_next() {
        assert_eq!(
            login_redirect_target("/members", ""),
            "/login?next=%2Fmembers"
        );
        assert_eq!(
            login_redirect_target("/members", "x=1"),
            "/login?next=%2Fmembers%3Fx%3D1"
        );
    }

    #[test]
    fn never_nests_a_login_url() {
        assert_eq!(login_redirect_target("/login", "next=%2Fbooks"), "/login");
    }
}
