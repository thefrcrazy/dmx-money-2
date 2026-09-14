//! Échéancier : liste filtrée et suggestions de récurrences (port de `pages/Scheduled.tsx`).

use crate::dates::{add_months, format_date, month_index, parse_date};
use crate::format::{date_day_month_year, js_number, number_fr};
use crate::metrics::cents;
use crate::models::{
    Periodicity, ScheduledDueRange, ScheduledTransaction, Transaction, TransactionType, TRANSFER_CATEGORY_ID,
};
use crate::snapshot::{is_selected, CategoryDisplay, Snapshot};
use crate::text::{compare_fr, normalize_description, search_tokens, SearchText};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScheduledQuery {
    pub accounts: Vec<String>,
    pub due_range: ScheduledDueRange,
    pub search: String,
    pub categories: Vec<String>,
    pub frequencies: Vec<Periodicity>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScheduledRow {
    pub scheduled: ScheduledTransaction,
    pub account_name: String,
    pub account_color: String,
    pub to_account_name: Option<String>,
    pub category: CategoryDisplay,
    pub budget_name: Option<String>,
    pub is_ended: bool,
    pub frequency_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScheduledSuggestion {
    /// Clé mémorisée quand la suggestion est ignorée.
    pub key: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub category: CategoryDisplay,
    pub account_id: String,
    pub account_name: String,
    pub account_color: String,
    pub to_account_id: Option<String>,
    pub frequency: Periodicity,
    pub next_date: String,
    pub occurrence_count: u32,
}

impl ScheduledSuggestion {
    /// Échéance à créer quand l'utilisateur accepte la suggestion.
    pub fn to_scheduled(&self) -> ScheduledTransaction {
        ScheduledTransaction {
            id: String::new(),
            description: self.description.clone(),
            amount: self.amount,
            transaction_type: self.transaction_type,
            frequency: self.frequency,
            account_id: self.account_id.clone(),
            next_date: self.next_date.clone(),
            category: self.category.id.clone(),
            to_account_id: self.to_account_id.clone(),
            include_in_forecast: Some(false),
            budget_id: None,
            end_date: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScheduledView {
    pub rows: Vec<ScheduledRow>,
    pub suggestions: Vec<ScheduledSuggestion>,
    pub has_filters: bool,
    pub total_count: u32,
}

fn is_ended(item: &ScheduledTransaction, today: NaiveDate) -> bool {
    item.end_date
        .as_deref()
        .and_then(parse_date)
        .is_some_and(|end| end < today)
}

fn display_category(snapshot: &Snapshot, transaction_type: TransactionType, category: &str) -> CategoryDisplay {
    if transaction_type == TransactionType::Transfer {
        snapshot.category_display(TRANSFER_CATEGORY_ID)
    } else {
        snapshot.category_display(category)
    }
}

pub fn scheduled_view(snapshot: &Snapshot, query: &ScheduledQuery, today: NaiveDate) -> ScheduledView {
    let tokens = search_tokens(&query.search);
    let due_limit = query.due_range.months().map(|months| add_months(today, months as i32));

    let mut rows: Vec<ScheduledRow> = snapshot
        .scheduled
        .iter()
        .filter(|item| is_selected(&query.accounts, &item.account_id))
        .filter(|item| match due_limit {
            Some(limit) => parse_date(&item.next_date).is_some_and(|due| due >= today && due <= limit),
            None => true,
        })
        .filter(|item| query.categories.is_empty() || query.categories.contains(&item.category))
        .filter(|item| query.frequencies.is_empty() || query.frequencies.contains(&item.frequency))
        .map(|item| {
            let account = snapshot.account(&item.account_id);
            let budget_name = item.budget_id.as_deref().map(|budget_id| {
                snapshot
                    .budget(budget_id)
                    .map(|budget| budget.name.clone())
                    .unwrap_or_else(|| "Budget lié".to_string())
            });
            ScheduledRow {
                scheduled: item.clone(),
                account_name: account.map(|account| account.name.clone()).unwrap_or_default(),
                account_color: account.map(|account| account.color.clone()).unwrap_or_default(),
                to_account_name: item
                    .to_account_id
                    .as_deref()
                    .and_then(|id| snapshot.account(id))
                    .map(|account| account.name.clone()),
                category: display_category(snapshot, item.transaction_type, &item.category),
                budget_name,
                is_ended: is_ended(item, today),
                frequency_label: item.frequency.label().to_string(),
            }
        })
        .filter(|row| {
            if tokens.is_empty() {
                return true;
            }
            let item = &row.scheduled;
            let prefix = match item.transaction_type {
                TransactionType::Income => "+",
                TransactionType::Transfer => "",
                TransactionType::Expense => "-",
            };
            let mut text = SearchText::new();
            text.push(&row.account_name)
                .push(row.to_account_name.as_deref().unwrap_or_default())
                .push(parse_date(&item.next_date).map(date_day_month_year).unwrap_or_default())
                .push(&item.next_date)
                .push(item.end_date.as_deref().unwrap_or_default())
                .push(
                    item.end_date
                        .as_deref()
                        .and_then(parse_date)
                        .map(date_day_month_year)
                        .unwrap_or_default(),
                )
                .push(&row.frequency_label)
                .push(&row.category.name)
                .push(item.transaction_type.label())
                .push(&item.description)
                .push(js_number(item.amount))
                .push(format!("{prefix}{} €", number_fr(item.amount, 2, 2)))
                .push(row.budget_name.as_deref().unwrap_or("Hors budget"))
                .push(if row.is_ended { "Terminé" } else { "Actif" });
            text.matches(&tokens)
        })
        .collect();

    rows.sort_by(|left, right| left.scheduled.next_date.cmp(&right.scheduled.next_date));

    ScheduledView {
        has_filters: !query.search.is_empty()
            || query.due_range != ScheduledDueRange::All
            || !query.categories.is_empty()
            || !query.frequencies.is_empty(),
        total_count: snapshot.scheduled.len() as u32,
        rows,
        suggestions: scheduled_suggestions(snapshot, &query.accounts, today),
    }
}

fn recurrence_identity(
    account_id: &str,
    to_account_id: Option<&str>,
    transaction_type: TransactionType,
    category: &str,
    description: &str,
) -> String {
    [
        account_id,
        to_account_id.unwrap_or_default(),
        transaction_type.as_str(),
        category,
        &normalize_description(description),
    ]
    .join("|")
}

/// Récurrences mensuelles détectées dans le journal : même libellé et montant sur au moins
/// deux mois consécutifs, sans échéance correspondante.
pub fn scheduled_suggestions(snapshot: &Snapshot, accounts: &[String], today: NaiveDate) -> Vec<ScheduledSuggestion> {
    struct Group<'a> {
        identity: String,
        description: String,
        transaction_type: TransactionType,
        category: String,
        account_id: String,
        to_account_id: Option<String>,
        records: Vec<&'a Transaction>,
    }

    let by_id: HashMap<&str, &Transaction> = snapshot
        .transactions
        .iter()
        .map(|transaction| (transaction.id.as_str(), transaction))
        .collect();
    let scheduled_identities: HashSet<String> = snapshot
        .scheduled
        .iter()
        .map(|item| {
            recurrence_identity(
                &item.account_id,
                item.to_account_id.as_deref(),
                item.transaction_type,
                &item.category,
                &item.description,
            )
        })
        .collect();

    let mut groups: Vec<Group> = Vec::new();
    let mut group_index: HashMap<String, usize> = HashMap::new();

    for transaction in &snapshot.transactions {
        if !is_selected(accounts, &transaction.account_id) {
            continue;
        }
        let description = transaction.description.trim();
        if description.is_empty() || transaction.amount <= 0.0 {
            continue;
        }

        let (transaction_type, category, to_account_id) =
            if transaction.category == TRANSFER_CATEGORY_ID || transaction.is_transfer {
                if transaction.transaction_type != TransactionType::Expense {
                    continue;
                }
                let Some(to_account) = transaction
                    .linked_transaction_id
                    .as_deref()
                    .and_then(|linked_id| by_id.get(linked_id))
                    .map(|linked| linked.account_id.clone())
                else {
                    continue;
                };
                (
                    TransactionType::Transfer,
                    TRANSFER_CATEGORY_ID.to_string(),
                    Some(to_account),
                )
            } else {
                (transaction.transaction_type, transaction.category.clone(), None)
            };

        let identity = recurrence_identity(
            &transaction.account_id,
            to_account_id.as_deref(),
            transaction_type,
            &category,
            description,
        );
        if scheduled_identities.contains(&identity) {
            continue;
        }

        let key = format!("{identity}|{}", cents(transaction.amount));
        match group_index.get(&key) {
            Some(index) => groups[*index].records.push(transaction),
            None => {
                group_index.insert(key, groups.len());
                groups.push(Group {
                    identity,
                    description: description.to_string(),
                    transaction_type,
                    category,
                    account_id: transaction.account_id.clone(),
                    to_account_id,
                    records: vec![transaction],
                });
            }
        }
    }

    let dismissed = &snapshot.settings.dismissed_scheduled_suggestions;
    let mut suggestions: Vec<ScheduledSuggestion> = groups
        .into_iter()
        .filter_map(|group| {
            let dated: Vec<(&Transaction, NaiveDate)> = group
                .records
                .iter()
                .filter_map(|record| parse_date(&record.date).map(|date| (*record, date)))
                .collect();
            let months: BTreeSet<i32> = dated.iter().map(|(_, date)| month_index(*date)).collect();
            if months.len() < 2 {
                return None;
            }

            let months: Vec<i32> = months.into_iter().collect();
            let (mut run, mut longest) = (1, 1);
            for pair in months.windows(2) {
                run = if pair[1] == pair[0] + 1 { run + 1 } else { 1 };
                longest = longest.max(run);
            }
            if longest < 2 {
                return None;
            }

            let (latest, latest_date) =
                dated.iter().fold(
                    dated[0],
                    |latest, current| {
                        if current.1 > latest.1 {
                            *current
                        } else {
                            latest
                        }
                    },
                );
            let mut next_date = add_months(latest_date, 1);
            while next_date <= today {
                next_date = add_months(next_date, 1);
            }

            let key = format!("{}|{}", group.identity, cents(latest.amount));
            if dismissed.contains(&key) {
                return None;
            }

            let account = snapshot.account(&group.account_id);
            Some(ScheduledSuggestion {
                key,
                description: group.description,
                amount: latest.amount,
                transaction_type: group.transaction_type,
                category: display_category(snapshot, group.transaction_type, &group.category),
                account_name: account
                    .map(|account| account.name.clone())
                    .unwrap_or_else(|| "Compte inconnu".to_string()),
                account_color: account.map(|account| account.color.clone()).unwrap_or_default(),
                account_id: group.account_id,
                to_account_id: group.to_account_id,
                frequency: Periodicity::Monthly,
                next_date: format_date(next_date),
                occurrence_count: months.len() as u32,
            })
        })
        .collect();

    suggestions.sort_by(|left, right| {
        left.next_date
            .cmp(&right.next_date)
            .then_with(|| compare_fr(&left.description, &right.description))
    });
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Account;

    fn tx(id: &str, date: &str, description: &str, amount: f64) -> Transaction {
        Transaction {
            id: id.into(),
            date: date.into(),
            account_id: "a1".into(),
            transaction_type: TransactionType::Expense,
            amount,
            category: "16".into(),
            description: description.into(),
            checked: true,
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
            transactions: vec![
                tx("t4", "2026-09-03", "Netflix", 13.49),
                tx("t3", "2026-08-03", "  netflix ", 13.49),
                tx("t2", "2026-07-20", "Cinéma", 12.0),
                tx("t1", "2026-05-20", "Cinéma", 12.0),
            ],
            scheduled: vec![ScheduledTransaction {
                id: "s1".into(),
                description: "Loyer".into(),
                amount: 800.0,
                transaction_type: TransactionType::Expense,
                frequency: Periodicity::Monthly,
                account_id: "a1".into(),
                next_date: "2026-10-01".into(),
                category: "1".into(),
                to_account_id: None,
                include_in_forecast: None,
                budget_id: None,
                end_date: Some("2026-09-01".into()),
            }],
            ..Snapshot::default()
        }
    }

    #[test]
    fn detects_consecutive_monthly_recurrences_only() {
        let today = parse_date("2026-09-15").unwrap();
        let suggestions = scheduled_suggestions(&snapshot(), &[], today);
        assert_eq!(suggestions.len(), 1);
        let suggestion = &suggestions[0];
        assert_eq!(suggestion.description, "Netflix");
        assert_eq!(suggestion.next_date, "2026-10-03");
        assert_eq!(suggestion.key, "a1||expense|16|netflix|1349");
    }

    #[test]
    fn due_range_and_ended_status() {
        let today = parse_date("2026-09-15").unwrap();
        let view = scheduled_view(&snapshot(), &ScheduledQuery::default(), today);
        assert_eq!(view.rows.len(), 1);
        assert!(view.rows[0].is_ended);

        let month = ScheduledQuery {
            due_range: ScheduledDueRange::Month,
            ..ScheduledQuery::default()
        };
        let last_day_included = scheduled_view(&snapshot(), &month, parse_date("2026-09-01").unwrap());
        assert_eq!(last_day_included.rows.len(), 1);
        let too_far = scheduled_view(&snapshot(), &month, parse_date("2026-08-15").unwrap());
        assert!(too_far.rows.is_empty());

        let search = scheduled_view(
            &snapshot(),
            &ScheduledQuery {
                search: "mensuel termine".into(),
                ..ScheduledQuery::default()
            },
            today,
        );
        assert_eq!(search.rows.len(), 1);
    }
}
