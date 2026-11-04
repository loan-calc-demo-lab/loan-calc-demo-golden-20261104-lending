//! Loan quote calculation for an annuity (fixed monthly payment) loan.
//!
//! Requirements: REQ-001 (monthly payment), REQ-002 (repayment schedule),
//! REQ-003 (input validation), REQ-004 (upfront fee), REQ-005 (zero-rate loans),
//! REQ-007 (APR and total cost of credit).
//! Amounts are rounded to cents at every step that a customer sees.

use serde::{Deserialize, Serialize};

pub const MAX_PRINCIPAL: f64 = 1_000_000.0;
pub const MAX_ANNUAL_RATE_PCT: f64 = 30.0;
pub const MAX_TERM_MONTHS: u32 = 360;

#[derive(Debug, Clone, Deserialize)]
pub struct QuoteRequest {
    /// Amount borrowed, in EUR.
    pub principal: f64,
    /// Nominal annual interest rate, in percent (e.g. 6.5).
    pub annual_rate_pct: f64,
    /// Loan term in months.
    pub term_months: u32,
    /// One-off arrangement fee paid at the start, in EUR (REQ-004).
    #[serde(default)]
    pub upfront_fee: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ScheduleRow {
    pub month: u32,
    pub payment: f64,
    pub principal: f64,
    pub interest: f64,
    pub balance: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Quote {
    pub monthly_payment: f64,
    pub total_interest: f64,
    pub upfront_fee: f64,
    /// Everything the customer pays: all instalments plus the upfront fee.
    pub total_payable: f64,
    /// REQ-007: all costs of the credit: total interest plus the arrangement fee (rulebook APR-R3).
    pub total_cost_of_credit: f64,
    /// REQ-007: annual percentage rate in percent, one decimal place (rulebook APR-R1).
    pub apr_pct: f64,
    pub schedule: Vec<ScheduleRow>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// REQ-003: reject inputs outside the product limits before calculating.
pub fn validate(req: &QuoteRequest) -> Result<(), ValidationError> {
    let err = |field, message: &str| Err(ValidationError { field, message: message.to_string() });
    if !req.principal.is_finite() || req.principal <= 0.0 || req.principal > MAX_PRINCIPAL {
        return err("principal", "must be greater than 0 and at most 1,000,000");
    }
    if !req.annual_rate_pct.is_finite() || req.annual_rate_pct < 0.0 || req.annual_rate_pct > MAX_ANNUAL_RATE_PCT {
        return err("annual_rate_pct", "must be between 0 and 30");
    }
    if req.term_months == 0 || req.term_months > MAX_TERM_MONTHS {
        return err("term_months", "must be between 1 and 360");
    }
    if !req.upfront_fee.is_finite() || req.upfront_fee < 0.0 || req.upfront_fee >= req.principal {
        return err("upfront_fee", "must be 0 or more and less than the principal");
    }
    Ok(())
}

/// REQ-001 / REQ-005: fixed monthly payment of an annuity loan.
pub fn monthly_payment(principal: f64, annual_rate_pct: f64, term_months: u32) -> f64 {
    let n = term_months as f64;
    let r = annual_rate_pct / 100.0 / 12.0;
    if r == 0.0 {
        return round2(principal / n);
    }
    round2(principal * r / (1.0 - (1.0 + r).powf(-n)))
}

/// REQ-007: annual percentage rate, rulebook section 3.
///
/// Solves `principal - upfront_fee = Σ payment_k × (1 + X)^(-k/12)` for the yearly rate X, using the actual
/// instalments of the schedule (the last one may differ by rounding), and returns X in percent rounded half up
/// to one decimal place. The present value falls as X rises, so bisection always converges.
pub fn apr_pct(principal: f64, upfront_fee: f64, schedule: &[ScheduleRow]) -> f64 {
    let received = principal - upfront_fee;
    let present_value = |x: f64| -> f64 {
        schedule.iter().map(|row| row.payment * (1.0 + x).powf(-(row.month as f64) / 12.0)).sum()
    };
    // Search between -99% and 1000% a year.
    let (mut low, mut high) = (-0.99_f64, 10.0_f64);
    for _ in 0..200 {
        let mid = (low + high) / 2.0;
        if present_value(mid) > received {
            low = mid;
        } else {
            high = mid;
        }
    }
    let x = (low + high) / 2.0;
    (x * 1000.0).round() / 10.0
}

/// REQ-001..REQ-005, REQ-007: full quote with repayment schedule, total cost of credit and APR.
pub fn quote(req: &QuoteRequest) -> Result<Quote, ValidationError> {
    validate(req)?;
    let r = req.annual_rate_pct / 100.0 / 12.0;
    let payment = monthly_payment(req.principal, req.annual_rate_pct, req.term_months);

    let mut balance = round2(req.principal);
    let mut schedule = Vec::with_capacity(req.term_months as usize);
    let mut total_interest = 0.0;
    let mut total_instalments = 0.0;

    for month in 1..=req.term_months {
        let interest = round2(balance * r);
        // REQ-002: the last instalment clears any rounding remainder.
        let this_payment = if month == req.term_months { round2(balance + interest) } else { payment };
        let principal_part = round2(this_payment - interest);
        balance = round2(balance - principal_part);
        total_interest += interest;
        total_instalments += this_payment;
        schedule.push(ScheduleRow { month, payment: this_payment, principal: principal_part, interest, balance });
    }

    let total_instalments = round2(total_instalments);
    let total_interest = round2(total_interest);
    let upfront_fee = round2(req.upfront_fee);
    let apr = apr_pct(req.principal, upfront_fee, &schedule);
    Ok(Quote {
        monthly_payment: payment,
        total_interest,
        upfront_fee,
        total_payable: round2(total_instalments + upfront_fee),
        total_cost_of_credit: round2(total_interest + upfront_fee),
        apr_pct: apr,
        schedule,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(principal: f64, rate: f64, term: u32, fee: f64) -> QuoteRequest {
        QuoteRequest { principal, annual_rate_pct: rate, term_months: term, upfront_fee: fee }
    }

    #[test]
    fn req001_known_monthly_payment() {
        // 10,000 EUR at 5% over 12 months: 856.07 EUR per month.
        assert_eq!(monthly_payment(10_000.0, 5.0, 12), 856.07);
    }

    #[test]
    fn req001_longer_loan() {
        // 25,000 EUR at 7.5% over 60 months: 500.95 EUR per month.
        assert_eq!(monthly_payment(25_000.0, 7.5, 60), 500.95);
    }

    #[test]
    fn req002_schedule_clears_the_balance() {
        let q = quote(&req(10_000.0, 5.0, 12, 0.0)).unwrap();
        assert_eq!(q.schedule.len(), 12);
        assert_eq!(q.schedule.last().unwrap().balance, 0.0);
        let repaid: f64 = q.schedule.iter().map(|r| r.principal).sum();
        assert_eq!(round2(repaid), 10_000.0);
    }

    #[test]
    fn req002_each_row_adds_up() {
        let q = quote(&req(25_000.0, 7.5, 60, 0.0)).unwrap();
        for row in &q.schedule {
            assert_eq!(round2(row.principal + row.interest), row.payment, "month {}", row.month);
        }
    }

    #[test]
    fn req003_rejects_out_of_range_inputs() {
        assert_eq!(validate(&req(0.0, 5.0, 12, 0.0)).unwrap_err().field, "principal");
        assert_eq!(validate(&req(2_000_000.0, 5.0, 12, 0.0)).unwrap_err().field, "principal");
        assert_eq!(validate(&req(10_000.0, -1.0, 12, 0.0)).unwrap_err().field, "annual_rate_pct");
        assert_eq!(validate(&req(10_000.0, 31.0, 12, 0.0)).unwrap_err().field, "annual_rate_pct");
        assert_eq!(validate(&req(10_000.0, 5.0, 0, 0.0)).unwrap_err().field, "term_months");
        assert_eq!(validate(&req(10_000.0, 5.0, 361, 0.0)).unwrap_err().field, "term_months");
        assert_eq!(validate(&req(10_000.0, 5.0, 12, 10_000.0)).unwrap_err().field, "upfront_fee");
        assert_eq!(validate(&req(10_000.0, f64::NAN, 12, 0.0)).unwrap_err().field, "annual_rate_pct");
    }

    #[test]
    fn req004_upfront_fee_is_in_total_payable() {
        let without = quote(&req(10_000.0, 5.0, 12, 0.0)).unwrap();
        let with = quote(&req(10_000.0, 5.0, 12, 250.0)).unwrap();
        assert_eq!(with.monthly_payment, without.monthly_payment);
        assert_eq!(round2(with.total_payable - without.total_payable), 250.0);
    }

    #[test]
    fn req005_zero_rate_loan() {
        let q = quote(&req(1_200.0, 0.0, 12, 0.0)).unwrap();
        assert_eq!(q.monthly_payment, 100.0);
        assert_eq!(q.total_interest, 0.0);
        assert_eq!(q.total_payable, 1_200.0);
    }
}
