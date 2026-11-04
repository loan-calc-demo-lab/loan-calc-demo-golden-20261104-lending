pub mod calc;
pub mod loans;
pub mod payouts_client;
pub mod topup;

use axum::{extract::{Path, State}, http::{HeaderMap, StatusCode}, response::IntoResponse, routing::{get, post}, Json, Router};
use payouts_client::{HttpPayoutClient, PayoutClient, PayoutError, PayoutRequest};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use topup::{Acceptance, OfferState, TopupError};

/// Shared state of the service: the loan book, the top-up offers, the client for Deposits and the clock.
#[derive(Clone)]
pub struct AppState {
    pub loans: Arc<Mutex<loans::LoanBook>>,
    pub offers: Arc<Mutex<topup::OfferBook>>,
    pub payouts: Arc<dyn PayoutClient>,
    /// Test-only offset added to the current time (house pattern "time in tests").
    pub clock_offset_secs: Arc<AtomicU64>,
}

impl AppState {
    /// Demo data, payouts through the Deposits service (env DEPOSITS_API_BASE_URL).
    pub fn demo() -> Self {
        Self::with_payouts(Arc::new(HttpPayoutClient::from_env()))
    }

    /// Demo data with the given client for Deposits (tests use RecordingPayoutClient).
    pub fn with_payouts(payouts: Arc<dyn PayoutClient>) -> Self {
        AppState {
            loans: Arc::new(Mutex::new(loans::LoanBook::demo())),
            offers: Arc::new(Mutex::new(topup::OfferBook::default())),
            payouts,
            clock_offset_secs: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn now_secs(&self) -> u64 {
        let real = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        real + self.clock_offset_secs.load(Ordering::SeqCst)
    }

    /// Tests: move the clock forward, e.g. to expire an offer.
    pub fn advance_clock(&self, seconds: u64) {
        self.clock_offset_secs.fetch_add(seconds, Ordering::SeqCst);
    }
}

/// The service with fresh demo data.
pub fn app() -> Router {
    app_with(AppState::demo())
}

pub fn app_with(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route("/loan/quote", post(quote_handler))
        .route("/customers/:customer_id/loans", get(customer_loans))
        .route("/loans/:loan_id/topup-offers", post(topup_offer))
        .route("/topup-offers/:offer_id/accept", post(accept_offer))
        .with_state(state)
}

async fn quote_handler(Json(req): Json<calc::QuoteRequest>) -> impl IntoResponse {
    match calc::quote(&req) {
        Ok(q) => (StatusCode::OK, Json(serde_json::to_value(q).unwrap())),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": "validation", "field": e.field, "message": e.message}))),
    }
}

/// The customer's existing loans (empty list for an unknown customer).
async fn customer_loans(State(state): State<AppState>, Path(customer_id): Path<String>) -> impl IntoResponse {
    let loans = state.loans.lock().unwrap().for_customer(&customer_id);
    Json(json!({ "customer_id": customer_id, "loans": loans }))
}

fn error(e: TopupError) -> (StatusCode, Json<Value>) {
    let mut body = json!({"error": e.error, "message": e.message});
    if let Some(f) = e.field {
        body["field"] = json!(f);
    }
    (StatusCode::from_u16(e.status).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY), Json(body))
}

/// REQ-008 AC1–AC4: POST /loans/{loan_id}/topup-offers – an indicative offer (TU-1..TU-4).
async fn topup_offer(State(state): State<AppState>, Path(loan_id): Path<String>, Json(req): Json<topup::OfferRequest>) -> impl IntoResponse {
    let loan = match state.loans.lock().unwrap().get(&loan_id) {
        Some(l) if l.customer_id == req.customer_id => l.clone(),
        _ => return error(topup::UNKNOWN_LOAN), // also another customer's loan: never confirm it exists (ARC-8)
    };
    match topup::figures(&loan, req.topup_amount, req.term_months) {
        Ok(figures) => {
            let offer = state.offers.lock().unwrap().add(figures, state.now_secs());
            (StatusCode::CREATED, Json(serde_json::to_value(offer).unwrap()))
        }
        Err(e) => error(e),
    }
}

/// REQ-008 AC5–AC7: POST /topup-offers/{offer_id}/accept (TU-5, ARC-4, ARC-5, ARC-7).
/// Lending re-checks and recalculates, changes the loan, then asks Deposits to pay out with the offer id as the
/// idempotency key. A repeat returns the first result and pays nothing; a failed payout is retried with the same key.
async fn accept_offer(State(state): State<AppState>, Path(offer_id): Path<String>, headers: HeaderMap,
                      Json(req): Json<topup::AcceptRequest>) -> impl IntoResponse {
    let Some(key) = headers.get("Idempotency-Key").and_then(|v| v.to_str().ok()).filter(|k| !k.is_empty()).map(String::from) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "missing_idempotency_key", "message": "The Idempotency-Key header is required."})));
    };
    let now = state.now_secs();
    // 1. Decide under the locks; nothing slow happens while they are held.
    let (offer, accepted_at, account) = {
        let mut offers = state.offers.lock().unwrap();
        match offers.keys.get(&key) {
            Some(o) if *o != offer_id => return error(topup::KEY_CONFLICT),
            _ => {}
        }
        let Some(stored) = offers.offers.get(&offer_id).cloned() else { return error(topup::UNKNOWN_OFFER) };
        if stored.offer.customer_id != req.customer_id {
            return error(topup::UNKNOWN_OFFER);
        }
        match stored.state {
            OfferState::Accepted(a) => return (StatusCode::OK, Json(serde_json::to_value(a).unwrap())), // TU-5e
            OfferState::Pending { accepted_at_secs } => {
                let account = state.loans.lock().unwrap().get(&stored.offer.loan_id).map(|l| l.repayment_account_id.clone()).unwrap_or_default();
                offers.keys.insert(key.clone(), offer_id.clone());
                (stored.offer, accepted_at_secs, account)
            }
            OfferState::Open => {
                if now > stored.created_secs + topup::OFFER_VALIDITY_SECS {
                    return error(topup::EXPIRED); // TU-4c
                }
                let mut loans = state.loans.lock().unwrap();
                let Some(loan) = loans.loans.get_mut(&stored.offer.loan_id) else { return error(topup::UNKNOWN_LOAN) };
                // TU-5a: check again and recalculate from Lending's own data.
                let fresh = match topup::figures(loan, stored.offer.topup_amount, stored.offer.term_months) {
                    Ok(f) => f,
                    Err(e) => return error(e),
                };
                // TU-5c: the new loan replaces the existing one.
                loan.original_principal = fresh.new_loan_amount;
                loan.annual_rate_pct = fresh.annual_rate_pct;
                loan.term_months = fresh.term_months;
                loan.payments_made = 0;
                loan.remaining_term_months = fresh.term_months;
                loan.outstanding_balance = fresh.new_loan_amount;
                loan.monthly_payment = fresh.monthly_payment;
                let account = loan.repayment_account_id.clone();
                let mut offer = stored.offer.clone();
                offer.amount_paid_out = fresh.amount_paid_out;
                offer.monthly_payment = fresh.monthly_payment;
                offers.offers.get_mut(&offer_id).unwrap().state = OfferState::Pending { accepted_at_secs: now };
                offers.keys.insert(key.clone(), offer_id.clone());
                (offer, now, account)
            }
        }
    };
    // 2. TU-5d: pay out through Deposits, idempotency key = offer id (ARC-4, ARC-7).
    let payouts = state.payouts.clone();
    let request = PayoutRequest { account_id: account.clone(), amount: offer.amount_paid_out, reference: offer.offer_id.clone() };
    let pay_key = offer.offer_id.clone();
    let receipt = tokio::task::spawn_blocking(move || payouts.payout(&pay_key, &request))
        .await
        .unwrap_or_else(|e| Err(PayoutError::Unavailable(e.to_string())));
    let receipt = match receipt {
        Ok(r) => r,
        Err(_) => return error(topup::PAYOUT_PENDING), // the loan change stays pending; a retry pays with the same key
    };
    let acceptance = Acceptance {
        offer_id: offer.offer_id.clone(),
        loan_id: offer.loan_id.clone(),
        status: "accepted".into(),
        accepted_at: topup::iso_utc(accepted_at),
        amount_paid_in: offer.amount_paid_out,
        account_id: account,
        new_monthly_payment: offer.monthly_payment,
        first_payment_date: topup::first_payment_date(accepted_at),
        payout_id: receipt.payout_id,
        withdrawal_reminder: topup::WITHDRAWAL_REMINDER.into(),
    };
    // REG-5: the offer as shown and the acceptance are kept with the offer.
    state.offers.lock().unwrap().offers.get_mut(&offer_id).unwrap().state = OfferState::Accepted(acceptance.clone());
    (StatusCode::OK, Json(serde_json::to_value(acceptance).unwrap()))
}
