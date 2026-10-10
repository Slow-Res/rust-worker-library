//! `/api/members` endpoints.

use gloo_net::http::Request;
use library_shared::{Member, MemberInput, MemberQuery, Page};

use super::{ApiResult, get_json, send_json, send_no_content, with_query};

pub async fn list(query: &MemberQuery) -> ApiResult<Page<Member>> {
    get_json(&with_query("/api/members", query)).await
}

pub async fn get(id: i64) -> ApiResult<Member> {
    get_json(&format!("/api/members/{id}")).await
}

pub async fn create(input: &MemberInput) -> ApiResult<Member> {
    let path = "/api/members";
    send_json(Request::post(path), path, input).await
}

pub async fn update(id: i64, input: &MemberInput) -> ApiResult<Member> {
    let path = format!("/api/members/{id}");
    send_json(Request::put(&path), &path, input).await
}

pub async fn delete(id: i64) -> ApiResult<()> {
    let path = format!("/api/members/{id}");
    send_no_content(Request::delete(&path), &path).await
}
