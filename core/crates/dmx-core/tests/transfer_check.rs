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
    assert!(engine.toggle_transactions_checked(&[incoming.clone()]).unwrap());

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
    assert!(!engine.toggle_transactions_checked(&[outgoing.id.clone()]).unwrap());
    assert!(engine
        .snapshot()
        .unwrap()
        .transactions
        .iter()
        .all(|transaction| !transaction.checked));
}
