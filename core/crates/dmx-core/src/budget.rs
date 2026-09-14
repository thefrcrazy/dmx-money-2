//! Page Budget : enveloppes du mois, rythme de dépense et suggestions (port de `pages/Budget.tsx`).

use crate::dashboard::visible_budgets;
use crate::dates::{add_months, days_in_month, end_of_month, parse_date, start_of_month};
use crate::format::{date_day_month, date_day_month_year, js_number, month_year_long};
use crate::metrics::{cents, euros};
use crate::models::{string_enum, Budget, Transaction, TransactionType, TRANSFER_CATEGORY_ID};
use crate::snapshot::{is_selected, CategoryDisplay, Snapshot};
use crate::text::{compare_fr, search_tokens, SearchText};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BudgetQuery {
    pub accounts: Vec<String>,
    pub search: String,
    pub categories: Vec<String>,
}

string_enum!(BudgetState, default = ToConfigure, {
    ToConfigure => "to_configure",
    UnderControl => "under_control",
    Overrun => "overrun",
});

impl BudgetState {
    pub fn label(self) -> &'static str {
        match self {
            BudgetState::ToConfigure => "À configurer",
            BudgetState::UnderControl => "Sous contrôle",
            BudgetState::Overrun => "Dépassement",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinkedScheduled {
    pub scheduled_id: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub next_date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetEnvelope {
    pub budget: Budget,
    pub account_name: String,
    pub spent: f64,
    pub remaining: f64,
    pub progress: f64,
    pub linked_scheduled: Vec<LinkedScheduled>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetCategoryRow {
    pub category: CategoryDisplay,
    pub budgeted: f64,
    pub spent: f64,
    pub remaining: f64,
    pub progress: f64,
    pub is_over_budget: bool,
    pub is_unbudgeted: bool,
    pub envelopes: Vec<BudgetEnvelope>,
    pub linked_scheduled_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetSuggestion {
    /// Clé `categorie|compte` (ou `categorie|all`) mémorisée quand la suggestion est ignorée.
    pub key: String,
    pub name: String,
    pub amount: f64,
    pub category: CategoryDisplay,
    pub account_id: Option<String>,
    pub account_name: String,
    pub month_count: u32,
    pub current_month_spent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetOverview {
    /// Ex. « septembre 2026 »
    pub month_label: String,
    /// Ex. « 14 sept. »
    pub today_label: String,
    pub total_budgeted: f64,
    pub total_spent: f64,
    pub remaining: f64,
    pub progress: f64,
    pub expected_spend: f64,
    pub pace_delta: f64,
    pub remaining_per_day: f64,
    pub budget_count: u32,
    pub expense_count: u32,
    pub state: BudgetState,
    pub over_budget_count: u32,
    pub unbudgeted_count: u32,
    /// Catégories budgétées, après recherche et filtre.
    pub categories: Vec<BudgetCategoryRow>,
    /// Nombre de catégories budgétées avant filtres.
    pub budgeted_category_count: u32,
    pub suggestions: Vec<BudgetSuggestion>,
}

struct CategoryTotals {
    id: String,
    budgeted: i64,
    spent: i64,
}

fn progress(spent: i64, budgeted: i64) -> f64 {
    if budgeted > 0 {
        spent as f64 / budgeted as f64 * 100.0
    } else {
        0.0
    }
}

fn is_month_expense(transaction: &Transaction, start: NaiveDate, end: NaiveDate) -> bool {
    transaction.transaction_type == TransactionType::Expense
        && transaction.category != TRANSFER_CATEGORY_ID
        && parse_date(&transaction.date).is_some_and(|date| date >= start && date <= end)
}

pub fn budget_overview(snapshot: &Snapshot, query: &BudgetQuery, today: NaiveDate) -> BudgetOverview {
    let month_start = start_of_month(today);
    let month_end = end_of_month(today);
    let days = days_in_month(today) as i64;
    let current_day = (today.day() as i64).min(days);
    let remaining_days = (days - current_day + 1).max(1);

    let visible: Vec<&Budget> = visible_budgets(snapshot, &query.accounts).collect();
    let month_expenses: Vec<&Transaction> = snapshot
        .transactions
        .iter()
        .filter(|transaction| is_selected(&query.accounts, &transaction.account_id))
        .filter(|transaction| is_month_expense(transaction, month_start, month_end))
        .collect();

    let mut totals: Vec<CategoryTotals> = Vec::new();
    let entry = |totals: &mut Vec<CategoryTotals>, id: &str| -> usize {
        match totals.iter().position(|item| item.id == id) {
            Some(index) => index,
            None => {
                totals.push(CategoryTotals {
                    id: id.to_string(),
                    budgeted: 0,
                    spent: 0,
                });
                totals.len() - 1
            }
        }
    };
    for budget in &visible {
        let index = entry(&mut totals, &budget.category);
        totals[index].budgeted += cents(budget.amount);
    }
    for transaction in &month_expenses {
        let index = entry(&mut totals, &transaction.category);
        totals[index].spent += cents(transaction.amount);
    }

    let mut linked_by_budget: HashMap<&str, Vec<LinkedScheduled>> = HashMap::new();
    for item in &snapshot.scheduled {
        if let Some(budget_id) = item.budget_id.as_deref() {
            linked_by_budget.entry(budget_id).or_default().push(LinkedScheduled {
                scheduled_id: item.id.clone(),
                description: item.description.clone(),
                amount: item.amount,
                transaction_type: item.transaction_type,
                next_date: item.next_date.clone(),
            });
        }
    }
    for items in linked_by_budget.values_mut() {
        items.sort_by(|left, right| left.next_date.cmp(&right.next_date));
    }

    let mut rows: Vec<BudgetCategoryRow> = totals
        .iter()
        .map(|totals| {
            let mut envelopes: Vec<BudgetEnvelope> = visible
                .iter()
                .filter(|budget| budget.category == totals.id)
                .map(|budget| {
                    let spent: i64 = month_expenses
                        .iter()
                        .filter(|transaction| {
                            transaction.category == budget.category
                                && budget
                                    .account_id
                                    .as_deref()
                                    .is_none_or(|account_id| account_id == transaction.account_id)
                        })
                        .map(|transaction| cents(transaction.amount))
                        .sum();
                    let amount = cents(budget.amount);
                    BudgetEnvelope {
                        budget: (*budget).clone(),
                        account_name: budget
                            .account_id
                            .as_deref()
                            .and_then(|account_id| snapshot.account(account_id))
                            .map(|account| account.name.clone())
                            .unwrap_or_else(|| "Tous les comptes".to_string()),
                        spent: euros(spent),
                        remaining: euros(amount - spent),
                        progress: progress(spent, amount),
                        linked_scheduled: linked_by_budget.get(budget.id.as_str()).cloned().unwrap_or_default(),
                    }
                })
                .collect();
            envelopes.sort_by(|left, right| {
                left.remaining
                    .total_cmp(&right.remaining)
                    .then_with(|| compare_fr(&left.budget.name, &right.budget.name))
            });

            let linked_scheduled_count = envelopes
                .iter()
                .map(|envelope| envelope.linked_scheduled.len() as u32)
                .sum();
            BudgetCategoryRow {
                category: snapshot.category_display(&totals.id),
                budgeted: euros(totals.budgeted),
                spent: euros(totals.spent),
                remaining: euros(totals.budgeted - totals.spent),
                progress: if totals.budgeted > 0 {
                    progress(totals.spent, totals.budgeted)
                } else if totals.spent > 0 {
                    100.0
                } else {
                    0.0
                },
                is_over_budget: totals.budgeted > 0 && totals.spent > totals.budgeted,
                is_unbudgeted: totals.budgeted == 0 && totals.spent > 0,
                envelopes,
                linked_scheduled_count,
            }
        })
        .collect();

    rows.sort_by(|left, right| {
        right
            .is_over_budget
            .cmp(&left.is_over_budget)
            .then(right.is_unbudgeted.cmp(&left.is_unbudgeted))
            .then_with(|| {
                right
                    .budgeted
                    .max(right.spent)
                    .total_cmp(&left.budgeted.max(left.spent))
            })
    });

    let unbudgeted_count = rows.iter().filter(|row| row.is_unbudgeted).count() as u32;
    let budgeted_rows: Vec<BudgetCategoryRow> = rows.into_iter().filter(|row| row.budgeted > 0.0).collect();
    let over_budget_count = budgeted_rows.iter().filter(|row| row.is_over_budget).count() as u32;
    let budgeted_category_count = budgeted_rows.len() as u32;

    let tokens = search_tokens(&query.search);
    let categories = budgeted_rows
        .into_iter()
        .filter(|row| query.categories.is_empty() || query.categories.contains(&row.category.id))
        .filter(|row| {
            if tokens.is_empty() {
                return true;
            }
            let mut text = SearchText::new();
            text.push(&row.category.name)
                .push(js_number(row.budgeted))
                .push(js_number(row.spent))
                .push(js_number(row.remaining));
            for envelope in &row.envelopes {
                text.push(&envelope.budget.name)
                    .push(&envelope.account_name)
                    .push(js_number(envelope.budget.amount))
                    .push(js_number(envelope.spent))
                    .push(js_number(envelope.remaining));
                for item in &envelope.linked_scheduled {
                    text.push(&item.description)
                        .push(js_number(item.amount))
                        .push(&item.next_date);
                    if let Some(date) = parse_date(&item.next_date) {
                        text.push(date_day_month_year(date));
                    }
                }
            }
            text.matches(&tokens)
        })
        .collect();

    let total_budgeted: i64 = visible.iter().map(|budget| cents(budget.amount)).sum();
    let total_spent: i64 = month_expenses.iter().map(|transaction| cents(transaction.amount)).sum();
    let remaining = total_budgeted - total_spent;
    let expected_spend = total_budgeted as f64 * current_day as f64 / days as f64;

    BudgetOverview {
        month_label: month_year_long(today),
        today_label: date_day_month(today),
        total_budgeted: euros(total_budgeted),
        total_spent: euros(total_spent),
        remaining: euros(remaining),
        progress: progress(total_spent, total_budgeted),
        expected_spend: expected_spend / 100.0,
        pace_delta: (total_spent as f64 - expected_spend) / 100.0,
        remaining_per_day: euros(remaining) / remaining_days as f64,
        budget_count: visible.len() as u32,
        expense_count: month_expenses.len() as u32,
        state: if total_budgeted == 0 {
            BudgetState::ToConfigure
        } else if remaining >= 0 {
            BudgetState::UnderControl
        } else {
            BudgetState::Overrun
        },
        over_budget_count,
        unbudgeted_count,
        categories,
        budgeted_category_count,
        suggestions: budget_suggestions(snapshot, &query.accounts, today),
    }
}

/// Suggestions d'enveloppes à partir des dépenses des six derniers mois sans budget.
pub fn budget_suggestions(snapshot: &Snapshot, accounts: &[String], today: NaiveDate) -> Vec<BudgetSuggestion> {
    struct Group {
        key: String,
        category: String,
        account_id: Option<String>,
        total: i64,
        current_month_spent: i64,
        months: BTreeSet<(i32, u32)>,
    }

    let group_by_single_account = accounts.len() == 1;
    let start = start_of_month(add_months(today, -5));
    let month_end = end_of_month(today);
    let current_month = (today.year(), today.month());
    let dismissed = &snapshot.settings.dismissed_budget_suggestions;

    let has_existing_budget = |category: &str, account_id: Option<&str>| {
        snapshot.budgets.iter().any(|budget| {
            budget.category == category
                && match (budget.account_id.as_deref(), account_id) {
                    (None, _) | (_, None) => true,
                    (Some(budget_account), Some(account)) => budget_account == account,
                }
        })
    };

    let mut groups: Vec<Group> = Vec::new();
    for transaction in snapshot
        .transactions
        .iter()
        .filter(|transaction| is_selected(accounts, &transaction.account_id))
    {
        if transaction.transaction_type != TransactionType::Expense || transaction.category == TRANSFER_CATEGORY_ID {
            continue;
        }
        let Some(date) = parse_date(&transaction.date).filter(|date| *date >= start && *date <= month_end) else {
            continue;
        };
        let account_id = group_by_single_account.then(|| transaction.account_id.clone());
        if has_existing_budget(&transaction.category, account_id.as_deref()) {
            continue;
        }
        let key = format!("{}|{}", transaction.category, account_id.as_deref().unwrap_or("all"));
        if dismissed.contains(&key) {
            continue;
        }

        let index = match groups.iter().position(|group| group.key == key) {
            Some(index) => index,
            None => {
                groups.push(Group {
                    key,
                    category: transaction.category.clone(),
                    account_id,
                    total: 0,
                    current_month_spent: 0,
                    months: BTreeSet::new(),
                });
                groups.len() - 1
            }
        };
        let group = &mut groups[index];
        let amount = cents(transaction.amount);
        let month = (date.year(), date.month());
        group.total += amount;
        group.months.insert(month);
        if month == current_month {
            group.current_month_spent += amount;
        }
    }

    let mut suggestions: Vec<BudgetSuggestion> = groups
        .into_iter()
        .map(|group| {
            let month_count = group.months.len().max(1) as i64;
            let average = (group.total + month_count - 1).div_euclid(month_count);
            let category = snapshot.category_display(&group.category);
            BudgetSuggestion {
                name: category.name.clone(),
                amount: euros(average.max(group.current_month_spent)),
                category,
                account_name: group
                    .account_id
                    .as_deref()
                    .and_then(|account_id| snapshot.account(account_id))
                    .map(|account| account.name.clone())
                    .unwrap_or_else(|| "Tous les comptes".to_string()),
                account_id: group.account_id,
                key: group.key,
                month_count: group.months.len() as u32,
                current_month_spent: euros(group.current_month_spent),
            }
        })
        .filter(|suggestion| suggestion.amount > 0.0)
        .collect();
    suggestions.sort_by(|left, right| right.amount.total_cmp(&left.amount));
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Account, Category};

    fn tx(id: &str, date: &str, amount: f64, category: &str) -> Transaction {
        Transaction {
            id: id.into(),
            date: date.into(),
            account_id: "a1".into(),
            transaction_type: TransactionType::Expense,
            amount,
            category: category.into(),
            description: String::new(),
            checked: false,
            is_transfer: false,
            linked_transaction_id: None,
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            accounts: vec![Account {
                id: "a1".into(),
                name: "Courant".into(),
                account_type: "Courant".into(),
                initial_balance: 0.0,
                color: "#000".into(),
                icon: "Wallet".into(),
            }],
            categories: vec![
                Category {
                    id: "5".into(),
                    name: "Alimentation".into(),
                    icon: "ShoppingBag".into(),
                    color: "#ef4444".into(),
                },
                Category {
                    id: "9".into(),
                    name: "Carburant".into(),
                    icon: "Fuel".into(),
                    color: "#b45309".into(),
                },
            ],
            budgets: vec![Budget {
                id: "b1".into(),
                name: "Courses".into(),
                amount: 300.0,
                category: "5".into(),
                account_id: None,
            }],
            transactions: vec![
                tx("t5", "2026-09-12", 350.0, "5"),
                tx("t4", "2026-09-05", 60.0, "9"),
                tx("t3", "2026-08-10", 40.0, "9"),
                tx("t2", "2026-07-10", 50.0, "9"),
                tx("t1", "2026-02-10", 999.0, "9"),
            ],
            ..Snapshot::default()
        }
    }

    #[test]
    fn totals_pace_and_state() {
        let today = parse_date("2026-09-15").unwrap();
        let overview = budget_overview(&snapshot(), &BudgetQuery::default(), today);

        assert_eq!(overview.total_budgeted, 300.0);
        assert_eq!(overview.total_spent, 410.0);
        assert_eq!(overview.remaining, -110.0);
        assert_eq!(overview.state, BudgetState::Overrun);
        assert_eq!(overview.expected_spend, 150.0);
        assert_eq!(overview.pace_delta, 260.0);
        assert_eq!(overview.remaining_per_day, -110.0 / 16.0);
        assert_eq!(overview.over_budget_count, 1);
        assert_eq!(overview.unbudgeted_count, 1);
        assert_eq!(overview.categories.len(), 1);
        assert_eq!(overview.categories[0].envelopes[0].remaining, -50.0);
        assert_eq!(overview.month_label, "septembre 2026");
    }

    #[test]
    fn suggestions_cover_unbudgeted_categories_of_the_last_six_months() {
        let today = parse_date("2026-09-15").unwrap();
        let suggestions = budget_suggestions(&snapshot(), &[], today);

        assert_eq!(suggestions.len(), 1);
        let suggestion = &suggestions[0];
        assert_eq!(suggestion.key, "9|all");
        assert_eq!(suggestion.month_count, 3);
        assert_eq!(suggestion.amount, 60.0);

        let mut dismissed = snapshot();
        dismissed.settings.dismissed_budget_suggestions = vec!["9|all".into()];
        assert!(budget_suggestions(&dismissed, &[], today).is_empty());
    }
}
