//! `/api/books` endpoints.

use gloo_net::http::Request;
use library_shared::{Book, BookInput, BookQuery, Page};

use super::{ApiResult, get_json, send_json, send_no_content, with_query};

pub async fn list(query: &BookQuery) -> ApiResult<Page<Book>> {
    get_json(&with_query("/api/books", query)).await
}

pub async fn get(id: i64) -> ApiResult<Book> {
    get_json(&format!("/api/books/{id}")).await
}

pub async fn create(input: &BookInput) -> ApiResult<Book> {
    let path = "/api/books";
    send_json(Request::post(path), path, input).await
}

pub async fn update(id: i64, input: &BookInput) -> ApiResult<Book> {
    let path = format!("/api/books/{id}");
    send_json(Request::put(&path), &path, input).await
}

pub async fn delete(id: i64) -> ApiResult<()> {
    let path = format!("/api/books/{id}");
    send_no_content(Request::delete(&path), &path).await
}
