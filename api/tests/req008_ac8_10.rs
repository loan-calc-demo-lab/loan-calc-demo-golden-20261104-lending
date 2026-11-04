// Tests for REQ-008 AC8, AC9, AC10
mod common;

use loan_api;
use serde_json::json;

#[tokio::test]
async fn req008_ac8_amount_out_of_range() {
    let app = loan_api::app();
    let path = "/loans/PL-1001/topup-offers";
    let (status, body) = common::post_json(
        &app,
        path,
        json!({
            "customer_id": "CUST-1001",
            "topup_amount": 999,
            "term_months": 60
        }),
        &[],
    )
    .await;
    assert_eq!(status, 422);
    assert_eq!(
        body["message"].as_str().unwrap(),
        "Please choose a top-up amount between €1,000 and €20,000."
    );
}

#[tokio::test]
async fn req008_ac9_new_loan_too_large() {
    let app = loan_api::app();
    // PL-1005 has more than €10,000 outstanding, so the largest allowed top-up (€20,000) exceeds €30,000 (TU-2b).
    let path = "/loans/PL-1005/topup-offers";
    let (status, body) = common::post_json(
        &app,
        path,
        json!({
            "customer_id": "CUST-1005",
            "topup_amount": 20000,
            "term_months": 60
        }),
        &[],
    )
    .await;
    assert_eq!(status, 422);
    assert_eq!(
        body["message"].as_str().unwrap(),
        "The new loan amount can be at most €30,000."
    );
}

#[tokio::test]
async fn req008_ac10_term_out_of_range() {
    let app = loan_api::app();
    let path = "/loans/PL-1001/topup-offers";
    let (status, body) = common::post_json(
        &app,
        path,
        json!({
            "customer_id": "CUST-1001",
            "topup_amount": 5000,
            "term_months": 11
        }),
        &[],
    )
    .await;
    assert_eq!(status, 422);
    assert_eq!(
        body["message"].as_str().unwrap(),
        "Please choose a term between 12 and 84 months."
    );
}
