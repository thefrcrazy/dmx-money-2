//! Génération des transactions dues (port de `processDueScheduledItems`, `BankContext.tsx`).
//!
//! Les identifiants `scheduled:{id}:{date}:{single|from|to}` sont ceux de 1.x : une occurrence
//! déjà générée par l'ancienne application n'est jamais dupliquée.

use crate::dates::{add_months, format_date, next_occurrence, parse_date};
use crate::db::DbPool;
use crate::error::{CoreResult, DbContext};
use crate::models::{Periodicity, ScheduledTransaction, Transaction, TransactionType, TRANSFER_CATEGORY_ID};
use crate::repo;
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub enum ScheduledOutcome {
    Unchanged,
    Advance(String),
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledPlan {
    pub transactions: Vec<Transaction>,
    pub outcome: ScheduledOutcome,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ProcessDueResult {
    pub created_transactions: u32,
    pub updated_scheduled: u32,
    pub deleted_scheduled: u32,
}

pub fn scheduled_transaction_id(scheduled_id: &str, occurrence_date: &str, side: &str) -> String {
    format!("scheduled:{scheduled_id}:{occurrence_date}:{side}")
}

fn occurrence_transactions(item: &ScheduledTransaction, occurrence: &str) -> Vec<Transaction> {
    match (item.transaction_type, item.to_account_id.as_deref()) {
        (TransactionType::Transfer, Some(to_account_id)) => {
            let from_id = scheduled_transaction_id(&item.id, occurrence, "from");
            let to_id = scheduled_transaction_id(&item.id, occurrence, "to");
            vec![
                Transaction {
                    id: from_id.clone(),
                    date: occurrence.to_string(),
                    account_id: item.account_id.clone(),
                    transaction_type: TransactionType::Expense,
                    amount: item.amount,
                    category: TRANSFER_CATEGORY_ID.to_string(),
                    description: item.description.clone(),
                    checked: false,
                    is_transfer: true,
                    linked_transaction_id: Some(to_id.clone()),
                },
                Transaction {
                    id: to_id,
                    date: occurrence.to_string(),
                    account_id: to_account_id.to_string(),
                    transaction_type: TransactionType::Income,
                    amount: item.amount,
                    category: TRANSFER_CATEGORY_ID.to_string(),
                    description: item.description.clone(),
                    checked: false,
                    is_transfer: true,
                    linked_transaction_id: Some(from_id),
                },
            ]
        }
        _ => vec![Transaction {
            id: scheduled_transaction_id(&item.id, occurrence, "single"),
            date: occurrence.to_string(),
            account_id: item.account_id.clone(),
            transaction_type: if item.transaction_type == TransactionType::Income {
                TransactionType::Income
            } else {
                TransactionType::Expense
            },
            amount: item.amount,
            category: item.category.clone(),
            description: item.description.clone(),
            checked: false,
            is_transfer: false,
            linked_transaction_id: None,
        }],
    }
}

/// Calcule, sans écrire, les transactions dues d'une échéance et son nouvel état.
pub fn plan_due(item: &ScheduledTransaction, today: NaiveDate) -> ScheduledPlan {
    let unchanged = ScheduledPlan {
        transactions: Vec::new(),
        outcome: ScheduledOutcome::Unchanged,
    };
    let Some(mut next) = parse_date(&item.next_date) else {
        return unchanged;
    };
    let end = item.end_date.as_deref().and_then(parse_date);

    let mut transactions = Vec::new();
    let mut modified = false;

    while next <= today {
        modified = true;
        if end.is_some_and(|end| next > end) {
            break;
        }

        transactions.extend(occurrence_transactions(item, &format_date(next)));

        let following = next_occurrence(next, item.frequency).unwrap_or_else(|| add_months(next, 1200));
        if following <= next {
            break;
        }
        next = following;
    }

    let outcome = if !modified {
        ScheduledOutcome::Unchanged
    } else if item.frequency == Periodicity::Once {
        ScheduledOutcome::Delete
    } else {
        let next_date = format_date(next);
        if next_date == item.next_date {
            ScheduledOutcome::Unchanged
        } else {
            ScheduledOutcome::Advance(next_date)
        }
    };

    ScheduledPlan { transactions, outcome }
}

/// Crée les transactions dues et avance les échéances. Une échéance invalide (compte supprimé)
/// est ignorée sans bloquer les autres.
pub async fn process_due(pool: &DbPool, today: NaiveDate) -> CoreResult<ProcessDueResult> {
    let mut result = ProcessDueResult::default();

    for item in repo::list_scheduled(pool).await? {
        let plan = plan_due(&item, today);
        if plan.transactions.is_empty() && plan.outcome == ScheduledOutcome::Unchanged {
            continue;
        }

        let outcome: CoreResult<ProcessDueResult> = async {
            let mut partial = ProcessDueResult::default();
            let mut tx = pool.begin().await.ctx("traitement des échéances")?;
            for transaction in &plan.transactions {
                if repo::insert_transaction_if_absent(&mut tx, transaction).await? {
                    partial.created_transactions += 1;
                }
            }
            match &plan.outcome {
                ScheduledOutcome::Advance(next_date) => {
                    let mut updated = item.clone();
                    updated.next_date = next_date.clone();
                    repo::update_scheduled(&mut tx, &updated).await?;
                    partial.updated_scheduled += 1;
                }
                ScheduledOutcome::Delete => {
                    repo::delete_scheduled(&mut tx, &item.id).await?;
                    partial.deleted_scheduled += 1;
                }
                ScheduledOutcome::Unchanged => {}
            }
            tx.commit().await.ctx("traitement des échéances")?;
            Ok(partial)
        }
        .await;

        match outcome {
            Ok(partial) => {
                result.created_transactions += partial.created_transactions;
                result.updated_scheduled += partial.updated_scheduled;
                result.deleted_scheduled += partial.deleted_scheduled;
            }
            Err(error) => log::warn!("Échéance {} ignorée : {error}", item.id),
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        parse_date(value).unwrap()
    }

    fn scheduled(frequency: Periodicity, next_date: &str) -> ScheduledTransaction {
        ScheduledTransaction {
            id: "s1".into(),
            description: "Loyer".into(),
            amount: 800.0,
            transaction_type: TransactionType::Expense,
            frequency,
            account_id: "a1".into(),
            next_date: next_date.into(),
            category: "1".into(),
            to_account_id: None,
            include_in_forecast: None,
            budget_id: None,
            end_date: None,
        }
    }

    #[test]
    fn generates_every_missed_monthly_occurrence() {
        let plan = plan_due(&scheduled(Periodicity::Monthly, "2026-07-31"), date("2026-09-30"));
        let ids: Vec<_> = plan.transactions.iter().map(|tx| tx.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "scheduled:s1:2026-07-31:single",
                "scheduled:s1:2026-08-31:single",
                "scheduled:s1:2026-09-30:single",
            ]
        );
        assert_eq!(plan.outcome, ScheduledOutcome::Advance("2026-10-30".into()));
    }

    #[test]
    fn future_items_are_untouched() {
        let plan = plan_due(&scheduled(Periodicity::Weekly, "2026-10-01"), date("2026-09-30"));
        assert!(plan.transactions.is_empty());
        assert_eq!(plan.outcome, ScheduledOutcome::Unchanged);
    }

    #[test]
    fn once_generates_a_single_occurrence_then_deletes() {
        let plan = plan_due(&scheduled(Periodicity::Once, "2026-09-01"), date("2026-09-30"));
        assert_eq!(plan.transactions.len(), 1);
        assert_eq!(plan.outcome, ScheduledOutcome::Delete);
    }

    #[test]
    fn stops_after_the_end_date() {
        let mut item = scheduled(Periodicity::Monthly, "2026-07-15");
        item.end_date = Some("2026-08-20".into());
        let plan = plan_due(&item, date("2026-10-01"));
        assert_eq!(plan.transactions.len(), 2);
        assert_eq!(plan.outcome, ScheduledOutcome::Advance("2026-09-15".into()));

        let ended = ScheduledTransaction {
            next_date: "2026-09-15".into(),
            ..item
        };
        let plan = plan_due(&ended, date("2026-10-01"));
        assert!(plan.transactions.is_empty());
        assert_eq!(plan.outcome, ScheduledOutcome::Unchanged);
    }

    #[test]
    fn transfers_create_linked_pairs() {
        let mut item = scheduled(Periodicity::Monthly, "2026-09-05");
        item.transaction_type = TransactionType::Transfer;
        item.to_account_id = Some("a2".into());
        let plan = plan_due(&item, date("2026-09-05"));

        assert_eq!(plan.transactions.len(), 2);
        let (from, to) = (&plan.transactions[0], &plan.transactions[1]);
        assert_eq!(from.transaction_type, TransactionType::Expense);
        assert_eq!(to.account_id, "a2");
        assert_eq!(from.linked_transaction_id.as_deref(), Some(to.id.as_str()));
        assert_eq!(to.linked_transaction_id.as_deref(), Some(from.id.as_str()));
        assert!(from.is_transfer && to.category == TRANSFER_CATEGORY_ID);
    }
}
