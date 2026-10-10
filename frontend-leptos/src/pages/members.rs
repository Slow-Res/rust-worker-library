//! `/members`: searchable member list (librarians only).

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_query_map};
use library_shared::{Member, MemberQuery};

use crate::api::{self, ApiRequestError};
use crate::browser::confirm;
use crate::components::error_message::ErrorMessage;
use crate::components::pagination::{OutOfRange, Pagination};
use crate::components::search_box::SearchBox;
use crate::url::{build_url, page_param, parse_page};

#[component]
pub fn Members() -> impl IntoView {
    let navigate = use_navigate();
    let query = use_query_map();
    let q = Memo::new(move |_| query.read().get("q").unwrap_or_default());
    let page = Memo::new(move |_| parse_page(query.read().get("page").as_deref()));

    let version = RwSignal::new(0u32);
    let action_error = RwSignal::new(None::<ApiRequestError>);

    let members = LocalResource::new(move || {
        version.track();
        let request = MemberQuery {
            q: Some(q.get()).filter(|q| !q.trim().is_empty()),
            page: Some(page.get()).filter(|p| *p > 1),
            per_page: None,
        };
        async move { api::members::list(&request).await }
    });

    // Search changes replace the history entry (no entry per keystroke
    // pause); page changes push one so Back returns to the previous page.
    let go = move |q: &str, page: u32, replace: bool| {
        navigate(
            &build_url("/members", &[("q", q), ("page", &page_param(page))]),
            NavigateOptions {
                replace,
                ..Default::default()
            },
        );
    };
    let on_search = {
        let go = go.clone();
        Callback::new(move |text: String| go(text.trim(), 1, true))
    };
    let on_page = Callback::new(move |next: u32| go(&q.get_untracked(), next, false));

    let delete = move |member: Member| {
        if !confirm(&format!("Delete member \"{}\"?", member.name)) {
            return;
        }
        action_error.set(None);
        spawn_local(async move {
            match api::members::delete(member.id).await {
                Ok(()) => version.update(|v| *v += 1),
                Err(e) => action_error.set(Some(e)),
            }
        });
    };

    view! {
        <h1>"Members"</h1>
        <div class="toolbar">
            <SearchBox value=q on_change=on_search placeholder="Search name or email"/>
            <a href="/members/new" role="button">
                "Add member"
            </a>
        </div>
        {move || action_error.get().map(|e| view! { <ErrorMessage error=e/> })}
        <Suspense fallback=|| view! { <p aria-busy="true">"Loading…"</p> }>
            {move || {
                members
                    .get()
                    .map(|result| match result {
                        Err(e) => view! { <ErrorMessage error=e/> }.into_any(),
                        Ok(list) if list.items.is_empty() && page.get() > 1 => {
                            view! { <OutOfRange on_page/> }.into_any()
                        }
                        Ok(list) if list.items.is_empty() => {
                            view! { <p>"No members found."</p> }.into_any()
                        }
                        Ok(list) => {
                            let (per_page, total) = (list.per_page, list.total);
                            view! {
                                <table>
                                    <thead>
                                        <tr>
                                            <th>"Name"</th>
                                            <th>"Email"</th>
                                            <th>"Active loans"</th>
                                            <th></th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {list
                                            .items
                                            .into_iter()
                                            .map(|member| member_row(member, delete))
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

fn member_row(
    member: Member,
    delete: impl Fn(Member) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let id = member.id;
    let for_delete = member.clone();
    view! {
        <tr>
            <td>{member.name}</td>
            <td>{member.email}</td>
            <td>{member.active_loans}</td>
            <td class="actions">
                <a href=format!("/members/{id}") role="button" class="outline">
                    "View"
                </a>
                <a href=format!("/members/{id}/edit") role="button" class="outline">
                    "Edit"
                </a>
                <button class="outline secondary" on:click=move |_| delete(for_delete.clone())>
                    "Delete"
                </button>
            </td>
        </tr>
    }
}
