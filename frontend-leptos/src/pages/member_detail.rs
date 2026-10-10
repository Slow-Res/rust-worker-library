//! `/members/:id`: member info and their loan history.

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use library_shared::{LoanQuery, LoanStatus};

use crate::api::{self, ApiRequestError};
use crate::browser::local_date;
use crate::components::error_message::ErrorMessage;
use crate::components::pagination::{OutOfRange, Pagination};
use crate::pages::loans::LoansTable;
use crate::pages::not_found::NotFound;
use crate::url::{build_url, page_param, parse_id, parse_page};

#[component]
pub fn MemberDetail() -> impl IntoView {
    let params = use_params_map();
    let Some(id) = parse_id(params.read_untracked().get("id").as_deref()) else {
        return view! { <NotFound/> }.into_any();
    };
    let navigate = use_navigate();
    let query = use_query_map();
    let page = Memo::new(move |_| parse_page(query.read().get("page").as_deref()));

    let version = RwSignal::new(0u32);
    let member = LocalResource::new(move || {
        version.track();
        async move { api::members::get(id).await }
    });
    let loans = LocalResource::new(move || {
        version.track();
        let request = LoanQuery {
            status: Some(LoanStatus::All),
            member_id: Some(id),
            page: Some(page.get()).filter(|p| *p > 1),
            ..Default::default()
        };
        async move { api::loans::list(&request).await }
    });

    let on_page = Callback::new(move |next: u32| {
        // Push (not replace) so Back returns to the previous page.
        navigate(
            &build_url(&format!("/members/{id}"), &[("page", &page_param(next))]),
            Default::default(),
        );
    });
    // Returning a loan changes both the loan list and the member's active-loan count.
    let on_changed = Callback::new(move |()| version.update(|v| *v += 1));
    // Owned here so a Return error survives the refetch that replaces the table.
    let return_error = RwSignal::new(None::<ApiRequestError>);

    // The loans section is only rendered inside the `Ok` arm below, so an
    // unknown member shows just the not-found message.
    view! {
        <Suspense fallback=|| view! { <p aria-busy="true">"Loading…"</p> }>
            {move || {
                member
                    .get()
                    .map(|result| match result {
                        Err(e) if e.status == 404 => view! { <NotFound/> }.into_any(),
                        Err(e) => view! { <ErrorMessage error=e/> }.into_any(),
                        Ok(m) => {
                            view! {
                                <h1>{m.name}</h1>
                                <p>
                                    {m.email} " · " {m.active_loans} " active loans · member since "
                                    {local_date(m.created_at)}
                                </p>
                                <div class="actions">
                                    <a href=format!("/loans/new?member_id={id}") role="button">
                                        "Lend a book"
                                    </a>
                                    <a href=format!("/members/{id}/edit") role="button" class="outline">
                                        "Edit"
                                    </a>
                                </div>
                                <h2>"Loans"</h2>
                                <Suspense fallback=|| {
                                    view! { <p aria-busy="true">"Loading…"</p> }
                                }>
                                    {move || {
                                        loans
                                            .get()
                                            .map(|result| match result {
                                                Err(e) => view! { <ErrorMessage error=e/> }.into_any(),
                                                Ok(list) if list.items.is_empty() && page.get() > 1 => {
                                                    view! { <OutOfRange on_page/> }.into_any()
                                                }
                                                Ok(list) => {
                                                    let (per_page, total) = (list.per_page, list.total);
                                                    view! {
                                                        <LoansTable loans=list.items on_changed error=return_error/>
                                                        <Pagination page=page.get() per_page total on_page/>
                                                    }
                                                        .into_any()
                                                }
                                            })
                                    }}
                                </Suspense>
                            }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
    .into_any()
}
