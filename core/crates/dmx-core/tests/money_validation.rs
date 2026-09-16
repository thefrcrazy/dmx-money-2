use dmx_core::{backup, ops, Engine};

#[test]
fn invalid_backup_amount_does_not_replace_existing_data() {
    let engine = Engine::open_in_memory().unwrap();
    let id = engine
        .save_account(ops::AccountDraft {
            name: "À conserver".into(),
            initial_balance: 120.0,
            ..ops::new_account_draft()
        })
        .unwrap();
    let mut file = backup::decode_backup(&engine.export_backup().unwrap()).unwrap();
    file.data.accounts[0].initial_balance = 1e20;
    let content = backup::encode_backup(&file).unwrap();
    assert!(engine.restore_backup(&content, backup::RestoreMode::Replace).is_err());
    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.accounts.len(), 1);
    assert_eq!(snapshot.accounts[0].id, id);
    assert_eq!(snapshot.accounts[0].initial_balance, 120.0);
}
