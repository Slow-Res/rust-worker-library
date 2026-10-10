//! `/loans`: loans by status tab, with a reusable table (also used on the
//! member detail page).

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_query_map};
use library_shared::{Loan, LoanQuery, LoanStatus};

use crate::api::{self, ApiRequestError};
use crate::browser::local_date;
use crate::components::error_message::ErrorMessage;
use crate::components::pagination::{OutOfRange, Pagination};
use crate::format::loan_status_label;
use crate::url::{build_url, loan_status_param, page_param, parse_loan_status, parse_page};

const TABS: [(LoanStatus, &str); 4] = [
    (LoanStatus::Active, "Active"),
    (LoanStatus::Overdue, "Overdue"),
    (LoanStatus::Returned, "Returned"),
    (LoanStatus::All, "All"),
];

#[component]
pub fn Loans() -> impl IntoView {
    let navigate = use_navigate();
    let query = use_query_map();
    let status = Memo::new(move |_| parse_loan_status(query.read().get("status").as_deref()));
    let page = Memo::new(move |_| parse_page(query.read().get("page").as_deref()));

    let version = RwSignal::new(0u32);
    let loans = LocalResource::new(move || {
        version.track();
        let request = LoanQuery {
            status: Some(status.get()),
            page: Some(page.get()).filter(|p| *p > 1),
            ..Default::default()
        };
        async move { api::loans::list(&request).await }
    });

    let on_page = Callback::new(move |next: u32| {
        navigate(
            &build_url(
                "/loans",
                &[
                    ("status", loan_status_param(status.get_untracked())),
                    ("page", &page_param(next)),
                ],
            ),
            // Push (not replace) so Back returns to the previous page.
            Default::default(),
        );
    });
    let on_changed = Callback::new(move |()| version.update(|v| *v += 1));
    // Lives here (not in `LoansTable`) so a Return error survives the refetch
    // that replaces the table.
    let return_error = RwSignal::new(None::<ApiRequestError>);

    view! {
        <h1>"Loans"</h1>
        <div class="toolbar">
            <div role="group">
                {TABS
                    .into_iter()
                    .map(|(tab, label)| {
                        // Plain links: the status lives in the URL, so a tab is just a URL.
                        let href = build_url("/loans", &[("status", loan_status_param(tab))]);
                        view! {
                            <a
                                href=href
                                role="button"
                                class=move || if status.get() == tab { "" } else { "outline" }
                                aria-current=move || (status.get() == tab).then_some("page")
                            >
                                {label}
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
            <a href="/loans/new" role="button">
                "New loan"
            </a>
        </div>
        <Suspense fallback=|| view! { <p aria-busy="true">"Loading…"</p> }>
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
}

/// Table of loans with a Return button on unreturned rows. `on_changed`
/// tells the parent to refetch after a return attempt (also a failed one, so
/// a stale row disappears); `error` is owned by the parent for that reason.
#[component]
pub fn LoansTable(
    loans: Vec<Loan>,
    on_changed: Callback<()>,
    error: RwSignal<Option<ApiRequestError>>,
) -> impl IntoView {
    let return_loan = move |id: i64| {
        error.set(None);
        spawn_local(async move {
            match api::loans::return_loan(id).await {
                Ok(_) => on_changed.run(()),
                Err(e) => {
                    error.set(Some(e));
                    on_changed.run(());
                }
            }
        });
    };

    if loans.is_empty() {
        return view! {
            {move || error.get().map(|e| view! { <ErrorMessage error=e/> })}
            <p>"No loans found."</p>
        }
        .into_any();
    }

    view! {
        {move || error.get().map(|e| view! { <ErrorMessage error=e/> })}
        <table>
            <thead>
                <tr>
                    <th>"Book"</th>
                    <th>"Member"</th>
                    <th>"Borrowed"</th>
                    <th>"Due"</th>
                    <th>"Status"</th>
                    <th></th>
                </tr>
            </thead>
            <tbody>
                {loans
                    .into_iter()
                    .map(|loan| {
                        let id = loan.id;
                        let returned = loan.returned_at.is_some();
                        view! {
                            <tr class:overdue=loan.is_overdue && !returned>
                                <td>
                                    <a href=format!("/books/{}/edit", loan.book_id)>{loan.book_title.clone()}</a>
                                </td>
                                <td>
                                    <a href=format!("/members/{}", loan.member_id)>{loan.member_name.clone()}</a>
                                </td>
                                <td>{local_date(loan.borrowed_at)}</td>
                                <td>{local_date(loan.due_at)}</td>
                                <td>{loan_status_label(&loan)}</td>
                                <td class="actions">
                                    {(!returned)
                                        .then(|| {
                                            view! {
                                                <button class="outline" on:click=move |_| return_loan(id)>
                                                    "Return"
                                                </button>
                                            }
                                        })}
                                </td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </tbody>
        </table>
    }
    .into_any()
}
