## Scope
- **AC1 (US1)** Given a loan that is in arrears, when the customer requests a top-up, then the system shows *Customer message TU-1a* "A top-up is not possible while a payment is overdue.".
- **AC2 (US1)** Given a loan with fewer than 6 payments made, when the customer requests a top-up, then the system shows *Customer message TU-1b* "A top-up is possible after your sixth monthly payment.".
- **AC3 (US1)** Given a loan without a repayment current account, when the customer requests a top-up, then the system shows *Customer message TU-1c* "We could not find your current account. Please contact us.".
- **AC4 (US1)** Given a loan that meets eligibility, when the customer requests a top-up, then the system proceeds to calculate the indicative offer.
- **AC5 (US2)** Given a top-up request of €5,000 for 60 months (reference R1), when eligibility is met, then the indicative offer displays:
  - top-up amount €5,000
  - term 60 months
  - new loan amount €13,940.27
  - rate 9.9 %
  - arrangement fee €50.00
  - amount paid out €4,950.00
  - monthly payment €295.50
  - APR 10.5 %
  - total cost of credit €3,840.05
  - total payable €17,780.32
  - label "Indicative offer – subject to final checks"
  as per **Top-up reference value R1**.
- **AC6 (US2)** Given a top-up request of €7,000 for 72 months (reference R2), when eligibility is met, then the indicative offer displays:
  - top-up amount €7,000
  - term 72 months
  - new loan amount €15,940.27
  - rate 8.9 %
  - arrangement fee €70.00
  - amount paid out €6,930.00
  - monthly payment €286.54
  - APR 9.4 %
  - total cost of credit €4,760.72
  - total payable €20,700.99
  as per **Top-up reference value R2**.
- **AC7 (US2)** Given a top-up request of €1,000 for 48 months (reference R3), when eligibility is met, then the indicative offer displays:
  - top-up amount €1,000
  - term 48 months
  - new loan amount €9,940.27
  - rate 9.9 %
  - arrangement fee €50.00
  - amount paid out €950.00
  - monthly payment €251.63
  - APR 10.7 %
  - total cost of credit €2,188.19
  - total payable €12,128.46
  as per **Top-up reference value R3**.
- **AC8 (US2)** Given a top-up amount outside €1,000-€20,000, when the customer enters the amount, then the system shows *Customer message TU-2a* "Please choose a top-up amount between €1,000 and €20,000.".
- **AC9 (US2)** Given a new loan amount that would exceed €30,000, when the calculation is performed, then the system shows *Customer message TU-2b* "The new loan amount can be at most €30,000.".
- **AC10 (US2)** Given a term outside 12-84 months, when the customer selects the term, then the system shows *Customer message TU-2c"Please choose a term between 12 and 84 months.".
- **AC11 (US3)** Given an indicative offer has been shown, when 24 hours pass without acceptance, then the offer expires and the system shows *Customer message TU-4c* "This offer has expired. Please ask for a new offer.".
- **AC12 (US4)** Given the customer accepts a valid offer, when the acceptance is processed, then the system:
  - deducts the arrangement fee from the payout
  - credits the net amount to the repayment account (e.g., account balance after payout R1: €6,200.00)
  - creates a new loan with the combined amount and new monthly payment
  - shows a confirmation containing the amount paid in, the account, the new monthly payment, the first payment date and a 14-day withdrawal reminder (TU-5f)
  - stores the offer details as shown and the acceptance timestamp (REG-5).
- **AC13 (US5)** Given the customer attempts to accept the same offer a second time, when the second acceptance is received, then the system returns the result of the first acceptance and no additional payout occurs (TU-5e).

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
| AC1 | System detects loan in arrears and returns TU-1a message. | TU-1a | 422 with message "A top-up is not possible while a payment is overdue." |
| AC2 | System detects fewer than 6 payments made and returns TU-1b message. | TU-1b | 422 with message "A top-up is possible after your sixth monthly payment." |
| AC3 | System detects missing repayment account and returns TU-1c message. | TU-1c | 422 with message "We could not find your current account. Please contact us." |
| AC4 | Eligibility passed, calculate indicative offer per R1-R3 and return 201 with offer details. | TU-2a-TU-4b | 201 |
| AC5 | Offer validity set to now+24h; after 24h acceptance returns TU-4c message. | TU-4c | 410 with message "This offer has expired. Please ask for a new offer." |
| AC6 | Idempotency-Key required; first acceptance triggers payout via Deposits, returns 200 with acceptance details. | TU-5a-TU-5f, ARC-7 | 200 |
| AC7 | Repeat acceptance with same Idempotency-Key returns stored 200 response, no extra payout. | TU-5e-TU-5f | 200 |
| AC8 | Idempotency-Key conflict with different payload or offer returns 409. | TU-5g | 409 |
| AC9 | Validation errors for amount, loan total, term produce respective TU-2a-TU-2c messages. | TU-2a-TU-2c | 422 |
| AC10 | Propagate Deposits service errors (404, 409, 422, 503) unchanged. | ARC-7 | Corresponding status codes |
| AC11 | Acceptance processing deducts arrangement fee, credits net amount, creates new loan, stores offer and timestamp per REG-5, returns confirmation with withdrawal reminder. | REG-5, TU-5f | 200 |
| AC12 | Second acceptance of same offer returns first result, no payout. | TU-5e | 200 |


## Test plan
- `api/tests/req008_ac1.rs`: tests AC1 eligibility overdue payment message.
- `api/tests/req008_ac2.rs`: tests AC2 eligibility insufficient payments message.
- `api/tests/req008_ac3.rs`: tests AC3 missing repayment account message.
- `api/tests/req008_ac4.rs`: tests AC4 successful offer calculation.
- `api/tests/req008_ac5.rs`: tests AC5 offer expiry after 24h.
- `api/tests/req008_ac6.rs`: tests AC6 acceptance with payout.
- `api/tests/req008_ac7.rs`: tests AC7 repeat acceptance idempotency.
- `api/tests/req008_ac8.rs`: tests AC8 Idempotency-Key conflict.
- `api/tests/req008_ac9.rs`: tests AC9 top-up amount out of range.
- `api/tests/req008_ac10.rs`: tests AC10 new loan amount exceeds limit.
- `api/tests/req008_ac11.rs`: tests AC11 term outside allowed range.
- `api/tests/req008_ac12.rs`: tests AC12 successful acceptance and payout details.
- `api/tests/req008_ac13.rs`: tests AC13 second acceptance returns first result.


## Out of scope
- UI changes in the iOS app.
- OAuth2 authentication handling.
- Persistent storage beyond in-memory demo.