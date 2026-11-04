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
async fn quote_returns_payment_and_schedule() {
    let (status, v) = post_quote(json!({"principal": 10000, "annual_rate_pct": 5, "term_months": 12, "upfront_fee": 250})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["monthly_payment"], 856.07);
    assert_eq!(v["upfront_fee"], 250.0);
    assert_eq!(v["schedule"].as_array().unwrap().len(), 12);
}

#[tokio::test]
async fn upfront_fee_is_optional() {
    let (status, v) = post_quote(json!({"principal": 1200, "annual_rate_pct": 0, "term_months": 12})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["total_payable"], 1200.0);
}

#[tokio::test]
async fn invalid_input_is_rejected_with_the_field_name() {
    let (status, v) = post_quote(json!({"principal": 10000, "annual_rate_pct": 45, "term_months": 12})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["field"], "annual_rate_pct");
}

#[tokio::test]
async fn existing_loans_of_a_customer() {
    let res = loan_api::app()
        .oneshot(Request::get("/customers/CUST-1001/loans").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["loans"][0]["loan_id"], "PL-1001");
    assert_eq!(v["loans"][0]["in_arrears"], false);
}

mod common;

#[tokio::test]
async fn helpers_post_json_with_headers() {
    let app = loan_api::app();
    let (status, v) = common::post_json(&app, "/loan/quote",
        json!({"principal": 1200, "annual_rate_pct": 0, "term_months": 12}), &[("x-demo", "1")]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["monthly_payment"], 100.0);
}
