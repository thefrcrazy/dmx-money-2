use chrono::NaiveDate;
use dmx_core::{
    backup, db,
    import::{self, ImportTarget, StatementImportRequest},
    models::TransactionType,
    ops::{self, AccountDraft},
    repo, snapshot,
    sync::{self, RemoteChange},
};

async fn account(pool: &db::DbPool, name: &str) -> String {
    ops::save_account(
        pool,
        AccountDraft {
            name: name.into(),
            ..ops::new_account_draft()
        },
    )
    .await
    .unwrap()
}
fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 3).unwrap()
}
fn remote(entity: &str, id: &str, payload: Option<String>) -> RemoteChange {
    RemoteChange {
        entity: entity.into(),
        record_id: id.into(),
        deleted: payload.is_none(),
        updated_at: "2090-01-01T00:00:00.000Z".into(),
        payload,
    }
}

#[tokio::test]
async fn native_three_way_merge_preserves_remote_amount_and_rejects_conflict() {
    let pool = db::open_memory_pool().await.unwrap();
    let account = account(&pool, "Fictif").await;
    let snap = snapshot::load(&pool).await.unwrap();
    let mut draft = ops::new_transaction_draft(&snap, &[account], date());
    draft.amount = 100.0;
    draft.category_id = "food".into();
    let id = ops::save_transaction(&pool, draft).await.unwrap().remove(0);
    let base = ops::transaction_draft(&snapshot::load(&pool).await.unwrap(), &id).unwrap();
    let mut remote = base.clone();
    remote.amount = 120.0;
    ops::save_transaction(&pool, remote).await.unwrap();
    let mut edit = base.clone();
    edit.description = "Libellé local".into();
    ops::save_transaction_with_base(&pool, edit, base.clone())
        .await
        .unwrap();
    let current = repo::list_transactions(&pool).await.unwrap().remove(0);
    assert_eq!((current.amount, current.description.as_str()), (120.0, "Libellé local"));
    let mut conflict = base.clone();
    conflict.amount = 130.0;
    let error = ops::save_transaction_with_base(&pool, conflict, base)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("Conflit"));
    assert_eq!(repo::list_transactions(&pool).await.unwrap()[0].amount, 120.0);
}

#[tokio::test]
async fn merge_from_income_side_keeps_transfer_pair_and_remote_checked_state() {
    let pool = db::open_memory_pool().await.unwrap();
    let from = account(&pool, "Source").await;
    let to = account(&pool, "Destination").await;
    let mut draft = ops::new_transaction_draft(&snapshot::load(&pool).await.unwrap(), &[from], date());
    draft.kind = TransactionType::Transfer;
    draft.amount = 100.0;
    draft.to_account_id = Some(to);
    let ids = ops::save_transaction(&pool, draft).await.unwrap();
    let base = ops::transaction_draft(&snapshot::load(&pool).await.unwrap(), &ids[1]).unwrap();
    let mut changed = base.clone();
    changed.amount = 120.0;
    ops::save_transaction(&pool, changed).await.unwrap();
    ops::set_transactions_checked(&pool, &ids, true).await.unwrap();
    let mut local = base.clone();
    local.description = "Note".into();
    ops::save_transaction_with_base(&pool, local, base).await.unwrap();
    let rows = repo::list_transactions(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .all(|item| item.amount == 120.0 && item.description == "Note" && item.checked));
}

#[tokio::test]
async fn ofx_fitids_survive_reimport_backup_and_incremental_identical_purchase() {
    let pool = db::open_memory_pool().await.unwrap();
    let id = account(&pool, "Banque fictive").await;
    let parse = |ids: &[&str]| {
        let blocks: String = ids
            .iter()
            .map(|id| format!("<STMTTRN><DTPOSTED>20261003<TRNAMT>-20<NAME>CAFE<FITID>{id}</STMTTRN>"))
            .collect();
        import::parse_ofx_transactions(&format!("<OFX><FI><ORG>FI<FID>123</FI><STMTRS><BANKACCTFROM><BANKID>123<ACCTID>ACCOUNT<ACCTTYPE>CHECKING</BANKACCTFROM>{blocks}</STMTRS></OFX>"), date()).unwrap()
    };
    let request = |transactions| StatementImportRequest {
        transactions,
        target: ImportTarget::Existing(id.clone()),
        category_mapping: vec![],
    };
    let first = import::import_statement(&pool, request(parse(&["one", "two"])))
        .await
        .unwrap();
    assert_eq!((first.imported, first.duplicates), (2, 0));
    let again = import::import_statement(&pool, request(parse(&["one", "two"])))
        .await
        .unwrap();
    assert_eq!((again.imported, again.duplicates), (0, 2));
    let new = import::import_statement(&pool, request(parse(&["three"])))
        .await
        .unwrap();
    assert_eq!((new.imported, new.duplicates), (1, 0));
    let before = repo::list_transactions(&pool).await.unwrap();
    assert!(before
        .iter()
        .all(|item| item.bank_source.is_some() && item.bank_transaction_id.is_some()));
    let content = backup::export_backup(&pool).await.unwrap();
    let restored = db::open_memory_pool().await.unwrap();
    backup::restore_backup(&restored, &content, backup::RestoreMode::Replace)
        .await
        .unwrap();
    let mut restored_rows = repo::list_transactions(&restored).await.unwrap();
    let mut expected = before.clone();
    restored_rows.sort_by(|a, b| a.id.cmp(&b.id));
    expected.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(restored_rows, expected);
    assert_eq!(
        import::bank_import_id(
            &id,
            before[0].bank_source.as_deref(),
            before[0].bank_transaction_id.as_deref()
        ),
        before[0].id
    );
}

#[tokio::test]
async fn out_of_order_account_dependency_is_staged_then_applied_without_orphans() {
    let source = db::open_memory_pool().await.unwrap();
    let dest = db::open_memory_pool().await.unwrap();
    let id = account(&source, "Fictif").await;
    let mut draft = ops::new_transaction_draft(
        &snapshot::load(&source).await.unwrap(),
        std::slice::from_ref(&id),
        date(),
    );
    draft.amount = 10.0;
    draft.category_id = "food".into();
    ops::save_transaction(&source, draft).await.unwrap();
    let item = repo::list_transactions(&source).await.unwrap().remove(0);
    let report = sync::apply_remote_changes(
        &dest,
        vec![remote(
            "transactions",
            &item.id,
            Some(serde_json::to_string(&item).unwrap()),
        )],
    )
    .await
    .unwrap();
    assert_eq!((report.applied, report.skipped), (0, 1));
    assert!(repo::list_transactions(&dest).await.unwrap().is_empty());
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM sync_pending_remote")
        .fetch_one(&dest)
        .await
        .unwrap();
    assert_eq!(pending, 1);
    let account = repo::list_accounts(&source).await.unwrap().remove(0);
    let report = sync::apply_remote_changes(
        &dest,
        vec![remote("accounts", &id, Some(serde_json::to_string(&account).unwrap()))],
    )
    .await
    .unwrap();
    assert_eq!(report.applied, 2);
    assert_eq!(repo::list_transactions(&dest).await.unwrap(), vec![item]);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&dest)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn split_transfer_and_tombstones_are_applied_only_as_complete_pair() {
    let source = db::open_memory_pool().await.unwrap();
    let dest = db::open_memory_pool().await.unwrap();
    let from = account(&source, "A").await;
    let to = account(&source, "B").await;
    let accounts = repo::list_accounts(&source).await.unwrap();
    sync::apply_remote_changes(
        &dest,
        accounts
            .iter()
            .map(|item| remote("accounts", &item.id, Some(serde_json::to_string(item).unwrap())))
            .collect(),
    )
    .await
    .unwrap();
    let mut draft = ops::new_transaction_draft(&snapshot::load(&source).await.unwrap(), &[from], date());
    draft.kind = TransactionType::Transfer;
    draft.amount = 10.0;
    draft.to_account_id = Some(to);
    ops::save_transaction(&source, draft).await.unwrap();
    let items = repo::list_transactions(&source).await.unwrap();
    let changes: Vec<_> = items
        .iter()
        .map(|item| remote("transactions", &item.id, Some(serde_json::to_string(item).unwrap())))
        .collect();
    sync::apply_remote_changes(&dest, vec![changes[0].clone()])
        .await
        .unwrap();
    assert!(repo::list_transactions(&dest).await.unwrap().is_empty());
    sync::apply_remote_changes(&dest, vec![changes[1].clone()])
        .await
        .unwrap();
    assert_eq!(repo::list_transactions(&dest).await.unwrap().len(), 2);
    sync::apply_remote_changes(&dest, vec![remote("transactions", &items[0].id, None)])
        .await
        .unwrap();
    assert_eq!(repo::list_transactions(&dest).await.unwrap().len(), 2);
    sync::apply_remote_changes(&dest, vec![remote("transactions", &items[1].id, None)])
        .await
        .unwrap();
    assert!(repo::list_transactions(&dest).await.unwrap().is_empty());
}

#[tokio::test]
async fn account_and_group_write_roll_back_together_on_settings_failure() {
    let pool = db::open_memory_pool().await.unwrap();
    let id = account(&pool, "Avant").await;
    sqlx::query(
        "CREATE TRIGGER reject_group BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT, 'fixture failure'); END",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut draft = ops::account_draft(&snapshot::load(&pool).await.unwrap(), &id).unwrap();
    draft.name = "Après".into();
    draft.group = Some("Fictif".into());
    assert!(ops::save_account(&pool, draft).await.is_err());
    assert_eq!(repo::list_accounts(&pool).await.unwrap()[0].name, "Avant");
}

#[test]
fn graph_limits_reject_millions_of_days_and_bound_pure_ranges() {
    let far = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
    assert!(dmx_core::limits::graph_range(date(), far, 1).is_err());
    assert!(dmx_core::limits::graph_range(date(), date(), 500_001).is_err());
    let (start, end) =
        dmx_core::predictions::projection_range(dmx_core::models::TimeRange::Custom, Some("9999-12-31"), false, date());
    assert_eq!((end - start).num_days() + 1, 1826);
}

#[tokio::test]
async fn large_accepted_amounts_do_not_wrap_aggregates() {
    let pool = db::open_memory_pool().await.unwrap();
    let id = account(&pool, "Fictif").await;
    let mut snap = snapshot::load(&pool).await.unwrap();
    let item = dmx_core::models::Transaction {
        id: "fictif".into(),
        date: "2026-10-03".into(),
        account_id: id,
        transaction_type: TransactionType::Income,
        amount: 90_071_992_547_409.9,
        category: "test".into(),
        description: String::new(),
        checked: true,
        is_transfer: false,
        linked_transaction_id: None,
        bank_source: None,
        bank_transaction_id: None,
    };
    assert!(dmx_core::metrics::is_valid_money(item.amount));
    snap.transactions = vec![item; 1025];
    let summary = dmx_core::metrics::balance_summary(&snap, &[]);
    assert!(summary.current_balance > 0.0);
    assert_eq!(
        summary.current_balance,
        dmx_core::metrics::euros(i128::from(dmx_core::metrics::cents(snap.transactions[0].amount)) * 1025)
    );
    assert_eq!(
        dmx_core::journal::running_balances(&snap)["fictif"],
        i128::from(dmx_core::metrics::cents(snap.transactions[0].amount)) * 1025
    );
}

#[cfg(unix)]
#[tokio::test]
async fn sqlite_and_sidecars_are_private_even_when_existing_modes_are_permissive() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!("dmx-private-fixture-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    db::protect_data_directory(&path).unwrap();
    let database = path.join(db::DATABASE_FILE_NAME);
    std::fs::write(&database, []).unwrap();
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o644)).unwrap();
    let pool = db::open_pool(&database).await.unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o700);
    for file in [
        database.clone(),
        path.join(format!("{}-wal", db::DATABASE_FILE_NAME)),
        path.join(format!("{}-shm", db::DATABASE_FILE_NAME)),
    ] {
        assert_eq!(std::fs::metadata(file).unwrap().permissions().mode() & 0o777, 0o600);
    }
    pool.close().await;
    std::fs::remove_dir_all(&path).unwrap();
}

#[tokio::test]
async fn housekeeping_keeps_legacy_receipts_and_live_sessions() {
    let pool = db::open_memory_pool().await.unwrap();
    sqlx::query("INSERT INTO mobile_mutation_receipts(id,fingerprint,replay_until) VALUES ('legacy','a',NULL),('expired','b','2000-01-01T00:00:00Z'),('live','c','2090-01-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO mobile_sessions(id,session_hash,csrf_hash,expires_at,created_at) VALUES ('expired','e','e','2000-01-01T00:00:00Z','2000-01-01T00:00:00Z'),('live','l','l',strftime('%Y-%m-%dT%H:%M:%fZ','now','+1 day'),strftime('%Y-%m-%dT%H:%M:%fZ','now'))").execute(&pool).await.unwrap();
    db::purge_expired_companion_records(&pool).await.unwrap();
    let receipts: Vec<String> = sqlx::query_scalar("SELECT id FROM mobile_mutation_receipts ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(receipts, vec!["legacy", "live"]);
    let sessions: Vec<String> = sqlx::query_scalar("SELECT id FROM mobile_sessions")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(sessions, vec!["live"]);
}

#[tokio::test]
async fn inline_baseline_rejects_conflicting_cell_without_mutation() {
    let pool = db::open_memory_pool().await.unwrap();
    let id = account(&pool, "Fictif").await;
    let mut draft = ops::new_transaction_draft(&snapshot::load(&pool).await.unwrap(), &[id], date());
    draft.amount = 10.0;
    draft.category_id = "test".into();
    let id = ops::save_transaction(&pool, draft).await.unwrap().remove(0);
    ops::update_transaction_inline(&pool, &id, ops::InlineEdit::Amount(20.0))
        .await
        .unwrap();
    assert!(ops::update_transaction_inline_with_base(
        &pool,
        &id,
        ops::InlineEdit::Amount(30.0),
        ops::InlineEdit::Amount(10.0)
    )
    .await
    .is_err());
    assert_eq!(repo::list_transactions(&pool).await.unwrap()[0].amount, 20.0);
}

#[test]
fn csv_preview_counts_multiline_rows_without_retaining_entire_file() {
    let content = format!("date;amount;note\n{}", "2026-10-03;-1;\"two\nlines\"\n".repeat(1000));
    let preview = import::preview_csv(
        &content,
        import::CsvOptions {
            separator: ';',
            has_header: true,
        },
    )
    .unwrap();
    assert_eq!(preview.row_count, 1000);
    assert_eq!(preview.rows.len(), 5);
    assert_eq!(preview.rows[0][2], "two\nlines");
}

#[tokio::test]
async fn oversized_settings_horizons_are_rejected_without_a_partial_write() {
    let pool = db::open_memory_pool().await.unwrap();
    let before = dmx_core::settings::load_app_settings(&pool).await.unwrap();
    let result = dmx_core::settings::apply_change(
        &pool,
        dmx_core::settings::SettingsChange::PredictionCustomEndDate("9999-12-31".into()),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(dmx_core::settings::load_app_settings(&pool).await.unwrap(), before);
}

#[test]
fn ancient_recurrences_fast_forward_without_changing_month_end_semantics() {
    use dmx_core::{dates, models::Periodicity};
    let parse = |value| dates::parse_date(value).unwrap();
    assert_eq!(
        dates::first_occurrence_on_or_after(parse("0001-01-01"), Periodicity::Daily, parse("2026-10-03")),
        Some(parse("2026-10-03"))
    );
    for frequency in [
        Periodicity::Weekly,
        Periodicity::Biweekly,
        Periodicity::Bimonthly,
        Periodicity::Fourweekly,
        Periodicity::Monthly,
        Periodicity::Bimestrial,
        Periodicity::Quarterly,
        Periodicity::Fourmonthly,
        Periodicity::Semiannual,
        Periodicity::Annual,
        Periodicity::Biennial,
    ] {
        for start in ["1900-01-31", "1996-02-29", "2000-02-29", "2020-07-31"] {
            let floor = parse("2101-10-03");
            let mut sequential = parse(start);
            while sequential < floor {
                sequential = dates::next_occurrence(sequential, frequency).unwrap();
            }
            assert_eq!(
                dates::first_occurrence_on_or_after(parse(start), frequency, floor),
                Some(sequential),
                "{frequency:?} {start}"
            );
        }
    }
}

#[tokio::test]
async fn sqlite_is_updated_and_defensive_mode_cannot_enable_writable_schema() {
    let pool = db::open_memory_pool().await.unwrap();
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, "3.51.3");
    sqlx::query("PRAGMA writable_schema=ON").execute(&pool).await.unwrap();
    let writable: i64 = sqlx::query_scalar("PRAGMA writable_schema")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(writable, 0);
}

#[tokio::test]
async fn split_transfer_conversion_waits_for_old_partner_to_detach_in_either_order() {
    for deletion_first in [false, true] {
        let source = db::open_memory_pool().await.unwrap();
        let dest = db::open_memory_pool().await.unwrap();
        let from = account(&source, "Source").await;
        let to = account(&source, "Destination").await;
        let mut draft = ops::new_transaction_draft(&snapshot::load(&source).await.unwrap(), &[from], date());
        draft.kind = TransactionType::Transfer;
        draft.amount = 100.0;
        draft.to_account_id = Some(to);
        let ids = ops::save_transaction(&source, draft).await.unwrap();
        let first = sync::pending_changes(&source, 500).await.unwrap();
        sync::apply_remote_changes(&dest, first.iter().map(as_remote_change).collect())
            .await
            .unwrap();
        sync::acknowledge_changes(&source, &first).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let mut converted = ops::transaction_draft(&snapshot::load(&source).await.unwrap(), &ids[0]).unwrap();
        converted.kind = TransactionType::Expense;
        converted.category_id = "5".into();
        converted.to_account_id = None;
        ops::save_transaction(&source, converted).await.unwrap();
        let changes: Vec<_> = sync::pending_changes(&source, 500)
            .await
            .unwrap()
            .iter()
            .filter(|change| change.entity == "transactions")
            .map(as_remote_change)
            .collect();
        assert_eq!(changes.len(), 2);
        let early = changes
            .iter()
            .find(|change| change.deleted == deletion_first)
            .unwrap()
            .clone();
        let later = changes
            .iter()
            .find(|change| change.deleted != deletion_first)
            .unwrap()
            .clone();
        sync::apply_remote_changes(&dest, vec![early]).await.unwrap();
        let before = repo::list_transactions(&dest).await.unwrap();
        assert_eq!(before.len(), 2);
        assert!(before.iter().all(|item| item.is_transfer));
        sync::apply_remote_changes(&dest, vec![later]).await.unwrap();
        let after = repo::list_transactions(&dest).await.unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, ids[0]);
        assert!(!after[0].is_transfer);
        assert!(after[0].linked_transaction_id.is_none());
    }
}

fn as_remote_change(change: &sync::SyncChange) -> RemoteChange {
    RemoteChange {
        entity: change.entity.clone(),
        record_id: change.record_id.clone(),
        deleted: change.deleted,
        updated_at: change.updated_at.clone(),
        payload: change.payload.clone(),
    }
}

#[tokio::test]
async fn fitid_reimport_preserves_a_manual_move_and_note() {
    let pool = db::open_memory_pool().await.unwrap();
    let original = account(&pool, "Compte initial").await;
    let moved = account(&pool, "Compte corrigé").await;
    let parsed = import::parse_ofx_transactions("<OFX><STMTRS><BANKACCTFROM><ACCTID>A<ACCTTYPE>CHECKING</BANKACCTFROM><STMTTRN><DTPOSTED>20261003<TRNAMT>-20<NAME>CAFE<FITID>one</STMTTRN></STMTRS></OFX>", date()).unwrap();
    let request = || StatementImportRequest {
        transactions: parsed.clone(),
        target: ImportTarget::Existing(original.clone()),
        category_mapping: vec![],
    };
    import::import_statement(&pool, request()).await.unwrap();
    let before = snapshot::load(&pool).await.unwrap();
    let base = ops::transaction_draft(&before, &before.transactions[0].id).unwrap();
    let mut edited = base.clone();
    edited.account_id = moved.clone();
    edited.description = "Note conservée".into();
    ops::save_transaction_with_base(&pool, edited, base).await.unwrap();
    let report = import::import_statement(&pool, request()).await.unwrap();
    assert_eq!((report.imported, report.duplicates), (0, 1));
    let rows = repo::list_transactions(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].account_id, moved);
    assert_eq!(rows[0].description, "Note conservée");
}
