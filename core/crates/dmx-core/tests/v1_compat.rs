//! Reprise d'une base DmxMoney 1.x réelle (schéma et sérialisations d'origine).

use dmx_core::backup::RestoreMode;
use dmx_core::dates::parse_date;
use dmx_core::journal::JournalQuery;
use dmx_core::models::{Theme, TransactionType};
use dmx_core::{Engine, EngineConfig};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};
use std::path::{Path, PathBuf};

fn fixture_sql() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/v1-database.sql");
    std::fs::read_to_string(path).expect("fixture v1 lisible")
}

fn create_v1_database(root: &Path) -> PathBuf {
    let path = root.join("com.dmxmoney.desktop").join(dmx_core::db::DATABASE_FILE_NAME);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut connection = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .connect()
            .await
            .unwrap();
        sqlx::raw_sql(&fixture_sql()).execute(&mut connection).await.unwrap();
        connection.close().await.unwrap();
    });
    path
}

#[test]
fn opens_and_upgrades_a_v1_database() {
    let root = std::env::temp_dir().join(format!("dmx-v1-compat-{}", uuid::Uuid::new_v4()));
    let legacy = create_v1_database(&root);

    let engine = Engine::open(EngineConfig {
        data_dir: root.join("dmx-money-2"),
        legacy_database_paths: vec![legacy.clone()],
    })
    .unwrap();
    assert!(engine.open_report().imported_legacy_database.is_some());

    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.accounts.len(), 2);
    assert_eq!(snapshot.transactions.len(), 5);

    let settings = &snapshot.settings;
    assert_eq!(settings.theme, Theme::Dark);
    assert_eq!(settings.accent_color(), Some("#AF52DE"));
    assert_eq!(
        settings.account_groups.get("acc-epargne").map(String::as_str),
        Some("Épargne")
    );
    assert_eq!(settings.last_seen_version.as_deref(), Some("1.0.22"));
    assert_eq!(settings.prediction_alert_threshold, 100.0);
    assert_eq!(settings.prediction_fake_transactions.len(), 1);
    assert_eq!(settings.analytics_hidden_expense_categories, vec!["transfer"]);

    // Migration applicative 1.x : l'échéance incluse dans les prévisions devient un budget lié.
    assert_eq!(snapshot.budgets.len(), 2);
    let courses = snapshot.scheduled.iter().find(|item| item.id == "sch-courses").unwrap();
    let linked = snapshot.budget(courses.budget_id.as_deref().unwrap()).unwrap();
    assert_eq!((linked.name.as_str(), linked.amount), ("Courses hebdo", 120.0));

    let dashboard = engine.dashboard(&[], parse_date("2026-09-15").unwrap()).unwrap();
    assert_eq!(dashboard.balances.current_balance, 8065.8);
    assert_eq!(dashboard.balances.checked_balance, 6500.0 + 2500.0 - 84.2);

    let draft = engine
        .transaction_draft(Some("tr-to"), &[], parse_date("2026-09-15").unwrap())
        .unwrap();
    assert_eq!(draft.kind, TransactionType::Transfer);
    assert_eq!(draft.account_id, "acc-courant");
    assert_eq!(draft.to_account_id.as_deref(), Some("acc-epargne"));

    // Les occurrences déjà générées par 1.x ne sont pas dupliquées.
    let processed = engine.process_due_scheduled(parse_date("2026-10-02").unwrap()).unwrap();
    assert_eq!(processed.created_transactions, 3);
    let journal = engine
        .journal(&JournalQuery {
            search: "loyer".into(),
            ..JournalQuery::default()
        })
        .unwrap();
    assert_eq!(journal.rows.len(), 2);

    // La base d'origine est intacte.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let legacy_count: i64 = runtime.block_on(async {
        let mut connection = SqliteConnectOptions::new()
            .filename(&legacy)
            .read_only(true)
            .connect()
            .await
            .unwrap();
        let count = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        count
    });
    assert_eq!(legacy_count, 5);

    // Une sauvegarde .dmx relit toutes les données dans une base neuve.
    let backup = engine.export_backup().unwrap();
    let restored = Engine::open_in_memory().unwrap();
    let summary = restored.restore_backup(&backup, RestoreMode::Replace).unwrap();
    assert_eq!(summary.transactions, 8);
    let restored_snapshot = restored.snapshot().unwrap();
    assert_eq!(restored_snapshot.transactions.len(), 8);
    assert_eq!(restored_snapshot.settings.custom_groups, vec!["Épargne"]);

    drop(engine);
    let _ = std::fs::remove_dir_all(root);
}

/// Le dossier de la 2.x contient déjà une base plus pauvre (copie d'un ancien build) :
/// la base 1.x plus complète doit être détectée, puis reprise sans rien perdre.
#[test]
fn a_richer_legacy_database_is_detected_and_can_be_adopted() {
    let root = std::env::temp_dir().join(format!("dmx-adopt-{}", uuid::Uuid::new_v4()));
    let legacy = create_v1_database(&root);

    // Base « 2.x » déjà en place : un seul compte, aucune opération.
    let data_dir = root.join("com.dmxmoney.app");
    std::fs::create_dir_all(&data_dir).unwrap();
    {
        let engine = Engine::open(EngineConfig {
            data_dir: data_dir.clone(),
            legacy_database_paths: Vec::new(),
        })
        .unwrap();
        engine
            .save_account(dmx_core::ops::AccountDraft {
                name: "Vide".into(),
                ..dmx_core::ops::new_account_draft()
            })
            .unwrap();
    }

    let engine = Engine::open(EngineConfig {
        data_dir: data_dir.clone(),
        legacy_database_paths: vec![legacy.clone()],
    })
    .unwrap();

    let candidate = engine
        .open_report()
        .legacy_candidate
        .clone()
        .expect("base 1.x plus complète détectée");
    assert_eq!(candidate.path, legacy.to_string_lossy());
    assert!(candidate.transactions > 0);

    let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 13).unwrap();
    let adoption = engine.adopt_legacy_database(&candidate.path, today).unwrap();
    assert!(adoption.summary.transactions > 0);
    assert!(
        std::path::Path::new(&adoption.backup_file).exists(),
        "sauvegarde .dmx écrite"
    );

    // Les données 1.x sont là, et la base d'origine n'a pas bougé.
    let snapshot = engine.snapshot().unwrap();
    assert!(!snapshot.transactions.is_empty());
    assert!(snapshot.accounts.iter().all(|account| account.name != "Vide"));
    assert!(legacy.exists());
}
