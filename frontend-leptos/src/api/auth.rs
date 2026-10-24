//! `/api/auth/*` endpoints.

use gloo_net::http::Request;
use library_shared::{CurrentUser, LoginRequest};

use super::{ApiResult, get_json, send_json, send_no_content};

pub async fn login(request: &LoginRequest) -> ApiResult<CurrentUser> {
    let path = "/api/auth/login";
    send_json(Request::post(path), path, request).await
}

pub async fn logout() -> ApiResult<()> {
    let path = "/api/auth/logout";
    send_no_content(Request::post(path), path).await
}

pub async fn me() -> ApiResult<CurrentUser> {
    get_json("/api/auth/me").await
}
