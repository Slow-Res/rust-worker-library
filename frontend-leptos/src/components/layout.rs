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
                    {move || match auth.user.get() {
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
                    }}
                </ul>
            </nav>
        </header>
        <main class="container">{children()}</main>
    }
}
