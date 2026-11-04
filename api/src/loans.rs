//! Existing personal loans of the (fictional) demo customers, held in memory.
//!
//! This stands in for the loan account system of record. Balances are derived from the loan's own repayment
//! schedule, so they always agree with the quote calculation in `calc`.

use crate::calc::{quote, round2, QuoteRequest};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Loan {
    pub loan_id: String,
    pub customer_id: String,
    pub product: String,
    pub original_principal: f64,
    pub annual_rate_pct: f64,
    pub term_months: u32,
    pub payments_made: u32,
    pub remaining_term_months: u32,
    pub outstanding_balance: f64,
    pub monthly_payment: f64,
    pub in_arrears: bool,
    /// Current account the loan is paid from (held in Deposits).
    pub repayment_account_id: String,
}

fn existing_loan(id: &str, customer: &str, principal: f64, rate: f64, term: u32, paid: u32, arrears: bool, account: &str) -> Loan {
    let q = quote(&QuoteRequest { principal, annual_rate_pct: rate, term_months: term, upfront_fee: 0.0 }).expect("valid demo loan");
    let balance = if paid == 0 { round2(principal) } else { q.schedule[(paid - 1) as usize].balance };
    Loan {
        loan_id: id.into(),
        customer_id: customer.into(),
        product: "Personal loan".into(),
        original_principal: principal,
        annual_rate_pct: rate,
        term_months: term,
        payments_made: paid,
        remaining_term_months: term - paid,
        outstanding_balance: balance,
        monthly_payment: q.monthly_payment,
        in_arrears: arrears,
        repayment_account_id: account.into(),
    }
}

/// All loans, keyed by loan id.
#[derive(Debug, Clone)]
pub struct LoanBook {
    pub loans: BTreeMap<String, Loan>,
}

impl LoanBook {
    /// Fictional demo data: one loan in good standing, one in arrears, one too new, (REQ-008 AC3) one repaid
    /// without a current account with the bank, and (REQ-008 AC9) one large enough that a top-up can exceed €30,000.
    pub fn demo() -> Self {
        let mut loans = BTreeMap::new();
        for l in [
            existing_loan("PL-1001", "CUST-1001", 12_000.0, 8.9, 60, 18, false, "CA-2001"),
            existing_loan("PL-1002", "CUST-1002", 8_000.0, 9.5, 48, 20, true, "CA-2002"),
            existing_loan("PL-1003", "CUST-1003", 5_000.0, 10.0, 36, 3, false, "CA-2003"),
            existing_loan("PL-1004", "CUST-1004", 6_000.0, 9.5, 48, 12, false, ""),
            existing_loan("PL-1005", "CUST-1005", 20_000.0, 8.9, 60, 12, false, "CA-2005"),
        ] {
            loans.insert(l.loan_id.clone(), l);
        }
        LoanBook { loans }
    }

    pub fn for_customer(&self, customer_id: &str) -> Vec<Loan> {
        self.loans.values().filter(|l| l.customer_id == customer_id).cloned().collect()
    }

    pub fn get(&self, loan_id: &str) -> Option<&Loan> {
        self.loans.get(loan_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_loan_balance_follows_its_schedule() {
        let book = LoanBook::demo();
        let l = book.get("PL-1001").unwrap();
        assert_eq!(l.monthly_payment, 248.52);
        assert_eq!(l.remaining_term_months, 42);
        assert!(l.outstanding_balance > 0.0 && l.outstanding_balance < 12_000.0);
    }

    #[test]
    fn loans_are_listed_per_customer() {
        let book = LoanBook::demo();
        assert_eq!(book.for_customer("CUST-1001").len(), 1);
        assert!(book.for_customer("CUST-9999").is_empty());
    }
}
