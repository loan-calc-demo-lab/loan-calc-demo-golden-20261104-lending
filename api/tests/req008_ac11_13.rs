mod common;

use serde_json::json;
use loan_api::app_with;
use loan_api::AppState;
use loan_api::payouts_client::RecordingPayoutClient;
use std::sync::Arc;

#[tokio::test]
async fn req008_ac11_offer_expires_after_24h() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let mut state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (status, offer) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[]).await;
    assert_eq!(status, 201);
    let offer_id = offer["offer_id"].as_str().unwrap();
    // advance clock beyond 24h
    state.advance_clock(24 * 3600 + 1);
    let accept_path = format!("/topup-offers/{}/accept", offer_id);
    let (status2, body2) = common::post_json(&app, &accept_path, json!({"customer_id": "CUST-1001"}), &[ ("Idempotency-Key", "k1") ]).await;
    assert_eq!(status2, 410);
    let body = body2;
    assert_eq!(body["message"], "This offer has expired. Please ask for a new offer.");
}

#[tokio::test]
async fn req008_ac12_accept_payout_and_confirmation() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let mut state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    // create offer
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (_, offer) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[]).await;
    let offer_id = offer["offer_id"].as_str().unwrap();
    // accept
    let accept_path = format!("/topup-offers/{}/accept", offer_id);
    let (status2, body2) = common::post_json(&app, &accept_path, json!({"customer_id": "CUST-1001"}), &[ ("Idempotency-Key", "k2") ]).await;
    assert_eq!(status2, 200);
    let acc = body2;
    assert_eq!(acc["amount_paid_in"], 4950.0);
    assert_eq!(acc["account_id"], "CA-2001");
    assert_eq!(acc["new_monthly_payment"], 295.5);
    // check payout recorded
    assert_eq!(payouts.calls().len(), 1);
    let payout = &payouts.calls()[0];
    assert_eq!(payout.amount, 4950.0);
    assert_eq!(payout.account_id, "CA-2001");
}

#[tokio::test]
async fn req008_ac13_repeat_accept_idempotent() {
    let payouts = Arc::new(RecordingPayoutClient::new());
    let mut state = loan_api::AppState::with_payouts(payouts.clone());
    let app = loan_api::app_with(state.clone());
    // create offer
    let req = json!({
        "customer_id": "CUST-1001",
        "topup_amount": 5000.0,
        "term_months": 60
    });
    let (_, offer) = common::post_json(&app, "/loans/PL-1001/topup-offers", req, &[]).await;
    let offer_id = offer["offer_id"].as_str().unwrap();
    // first accept
    let accept_path = format!("/topup-offers/{}/accept", offer_id);
    let (status2, body2) = common::post_json(&app, &accept_path, json!({"customer_id": "CUST-1001"}), &[ ("Idempotency-Key", "k2") ]).await;
    assert_eq!(status2, 200);
    // second accept with same Idempotency-Key
    let (status3, body3) = common::post_json(&app, &accept_path, json!({"customer_id": "CUST-1001"}), &[ ("Idempotency-Key", "k2") ]).await;
    assert_eq!(status3, 200);
    // only one payout recorded
    assert_eq!(payouts.calls().len(), 1);
}
