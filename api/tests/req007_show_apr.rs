use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn post_quote(body: Value) -> (StatusCode, Value) {
    let res = loan_api::app()
        .oneshot(
            Request::post("/loan/quote")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn req007_apr_with_arrangement_fee() {
    let (status, v) = post_quote(json!({"principal": 10000, "annual_rate_pct": 5, "term_months": 12, "upfront_fee": 250})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["apr_pct"], 10.2);
    assert_eq!(v["total_cost_of_credit"], 522.89);
}

#[tokio::test]
async fn req007_apr_without_arrangement_fee() {
    let (status, v) = post_quote(json!({"principal": 10000, "annual_rate_pct": 5, "term_months": 12, "upfront_fee": 0})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["apr_pct"], 5.1);
    assert_eq!(v["total_cost_of_credit"], 272.89);
}

#[tokio::test]
async fn req007_apr_zero_rate_with_fee() {
    let (status, v) = post_quote(json!({"principal": 1200, "annual_rate_pct": 0, "term_months": 12, "upfront_fee": 60})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["apr_pct"], 10.0);
    assert_eq!(v["total_cost_of_credit"], 60.0);
}
