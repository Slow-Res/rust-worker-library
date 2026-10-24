//! `/api/loans` endpoints.

use gloo_net::http::Request;
use library_shared::{Loan, LoanQuery, NewLoan, Page};

use super::{ApiResult, get_json, post_json, send_json, with_query};

pub async fn list(query: &LoanQuery) -> ApiResult<Page<Loan>> {
    get_json(&with_query("/api/loans", query)).await
}

pub async fn create(input: &NewLoan) -> ApiResult<Loan> {
    let path = "/api/loans";
    send_json(Request::post(path), path, input).await
}

pub async fn return_loan(id: i64) -> ApiResult<Loan> {
    post_json(&format!("/api/loans/{id}/return")).await
}
