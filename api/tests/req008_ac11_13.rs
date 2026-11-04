// Fixed tests for REQ-008 AC11-13
mod common;

use serde_json::json;
use loan_api::app_with;
use loan_api::AppState;
use loan_api::payouts_client::RecordingPayoutClient;
use std::sync::Arc;

#[tokio::test]
async fn req008_ac11_offer_success() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (status, body) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[]).await;
    assert_eq!(status, 200);
    // Verify all expected fields from R1 (values taken from knowledge base)
    assert_eq!(body["new_loan_amount"], 13940.27);
    assert_eq!(body["annual_rate_pct"], 9.9);
    assert_eq!(body["arrangement_fee"], 50.0);
    assert_eq!(body["amount_paid_out"], 4950.0);
    assert_eq!(body["monthly_payment"], 295.50);
    assert_eq!(body["apr_pct"], 10.5);
    assert_eq!(body["total_cost_of_credit"], 3840.05);
    assert_eq!(body["total_payable"], 17780.32);
}

#[tokio::test]
async fn req008_ac12_accept_success_and_payout() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    // first create offer
    let offer_req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (offer_status, offer_body) = common::post_json(&app, "/loans/PL-1001/topup-offers", offer_req, &[]).await;
    assert_eq!(offer_status, 200);
    let offer_id = offer_body["offer_id"].as_str().unwrap();
    // accept the offer
    let accept_req = json!({"offer_id": offer_id});
    let (accept_status, accept_body) = common::post_json(&app, "/loans/PL-1001/topup-accept", accept_req, &[ ("Idempotency-Key", "k1") ]).await;
    assert_eq!(accept_status, 200);
    assert_eq!(accept_body["status"], "accepted");
    // payout should be recorded once
    assert_eq!(payouts.calls().len(), 1);
    let call = &payouts.calls()[0];
    assert_eq!(call.amount, 4950.0);
    assert_eq!(call.account_id, "CA-2001");
}

#[tokio::test]
async fn req008_ac13_offer_expired() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    // create offer
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (status, body) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[]).await;
    assert_eq!(status, 200);
    let offer_id = body["offer_id"].as_str().unwrap();
    // advance clock beyond validity (24h + 1s)
    state.advance_clock(24 * 3600 + 1);
    // attempt accept
    let accept_req = json!({"offer_id": offer_id});
    let (acc_status, acc_body) = common::post_json(&app, "/loans/PL-1001/topup-accept", accept_req, &[ ("Idempotency-Key", "k1") ]).await;
    assert_eq!(acc_status, 410);
    assert_eq!(acc_body["message"], "This offer has expired. Please ask for a new offer.");
}
