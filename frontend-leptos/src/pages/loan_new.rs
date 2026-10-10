//! `/loans/new`: pick a book and a member, then lend.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_query_map};
use library_shared::{Availability, BookQuery, MemberQuery, NewLoan};

use crate::api::{self, ApiRequestError};
use crate::components::error_message::ErrorMessage;
use crate::components::search_box::SearchBox;
use crate::url::parse_id;

#[component]
pub fn LoanNew() -> impl IntoView {
    let navigate = use_navigate();
    let query = use_query_map();
    // `?book_id=` / `?member_id=` prefill the pickers (from "Lend" buttons).
    let (initial_book, initial_member) = {
        let q = query.read_untracked();
        (
            parse_id(q.get("book_id").as_deref()),
            parse_id(q.get("member_id").as_deref()),
        )
    };

    let book_search = RwSignal::new(String::new());
    let member_search = RwSignal::new(String::new());
    // The chosen option (id + label) lives in state, not just in the <select>,
    // so it stays visible and submittable when a new search replaces the results.
    let book_choice = RwSignal::new(None::<(i64, String)>);
    let member_choice = RwSignal::new(None::<(i64, String)>);

    let books = LocalResource::new(move || {
        let request = BookQuery {
            q: Some(book_search.get()).filter(|q| !q.trim().is_empty()),
            availability: Some(Availability::Available),
            ..Default::default()
        };
        async move { api::books::list(&request).await }
    });
    let members = LocalResource::new(move || {
        let request = MemberQuery {
            q: Some(member_search.get()).filter(|q| !q.trim().is_empty()),
            ..Default::default()
        };
        async move { api::members::list(&request).await }
    });
    // The prefilled entities may not be on the first page of search results,
    // so fetch them by id and merge them into the options.
    let prefill_book = LocalResource::new(move || async move {
        match initial_book {
            Some(id) => api::books::get(id).await.map(Some),
            None => Ok(None),
        }
    });
    let prefill_member = LocalResource::new(move || async move {
        match initial_member {
            Some(id) => api::members::get(id).await.map(Some),
            None => Ok(None),
        }
    });

    // Once a prefilled entity loads, make it the initial choice.
    Effect::new(move |_| {
        if let Some(Ok(Some(book))) = prefill_book.get()
            && book_choice.get_untracked().is_none()
        {
            book_choice.set(Some((
                book.id,
                book_label(&book.title, &book.author, book.available_copies),
            )));
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(Some(m))) = prefill_member.get()
            && member_choice.get_untracked().is_none()
        {
            member_choice.set(Some((m.id, member_label(&m.name, &m.email))));
        }
    });

    let book_options = Signal::derive(move || match books.get() {
        Some(Ok(page)) => page
            .items
            .into_iter()
            .map(|b| (b.id, book_label(&b.title, &b.author, b.available_copies)))
            .collect(),
        _ => Vec::new(),
    });
    let member_options = Signal::derive(move || match members.get() {
        Some(Ok(page)) => page
            .items
            .into_iter()
            .map(|m| (m.id, member_label(&m.name, &m.email)))
            .collect(),
        _ => Vec::new(),
    });

    // Load errors for either picker (first one wins).
    let load_error = Signal::derive(move || {
        let books_err = books.get().and_then(Result::err);
        let members_err = members.get().and_then(Result::err);
        let book_prefill_err = prefill_book.get().and_then(Result::err);
        let member_prefill_err = prefill_member.get().and_then(Result::err);
        books_err
            .or(members_err)
            .or(book_prefill_err)
            .or(member_prefill_err)
    });

    let error = RwSignal::new(None::<ApiRequestError>);
    let busy = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (Some(book), Some(member)) = (
            book_choice.get_untracked().map(|(id, _)| id),
            member_choice.get_untracked().map(|(id, _)| id),
        ) else {
            error.set(Some(ApiRequestError {
                status: 0,
                code: "validation_error".into(),
                message: "Choose a book and a member".into(),
                fields: Default::default(),
            }));
            return;
        };
        error.set(None);
        busy.set(true);
        let navigate = navigate.clone();
        spawn_local(async move {
            let request = NewLoan {
                book_id: book,
                member_id: member,
            };
            match api::loans::create(&request).await {
                Ok(_) => navigate("/loans", Default::default()),
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    view! {
        <h1>"New loan"</h1>
        <form on:submit=on_submit>
            <Picker
                label="Book (available only)"
                search=book_search
                selected=book_choice
                options=book_options
            />
            <Picker label="Member" search=member_search selected=member_choice options=member_options/>
            {move || load_error.get().map(|e| view! { <ErrorMessage error=e/> })}
            {move || error.get().map(|e| view! { <ErrorMessage error=e/> })}
            <div class="actions">
                <button type="submit" aria-busy=move || busy.get().then_some("true") disabled=move || busy.get()>
                    "Lend"
                </button>
                <a href="/loans" role="button" class="secondary outline">
                    "Cancel"
                </a>
            </div>
        </form>
    }
}

fn member_label(name: &str, email: &str) -> String {
    format!("{name} ({email})")
}

fn book_label(title: &str, author: &str, available: i64) -> String {
    format!("{title} - {author} ({available} available)")
}

/// A search box plus a `<select>` of matches. The current choice is always
/// listed first, even if the latest search no longer returns it.
#[component]
fn Picker(
    label: &'static str,
    search: RwSignal<String>,
    selected: RwSignal<Option<(i64, String)>>,
    #[prop(into)] options: Signal<Vec<(i64, String)>>,
) -> impl IntoView {
    // Chosen option first, then the rest of the results without a duplicate.
    let all_options = Memo::new(move |_| {
        let chosen = selected.get();
        let mut all: Vec<(i64, String)> = chosen.iter().cloned().collect();
        for (id, text) in options.get() {
            if chosen.as_ref().is_none_or(|(c, _)| *c != id) {
                all.push((id, text));
            }
        }
        all
    });
    let on_change = move |ev| {
        let id = parse_id(Some(&event_target_value(&ev)));
        selected.set(id.and_then(|id| {
            all_options
                .get_untracked()
                .into_iter()
                .find(|(option_id, _)| *option_id == id)
        }));
    };

    view! {
        <label>
            {label} <SearchBox
                value=search
                on_change=Callback::new(move |text: String| search.set(text.trim().to_string()))
                placeholder="Type to search"
            /> <select on:change=on_change>
                <option value="" selected=move || selected.with(Option::is_none)>
                    "Choose…"
                </option>
                // Reactive list: re-renders whenever the options change. The
                // `selected` attribute (not a value on the <select>) keeps the
                // choice correct even when options arrive later.
                {move || {
                    all_options
                        .get()
                        .into_iter()
                        .map(|(id, text)| {
                            view! {
                                <option
                                    value=id.to_string()
                                    selected=move || {
                                        selected.with(|s| s.as_ref().is_some_and(|(c, _)| *c == id))
                                    }
                                >
                                    {text}
                                </option>
                            }
                        })
                        .collect_view()
                }}
            </select>
        </label>
    }
}
