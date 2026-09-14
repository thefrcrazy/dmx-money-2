//! Vue d'ensemble (port de `pages/Dashboard.tsx`).
//!
//! Corrections par rapport à 1.x : les catégories du mois comparent aussi l'année, et la liste
//! des comptes comme les échéances à venir respectent le filtre de comptes.

use crate::dates::{days_between, parse_date, same_month};
use crate::metrics::{
    balance_summary, cents, euros, monthly_summary, relevant_transactions, signed_cents, BalanceSummary, MonthlySummary,
};
use crate::models::{Account, Budget, Transaction, TransactionType, TRANSFER_CATEGORY_ID};
use crate::snapshot::{is_selected, CategoryDisplay, Snapshot};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpcomingScheduled {
    pub scheduled_id: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub next_date: String,
    /// Jours restants (négatif si l'échéance est en retard).
    pub days_until: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CategoryShare {
    pub category: CategoryDisplay,
    pub amount: f64,
    pub percentage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardAccount {
    pub account: Account,
    pub balance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecentTransaction {
    pub transaction: Transaction,
    pub category: CategoryDisplay,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct BudgetGauge {
    pub total_budgeted: f64,
    pub spent: f64,
    pub remaining: f64,
    /// Pourcentage consommé, plafonné à 100.
    pub progress: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardView {
    pub balances: BalanceSummary,
    pub month: MonthlySummary,
    pub upcoming: Vec<UpcomingScheduled>,
    pub top_categories: Vec<CategoryShare>,
    pub accounts: Vec<DashboardAccount>,
    pub accounts_total: f64,
    pub recent_transactions: Vec<RecentTransaction>,
    pub budget: BudgetGauge,
}

/// Budgets visibles : sans compte, ou rattachés à un compte sélectionné.
pub fn visible_budgets<'a>(snapshot: &'a Snapshot, filter: &'a [String]) -> impl Iterator<Item = &'a Budget> + 'a {
    snapshot.budgets.iter().filter(move |budget| {
        filter.is_empty()
            || budget
                .account_id
                .as_deref()
                .is_none_or(|account_id| filter.iter().any(|selected| selected == account_id))
    })
}

pub fn dashboard(snapshot: &Snapshot, filter: &[String], today: NaiveDate) -> DashboardView {
    let balances = balance_summary(snapshot, filter);
    let month = monthly_summary(snapshot, filter, today);

    let mut upcoming: Vec<UpcomingScheduled> = snapshot
        .scheduled
        .iter()
        .filter(|item| {
            is_selected(filter, &item.account_id)
                || item
                    .to_account_id
                    .as_deref()
                    .is_some_and(|to_account| is_selected(filter, to_account))
        })
        .filter_map(|item| {
            let next = parse_date(&item.next_date)?;
            Some(UpcomingScheduled {
                scheduled_id: item.id.clone(),
                description: item.description.clone(),
                amount: item.amount,
                transaction_type: item.transaction_type,
                next_date: item.next_date.clone(),
                days_until: days_between(today, next),
            })
        })
        .collect();
    upcoming.sort_by(|left, right| left.next_date.cmp(&right.next_date));
    upcoming.truncate(3);

    let monthly_expense_cents = cents(month.expenses);
    let mut expenses_by_category: Vec<(String, i64)> = Vec::new();
    for transaction in relevant_transactions(snapshot, filter) {
        if transaction.transaction_type != TransactionType::Expense
            || !parse_date(&transaction.date).is_some_and(|date| same_month(date, today))
        {
            continue;
        }
        match expenses_by_category
            .iter_mut()
            .find(|(category, _)| *category == transaction.category)
        {
            Some((_, total)) => *total += cents(transaction.amount),
            None => expenses_by_category.push((transaction.category.clone(), cents(transaction.amount))),
        }
    }
    let mut top_categories: Vec<CategoryShare> = expenses_by_category
        .into_iter()
        .map(|(category, amount)| CategoryShare {
            category: snapshot.category_display(&category),
            amount: euros(amount),
            percentage: if monthly_expense_cents > 0 {
                amount as f64 / monthly_expense_cents as f64 * 100.0
            } else {
                0.0
            },
        })
        .collect();
    top_categories.sort_by(|left, right| right.amount.total_cmp(&left.amount));

    let mut deltas: HashMap<&str, i64> = HashMap::new();
    for transaction in relevant_transactions(snapshot, filter) {
        *deltas.entry(transaction.account_id.as_str()).or_default() += signed_cents(transaction);
    }
    let accounts: Vec<DashboardAccount> = snapshot
        .accounts
        .iter()
        .filter(|account| is_selected(filter, &account.id))
        .map(|account| DashboardAccount {
            account: account.clone(),
            balance: euros(cents(account.initial_balance) + deltas.get(account.id.as_str()).copied().unwrap_or(0)),
        })
        .collect();

    let recent_transactions = relevant_transactions(snapshot, filter)
        .take(5)
        .map(|transaction| RecentTransaction {
            transaction: transaction.clone(),
            category: snapshot.category_display(&transaction.category),
        })
        .collect();

    let total_budgeted: i64 = visible_budgets(snapshot, filter)
        .map(|budget| cents(budget.amount))
        .sum();
    let spent: i64 = relevant_transactions(snapshot, filter)
        .filter(|transaction| {
            transaction.transaction_type == TransactionType::Expense
                && transaction.category != TRANSFER_CATEGORY_ID
                && parse_date(&transaction.date).is_some_and(|date| same_month(date, today))
        })
        .map(|transaction| cents(transaction.amount))
        .sum();

    DashboardView {
        balances,
        month,
        upcoming,
        top_categories,
        accounts_total: balances.current_balance,
        accounts,
        recent_transactions,
        budget: BudgetGauge {
            total_budgeted: euros(total_budgeted),
            spent: euros(spent),
            remaining: euros(total_budgeted - spent),
            progress: if total_budgeted > 0 {
                (spent as f64 / total_budgeted as f64 * 100.0).min(100.0)
            } else {
                0.0
            },
        },
    }
}
