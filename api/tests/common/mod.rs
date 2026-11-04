//! Shared helpers for HTTP tests. Use them in every test file:
//!
//!     mod common;
//!     use common::{get_json, post_json};
//!
//!     let app = loan_api::app();                    // one service instance; clone it for several requests
//!     let (status, body) = post_json(&app, "/path", json!({...}), &[("Idempotency-Key", "k1")]).await;
//!     let (status, body) = get_json(&app, "/path").await;
//!
//! Several requests on the SAME `app` share its state (e.g. an offer created by the first request can be accepted
//! by the second). A new `loan_api::app()` starts with fresh demo data.
#![allow(dead_code)]

use axum::{body::Body, http::{Request, StatusCode}, Router};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// POST a JSON body with optional extra headers; returns the status and the JSON answer (Null if empty).
pub async fn post_json(app: &Router, path: &str, body: Value, headers: &[(&str, &str)]) -> (StatusCode, Value) {
    let mut req = Request::post(path).header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    send(app, req.body(Body::from(body.to_string())).unwrap()).await
}

/// GET a path; returns the status and the JSON answer (Null if empty).
pub async fn get_json(app: &Router, path: &str) -> (StatusCode, Value) {
    send(app, Request::get(path).body(Body::empty()).unwrap()).await
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or(Value::Null) };
    (status, value)
}
