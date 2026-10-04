//! Soldes et totaux mensuels (port de `src/hooks/useFinancialMetrics.ts`).
//!
//! Les sommes sont faites en centimes entiers pour éviter les erreurs d'arrondi flottant.

use crate::dates::{parse_date, same_month};
use crate::models::{Transaction, TransactionType, TRANSFER_CATEGORY_ID};
use crate::snapshot::{is_selected, Snapshot};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

/// Keep cents inside the exact integer range shared with JavaScript clients.
pub fn is_valid_money(value: f64) -> bool {
    value.is_finite() && (value * 100.0).abs() <= 9_007_199_254_740_991.0
}

pub fn cents(value: f64) -> i64 {
    if value.is_finite() {
        (value * 100.0).round() as i64
    } else {
        0
    }
}

pub fn euros(value: impl Into<i128>) -> f64 {
    value.into() as f64 / 100.0
}

/// Aggregate in i128: every accepted individual value fits i64, but their sum need not.
pub fn wide_cents(value: f64) -> i128 {
    i128::from(cents(value))
}
pub fn wide_signed_cents(transaction: &Transaction) -> i128 {
    i128::from(signed_cents(transaction))
}

/// Montant signé : un revenu augmente le solde, tout le reste le diminue.
pub fn signed_cents(transaction: &Transaction) -> i64 {
    if transaction.transaction_type == TransactionType::Income {
        cents(transaction.amount)
    } else {
        -cents(transaction.amount)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct BalanceSummary {
    pub current_balance: f64,
    pub checked_balance: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct MonthlySummary {
    pub income: f64,
    pub expenses: f64,
    pub saved: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountBalance {
    pub account_id: String,
    pub current_balance: f64,
    pub checked_balance: f64,
}

pub fn relevant_transactions<'a>(
    snapshot: &'a Snapshot,
    filter: &'a [String],
) -> impl Iterator<Item = &'a Transaction> + 'a {
    snapshot
        .transactions
        .iter()
        .filter(move |transaction| is_selected(filter, &transaction.account_id))
}

pub fn balance_summary(snapshot: &Snapshot, filter: &[String]) -> BalanceSummary {
    let initial: i128 = snapshot
        .accounts
        .iter()
        .filter(|account| is_selected(filter, &account.id))
        .map(|account| wide_cents(account.initial_balance))
        .sum();

    let (mut current, mut checked) = (initial, initial);
    for transaction in relevant_transactions(snapshot, filter) {
        let amount = wide_signed_cents(transaction);
        current += amount;
        if transaction.checked {
            checked += amount;
        }
    }

    BalanceSummary {
        current_balance: euros(current),
        checked_balance: euros(checked),
    }
}

pub fn is_internal_transfer(transaction: &Transaction) -> bool {
    transaction.is_transfer
        || transaction.category == TRANSFER_CATEGORY_ID
        || transaction.transaction_type == TransactionType::Transfer
}

pub fn monthly_summary(snapshot: &Snapshot, filter: &[String], today: NaiveDate) -> MonthlySummary {
    let (mut income, mut expenses) = (0_i128, 0_i128);
    for transaction in relevant_transactions(snapshot, filter) {
        if is_internal_transfer(transaction)
            || !parse_date(&transaction.date).is_some_and(|date| same_month(date, today))
        {
            continue;
        }
        if transaction.transaction_type == TransactionType::Income {
            income += wide_cents(transaction.amount);
        } else {
            expenses += wide_cents(transaction.amount);
        }
    }

    MonthlySummary {
        income: euros(income),
        expenses: euros(expenses),
        saved: euros(income - expenses),
    }
}

/// Soldes actuel et pointé de chaque compte, dans l'ordre des comptes.
pub fn account_balances(snapshot: &Snapshot) -> Vec<AccountBalance> {
    let mut deltas: HashMap<&str, (i128, i128)> = HashMap::new();
    for transaction in &snapshot.transactions {
        let entry = deltas.entry(transaction.account_id.as_str()).or_default();
        let amount = wide_signed_cents(transaction);
        entry.0 += amount;
        if transaction.checked {
            entry.1 += amount;
        }
    }

    snapshot
        .accounts
        .iter()
        .map(|account| {
            let initial = wide_cents(account.initial_balance);
            let (current, checked) = deltas.get(account.id.as_str()).copied().unwrap_or_default();
            AccountBalance {
                account_id: account.id.clone(),
                current_balance: euros(initial + current),
                checked_balance: euros(initial + checked),
            }
        })
        .collect()
}
