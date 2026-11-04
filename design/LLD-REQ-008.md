## Scope
- AC1-AC13: All acceptance criteria for loan top-up as defined in the impact analysis (Eligibility checks, offer calculation, expiry, idempotent acceptance and payout).

## Files
- api/src/topup.rs: new module implementing top-up offer generation and acceptance logic.
- api/src/payout_client.rs: client for calling the deposits-payouts-v1 API.
- api/src/lib.rs: add routes `/loans//topup-offers` (POST) and `/topup-offers//accept` (POST) with Idempotency-Key header handling.

## Types and functions
```rust
// In api/src/topup.rs
pub struct TopupRequest {
    pub customer_id: String,
    pub topup_amount: f64,
    pub term_months: u32,
}

pub struct OfferResponse {
    pub offer_id: String,
    pub loan_id: String,
    pub customer_id: String,
    pub indicative: bool,
    pub label: String,
    pub topup_amount: f64,
    pub term_months: u32,
    pub new_loan_amount: f64,
    pub annual_rate_pct: f64,
    pub arrangement_fee: f64,
    pub amount_paid_out: f64,
    pub monthly_payment: f64,
    pub apr_pct: f64,
    pub total_cost_of_credit: f64,
    pub total_payable: f64,
    pub created_at: String,
    pub valid_until: String,
}

pub struct AcceptRequest {}

pub struct AcceptResult {
    pub offer_id: String,
    pub loan_id: String,
    pub status: String,
    pub accepted_at: String,
    pub amount_paid_in: f64,
    pub account_id: String,
    pub new_monthly_payment: f64,
    pub first_payment_date: String,
    pub payout_id: String,
    pub withdrawal_reminder: String,
}

// Handler signatures in lib.rs
async fn topup_offer_handler(State(state): State<AppState>, Json(req): Json<TopupRequest>) -> impl IntoResponse;
async fn topup_accept_handler(State(state): State<AppState>, HeaderMap(headers): HeaderMap, Json(req): Json<AcceptRequest>) -> impl IntoResponse;

// In api/src/payout_client.rs
pub struct PayoutRequest { pub account_id: String, pub amount: f64, pub reference: String }
pub struct PayoutResponse { pub payout_id: String, pub account_id: String, pub amount: f64, pub reference: String, pub new_balance: f64, pub created_at: String }
pub async fn post_payout(state: &AppState, req: PayoutRequest, idempotency_key: &str) -> Result<PayoutResponse, ServiceError>;
```

## Behaviour
- **Eligibility (AC1-AC4)**: `topup_offer_handler` validates `customer_id` exists, `topup_amount` within allowed range, `term_months` allowed values, and loan is eligible (TU-1a-TU-1c). Errors map to 404 or 422 with rule-based messages.
- **Offer calculation (AC5-AC10)**: Uses existing loan calculation module (`calc::quote`) extended with top-up parameters, applies arrangement fee and APR rules (TU-2a-TU-4b). Returns 201 with `OfferResponse`.
- **Offer expiry (AC11)**: `valid_until` set to `created_at + 15 min`. If a later accept request is after expiry, returns 410.
- **Idempotent acceptance (AC12-AC13)**: `topup_accept_handler` reads `Idempotency-Key` header. Stores acceptance keyed by `(offer_id, Idempotency-Key)`. Re-play returns stored `AcceptResult` with 200 and no second payout (TU-5e). Mismatch key returns 409. If loan no longer eligible returns 422. If payout not confirmed returns 503 (retry same key) – ARC-7.
- **Payout call**: Calls `post_payout` with same `Idempotency-Key`. Handles 201, 200 (repeat), 404, 409, 422 as per contract.

## Test plan
- `api/tests/req008_ac1_eligibility.rs`: test unknown customer → 404.
- `api/tests/req008_ac2_amount_range.rs`: amount out of range → 422.
- `api/tests/req008_ac5_offer_calculation.rs`: valid request returns 201 with correct fields.
- `api/tests/req008_ac11_expiry.rs`: accept after expiry → 410.
- `api/tests/req008_ac12_idempotent_accept.rs`: repeat accept with same Idempotency-Key returns same result, no second payout.
- `api/tests/req008_ac13_payout_error.rs`: mock payout 503 and ensure 503 returned, retry possible.

## Out of scope
- UI changes in the iOS app.
- OAuth2 authentication handling.
- Persistent storage beyond in-memory demo structures.
