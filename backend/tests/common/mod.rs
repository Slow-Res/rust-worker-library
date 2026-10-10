//! Shared helpers for integration tests.
//!
//! Each file in `tests/` is compiled as its own crate and uses only some of
//! these helpers, hence the module-level `dead_code` allowance. Panicking on a
//! broken test setup is the right behavior here, but clippy's "allow in tests"
//! setting only covers `#[test]` functions, so allow it explicitly.
#![allow(dead_code, clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use library_backend::auth::AuthService;
use library_backend::build_router;
use library_backend::clock::FixedClock;
use library_backend::config::{Config, LogFormat};
use library_backend::state::AppState;
use serde_json::Value;
use sqlx::SqlitePool;
use tower::ServiceExt;

/// Status, headers and parsed JSON body (`Value::Null` when the body is empty).
pub type TestResponse = (StatusCode, HeaderMap, Value);

/// The real router wired to a fresh test database and a controllable clock.
pub struct TestApp {
    pub router: Router,
    pub auth: AuthService,
    pub clock: Arc<FixedClock>,
    pub pool: SqlitePool,
}

/// Start of the fake clock: 2027-01-15 08:00:00 UTC.
pub fn start_time() -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000, 0).expect("valid constant timestamp")
}

impl TestApp {
    /// Build the app from the pool that `#[sqlx::test]` hands to the test
    /// (already migrated, unique per test).
    pub fn new(pool: SqlitePool) -> Self {
        let clock = Arc::new(FixedClock::new(start_time()));
        let config = Arc::new(test_config());
        let state = AppState::new(pool.clone(), config, clock.clone());
        Self {
            auth: state.auth.clone(),
            router: build_router(state),
            clock,
            pool,
        }
    }

    pub async fn get(&self, uri: &str, cookie: Option<&str>) -> TestResponse {
        self.send(Method::GET, uri, None, cookie).await
    }

    pub async fn post_json(&self, uri: &str, body: &Value, cookie: Option<&str>) -> TestResponse {
        self.send(Method::POST, uri, Some(body), cookie).await
    }

    pub async fn put_json(&self, uri: &str, body: &Value, cookie: Option<&str>) -> TestResponse {
        self.send(Method::PUT, uri, Some(body), cookie).await
    }

    pub async fn delete(&self, uri: &str, cookie: Option<&str>) -> TestResponse {
        self.send(Method::DELETE, uri, None, cookie).await
    }

    /// Create a librarian through the auth service, log in over HTTP, and
    /// return the `Cookie` header value (`library_session=...`) to pass to
    /// the request helpers.
    pub async fn login(&self, username: &str, password: &str) -> String {
        self.auth
            .create_user(username, password)
            .await
            .expect("create test user");
        let body = serde_json::json!({ "username": username, "password": password });
        let (status, headers, _) = self.post_json("/api/auth/login", &body, None).await;
        assert_eq!(status, StatusCode::OK, "test login failed");
        let set_cookie = headers
            .get(header::SET_COOKIE)
            .expect("login sets a cookie")
            .to_str()
            .expect("cookie header is text");
        set_cookie
            .split(';')
            .next()
            .expect("cookie has a name=value pair")
            .to_owned()
    }

    /// Send a request through the router without opening a socket.
    /// `cookie` is a raw `Cookie` header value such as `library_session=...`.
    pub async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<&Value>,
        cookie: Option<&str>,
    ) -> TestResponse {
        self.send_raw(method, uri, body.map(Value::to_string), cookie)
            .await
    }

    /// Like [`send`](Self::send) but with an arbitrary JSON-typed body, so
    /// tests can post malformed JSON.
    pub async fn send_raw(
        &self,
        method: Method,
        uri: &str,
        body: Option<String>,
        cookie: Option<&str>,
    ) -> TestResponse {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        let request = match body {
            Some(text) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(text)),
            None => builder.body(Body::empty()),
        }
        .expect("valid test request");

        // `oneshot` consumes the service, so clone the (cheap) router per call.
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.expect("readable body").to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("response body is JSON")
        };
        (parts.status, parts.headers, json)
    }
}

fn test_config() -> Config {
    Config {
        database_url: "sqlite::memory:".to_owned(),
        bind_addr: ([127, 0, 0, 1], 0).into(),
        frontend_dir: None,
        cookie_secure: false,
        session_ttl_hours: 168,
        log_format: LogFormat::Pretty,
    }
}
