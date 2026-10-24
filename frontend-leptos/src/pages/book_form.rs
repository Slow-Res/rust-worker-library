//! `/books/new` and `/books/:id/edit`: the same form for both.

use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_params_map};
use library_shared::{Book, BookInput};

use crate::api::{self, ApiRequestError, ApiResult};
use crate::components::error_message::ErrorMessage;
use crate::components::field_error::FieldError;
use crate::pages::not_found::NotFound;
use crate::url::parse_id;

/// Raw text of each input. Forms deal in strings; conversion to the typed
/// [`BookInput`] happens once, on submit.
#[derive(Clone, Default)]
struct BookFormValues {
    title: String,
    author: String,
    isbn: String,
    published_year: String,
    total_copies: String,
}

impl BookFormValues {
    fn from_book(book: &Book) -> Self {
        Self {
            title: book.title.clone(),
            author: book.author.clone(),
            isbn: book.isbn.clone().unwrap_or_default(),
            published_year: book
                .published_year
                .map(|y| y.to_string())
                .unwrap_or_default(),
            total_copies: book.total_copies.to_string(),
        }
    }

    /// Converts to the request body. Only number parsing is checked here; the
    /// server owns every other validation rule and reports it per field.
    fn to_input(&self) -> Result<BookInput, BTreeMap<String, String>> {
        let mut errors = BTreeMap::new();
        let isbn = self.isbn.trim();
        let year = self.published_year.trim();
        let published_year = if year.is_empty() {
            None
        } else {
            year.parse::<i32>()
                .map_err(|_| {
                    errors.insert("published_year".to_string(), "must be a number".to_string())
                })
                .ok()
        };
        let total_copies = self.total_copies.trim().parse::<i64>().unwrap_or_else(|_| {
            errors.insert("total_copies".to_string(), "must be a number".to_string());
            0
        });
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(BookInput {
            title: self.title.clone(),
            author: self.author.clone(),
            isbn: (!isbn.is_empty()).then(|| isbn.to_string()),
            published_year,
            total_copies,
        })
    }
}

#[component]
pub fn BookNew() -> impl IntoView {
    view! {
        <h1>"Add book"</h1>
        <BookForm
            initial=BookFormValues { total_copies: "1".into(), ..Default::default() }
            save=|input| async move { api::books::create(&input).await.map(|_| ()) }
        />
    }
}

#[component]
pub fn BookEdit() -> impl IntoView {
    let params = use_params_map();
    let Some(id) = parse_id(params.read_untracked().get("id").as_deref()) else {
        return view! { <NotFound/> }.into_any();
    };
    let book = LocalResource::new(move || async move { api::books::get(id).await });

    view! {
        <Suspense fallback=|| view! { <p aria-busy="true">"Loading…"</p> }>
            {move || {
                book.get()
                    .map(|result| match result {
                        Err(e) if e.status == 404 => view! { <NotFound/> }.into_any(),
                        Err(e) => view! { <ErrorMessage error=e/> }.into_any(),
                        Ok(book) => {
                            view! {
                                <h1>"Edit book"</h1>
                                <BookForm
                                    initial=BookFormValues::from_book(&book)
                                    save=move |input| async move {
                                        api::books::update(id, &input).await.map(|_| ())
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

/// `save` is any async function from the typed input to a result, so create
/// and edit share this component. On success we go back to the book list.
#[component]
fn BookForm<F, Fut>(initial: BookFormValues, save: F) -> impl IntoView
where
    F: Fn(BookInput) -> Fut + Clone + 'static,
    Fut: std::future::Future<Output = ApiResult<()>> + 'static,
{
    let navigate = use_navigate();
    let title = RwSignal::new(initial.title);
    let author = RwSignal::new(initial.author);
    let isbn = RwSignal::new(initial.isbn);
    let year = RwSignal::new(initial.published_year);
    let copies = RwSignal::new(initial.total_copies);
    let error = RwSignal::new(None::<ApiRequestError>);
    let local_errors = RwSignal::new(BTreeMap::<String, String>::new());
    let busy = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        error.set(None);
        let values = BookFormValues {
            title: title.get_untracked(),
            author: author.get_untracked(),
            isbn: isbn.get_untracked(),
            published_year: year.get_untracked(),
            total_copies: copies.get_untracked(),
        };
        let input = match values.to_input() {
            Ok(input) => input,
            Err(errors) => {
                local_errors.set(errors);
                return;
            }
        };
        local_errors.set(BTreeMap::new());
        busy.set(true);
        let (save, navigate) = (save.clone(), navigate.clone());
        spawn_local(async move {
            let result = save(input).await;
            busy.set(false);
            match result {
                Ok(()) => navigate("/books", Default::default()),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // Server field errors and local parse errors share one lookup.
    let field = move |name: &'static str| {
        Signal::derive(move || {
            local_errors
                .with(|l| l.get(name).cloned())
                .or_else(|| error.get().and_then(|e| e.field(name)))
        })
    };
    let invalid = move |name: &'static str| field(name).get().is_some().then_some("true");

    view! {
        // `novalidate` turns off the browser's own validation bubbles so the
        // server's field errors (shown by <FieldError>) are what users see.
        <form novalidate on:submit=on_submit>
            <label>
                "Title"
                <input
                    aria-invalid=move || invalid("title")
                    prop:value=move || title.get()
                    on:input=move |ev| title.set(event_target_value(&ev))
                />
                <FieldError message=field("title")/>
            </label>
            <label>
                "Author"
                <input
                    aria-invalid=move || invalid("author")
                    prop:value=move || author.get()
                    on:input=move |ev| author.set(event_target_value(&ev))
                />
                <FieldError message=field("author")/>
            </label>
            <label>
                "ISBN (optional)"
                <input
                    aria-invalid=move || invalid("isbn")
                    prop:value=move || isbn.get()
                    on:input=move |ev| isbn.set(event_target_value(&ev))
                />
                <FieldError message=field("isbn")/>
            </label>
            <label>
                "Published year (optional)"
                <input
                    type="number"
                    aria-invalid=move || invalid("published_year")
                    prop:value=move || year.get()
                    on:input=move |ev| year.set(event_target_value(&ev))
                />
                <FieldError message=field("published_year")/>
            </label>
            <label>
                "Total copies"
                <input
                    type="number"
                    aria-invalid=move || invalid("total_copies")
                    prop:value=move || copies.get()
                    on:input=move |ev| copies.set(event_target_value(&ev))
                />
                <FieldError message=field("total_copies")/>
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
                <a href="/books" role="button" class="secondary outline">
                    "Cancel"
                </a>
            </div>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> BookFormValues {
        BookFormValues {
            title: "Dune".into(),
            author: "Frank Herbert".into(),
            isbn: " 978-0-441-17271-9 ".into(),
            published_year: "1965".into(),
            total_copies: "3".into(),
        }
    }

    #[test]
    fn converts_valid_values() {
        let input = values().to_input().unwrap();
        assert_eq!(input.isbn.as_deref(), Some("978-0-441-17271-9"));
        assert_eq!(input.published_year, Some(1965));
        assert_eq!(input.total_copies, 3);
    }

    #[test]
    fn blank_optionals_become_none() {
        let v = BookFormValues {
            isbn: "  ".into(),
            published_year: String::new(),
            ..values()
        };
        let input = v.to_input().unwrap();
        assert_eq!(input.isbn, None);
        assert_eq!(input.published_year, None);
    }

    #[test]
    fn reports_every_unparsable_number() {
        let v = BookFormValues {
            published_year: "abc".into(),
            total_copies: "".into(),
            ..values()
        };
        let errors = v.to_input().unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors.contains_key("published_year"));
        assert!(errors.contains_key("total_copies"));
    }
}
