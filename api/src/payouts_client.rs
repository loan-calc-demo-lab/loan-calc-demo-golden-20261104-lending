//! REQ-008: the call from Lending to Deposits for a payout (contract deposits-payouts-v1, POST /payouts).
//!
//! House pattern "calls to another domain": a trait held in AppState, an HTTP implementation and a test double.
//! The trait is synchronous; handlers call it inside `tokio::task::spawn_blocking`.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutRequest {
    pub account_id: String,
    pub amount: f64,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PayoutReceipt {
    pub payout_id: String,
    pub new_balance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PayoutError {
    /// Deposits could not be reached or did not answer in time: retry later with the same key (ARC-7).
    Unavailable(String),
    /// Deposits answered with an error status.
    Rejected(u16, String),
}

pub trait PayoutClient: Send + Sync {
    /// POST {DEPOSITS_API_BASE_URL}/payouts with header Idempotency-Key = idempotency_key.
    fn payout(&self, idempotency_key: &str, request: &PayoutRequest) -> Result<PayoutReceipt, PayoutError>;
}

/// The real client: reqwest (blocking) with a 5 s timeout (ARC-10); base URL from env DEPOSITS_API_BASE_URL.
pub struct HttpPayoutClient {
    base_url: String,
    client: std::sync::OnceLock<reqwest::blocking::Client>,
}

impl HttpPayoutClient {
    pub fn from_env() -> Self {
        let base_url = std::env::var("DEPOSITS_API_BASE_URL").unwrap_or_else(|_| "http://localhost:8081".into());
        // The blocking client is built on first use, inside spawn_blocking: it must not be created in the async runtime.
        HttpPayoutClient { base_url: base_url.trim_end_matches('/').to_string(), client: std::sync::OnceLock::new() }
    }
}

impl PayoutClient for HttpPayoutClient {
    fn payout(&self, idempotency_key: &str, request: &PayoutRequest) -> Result<PayoutReceipt, PayoutError> {
        let res = self
            .client
            .get_or_init(reqwest::blocking::Client::new)
            .post(format!("{}/payouts", self.base_url))
            .timeout(Duration::from_secs(5))
            .header("Idempotency-Key", idempotency_key)
            .json(request)
            .send()
            .map_err(|e| PayoutError::Unavailable(e.to_string()))?;
        let status = res.status().as_u16();
        if status == 200 || status == 201 {
            res.json::<PayoutReceipt>().map_err(|e| PayoutError::Unavailable(e.to_string()))
        } else if status >= 500 {
            Err(PayoutError::Unavailable(format!("HTTP {status}")))
        } else {
            Err(PayoutError::Rejected(status, res.text().unwrap_or_default()))
        }
    }
}

/// One call the test double received.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedPayout {
    pub idempotency_key: String,
    pub account_id: String,
    pub amount: f64,
    pub reference: String,
}

/// Test double: records every call; answers a new key with a receipt and a repeated key with the first receipt.
/// `fail_next(n)` makes the next n calls fail as unavailable (to test retries).
pub struct RecordingPayoutClient {
    pub recorded: Mutex<Vec<RecordedPayout>>,
    receipts: Mutex<Vec<(String, PayoutReceipt)>>,
    failures: Mutex<u32>,
}

impl RecordingPayoutClient {
    pub fn new() -> Self {
        RecordingPayoutClient { recorded: Mutex::new(vec![]), receipts: Mutex::new(vec![]), failures: Mutex::new(0) }
    }

    /// Calls that reached Deposits successfully, i.e. payouts made (a repeated key is not a new payout).
    pub fn calls(&self) -> Vec<RecordedPayout> {
        self.recorded.lock().unwrap().clone()
    }

    pub fn fail_next(&self, n: u32) {
        *self.failures.lock().unwrap() = n;
    }
}

impl Default for RecordingPayoutClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PayoutClient for RecordingPayoutClient {
    fn payout(&self, idempotency_key: &str, request: &PayoutRequest) -> Result<PayoutReceipt, PayoutError> {
        {
            let mut f = self.failures.lock().unwrap();
            if *f > 0 {
                *f -= 1;
                return Err(PayoutError::Unavailable("simulated outage".into()));
            }
        }
        let mut receipts = self.receipts.lock().unwrap();
        if let Some((_, r)) = receipts.iter().find(|(k, _)| k == idempotency_key) {
            return Ok(r.clone());
        }
        let receipt = PayoutReceipt { payout_id: format!("PO-{}", receipts.len() + 1), new_balance: 0.0 };
        receipts.push((idempotency_key.to_string(), receipt.clone()));
        self.recorded.lock().unwrap().push(RecordedPayout {
            idempotency_key: idempotency_key.to_string(),
            account_id: request.account_id.clone(),
            amount: request.amount,
            reference: request.reference.clone(),
        });
        Ok(receipt)
    }
}
