use dmx_core::models::{Periodicity, TransactionType};
use dmx_core::{analytics::AnalyticsQuery, journal::JournalQuery, ops, predictions::PredictionQuery, Engine};

#[test]
fn catching_up_scheduled_payments_refreshes_every_financial_view_without_duplicates() {
    let engine = Engine::open_in_memory().unwrap();
    let account = engine
        .save_account(ops::AccountDraft {
            name: "Courant".into(),
            initial_balance: 1000.0,
            ..ops::new_account_draft()
        })
        .unwrap();
    let day = chrono::NaiveDate::from_ymd_opt(2026, 9, 16).unwrap();
    let mut scheduled = ops::new_scheduled_draft(&engine.snapshot().unwrap(), day);
    scheduled.description = "Échéance test".into();
    scheduled.account_id = account.clone();
    scheduled.category_id = "5".into();
    scheduled.kind = TransactionType::Expense;
    scheduled.amount = 25.0;
    scheduled.frequency = Periodicity::Daily;
    scheduled.next_date = "2026-09-14".into();
    engine.save_scheduled(scheduled).unwrap();
    // Populate caches before processing the three missed occurrences.
    assert_eq!(engine.dashboard(&[], day).unwrap().balances.current_balance, 1000.0);
    assert_eq!(engine.process_due_scheduled(day).unwrap().created_transactions, 3);
    assert_eq!(
        engine
            .journal(&JournalQuery::default())
            .unwrap()
            .total_transaction_count,
        3
    );
    let dashboard = engine.dashboard(&[], day).unwrap();
    assert_eq!(dashboard.balances.current_balance, 925.0);
    assert_eq!(dashboard.balances.checked_balance, 1000.0);
    assert_eq!(dashboard.month.expenses, 75.0);
    assert_eq!(
        engine
            .predictions(&PredictionQuery::default(), day)
            .unwrap()
            .current_total_balance,
        925.0
    );
    assert_eq!(engine.tray_summary().unwrap().total, 925.0);
    assert_eq!(
        engine
            .analytics(&AnalyticsQuery::default(), day)
            .unwrap()
            .expenses_by_category
            .iter()
            .map(|slice| slice.value)
            .sum::<f64>(),
        75.0
    );
    assert_eq!(engine.process_due_scheduled(day).unwrap().created_transactions, 0);
    assert_eq!(engine.snapshot().unwrap().transactions.len(), 3);
    // A new due payment added later the same day must still be picked up.
    let mut next = ops::new_scheduled_draft(&engine.snapshot().unwrap(), day);
    next.description = "Ajout le même jour".into();
    next.account_id = account;
    next.category_id = "5".into();
    next.amount = 10.0;
    next.next_date = "2026-09-16".into();
    next.frequency = Periodicity::Once;
    engine.save_scheduled(next).unwrap();
    assert_eq!(engine.process_due_scheduled(day).unwrap().created_transactions, 1);
    assert_eq!(engine.dashboard(&[], day).unwrap().balances.current_balance, 915.0);
}
