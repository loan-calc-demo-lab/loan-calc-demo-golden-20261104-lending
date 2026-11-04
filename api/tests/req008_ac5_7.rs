mod common;

use serde_json::json;
use loan_api::app_with;
use loan_api::AppState;
use loan_api::payouts_client::RecordingPayoutClient;
use std::sync::Arc;

#[tokio::test]
async fn req008_ac5_r1_offer_fields() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (status, body) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[ ("Idempotency-Key", "k1") ]).await;
    assert_eq!(status, 201);
    let offer: serde_json::Value = body;
    assert_eq!(offer["topup_amount"], 5000.0);
    assert_eq!(offer["term_months"], 60);
    assert_eq!(offer["new_loan_amount"], 13940.27);
    assert_eq!(offer["annual_rate_pct"], 9.9);
    assert_eq!(offer["arrangement_fee"], 50.0);
    assert_eq!(offer["amount_paid_out"], 4950.0);
    assert_eq!(offer["monthly_payment"], 295.5);
    assert_eq!(offer["apr_pct"], 10.5);
    assert_eq!(offer["total_cost_of_credit"], 3840.05);
    assert_eq!(offer["total_payable"], 17780.32);
}

#[tokio::test]
async fn req008_ac6_r2_offer_fields() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 7000.0,
        "term_months": 72
    });
    let (status, body) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[ ("Idempotency-Key", "k2") ]).await;
    assert_eq!(status, 201);
    let offer: serde_json::Value = body;
    assert_eq!(offer["topup_amount"], 7000.0);
    assert_eq!(offer["term_months"], 72);
    assert_eq!(offer["new_loan_amount"], 15940.27);
    assert_eq!(offer["annual_rate_pct"], 8.9);
    assert_eq!(offer["arrangement_fee"], 70.0);
    assert_eq!(offer["amount_paid_out"], 6930.0);
    assert_eq!(offer["monthly_payment"], 286.54);
    assert_eq!(offer["apr_pct"], 9.4);
    assert_eq!(offer["total_cost_of_credit"], 4760.72);
    assert_eq!(offer["total_payable"], 20700.99);
}

#[tokio::test]
async fn req008_ac7_r3_offer_fields() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 1000.0,
        "term_months": 48
    });
    let (status, body) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[ ("Idempotency-Key", "k3") ]).await;
    assert_eq!(status, 201);
    let offer: serde_json::Value = body;
    assert_eq!(offer["topup_amount"], 1000.0);
    assert_eq!(offer["term_months"], 48);
    assert_eq!(offer["new_loan_amount"], 9940.27);
    assert_eq!(offer["annual_rate_pct"], 9.9);
    assert_eq!(offer["arrangement_fee"], 50.0);
    assert_eq!(offer["amount_paid_out"], 950.0);
    assert_eq!(offer["monthly_payment"], 251.63);
    assert_eq!(offer["apr_pct"], 10.7);
    assert_eq!(offer["total_cost_of_credit"], 2188.19);
    assert_eq!(offer["total_payable"], 12128.46);
}
