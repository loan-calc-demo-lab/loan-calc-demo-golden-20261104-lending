mod common;

use serde_json::json;
use loan_api::app_with;
use loan_api::AppState;
use loan_api::payouts_client::RecordingPayoutClient;
use std::sync::Arc;

#[tokio::test]
async fn req008_ac1_in_arrears_message() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1002",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let resp = common::post_json(&app, "/loans/PL-1002/topup-offers", req, &[ ("Idempotency-Key", "k1") ]).await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(&resp.body()).unwrap();
    assert_eq!(body["message"], "A top-up is not possible while a payment is overdue.");
}

#[tokio::test]
async fn req008_ac2_too_new_message() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1003",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let resp = common::post_json(&app, "/loans/PL-1003/topup-offers", req, &[ ("Idempotency-Key", "k1") ]).await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(&resp.body()).unwrap();
    assert_eq!(body["message"], "A top-up is possible after your sixth monthly payment.");
}

#[tokio::test]
async fn req008_ac3_no_account_message() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-9999",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let resp = common::post_json(&app, "/loans/PL-9999/topup-offers", req, &[ ("Idempotency-Key", "k1") ]).await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(&resp.body()).unwrap();
    assert_eq!(body["message"], "We could not find your current account. Please contact us.");
}
