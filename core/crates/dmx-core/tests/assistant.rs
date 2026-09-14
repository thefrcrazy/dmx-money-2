//! Assistant : analyse de la demande et réponses chiffrées par le noyau.

use dmx_core::assistant::{self, AssistantIntent};
use dmx_core::engine::Engine;
use dmx_core::models::TransactionType;
use dmx_core::ops;

/// Les montants français utilisent une espace fine insécable : on la normalise pour comparer.
fn plain(text: &str) -> String {
    text.replace(['\u{202f}', '\u{a0}'], " ")
}

fn today() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 9, 14).unwrap()
}

fn engine_with_account() -> (Engine, String) {
    let engine = Engine::open_in_memory().unwrap();
    let id = engine
        .save_account(ops::AccountDraft {
            name: "Compte Courant".into(),
            initial_balance: 1_000.0,
            ..ops::new_account_draft()
        })
        .unwrap();
    (engine, id)
}

#[test]
fn adds_an_expense_from_a_sentence() {
    let (engine, account) = engine_with_account();

    let reply = engine
        .assistant("ajoute 12,50 € en alimentation", today(), true)
        .unwrap();

    assert!(reply.changed, "l'opération est enregistrée");
    assert!(reply.summary.contains("12,50"), "{}", reply.summary);
    assert!(reply.summary.contains("Alimentation"), "{}", reply.summary);
    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.transactions.len(), 1);
    let transaction = &snapshot.transactions[0];
    assert_eq!(transaction.amount, 12.5);
    assert_eq!(transaction.transaction_type, TransactionType::Expense);
    assert_eq!(transaction.account_id, account);
    assert_eq!(transaction.date, "2026-09-14");
}

#[test]
fn recognises_an_income_and_a_thousands_separator() {
    let (engine, _) = engine_with_account();

    let reply = engine
        .assistant("j'ai reçu un salaire de 2 450 euros", today(), true)
        .unwrap();

    assert!(reply.changed);
    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.transactions.len(), 1);
    assert_eq!(snapshot.transactions[0].amount, 2_450.0);
    assert_eq!(snapshot.transactions[0].transaction_type, TransactionType::Income);
}

#[test]
fn a_dry_run_changes_nothing() {
    let (engine, _) = engine_with_account();

    let reply = engine.assistant("ajoute 30 € en carburant", today(), false).unwrap();

    assert!(!reply.changed);
    assert!(engine.snapshot().unwrap().transactions.is_empty());
    match reply.intent {
        Some(AssistantIntent::AddTransaction(draft)) => {
            assert_eq!(draft.amount, 30.0);
            assert_eq!(draft.kind, TransactionType::Expense);
        }
        other => panic!("intention inattendue : {other:?}"),
    }
}

#[test]
fn answers_the_balance_question() {
    let (engine, _) = engine_with_account();

    let reply = engine.assistant("quel est mon solde ?", today(), true).unwrap();

    assert!(!reply.changed);
    assert!(plain(&reply.summary).contains("1 000,00"), "{}", reply.summary);
    assert!(reply.details.iter().any(|line| line.contains("Compte Courant")));
}

#[test]
fn answers_the_balance_of_one_account() {
    let (engine, _) = engine_with_account();
    engine
        .save_account(ops::AccountDraft {
            name: "Livret A".into(),
            initial_balance: 5_000.0,
            ..ops::new_account_draft()
        })
        .unwrap();

    let reply = engine.assistant("combien sur le livret A ?", today(), true).unwrap();

    assert!(reply.summary.starts_with("Livret A"), "{}", reply.summary);
    assert!(plain(&reply.summary).contains("5 000,00"), "{}", reply.summary);
}

#[test]
fn answers_the_budget_question() {
    let (engine, _) = engine_with_account();
    engine
        .save_budget(ops::BudgetDraft {
            name: "Alimentation".into(),
            amount: 400.0,
            category_id: "5".into(),
            ..engine.budget_draft(None, &[]).unwrap()
        })
        .unwrap();
    engine.assistant("ajoute 120 € en alimentation", today(), true).unwrap();

    let reply = engine
        .assistant("combien me reste-t-il en alimentation ?", today(), true)
        .unwrap();

    assert!(reply.summary.contains("Alimentation"), "{}", reply.summary);
    assert!(reply.summary.contains("280,00"), "{}", reply.summary);
}

#[test]
fn answers_the_month_summary() {
    let (engine, _) = engine_with_account();
    engine.assistant("ajoute 40 € en carburant", today(), true).unwrap();

    let reply = engine.assistant("résumé du mois", today(), true).unwrap();

    assert!(reply.summary.contains("septembre 2026"), "{}", reply.summary);
    assert!(reply.summary.contains("40,00"), "{}", reply.summary);
}

#[test]
fn structured_intent_resolves_names() {
    let (engine, account) = engine_with_account();

    let intent = engine
        .assistant_draft(
            18.9,
            TransactionType::Expense,
            Some("Pharmacie"),
            Some("Compte Courant"),
            Some("Ordonnance"),
            today(),
        )
        .unwrap();
    let reply = engine.assistant_execute(intent, today()).unwrap();

    assert!(reply.changed);
    let snapshot = engine.snapshot().unwrap();
    let transaction = &snapshot.transactions[0];
    assert_eq!(transaction.amount, 18.9);
    assert_eq!(transaction.account_id, account);
    assert_eq!(transaction.category, "14");
    assert_eq!(transaction.description, "Ordonnance");
}

#[test]
fn an_unknown_sentence_is_reported() {
    let (engine, _) = engine_with_account();

    let reply = engine.assistant("raconte-moi une blague", today(), true).unwrap();

    assert!(!reply.changed);
    assert!(reply.intent.is_none());
    assert!(reply.summary.contains("pas compris"), "{}", reply.summary);
}

#[test]
fn amount_parsing_accepts_both_separators() {
    let engine = Engine::open_in_memory().unwrap();
    let snapshot = engine.snapshot().unwrap();
    for (sentence, expected) in [
        ("ajoute 12,50 en alimentation", 12.5),
        ("ajoute 12.50 en alimentation", 12.5),
        ("ajoute 1 200 en loyer", 1_200.0),
        ("ajoute 7 euros en pharmacie", 7.0),
    ] {
        match assistant::interpret(&snapshot, sentence, today()) {
            Some(AssistantIntent::AddTransaction(draft)) => assert_eq!(draft.amount, expected, "{sentence}"),
            other => panic!("{sentence} → {other:?}"),
        }
    }
}
