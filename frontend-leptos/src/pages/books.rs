//! `/books`: public catalogue with search, availability filter and (when
//! logged in) add / edit / lend / delete.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_query_map};
use library_shared::{Availability, Book, BookQuery};

use crate::api::{self, ApiRequestError};
use crate::auth::use_auth;
use crate::browser::confirm;
use crate::components::error_message::ErrorMessage;
use crate::components::pagination::Pagination;
use crate::components::search_box::SearchBox;
use crate::format::copies_label;
use crate::url::{availability_param, build_url, page_param, parse_availability, parse_page};

#[component]
pub fn Books() -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();
    let query = use_query_map();

    // The URL is the source of truth for search state, so refresh and the
    // Back button just work. A `Memo` is a derived signal that only notifies
    // when its value actually changes.
    let q = Memo::new(move |_| query.read().get("q").unwrap_or_default());
    let availability =
        Memo::new(move |_| parse_availability(query.read().get("availability").as_deref()));
    let page = Memo::new(move |_| parse_page(query.read().get("page").as_deref()));

    // Bumped after a mutation to make the resource below fetch again.
    let version = RwSignal::new(0u32);
    let action_error = RwSignal::new(None::<ApiRequestError>);

    // A `LocalResource` runs an async fetch on the client and refetches
    // whenever a signal read inside the closure changes (`q`, `page`, ...).
    // "Local" means the future need not be `Send`, which is what `gloo-net` gives us.
    let books = LocalResource::new(move || {
        version.track();
        let request = BookQuery {
            q: Some(q.get()).filter(|q| !q.trim().is_empty()),
            availability: Some(availability.get()).filter(|a| *a != Availability::All),
            page: Some(page.get()).filter(|p| *p > 1),
            per_page: None,
        };
        async move { api::books::list(&request).await }
    });

    // Changing any filter returns to page 1; only paging keeps the others.
    let go = move |q: &str, availability: Availability, page: u32| {
        navigate(
            &build_url(
                "/books",
                &[
                    ("q", q),
                    ("availability", availability_param(availability)),
                    ("page", &page_param(page)),
                ],
            ),
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    };

    let on_search = {
        let go = go.clone();
        Callback::new(move |text: String| go(text.trim(), availability.get_untracked(), 1))
    };
    let on_filter = {
        let go = go.clone();
        move |ev| {
            let value = event_target_value(&ev);
            go(&q.get_untracked(), parse_availability(Some(&value)), 1);
        }
    };
    let on_page = Callback::new(move |next: u32| {
        go(&q.get_untracked(), availability.get_untracked(), next);
    });

    let delete = move |book: Book| {
        if !confirm(&format!("Delete \"{}\"?", book.title)) {
            return;
        }
        action_error.set(None);
        spawn_local(async move {
            match api::books::delete(book.id).await {
                Ok(()) => version.update(|v| *v += 1),
                Err(e) => action_error.set(Some(e)),
            }
        });
    };

    view! {
        <hgroup>
            <h1>"Books"</h1>
        </hgroup>
        <div class="toolbar">
            <SearchBox value=q on_change=on_search placeholder="Search title, author or ISBN"/>
            <select
                aria-label="Availability"
                on:change=on_filter
            >
                // `selected` on each option (not `prop:value` on the select):
                // props are applied before children exist, so a value set on
                // the <select> is ignored on first render.
                <option value="" selected=move || availability.get() == Availability::All>
                    "All"
                </option>
                <option
                    value="available"
                    selected=move || availability.get() == Availability::Available
                >
                    "Available"
                </option>
                <option value="on_loan" selected=move || availability.get() == Availability::OnLoan>
                    "On loan"
                </option>
            </select>
            <Show when=move || auth.user.get().is_some()>
                <a href="/books/new" role="button">
                    "Add book"
                </a>
            </Show>
        </div>
        {move || action_error.get().map(|e| view! { <ErrorMessage error=e/> })}
        // `Suspense` shows its fallback while any resource read inside it is
        // still loading, then swaps in the real content.
        <Suspense fallback=|| view! { <p aria-busy="true">"Loading…"</p> }>
            {move || {
                books
                    .get()
                    .map(|result| match result {
                        Err(e) => view! { <ErrorMessage error=e/> }.into_any(),
                        Ok(list) if list.items.is_empty() => {
                            view! { <p>"No books found."</p> }.into_any()
                        }
                        Ok(list) => {
                            let (per_page, total) = (list.per_page, list.total);
                            view! {
                                <table>
                                    <thead>
                                        <tr>
                                            <th>"Title"</th>
                                            <th>"Author"</th>
                                            <th>"ISBN"</th>
                                            <th>"Year"</th>
                                            <th>"Available"</th>
                                            <Show when=move || auth.user.get().is_some()>
                                                <th></th>
                                            </Show>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {list
                                            .items
                                            .into_iter()
                                            .map(|book| book_row(book, auth.user, delete))
                                            .collect_view()}
                                    </tbody>
                                </table>
                                <Pagination page=page.get() per_page total on_page/>
                            }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

fn book_row(
    book: Book,
    user: RwSignal<Option<library_shared::CurrentUser>>,
    delete: impl Fn(Book) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let id = book.id;
    let for_delete = book.clone();
    view! {
        <tr>
            <td>{book.title}</td>
            <td>{book.author}</td>
            <td>{book.isbn.unwrap_or_default()}</td>
            <td>{book.published_year.map(|y| y.to_string()).unwrap_or_default()}</td>
            <td>{copies_label(book.available_copies, book.total_copies)}</td>
            <Show when=move || user.get().is_some()>
                <td class="actions">
                    <a href=format!("/books/{id}/edit") role="button" class="outline">
                        "Edit"
                    </a>
                    <a href=format!("/loans/new?book_id={id}") role="button" class="outline">
                        "Lend"
                    </a>
                    <button
                        class="outline secondary"
                        on:click={
                            let book = for_delete.clone();
                            move |_| delete(book.clone())
                        }
                    >
                        "Delete"
                    </button>
                </td>
            </Show>
        </tr>
    }
}
