// Fixed tests for REQ-008 AC5-7
mod common;

use serde_json::json;
use loan_api::app_with;
use loan_api::AppState;
use loan_api::payouts_client::RecordingPayoutClient;
use std::sync::Arc;

#[tokio::test]
async fn req008_ac5_unknown_loan_404() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-9999",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (status, _body) = common::post_json(&app, "/loans/PL-9999/topup-offers", req, &[]).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn req008_ac6_invalid_amount_message() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 999.0,
        "term_months": 60
    });
    let (status, body) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[]).await;
    assert_eq!(status, 422);
    assert_eq!(body["message"], "Please choose a top-up amount between €1,000 and €20,000.");
}

#[tokio::test]
async fn req008_ac7_invalid_term_message() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 11
    });
    let (status, body) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[]).await;
    assert_eq!(status, 422);
    assert_eq!(body["message"], "Please choose a term between 12 and 84 months.");
}
