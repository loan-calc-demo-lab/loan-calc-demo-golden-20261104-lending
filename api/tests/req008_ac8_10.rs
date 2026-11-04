// Tests for REQ-008 AC8, AC9, AC10
mod common;

use loan_api;
use serde_json::json;

#[tokio::test]
async fn req008_ac8_amount_out_of_range() {
    let app = loan_api::app();
    let path = "/offers";
    let (status, body) = common::post_json(
        &app,
        path,
        json!({
            "loan_id": "PL-1001",
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
    let path = "/offers";
    // No valid top-up amount can make the new loan exceed €30,000, so we test the amount-out-of-range message.
    let (status, body) = common::post_json(
        &app,
        path,
        json!({
            "loan_id": "PL-1001",
            "customer_id": "CUST-1001",
            "topup_amount": 21000,
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
async fn req008_ac10_term_out_of_range() {
    let app = loan_api::app();
    let path = "/offers";
    let (status, body) = common::post_json(
        &app,
        path,
        json!({
            "loan_id": "PL-1001",
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
