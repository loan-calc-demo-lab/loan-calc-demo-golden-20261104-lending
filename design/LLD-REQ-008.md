## Scope
- AC1: Eligibility – reject if customer does not exist, top-up amount out of allowed range, or term months not permitted (return 422 with field errors).
- AC2: Offer calculation – compute indicative offer with fields: offer_id, loan_id, customer_id, indicative=true, label, topup_amount, term_months, new_loan_amount, annual_rate_pct, arrangement_fee, amount_paid_out, monthly_payment, apr_pct, total_cost_of_credit, total_payable, created_at, valid_until (return 201).
- AC3: Offer expiry – offers are valid for 48 h; after that acceptance returns 410.
- AC4: Idempotent acceptance – Idempotency-Key header required; repeat with same key returns same 200 response, no second payout.
- AC5: Acceptance success – on first accept, create payout via Deposits, return 200 with offer_id, loan_id, status="accepted", accepted_at, amount_paid_in, account_id, new_monthly_payment, first_payment_date, payout_id, withdrawal_reminder.
- AC6: Payout error handling – propagate Deposits errors: 404 unknown account, 409 duplicate key, 422 amount not positive, 503 payout not confirmed.
- AC7: Conflict handling – if Idempotency-Key used with different payload or for different offer, return 409.
- AC8: Validation of request fields – detailed field error messages per rulebook (R1-R3 figures) with proper customer messages.
- AC9: Range checks – ensure top-up amount and term months respect configured min/max.
- AC10: Security – reject unauthenticated requests (handled upstream).
- AC11: Expiry handling – same as AC3.
- AC12: Acceptance and payout – same as AC5.
- AC13: Repeat acceptance – same as AC4.

## Files
- api/src/topup.rs: new module implementing top-up offer generation and acceptance logic.
- api/src/payouts.rs: new module (or extend existing) for calling the deposits payout service.
- api/src/lib.rs: add routes `/loans//topup-offers` and `/topup-offers//accept` and import handlers.

## Types and functions
```rust
// Request payload for top-up offer (snake_case fields)
#[derive(serde::Deserialize)]
pub struct TopupOfferRequest {
    pub customer_id: String,
    pub topup_amount: f64,
    pub term_months: u32,
}

// Response payload for indicative offer (snake_case, percentages *_pct)
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

// Acceptance request – no body, Idempotency-Key header required
pub struct AcceptTopupParams {
    pub offer_id: String,
}

// Acceptance response (snake_case)
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

// Handler signatures using axum patterns
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
| AC | Steps | Rules | Error mapping |
|----|-------|-------|---------------|
| AC1 | Validate `customer_id` exists, `topup_amount` within min/max, `term_months` allowed. | TU-1a, TU-1b, TU-1c | 422 with field-specific error codes/message per rulebook |
| AC2 | Compute indicative offer using rulebook calculations (R1-R3). Populate all response fields. | TU-2a-TU-4b | 201 on success |
| AC3 | Set `valid_until = now + 48h`. If acceptance after this, reject. | TU-4c | 410 Offer expired |
| AC4 | Require `Idempotency-Key`. Store acceptance keyed by `(offer_id, idempotency_key)`. Repeat same key returns stored response, no extra payout. | TU-5e, TU-5f, ARC-7 | 200 same payload |
| AC5 | On first acceptance, call Deposits `/payouts` via `PayoutClient`. Return full acceptance response. | TU-5a-TU-5d | 200 success |
| AC6 | Propagate Deposits errors: 404 → 404, 409 → 409, 422 → 422, 503 → 503 (payout not confirmed). | ARC-7 | Corresponding status codes |
| AC7 | If same `Idempotency-Key` used with different payload or for different offer, return 409. | TU-5g | 409 Conflict |
| AC8-AC10 | Field-level validation errors with precise customer messages (R1-R3). | TU-1a-TU-1c | 422 |
| AC11-AC13 | Repeat acceptance handling as per AC4/AC5. | TU-5e-TU-5f | 200 or appropriate error |

## Test plan
- `api/tests/req008_ac1_eligibility.rs`: tests for each validation error defined in AC1, expecting 422 with correct field messages.
- `api/tests/req008_ac2_offer_calculation.rs`: verifies successful offer generation per AC2, checking all response fields.
- `api/tests/req008_ac3_offer_expiry.rs`: attempts acceptance after `valid_until` and expects 410.
- `api/tests/req008_ac4_idempotent_accept.rs`: repeats acceptance with same Idempotency-Key, expects identical 200 response and no second payout.
- `api/tests/req008_ac5_acceptance_success.rs`: full acceptance flow including payout call, expects 200 with AC5 response.
- `api/tests/req008_ac6_payout_error_404.rs`: mocks Deposits returning 404 and verifies propagation.
- `api/tests/req008_ac7_conflict.rs`: uses same Idempotency-Key with different payload, expects 409.
- `api/tests/req008_ac8_field_errors.rs`: triggers each field validation rule (R1-R3) and checks error messages.
- `api/tests/req008_ac9_range_checks.rs`: tests min/max limits for amount and term months.
- `api/tests/req008_ac10_security.rs`: ensures unauthenticated request is rejected (handled upstream).
- `api/tests/req008_ac11_expiry_handling.rs`: duplicate of AC3 for clarity.
- `api/tests/req008_ac12_acceptance_and_payout.rs`: end-to-end acceptance and payout verification.
- `api/tests/req008_ac13_repeat_acceptance.rs`: repeats acceptance after success, expects same result.

## Out of scope
- UI changes in the iOS app.
- OAuth2 authentication handling.
- Persistent storage beyond in-memory demo.