//! Pointer une jambe de virement pointe aussi la jambe liée.

use dmx_core::engine::Engine;
use dmx_core::models::TransactionType;
use dmx_core::ops;

#[test]
fn checking_one_transfer_leg_checks_the_linked_leg() {
    let engine = Engine::open_in_memory().unwrap();
    let savings = engine
        .save_account(ops::AccountDraft {
            name: "Livret A".into(),
            ..ops::new_account_draft()
        })
        .unwrap();
    let current = engine
        .save_account(ops::AccountDraft {
            name: "Maxim".into(),
            ..ops::new_account_draft()
        })
        .unwrap();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 4).unwrap();
    let mut draft = engine.transaction_draft(None, &[], today).unwrap();
    draft.kind = TransactionType::Transfer;
    draft.amount = 150.0;
    draft.description = "Virement".into();
    draft.account_id = savings.clone();
    draft.to_account_id = Some(current.clone());
    let ids = engine.save_transaction(draft).unwrap();
    assert_eq!(ids.len(), 2);

    // Pointer l'entrée sur le compte courant…
    let incoming = engine
        .snapshot()
        .unwrap()
        .transactions
        .iter()
        .find(|transaction| transaction.account_id == current)
        .unwrap()
        .id
        .clone();
    assert!(engine
        .toggle_transactions_checked(std::slice::from_ref(&incoming))
        .unwrap());

    // … pointe aussi la sortie du Livret A, et l'inverse dépointe les deux.
    let snapshot = engine.snapshot().unwrap();
    assert!(snapshot.transactions.iter().all(|transaction| transaction.checked));
    let outgoing = snapshot
        .transactions
        .iter()
        .find(|transaction| transaction.account_id == savings)
        .unwrap();
    assert_eq!(
        outgoing.transaction_type,
        TransactionType::Expense,
        "sortie stockée en dépense"
    );
    assert!(!engine
        .toggle_transactions_checked(std::slice::from_ref(&outgoing.id))
        .unwrap());
    assert!(engine
        .snapshot()
        .unwrap()
        .transactions
        .iter()
        .all(|transaction| !transaction.checked));
}

#[test]
fn internal_transfers_change_balances_but_not_income_or_expense_charts() {
    let engine = Engine::open_in_memory().unwrap();
    let from = engine
        .save_account(ops::AccountDraft {
            name: "Courant".into(),
            initial_balance: 1000.0,
            ..ops::new_account_draft()
        })
        .unwrap();
    let to = engine
        .save_account(ops::AccountDraft {
            name: "Épargne".into(),
            ..ops::new_account_draft()
        })
        .unwrap();
    let day = chrono::NaiveDate::from_ymd_opt(2026, 9, 16).unwrap();
    let mut draft = engine.transaction_draft(None, &[], day).unwrap();
    draft.kind = TransactionType::Transfer;
    draft.amount = 150.0;
    draft.account_id = from.clone();
    draft.to_account_id = Some(to);
    engine.save_transaction(draft).unwrap();
    for filter in [vec![], vec![from]] {
        let dashboard = engine.dashboard(&filter, day).unwrap();
        assert_eq!(dashboard.month.income, 0.0);
        assert_eq!(dashboard.month.expenses, 0.0);
        assert!(dashboard.top_categories.is_empty());
        assert_eq!(
            dashboard.balances.current_balance,
            if filter.is_empty() { 1000.0 } else { 850.0 }
        );
        let query = dmx_core::analytics::AnalyticsQuery {
            accounts: filter,
            ..Default::default()
        };
        let analytics = engine.analytics(&query, day).unwrap();
        assert!(analytics.expenses_by_category.is_empty());
        assert!(analytics.income_by_category.is_empty());
        assert!(analytics
            .income_vs_expenses
            .iter()
            .all(|bar| bar.income == 0.0 && bar.expenses == 0.0));
    }
}
