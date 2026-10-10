//! Root component: auth provider, layout and the route table.

use leptos::prelude::*;
use leptos_router::components::{ParentRoute, Redirect, Route, Router, Routes};
use leptos_router::path;

use crate::auth::{AuthProvider, RequireAuth};
use crate::components::layout::Layout;
use crate::pages::{
    book_form::{BookEdit, BookNew},
    books::Books,
    loan_new::LoanNew,
    loans::Loans,
    login::Login,
    member_detail::MemberDetail,
    member_form::{MemberEdit, MemberNew},
    members::Members,
    not_found::NotFound,
};

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <AuthProvider>
                <Layout>
                    <Routes fallback=NotFound>
                        <Route path=path!("/") view=|| view! { <Redirect path="/books"/> }/>
                        <Route path=path!("/login") view=Login/>
                        <Route path=path!("/books") view=Books/>
                        // Everything inside this pathless parent requires a
                        // logged-in user; `RequireAuth` renders the child via <Outlet/>.
                        <ParentRoute path=path!("") view=RequireAuth>
                            <Route path=path!("/books/new") view=BookNew/>
                            <Route path=path!("/books/:id/edit") view=BookEdit/>
                            <Route path=path!("/members") view=Members/>
                            <Route path=path!("/members/new") view=MemberNew/>
                            <Route path=path!("/members/:id") view=MemberDetail/>
                            <Route path=path!("/members/:id/edit") view=MemberEdit/>
                            <Route path=path!("/loans") view=Loans/>
                            <Route path=path!("/loans/new") view=LoanNew/>
                        </ParentRoute>
                    </Routes>
                </Layout>
            </AuthProvider>
        </Router>
    }
}
