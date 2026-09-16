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

#[test]
fn transfers_in_french_and_english_preserve_direction_and_balances() {
    for sentence in [
        "virement de 50 € de Compte Courant vers Livret A",
        "transfère 50 euros depuis Compte Courant vers Livret A",
        "transfer 50 euros from Compte Courant to Livret A",
        "transfer 50 euros to Livret A from Compte Courant",
        "virement de 50 € vers Livret A depuis Compte Courant",
    ] {
        let (engine, from) = engine_with_account();
        let to = engine
            .save_account(ops::AccountDraft {
                name: "Livret A".into(),
                ..ops::new_account_draft()
            })
            .unwrap();
        let preview = engine.assistant(sentence, today(), false).unwrap();
        assert!(!preview.changed);
        assert_eq!(engine.snapshot().unwrap().transactions.len(), 0);
        let reply = engine.assistant(sentence, today(), true).unwrap();
        assert!(reply.changed, "{sentence}: {reply:?}");
        assert!(reply.summary.contains("Virement"));
        let snapshot = engine.snapshot().unwrap();
        assert_eq!(snapshot.transactions.len(), 2);
        let draft = match reply.intent.unwrap() {
            AssistantIntent::AddTransaction(draft) => draft,
            other => panic!("{other:?}"),
        };
        assert_eq!(draft.kind, TransactionType::Transfer);
        assert_eq!(draft.account_id, from);
        assert_eq!(draft.to_account_id, Some(to));
        assert_eq!(draft.amount, 50.0);
        assert!(plain(
            &engine
                .assistant("quel est mon solde ?", today(), false)
                .unwrap()
                .summary
        )
        .contains("1 000,00"));
    }
}

#[test]
fn incomplete_ambiguous_or_unknown_commands_do_not_write() {
    let (engine, _) = engine_with_account();
    engine
        .save_account(ops::AccountDraft {
            name: "Livret A".into(),
            ..ops::new_account_draft()
        })
        .unwrap();
    for sentence in [
        "virement de 50 euros vers Livret A",
        "transfer -50 from Compte Courant to Livret A",
        "transfer 50 from Unknown to Livret A",
        "virement de 50 de Compte Courant vers Compte Courant",
        "transfer 50 Compte Courant Livret A",
        "transfer from Compte Courant to Livret A",
        "what happened on 12 September?",
        "virement de 50 de Compte Courant vers Livret Absent",
    ] {
        let reply = engine.assistant(sentence, today(), true).unwrap();
        assert!(!reply.changed, "{sentence}: {reply:?}");
    }
    assert!(engine.snapshot().unwrap().transactions.is_empty());
    engine
        .save_account(ops::AccountDraft {
            name: "Livret A".into(),
            ..ops::new_account_draft()
        })
        .unwrap();
    assert!(
        !engine
            .assistant("transfer 50 from Compte Courant to Livret A", today(), true)
            .unwrap()
            .changed
    );
}

#[test]
fn english_read_intents_and_income_are_recognised() {
    let (engine, _) = engine_with_account();
    let snapshot = engine.snapshot().unwrap();
    assert!(matches!(
        assistant::interpret(&snapshot, "what is my balance?", today()),
        Some(AssistantIntent::Balance { .. })
    ));
    assert!(matches!(
        assistant::interpret(&snapshot, "remaining budget", today()),
        Some(AssistantIntent::BudgetRemaining { .. })
    ));
    assert!(matches!(
        assistant::interpret(&snapshot, "monthly summary", today()),
        Some(AssistantIntent::MonthSummary)
    ));
    assert!(matches!(
        assistant::interpret(&snapshot, "upcoming payments", today()),
        Some(AssistantIntent::Upcoming)
    ));
    assert!(matches!(
        assistant::interpret(&snapshot, "process due payments", today()),
        Some(AssistantIntent::ProcessDue)
    ));
    let result = engine.assistant("received salary 2450 euros", today(), true).unwrap();
    assert!(result.changed);
    assert_eq!(
        engine.snapshot().unwrap().transactions[0].transaction_type,
        TransactionType::Income
    );
}

#[test]
fn structured_entities_resolve_by_id_without_cross_matching() {
    let (engine, account) = engine_with_account();
    let category = engine
        .snapshot()
        .unwrap()
        .categories
        .iter()
        .find(|c| c.name == "Alimentation")
        .unwrap()
        .id
        .clone();
    let intent = engine
        .assistant_draft(
            25.0,
            TransactionType::Expense,
            Some(&category),
            Some(&account),
            None,
            today(),
        )
        .unwrap();
    let reply = engine.assistant_execute(intent, today()).unwrap();
    match reply.intent.unwrap() {
        AssistantIntent::AddTransaction(draft) => {
            assert_eq!(draft.account_id, account);
            assert_eq!(draft.category_id, category);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn transfer_matching_handles_overlapping_and_numbered_account_names() {
    let (engine, source) = engine_with_account();
    engine
        .save_account(ops::AccountDraft {
            name: "Compte".into(),
            ..ops::new_account_draft()
        })
        .unwrap();
    let destination = engine
        .save_account(ops::AccountDraft {
            name: "Livret 2026".into(),
            ..ops::new_account_draft()
        })
        .unwrap();
    let reply = engine
        .assistant(
            "transfer from Compte Courant to Livret 2026 amount 50 euros",
            today(),
            true,
        )
        .unwrap();
    match reply.intent.unwrap() {
        AssistantIntent::AddTransaction(draft) => {
            assert_eq!(draft.account_id, source);
            assert_eq!(draft.to_account_id, Some(destination));
            assert_eq!(draft.amount, 50.0);
        }
        other => panic!("{other:?}"),
    }
}
