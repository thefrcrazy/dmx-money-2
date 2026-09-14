//! Journal : solde courant par compte, budget restant, recherche plein texte et filtres
//! (port de `pages/Transactions.tsx`).

use crate::dates::{month_key, parse_date};
use crate::format::{currency_fr, date_day_month, date_numeric, js_number, weekday_day_month};
use crate::metrics::{cents, euros, signed_cents};
use crate::models::{string_enum, Budget, Transaction, TransactionType, TRANSFER_CATEGORY_ID};
use crate::snapshot::{is_selected, CategoryDisplay, Snapshot};
use crate::text::{search_tokens, SearchText};
use serde::Serialize;
use std::collections::HashMap;

string_enum!(CheckStatus, default = Checked, {
    Checked => "checked",
    Unchecked => "unchecked",
});

string_enum!(BudgetStatus, default = Budgeted, {
    Budgeted => "budgeted",
    Unbudgeted => "unbudgeted",
});

impl CheckStatus {
    pub fn label(self) -> &'static str {
        match self {
            CheckStatus::Checked => "Pointées",
            CheckStatus::Unchecked => "Non pointées",
        }
    }
}

impl BudgetStatus {
    pub fn label(self) -> &'static str {
        match self {
            BudgetStatus::Budgeted => "Avec budget",
            BudgetStatus::Unbudgeted => "Hors budget",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JournalQuery {
    pub accounts: Vec<String>,
    pub search: String,
    pub categories: Vec<String>,
    pub types: Vec<TransactionType>,
    pub statuses: Vec<CheckStatus>,
    pub budget_statuses: Vec<BudgetStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetRemaining {
    pub budget_id: String,
    pub budget_name: String,
    pub remaining: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalRow {
    pub transaction: Transaction,
    /// Solde du compte après cette opération.
    pub balance: f64,
    pub account_name: String,
    pub account_color: String,
    pub category: CategoryDisplay,
    pub display_type: TransactionType,
    pub budget: Option<BudgetRemaining>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalDayGroup {
    pub date: String,
    /// Ex. « lundi 14 sept. »
    pub label: String,
    pub start_index: u32,
    pub count: u32,
    pub net: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalView {
    pub rows: Vec<JournalRow>,
    pub day_groups: Vec<JournalDayGroup>,
    pub active_filter_count: u32,
    pub has_filters: bool,
    pub visible_net: f64,
    pub total_transaction_count: u32,
}

/// Dépenses (hors virements) par catégorie, compte et mois.
pub(crate) struct BudgetSpending {
    by_scope: HashMap<(String, Option<String>, String), i64>,
}

impl BudgetSpending {
    pub(crate) fn new(transactions: &[Transaction]) -> Self {
        let mut by_scope = HashMap::new();
        for transaction in transactions {
            if transaction.transaction_type != TransactionType::Expense || transaction.category == TRANSFER_CATEGORY_ID
            {
                continue;
            }
            let month = transaction.date.get(0..7).unwrap_or_default().to_string();
            let amount = cents(transaction.amount);
            *by_scope
                .entry((transaction.category.clone(), None, month.clone()))
                .or_insert(0) += amount;
            *by_scope
                .entry((
                    transaction.category.clone(),
                    Some(transaction.account_id.clone()),
                    month,
                ))
                .or_insert(0) += amount;
        }
        Self { by_scope }
    }

    fn spent(&self, category: &str, account_id: Option<&str>, month: &str) -> i64 {
        self.by_scope
            .get(&(category.to_string(), account_id.map(str::to_string), month.to_string()))
            .copied()
            .unwrap_or(0)
    }
}

/// Budget qui s'applique à une dépense : celui du compte en priorité, sinon un budget global.
pub fn applicable_budget<'a>(budgets: &'a [Budget], transaction: &Transaction) -> Option<&'a Budget> {
    if transaction.transaction_type != TransactionType::Expense || transaction.category == TRANSFER_CATEGORY_ID {
        return None;
    }
    let matching = budgets.iter().filter(|budget| {
        budget.category == transaction.category
            && budget
                .account_id
                .as_deref()
                .is_none_or(|account_id| account_id == transaction.account_id)
    });
    let mut global = None;
    for budget in matching {
        if budget.account_id.as_deref() == Some(transaction.account_id.as_str()) {
            return Some(budget);
        }
        global = global.or(Some(budget));
    }
    global
}

pub(crate) fn budget_remaining(
    snapshot: &Snapshot,
    spending: &BudgetSpending,
    transaction: &Transaction,
) -> Option<BudgetRemaining> {
    let budget = applicable_budget(&snapshot.budgets, transaction)?;
    let month = parse_date(&transaction.date)
        .map(month_key)
        .unwrap_or_else(|| transaction.date.get(0..7).unwrap_or_default().to_string());
    let spent = spending.spent(&budget.category, budget.account_id.as_deref(), &month);
    Some(BudgetRemaining {
        budget_id: budget.id.clone(),
        budget_name: budget.name.clone(),
        remaining: euros(cents(budget.amount) - spent),
    })
}

/// Solde de chaque compte après chaque transaction, dans l'ordre chronologique d'insertion.
pub fn running_balances(snapshot: &Snapshot) -> HashMap<String, i64> {
    let mut order: Vec<usize> = (0..snapshot.transactions.len()).collect();
    order.sort_by(|left, right| {
        snapshot.transactions[*left]
            .date
            .cmp(&snapshot.transactions[*right].date)
            .then(right.cmp(left))
    });

    let mut balances: HashMap<&str, i64> = snapshot
        .accounts
        .iter()
        .map(|account| (account.id.as_str(), cents(account.initial_balance)))
        .collect();

    let mut result = HashMap::with_capacity(order.len());
    for index in order {
        let transaction = &snapshot.transactions[index];
        let balance = balances.entry(transaction.account_id.as_str()).or_insert(0);
        *balance += signed_cents(transaction);
        result.insert(transaction.id.clone(), *balance);
    }
    result
}

fn search_text(
    snapshot: &Snapshot,
    transaction: &Transaction,
    category: &CategoryDisplay,
    budget: Option<&BudgetRemaining>,
    balance: f64,
) -> SearchText {
    let mut text = SearchText::new();
    if let Some(account) = snapshot.account(&transaction.account_id) {
        text.push(&account.name).push(&account.account_type);
    }
    text.push(&transaction.date);
    if let Some(date) = parse_date(&transaction.date) {
        text.push(date_numeric(date)).push(date_day_month(date));
    }
    let type_label = transaction.display_type().label();
    let prefix = if transaction.is_income() { "+" } else { "-" };
    text.push(&category.name)
        .push(type_label)
        .push(&transaction.description)
        .push(js_number(transaction.amount))
        .push(currency_fr(transaction.amount))
        .push(format!("{prefix}{}", currency_fr(transaction.amount)))
        .push(if transaction.checked {
            "Pointé coché validé"
        } else {
            "Non pointé non coché actuel"
        });
    match budget {
        Some(budget) => text.push(format!(
            "Budget budgété prévu {} {} restant",
            budget.budget_name,
            currency_fr(budget.remaining)
        )),
        None => text.push("Hors budget non budgété"),
    };
    text.push(js_number(balance)).push(currency_fr(balance));
    text
}

pub fn journal(snapshot: &Snapshot, query: &JournalQuery) -> JournalView {
    let balances = running_balances(snapshot);
    let spending = BudgetSpending::new(&snapshot.transactions);
    let tokens = search_tokens(&query.search);

    let mut rows = Vec::new();
    for transaction in &snapshot.transactions {
        if !is_selected(&query.accounts, &transaction.account_id) {
            continue;
        }
        if !query.categories.is_empty() && !query.categories.contains(&transaction.category) {
            continue;
        }
        let display_type = transaction.display_type();
        if !query.types.is_empty() && !query.types.contains(&display_type) {
            continue;
        }
        let status = if transaction.checked {
            CheckStatus::Checked
        } else {
            CheckStatus::Unchecked
        };
        if !query.statuses.is_empty() && !query.statuses.contains(&status) {
            continue;
        }
        let budget = budget_remaining(snapshot, &spending, transaction);
        let budget_status = if budget.is_some() {
            BudgetStatus::Budgeted
        } else {
            BudgetStatus::Unbudgeted
        };
        if !query.budget_statuses.is_empty() && !query.budget_statuses.contains(&budget_status) {
            continue;
        }

        let category = snapshot.category_display(&transaction.category);
        let balance = euros(balances.get(&transaction.id).copied().unwrap_or(0));

        if !tokens.is_empty()
            && !search_text(snapshot, transaction, &category, budget.as_ref(), balance).matches(&tokens)
        {
            continue;
        }

        let account = snapshot.account(&transaction.account_id);
        rows.push(JournalRow {
            transaction: transaction.clone(),
            balance,
            account_name: account.map(|account| account.name.clone()).unwrap_or_default(),
            account_color: account.map(|account| account.color.clone()).unwrap_or_default(),
            category,
            display_type,
            budget,
        });
    }

    let mut day_groups: Vec<JournalDayGroup> = Vec::new();
    let mut visible_net = 0_i64;
    for (index, row) in rows.iter().enumerate() {
        let amount = signed_cents(&row.transaction);
        visible_net += amount;
        match day_groups.last_mut() {
            Some(group) if group.date == row.transaction.date => {
                group.count += 1;
                group.net = euros(cents(group.net) + amount);
            }
            _ => day_groups.push(JournalDayGroup {
                date: row.transaction.date.clone(),
                label: parse_date(&row.transaction.date)
                    .map(weekday_day_month)
                    .unwrap_or_else(|| row.transaction.date.clone()),
                start_index: index as u32,
                count: 1,
                net: euros(amount),
            }),
        }
    }

    let active_filter_count =
        (query.categories.len() + query.types.len() + query.statuses.len() + query.budget_statuses.len()) as u32;

    JournalView {
        rows,
        day_groups,
        active_filter_count,
        has_filters: !query.search.is_empty() || active_filter_count > 0,
        visible_net: euros(visible_net),
        total_transaction_count: snapshot.transactions.len() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Account, Category};

    fn account(id: &str, initial: f64) -> Account {
        Account {
            id: id.into(),
            name: format!("Compte {id}"),
            account_type: "Courant".into(),
            initial_balance: initial,
            color: "#000".into(),
            icon: "Wallet".into(),
        }
    }

    fn tx(id: &str, date: &str, account: &str, kind: TransactionType, amount: f64, category: &str) -> Transaction {
        Transaction {
            id: id.into(),
            date: date.into(),
            account_id: account.into(),
            transaction_type: kind,
            amount,
            category: category.into(),
            description: format!("Opération {id}"),
            checked: false,
            is_transfer: false,
            linked_transaction_id: None,
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            accounts: vec![account("a1", 100.0), account("a2", 0.0)],
            // Ordre du journal : date décroissante, insertion décroissante.
            transactions: vec![
                tx("t4", "2026-09-10", "a1", TransactionType::Expense, 20.0, "5"),
                tx("t3", "2026-09-10", "a1", TransactionType::Expense, 30.0, "5"),
                tx("t2", "2026-09-02", "a2", TransactionType::Income, 50.0, "21"),
                tx("t1", "2026-08-30", "a1", TransactionType::Income, 1000.0, "21"),
            ],
            categories: vec![Category {
                id: "5".into(),
                name: "Alimentation".into(),
                icon: "ShoppingBag".into(),
                color: "#ef4444".into(),
            }],
            budgets: vec![Budget {
                id: "b1".into(),
                name: "Courses".into(),
                amount: 200.0,
                category: "5".into(),
                account_id: None,
            }],
            ..Snapshot::default()
        }
    }

    #[test]
    fn running_balance_follows_chronological_insertion_order() {
        let view = journal(&snapshot(), &JournalQuery::default());
        let balances: Vec<_> = view
            .rows
            .iter()
            .map(|row| (row.transaction.id.as_str(), row.balance))
            .collect();
        assert_eq!(
            balances,
            vec![("t4", 1050.0), ("t3", 1070.0), ("t2", 50.0), ("t1", 1100.0)]
        );
    }

    #[test]
    fn budget_remaining_uses_the_month_of_the_transaction() {
        let view = journal(&snapshot(), &JournalQuery::default());
        let remaining = view.rows[0].budget.as_ref().unwrap().remaining;
        assert_eq!(remaining, 150.0);
        assert!(view.rows[2].budget.is_none());
    }

    #[test]
    fn filters_and_accent_insensitive_search_combine() {
        let snapshot = snapshot();
        let view = journal(
            &snapshot,
            &JournalQuery {
                search: "alimentation 30".into(),
                ..JournalQuery::default()
            },
        );
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.rows[0].transaction.id, "t3");

        let view = journal(
            &snapshot,
            &JournalQuery {
                accounts: vec!["a1".into()],
                types: vec![TransactionType::Income],
                ..JournalQuery::default()
            },
        );
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.active_filter_count, 1);
        assert_eq!(view.visible_net, 1000.0);
    }

    #[test]
    fn groups_rows_by_day() {
        let view = journal(&snapshot(), &JournalQuery::default());
        assert_eq!(view.day_groups.len(), 3);
        assert_eq!(view.day_groups[0].count, 2);
        assert_eq!(view.day_groups[0].net, -50.0);
        assert_eq!(view.day_groups[0].label, "jeudi 10 sept.");
    }
}
