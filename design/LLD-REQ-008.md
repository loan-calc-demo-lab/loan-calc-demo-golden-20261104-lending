## Scope
- AC1-AC13: All acceptance criteria for loan top-up as defined in the impact analysis (Eligibility checks, offer calculation, expiry, idempotent acceptance and payout).

## Files
- api/src/topup.rs: new module implementing top-up offer generation and acceptance logic.
- api/src/payouts.rs: new module (or extend existing) for calling the deposits payout service.
- api/src/lib.rs: add routes `/loans//topup-offers` and `/topup-offers//accept` and import handlers.

## Types and functions
```rust
// Request payload for top-up offer
#[derive(serde::Deserialize)]
pub struct TopupOfferRequest {
    pub customer_id: String,
    pub topup_amount: f64,
    pub term_months: u32,
}

// Response payload for indicative offer
#[derive(serde::Serialize)]
pub struct TopupOfferResponse {
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

// Acceptance request (no body, Idempotency-Key header required)
pub struct AcceptTopupParams {
    pub offer_id: String,
}

// Acceptance response
#[derive(serde::Serialize)]
pub struct AcceptTopupResponse {
    pub offer_id: String,
    pub loan_id: String,
    pub status: String, // "accepted"
    pub accepted_at: String,
    pub amount_paid_in: f64,
    pub account_id: String,
    pub new_monthly_payment: f64,
    pub first_payment_date: String,
    pub payout_id: String,
    pub withdrawal_reminder: String,
}

// Handler signatures
async fn topup_offer_handler(
    State(state): State<AppState>,
    Json(req): Json<TopupOfferRequest>,
) -> impl IntoResponse;

async fn topup_accept_handler(
    State(state): State<AppState>,
    Path(params): Path<AcceptTopupParams>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse;
```

### House pattern calls
- Use `PayoutClient` (defined in `api/src/payouts.rs`) to call the deposits service.
- Record payout with `RecordingPayoutClient` in tests.
- Advance simulated time with `AppState::advance_clock` where needed.

## Behaviour
- **Eligibility (TU-1a-TU-1c)**: Validate `customer_id` exists, `topup_amount` within allowed range, `term_months` allowed. Return 422 with field errors on failure.
- **Offer calculation (TU-2a-TU-4b)**: Apply business rules from the rulebook to compute rates, fees, APR, total cost, etc. Return 201 with `TopupOfferResponse`.
- **Offer expiry (TU-4c)**: `valid_until` set to now + 48 h. If acceptance after expiry, return 410.
- **Idempotent acceptance (TU-5e-TU-5f, ARC-7)**: Require `Idempotency-Key` header. Store acceptance keyed by `(offer_id, idempotency_key)`. Subsequent identical calls return the same `AcceptTopupResponse` with status 200 and no additional payout.
- **Payout call (deposits-payouts-v1)**: Send POST `/payouts` with `Idempotency-Key`. Handle 201, 200 (repeat), 404, 409, 422 as per contract. Map errors to appropriate HTTP status for the client.
- **Error mapping**: Validation errors → 422, unknown loan/offer → 404, duplicate idempotency with different payload → 409, expired offer → 410, payout not confirmed → 503.

## Test plan
- `api/tests/req008_ac1_eligibility.rs`: tests for missing customer, amount out of range, term out of range → 422.
- `api/tests/req008_ac2_offer_calculation.rs`: happy path returns 201 with correct fields.
- `api/tests/req008_ac3_offer_expiry.rs`: acceptance after `valid_until` → 410.
- `api/tests/req008_ac4_idempotent_accept.rs`: repeat request with same Idempotency-Key returns same payload, no second payout.
- `api/tests/req008_ac5_payout_errors.rs`: simulate deposits returning 404, 409, 422 and verify propagation.

## Out of scope
- UI changes in the iOS app.
- OAuth2 authentication handling.
- Persistent storage beyond in-memory demo.
