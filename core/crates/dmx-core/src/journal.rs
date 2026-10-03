//! Journal : solde courant par compte, budget restant, recherche plein texte et filtres
//! (port de `pages/Transactions.tsx`).

use crate::dates::{month_key, parse_date};
use crate::format::{currency_fr, date_day_month, date_numeric, js_number, weekday_day_month};
use crate::metrics::{euros, wide_cents as cents, wide_signed_cents as signed_cents};
use crate::models::{string_enum, Account, Budget, Transaction, TransactionType, TRANSFER_CATEGORY_ID};
use crate::snapshot::{CategoryDisplay, Snapshot};
use crate::text::{search_tokens, SearchText};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

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
pub(crate) struct BudgetSpending<'a> {
    by_scope: HashMap<(&'a str, Option<&'a str>), HashMap<&'a str, i128>>,
}

impl<'a> BudgetSpending<'a> {
    pub(crate) fn new(transactions: &'a [Transaction]) -> Self {
        let mut by_scope = HashMap::new();
        for transaction in transactions {
            if transaction.transaction_type != TransactionType::Expense || transaction.category == TRANSFER_CATEGORY_ID
            {
                continue;
            }
            let month = transaction.date.get(0..7).unwrap_or_default();
            let amount = cents(transaction.amount);
            for account in [None, Some(transaction.account_id.as_str())] {
                *by_scope
                    .entry((transaction.category.as_str(), account))
                    .or_insert_with(HashMap::new)
                    .entry(month)
                    .or_insert(0) += amount;
            }
        }
        Self { by_scope }
    }

    fn spent(&self, category: &str, account_id: Option<&str>, month: &str) -> i128 {
        self.by_scope
            .get(&(category, account_id))
            .and_then(|months| months.get(month))
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

fn budget_remaining(budget: &Budget, spending: &BudgetSpending<'_>, transaction: &Transaction) -> BudgetRemaining {
    let month = parse_date(&transaction.date)
        .map(month_key)
        .unwrap_or_else(|| transaction.date.get(0..7).unwrap_or_default().to_string());
    let spent = spending.spent(&budget.category, budget.account_id.as_deref(), &month);
    BudgetRemaining {
        budget_id: budget.id.clone(),
        budget_name: budget.name.clone(),
        remaining: euros(cents(budget.amount) - spent),
    }
}

/// Solde de chaque compte après chaque transaction, dans l'ordre chronologique d'insertion.
pub fn running_balances(snapshot: &Snapshot) -> HashMap<String, i128> {
    let mut order: Vec<usize> = (0..snapshot.transactions.len()).collect();
    // Les instantanés SQLite sont déjà décroissants : leur inversion évite un second tri.
    // Le repli conserve le comportement de cette fonction pour un instantané non ordonné.
    if snapshot
        .transactions
        .windows(2)
        .all(|pair| pair[0].date >= pair[1].date)
    {
        order.reverse();
    } else {
        order.sort_by(|left, right| {
            snapshot.transactions[*left]
                .date
                .cmp(&snapshot.transactions[*right].date)
                .then(right.cmp(left))
        });
    }

    let mut balances: HashMap<&str, i128> = snapshot
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
    account: Option<&Account>,
    transaction: &Transaction,
    category: &CategoryDisplay,
    budget: Option<&BudgetRemaining>,
    balance: f64,
) -> SearchText {
    let mut text = SearchText::new();
    if let Some(account) = account {
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
    let selected_accounts: HashSet<_> = query.accounts.iter().map(String::as_str).collect();
    let selected_categories: HashSet<_> = query.categories.iter().map(String::as_str).collect();
    let accounts: HashMap<_, _> = snapshot
        .accounts
        .iter()
        .map(|account| (account.id.as_str(), account))
        .collect();
    let mut categories = HashMap::new();
    // Le premier budget rencontré est le plus récent, comme dans applicable_budget.
    let mut scoped_budgets = HashMap::new();
    let mut global_budgets = HashMap::new();
    for budget in &snapshot.budgets {
        if let Some(account) = budget.account_id.as_deref() {
            scoped_budgets
                .entry((budget.category.as_str(), account))
                .or_insert(budget);
        } else {
            global_budgets.entry(budget.category.as_str()).or_insert(budget);
        }
    }
    let mut remaining_by_month = HashMap::new();

    let mut rows = Vec::new();
    for transaction in &snapshot.transactions {
        if !selected_accounts.is_empty() && !selected_accounts.contains(transaction.account_id.as_str()) {
            continue;
        }
        if !selected_categories.is_empty() && !selected_categories.contains(transaction.category.as_str()) {
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
        let applicable = if transaction.transaction_type == TransactionType::Expense
            && transaction.category != TRANSFER_CATEGORY_ID
        {
            scoped_budgets
                .get(&(transaction.category.as_str(), transaction.account_id.as_str()))
                .or_else(|| global_budgets.get(transaction.category.as_str()))
        } else {
            None
        };
        let budget = applicable.map(|budget| {
            remaining_by_month
                .entry((budget.id.as_str(), transaction.date.get(0..7).unwrap_or_default()))
                .or_insert_with(|| budget_remaining(budget, &spending, transaction))
                .clone()
        });
        let budget_status = if budget.is_some() {
            BudgetStatus::Budgeted
        } else {
            BudgetStatus::Unbudgeted
        };
        if !query.budget_statuses.is_empty() && !query.budget_statuses.contains(&budget_status) {
            continue;
        }

        let category = categories
            .entry(transaction.category.as_str())
            .or_insert_with(|| snapshot.category_display(&transaction.category));
        let balance = euros(balances.get(&transaction.id).copied().unwrap_or(0));
        let account = accounts.get(transaction.account_id.as_str()).copied();

        if !tokens.is_empty() && !search_text(account, transaction, category, budget.as_ref(), balance).matches(&tokens)
        {
            continue;
        }

        rows.push(JournalRow {
            transaction: transaction.clone(),
            balance,
            account_name: account.map(|account| account.name.clone()).unwrap_or_default(),
            account_color: account.map(|account| account.color.clone()).unwrap_or_default(),
            category: category.clone(),
            display_type,
            budget,
        });
    }

    let mut day_groups: Vec<JournalDayGroup> = Vec::new();
    let mut visible_net = 0_i128;
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
            bank_source: None,
            bank_transaction_id: None,
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

    #[test]
    fn account_budget_overrides_global_and_uses_newest_matching_budget() {
        let mut snapshot = snapshot();
        snapshot.budgets.insert(
            0,
            Budget {
                id: "account-budget".into(),
                name: "Courses du compte".into(),
                amount: 80.0,
                category: "5".into(),
                account_id: Some("a1".into()),
            },
        );
        snapshot.budgets.push(Budget {
            id: "older-account-budget".into(),
            name: "Ancienne enveloppe".into(),
            amount: 500.0,
            category: "5".into(),
            account_id: Some("a1".into()),
        });
        let view = journal(&snapshot, &JournalQuery::default());
        for row in &view.rows[..2] {
            let budget = row.budget.as_ref().unwrap();
            assert_eq!(budget.budget_id, "account-budget");
            assert_eq!(budget.remaining, 30.0);
        }
        assert!(view.rows[2].budget.is_none());
    }

    #[test]
    fn running_balance_supports_snapshots_not_sorted_by_date() {
        let mut snapshot = snapshot();
        snapshot.transactions.swap(1, 3);
        let balances = running_balances(&snapshot);
        assert_eq!(balances["t1"], 110_000);
        assert_eq!(balances["t4"], 105_000);
        assert_eq!(balances["t3"], 107_000);
        assert_eq!(balances["t2"], 5_000);
    }

    #[test]
    fn large_journal_keeps_full_balances_when_filtering_visible_rows() {
        let mut snapshot = snapshot();
        snapshot.transactions = (0..20_000)
            .map(|index| {
                tx(
                    &format!("t{index}"),
                    "2026-09-10",
                    "a1",
                    TransactionType::Expense,
                    1.0,
                    "5",
                )
            })
            .collect();
        let full = journal(&snapshot, &JournalQuery::default());
        assert_eq!(full.rows.len(), 20_000);
        assert_eq!(full.day_groups[0].count, 20_000);
        assert_eq!(full.visible_net, -20_000.0);
        assert_eq!(full.rows[0].balance, -19_900.0);
        assert_eq!(full.rows.last().unwrap().balance, 99.0);
        assert_eq!(full.rows[0].budget.as_ref().unwrap().remaining, -19_800.0);

        let filtered = journal(
            &snapshot,
            &JournalQuery {
                search: "Opération t19999".into(),
                ..JournalQuery::default()
            },
        );
        assert_eq!(filtered.rows.len(), 1);
        assert_eq!(filtered.total_transaction_count, 20_000);
        assert_eq!(filtered.rows[0].balance, 99.0);
        assert_eq!(filtered.rows[0].budget.as_ref().unwrap().remaining, -19_800.0);
        assert_eq!(filtered.visible_net, -1.0);
    }
}
