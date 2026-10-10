//! Router assembly: API routes, middleware stack, and optional static frontend.

use std::time::Duration;

use axum::Router;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::books;
use crate::error::AppError;
use crate::health;
use crate::state::AppState;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const BODY_LIMIT_BYTES: usize = 64 * 1024;
const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Build the full application router.
pub fn build_router(state: AppState) -> Router {
    // Add a feature by merging its router here, e.g. `.merge(books::routes())`.
    let api = Router::new()
        .merge(health::routes())
        .merge(auth::routes())
        .merge(books::routes())
        .fallback(api_not_found);

    let mut app = Router::new().nest("/api", api);

    if let Some(dir) = &state.config.frontend_dir {
        // Unknown paths serve index.html so client-side routes survive a refresh.
        let index = ServeFile::new(dir.join("index.html"));
        app = app.fallback_service(ServeDir::new(dir).fallback(index));
    }

    // `with_state` turns `Router<AppState>` into a plain `Router` once every
    // handler's state requirement is satisfied.
    let app = app.with_state(state);

    // `ServiceBuilder` lists layers outermost-first: a request passes through them top to bottom.
    app.layer(
        ServiceBuilder::new()
            .layer(SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid))
            .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER))
            .layer(
                TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
                    let request_id = request
                        .headers()
                        .get(REQUEST_ID_HEADER)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("-");
                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        path = %request.uri().path(),
                        request_id,
                    )
                }),
            )
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                REQUEST_TIMEOUT,
            ))
            .layer(CompressionLayer::new())
            .layer(security_header(header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
            .layer(security_header(header::X_FRAME_OPTIONS, "DENY"))
            .layer(security_header(header::REFERRER_POLICY, "same-origin"))
            .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES)),
    )
}

fn security_header(name: HeaderName, value: &'static str) -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::if_not_present(name, HeaderValue::from_static(value))
}

/// JSON 404 for any `/api/*` path that matches no route.
async fn api_not_found() -> AppError {
    AppError::NotFound("route")
}
