//! The responses the routes answer, each a status and, where there is one, a body that is JSON.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use raoh::Issues;
use serde_json::json;

/// A 200 with `body`, which is what a value of the model encodes to, or any other JSON.
pub fn ok(body: String) -> Response {
    with_json(StatusCode::OK, body)
}

pub fn created(body: Option<String>) -> Response {
    match body {
        Some(body) => with_json(StatusCode::CREATED, body),
        None => StatusCode::CREATED.into_response(),
    }
}

/// raoh's issues, each with its path, and the messages by path.
pub fn bad_request(issues: &Issues) -> Response {
    with_json(
        StatusCode::BAD_REQUEST,
        json!({ "issues": issues.to_json(), "errors": issues.flatten() }).to_string(),
    )
}

/// A business case the model answered, by name.
pub fn unprocessable(error: &str) -> Response {
    with_json(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": error }).to_string(),
    )
}

/// A request that came to no answer: the run ended, an implementation failed, or the database did.
pub fn internal() -> Response {
    with_json(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": "internal" }).to_string(),
    )
}

fn with_json(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}
