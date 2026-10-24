//! Typed JSON client for the backend API.
//!
//! Every request goes through [`execute`], which turns a non-2xx response
//! into an [`ApiRequestError`] decoded from the shared `ApiError` envelope.
//! The browser sends the session cookie automatically because requests are
//! same-origin (Trunk proxies `/api` to the backend in development).

pub mod auth;
pub mod books;
pub mod loans;
pub mod members;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use gloo_net::http::{Request, RequestBuilder, Response};
use library_shared::ApiError;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A failed API call: either an error envelope from the server or a
/// transport problem (`status == 0`).
#[derive(Debug, Clone, PartialEq)]
pub struct ApiRequestError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub fields: BTreeMap<String, String>,
}

impl ApiRequestError {
    fn network(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: "network_error".into(),
            message: message.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Builds an error from a response body, falling back to a generic
    /// message when the body is not the `ApiError` envelope.
    fn from_body(status: u16, body: &str) -> Self {
        match serde_json::from_str::<ApiError>(body) {
            Ok(api) => Self {
                status,
                code: api.error.code,
                message: api.error.message,
                fields: api.error.fields.unwrap_or_default(),
            },
            Err(_) => Self {
                status,
                code: "unknown".into(),
                message: format!("Request failed (HTTP {status})"),
                fields: BTreeMap::new(),
            },
        }
    }

    /// The validation message for one input, if the server sent one.
    pub fn field(&self, name: &str) -> Option<String> {
        self.fields.get(name).cloned()
    }
}

impl fmt::Display for ApiRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

pub type ApiResult<T> = Result<T, ApiRequestError>;

type UnauthorizedHandler = Rc<dyn Fn()>;

thread_local! {
    // WASM is single-threaded, so a thread-local is a simple global slot.
    static ON_UNAUTHORIZED: RefCell<Option<UnauthorizedHandler>> = const { RefCell::new(None) };
}

/// Registers what to do when any (non-auth) request comes back 401. The auth
/// provider uses this to clear the current user and send the visitor to the
/// login form.
pub fn set_unauthorized_handler(handler: impl Fn() + 'static) {
    ON_UNAUTHORIZED.with(|slot| *slot.borrow_mut() = Some(Rc::new(handler)));
}

fn notify_unauthorized() {
    // Clone the handler out so the borrow is released before it runs.
    let handler = ON_UNAUTHORIZED.with(|slot| slot.borrow().clone());
    if let Some(handler) = handler {
        handler();
    }
}

/// Appends `?query` built from a shared query struct (`BookQuery`, ...).
/// Optional fields that are `None` are skipped by the structs themselves.
pub fn with_query<Q: Serialize>(path: &str, query: &Q) -> String {
    match serde_urlencoded::to_string(query) {
        Ok(qs) if !qs.is_empty() => format!("{path}?{qs}"),
        _ => path.to_string(),
    }
}

/// Sends the request and maps every failure to [`ApiRequestError`].
async fn execute(path: &str, request: Result<Request, gloo_net::Error>) -> ApiResult<Response> {
    let request = request.map_err(|e| ApiRequestError::network(e.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|_| ApiRequestError::network("Could not reach the server"))?;
    if response.ok() {
        return Ok(response);
    }
    let status = response.status();
    // Login/me report their own 401s; everything else means "session expired".
    if status == 401 && !path.starts_with("/api/auth/") {
        notify_unauthorized();
    }
    let body = response.text().await.unwrap_or_default();
    Err(ApiRequestError::from_body(status, &body))
}

async fn decode<T: DeserializeOwned>(response: Response) -> ApiResult<T> {
    response
        .json::<T>()
        .await
        .map_err(|e| ApiRequestError::network(format!("Unexpected response: {e}")))
}

pub(crate) async fn get_json<T: DeserializeOwned>(path: &str) -> ApiResult<T> {
    let response = execute(path, Request::get(path).build()).await?;
    decode(response).await
}

pub(crate) async fn send_json<B: Serialize, T: DeserializeOwned>(
    builder: RequestBuilder,
    path: &str,
    body: &B,
) -> ApiResult<T> {
    let response = execute(path, builder.json(body)).await?;
    decode(response).await
}

/// POST with no body that returns JSON (e.g. returning a loan).
pub(crate) async fn post_json<T: DeserializeOwned>(path: &str) -> ApiResult<T> {
    let response = execute(path, Request::post(path).build()).await?;
    decode(response).await
}

/// A request whose success response has no body (204).
pub(crate) async fn send_no_content(builder: RequestBuilder, path: &str) -> ApiResult<()> {
    execute(path, builder.build()).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use library_shared::{Availability, BookQuery};

    #[test]
    fn decodes_error_envelope_with_fields() {
        let body = r#"{"error":{"code":"validation_error","message":"Some fields are invalid","fields":{"title":"must not be empty"}}}"#;
        let err = ApiRequestError::from_body(422, body);
        assert_eq!(err.status, 422);
        assert_eq!(err.code, "validation_error");
        assert_eq!(err.field("title").as_deref(), Some("must not be empty"));
        assert_eq!(err.field("author"), None);
    }

    #[test]
    fn falls_back_when_body_is_not_json() {
        let err = ApiRequestError::from_body(502, "<html>bad gateway</html>");
        assert_eq!(err.code, "unknown");
        assert!(err.message.contains("502"));
        assert!(err.fields.is_empty());
    }

    #[test]
    fn with_query_skips_empty_and_encodes() {
        assert_eq!(
            with_query("/api/books", &BookQuery::default()),
            "/api/books"
        );
        let q = BookQuery {
            q: Some("war & peace".into()),
            availability: Some(Availability::OnLoan),
            page: Some(2),
            per_page: None,
        };
        assert_eq!(
            with_query("/api/books", &q),
            "/api/books?q=war+%26+peace&availability=on_loan&page=2"
        );
    }
}
