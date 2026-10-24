//! Top navigation bar and page container.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;

use crate::api;
use crate::auth::use_auth;

#[component]
pub fn Layout(children: Children) -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();

    let log_out = move |_| {
        let navigate = navigate.clone();
        spawn_local(async move {
            // Even if the server call fails, drop the local session view.
            let _ = api::auth::logout().await;
            // Tell a mounted `RequireAuth` this is a log out, not session
            // loss, so it does not redirect to /login. It resets the flag
            // itself (when it sees it, or when it unmounts), so no path
            // leaves it stuck.
            auth.logging_out.set(auth.guard_mounted.get_untracked());
            auth.user.set(None);
            navigate("/books", Default::default());
        });
    };

    view! {
        <header class="container">
            <nav>
                <ul>
                    <li>
                        <strong>
                            <A href="/books">"Library"</A>
                        </strong>
                    </li>
                    <li>
                        <A href="/books">"Books"</A>
                    </li>
                    // `<Show>` renders its children only while `when` is true.
                    <Show when=move || auth.user.get().is_some()>
                        <li>
                            <A href="/members">"Members"</A>
                        </li>
                        <li>
                            <A href="/loans">"Loans"</A>
                        </li>
                    </Show>
                </ul>
                <ul>
                    // Wait for the startup `/api/auth/me` check so logged-in
                    // users never see a flash of "Log in".
                    {move || auth.ready.get().then(|| match auth.user.get() {
                        Some(user) => {
                            view! {
                                <li>{user.username}</li>
                                <li>
                                    <button class="secondary outline" on:click=log_out.clone()>
                                        "Log out"
                                    </button>
                                </li>
                            }
                                .into_any()
                        }
                        None => {
                            view! {
                                <li>
                                    <A href="/login">"Log in"</A>
                                </li>
                            }
                                .into_any()
                        }
                    })}
                </ul>
            </nav>
        </header>
        <main class="container">{children()}</main>
    }
}
