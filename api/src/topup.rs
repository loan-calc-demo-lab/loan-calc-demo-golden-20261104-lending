//! REQ-008: top-up of an existing personal loan (contract lending-topup-v1).
//!
//! Rules from the Personal Loans rulebook "top-up" (TU-n) and the architecture standards (ARC-n):
//! eligibility TU-1, amount and term TU-2, price TU-3, indicative offer TU-4, acceptance and payout TU-5.
//! Every figure is calculated on the server from Lending's own data (ARC-5); the app sends only ids and choices.

use crate::calc::{quote, round2, QuoteRequest};
use crate::loans::Loan;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MIN_TOPUP: f64 = 1_000.0; // TU-2a
pub const MAX_TOPUP: f64 = 20_000.0; // TU-2a
pub const MAX_NEW_LOAN: f64 = 30_000.0; // TU-2b
pub const MIN_TERM: u32 = 12; // TU-2c
pub const MAX_TERM: u32 = 84; // TU-2c
pub const MIN_PAYMENTS_MADE: u32 = 6; // TU-1b
pub const OFFER_VALIDITY_SECS: u64 = 24 * 3600; // TU-4c
pub const LABEL: &str = "Indicative offer – subject to final checks"; // TU-4a
pub const WITHDRAWAL_REMINDER: &str =
    "You can withdraw from this top-up within 14 days without giving a reason."; // TU-5f, REG-3

#[derive(Debug, Clone, Deserialize)]
pub struct OfferRequest {
    pub customer_id: String,
    pub topup_amount: f64,
    pub term_months: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcceptRequest {
    pub customer_id: String,
}

/// TU-4b: the indicative offer, every figure in the order the rulebook lists them.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Offer {
    pub offer_id: String,
    pub loan_id: String,
    pub customer_id: String,
    pub indicative: bool,
    pub label: String,
    pub monthly_payment: f64,
    pub apr_pct: f64,
    pub total_cost_of_credit: f64,
    pub topup_amount: f64,
    pub arrangement_fee: f64,
    pub amount_paid_out: f64,
    pub new_loan_amount: f64,
    pub term_months: u32,
    pub annual_rate_pct: f64,
    pub total_payable: f64,
    pub created_at: String,
    pub valid_until: String,
}

/// TU-5f: what the customer sees after accepting.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Acceptance {
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

/// A rule the request breaks: HTTP status, error code, field (if any) and the rulebook's customer message.
#[derive(Debug, Clone, PartialEq)]
pub struct TopupError {
    pub status: u16,
    pub error: &'static str,
    pub field: Option<&'static str>,
    pub message: &'static str,
}

const fn rule(status: u16, error: &'static str, field: Option<&'static str>, message: &'static str) -> TopupError {
    TopupError { status, error, field, message }
}

pub const UNKNOWN_LOAN: TopupError = rule(404, "not_found", None, "unknown loan");
pub const UNKNOWN_OFFER: TopupError = rule(404, "not_found", None, "unknown offer");
pub const IN_ARREARS: TopupError = rule(422, "eligibility", None, "A top-up is not possible while a payment is overdue."); // TU-1a
pub const TOO_NEW: TopupError = rule(422, "eligibility", None, "A top-up is possible after your sixth monthly payment."); // TU-1b
pub const NO_ACCOUNT: TopupError = rule(422, "eligibility", None, "We could not find your current account. Please contact us."); // TU-1c
pub const AMOUNT_RANGE: TopupError = rule(422, "range", Some("topup_amount"), "Please choose a top-up amount between €1,000 and €20,000."); // TU-2a
pub const NEW_LOAN_TOO_LARGE: TopupError = rule(422, "range", Some("topup_amount"), "The new loan amount can be at most €30,000."); // TU-2b
pub const TERM_RANGE: TopupError = rule(422, "range", Some("term_months"), "Please choose a term between 12 and 84 months."); // TU-2c
pub const EXPIRED: TopupError = rule(410, "expired", None, "This offer has expired. Please ask for a new offer."); // TU-4c
pub const KEY_CONFLICT: TopupError = rule(409, "conflict", None, "This Idempotency-Key was used for a different offer.");
pub const PAYOUT_PENDING: TopupError = rule(503, "payout_pending", None, "The payout is not confirmed yet. Please try again.");

/// TU-1: eligibility, checked in this order; the first rule broken is reported.
pub fn check_eligible(loan: &Loan) -> Result<(), TopupError> {
    if loan.in_arrears {
        return Err(IN_ARREARS);
    }
    if loan.payments_made < MIN_PAYMENTS_MADE {
        return Err(TOO_NEW);
    }
    if !loan.repayment_account_id.starts_with("CA-") {
        return Err(NO_ACCOUNT);
    }
    Ok(())
}

/// TU-3a: nominal rate by new loan amount.
pub fn rate_for(new_loan_amount: f64) -> f64 {
    if new_loan_amount <= 15_000.0 { 9.9 } else { 8.9 }
}

/// TU-3b: 1 % of the top-up, at least €50, at most €300, rounded to cents.
pub fn arrangement_fee(topup_amount: f64) -> f64 {
    round2((topup_amount * 0.01).clamp(50.0, 300.0))
}

/// The figures of an offer for `loan` (TU-2, TU-3), without ids and times.
pub fn figures(loan: &Loan, topup_amount: f64, term_months: u32) -> Result<Offer, TopupError> {
    check_eligible(loan)?;
    if !topup_amount.is_finite() || topup_amount < MIN_TOPUP || topup_amount > MAX_TOPUP || topup_amount.fract() != 0.0 {
        return Err(AMOUNT_RANGE);
    }
    if !(MIN_TERM..=MAX_TERM).contains(&term_months) {
        return Err(TERM_RANGE);
    }
    let new_loan_amount = round2(loan.outstanding_balance + topup_amount);
    if new_loan_amount > MAX_NEW_LOAN {
        return Err(NEW_LOAN_TOO_LARGE);
    }
    let annual_rate_pct = rate_for(new_loan_amount);
    let fee = arrangement_fee(topup_amount);
    // TU-3c: calculated exactly like a new loan, the fee as the upfront fee.
    let q = quote(&QuoteRequest { principal: new_loan_amount, annual_rate_pct, term_months, upfront_fee: fee })
        .map_err(|_| AMOUNT_RANGE)?;
    Ok(Offer {
        offer_id: String::new(),
        loan_id: loan.loan_id.clone(),
        customer_id: loan.customer_id.clone(),
        indicative: true,
        label: LABEL.into(),
        monthly_payment: q.monthly_payment,
        apr_pct: q.apr_pct,
        total_cost_of_credit: q.total_cost_of_credit,
        topup_amount,
        arrangement_fee: fee,
        amount_paid_out: round2(topup_amount - fee), // TU-3d
        new_loan_amount,
        term_months,
        annual_rate_pct,
        total_payable: q.total_payable,
        created_at: String::new(),
        valid_until: String::new(),
    })
}

/// Where an offer stands. `Pending` = the loan was changed and the payout is not confirmed yet (ARC-7).
#[derive(Debug, Clone, PartialEq)]
pub enum OfferState {
    Open,
    Pending { accepted_at_secs: u64 },
    Accepted(Acceptance),
}

#[derive(Debug, Clone)]
pub struct StoredOffer {
    pub offer: Offer,
    pub created_secs: u64,
    pub state: OfferState,
}

/// Offers made so far, and which offer each Idempotency-Key was used for.
#[derive(Debug, Default)]
pub struct OfferBook {
    pub offers: HashMap<String, StoredOffer>,
    pub keys: HashMap<String, String>,
    count: u64,
}

impl OfferBook {
    pub fn add(&mut self, mut offer: Offer, now: u64) -> Offer {
        self.count += 1;
        offer.offer_id = format!("OF-{}-{}", offer.loan_id, self.count);
        offer.created_at = iso_utc(now);
        offer.valid_until = iso_utc(now + OFFER_VALIDITY_SECS);
        self.offers.insert(offer.offer_id.clone(), StoredOffer { offer: offer.clone(), created_secs: now, state: OfferState::Open });
        offer
    }
}

/// ISO 8601 UTC time for Unix seconds (no extra crates).
pub fn iso_utc(secs: u64) -> String {
    let (y, m, d) = civil(secs / 86_400);
    let rem = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// TU-5f: first payment one month after acceptance (the same day next month, or its last day).
pub fn first_payment_date(accepted_secs: u64) -> String {
    let (y, m, d) = civil(accepted_secs / 86_400);
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let last = match nm {
        2 if (ny % 4 == 0 && ny % 100 != 0) || ny % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    format!("{ny:04}-{nm:02}-{:02}", d.min(last))
}

fn civil(days: u64) -> (i64, u32, u32) {
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loans::LoanBook;

    #[test]
    fn req008_reference_values_r1_r2_r3() {
        let book = LoanBook::demo();
        let loan = book.get("PL-1001").unwrap();
        let r1 = figures(loan, 5_000.0, 60).unwrap();
        assert_eq!((r1.new_loan_amount, r1.annual_rate_pct, r1.arrangement_fee, r1.amount_paid_out), (13_940.27, 9.9, 50.0, 4_950.0));
        assert_eq!((r1.monthly_payment, r1.apr_pct, r1.total_cost_of_credit, r1.total_payable), (295.50, 10.5, 3_840.05, 17_780.32));
        let r2 = figures(loan, 7_000.0, 72).unwrap();
        assert_eq!((r2.new_loan_amount, r2.annual_rate_pct, r2.arrangement_fee, r2.amount_paid_out), (15_940.27, 8.9, 70.0, 6_930.0));
        assert_eq!((r2.monthly_payment, r2.apr_pct, r2.total_cost_of_credit, r2.total_payable), (286.54, 9.4, 4_760.72, 20_700.99));
        assert_eq!(figures(book.get("PL-1004").unwrap(), 5_000.0, 60).unwrap_err(), NO_ACCOUNT);
        let r3 = figures(loan, 1_000.0, 48).unwrap();
        assert_eq!((r3.new_loan_amount, r3.annual_rate_pct, r3.arrangement_fee, r3.amount_paid_out), (9_940.27, 9.9, 50.0, 950.0));
        assert_eq!((r3.monthly_payment, r3.apr_pct, r3.total_cost_of_credit, r3.total_payable), (251.63, 10.7, 2_188.19, 12_128.46));
    }

    #[test]
    fn req008_eligibility_and_ranges() {
        let book = LoanBook::demo();
        assert_eq!(figures(book.get("PL-1002").unwrap(), 5_000.0, 60).unwrap_err(), IN_ARREARS);
        assert_eq!(figures(book.get("PL-1003").unwrap(), 5_000.0, 60).unwrap_err(), TOO_NEW);
        let l = book.get("PL-1001").unwrap();
        assert_eq!(figures(l, 999.0, 60).unwrap_err(), AMOUNT_RANGE);
        assert_eq!(figures(l, 20_001.0, 60).unwrap_err(), AMOUNT_RANGE);
        assert_eq!(figures(l, 5_000.0, 11).unwrap_err(), TERM_RANGE);
        assert_eq!(figures(l, 5_000.0, 85).unwrap_err(), TERM_RANGE);
    }

    #[test]
    fn dates() {
        assert_eq!(iso_utc(1_793_750_400), "2026-11-04T00:00:00Z");
        assert_eq!(first_payment_date(1_793_750_400), "2026-12-04");
        assert_eq!(first_payment_date(1_801_353_600), "2027-02-28"); // accepted 2027-01-31
    }
}
