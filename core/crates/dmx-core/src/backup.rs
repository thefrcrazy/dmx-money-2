//! Sauvegardes `.dmx` compatibles 1.x : JSON `{version, timestamp, data}` encodé en base64
//! (ce n'est pas un chiffrement). Un JSON brut est aussi accepté à la restauration.

use crate::db::DbPool;
use crate::error::{CoreError, CoreResult, DbContext};
use crate::models::{string_enum, Account, AppData, Budget, Category, ScheduledTransaction, Transaction};
use crate::repo;
use crate::settings::{self, SettingsPatch, SettingsValuesPatch};
use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD};
use base64::engine::DecodePaddingMode;
use base64::Engine as _;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub const BACKUP_VERSION: u32 = 1;
pub const BACKUP_EXTENSION: &str = "dmx";

string_enum!(RestoreMode, default = Replace, {
    Replace => "replace",
    Merge => "merge",
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub timestamp: String,
    pub data: BackupData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupData {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub scheduled: Vec<ScheduledTransaction>,
    #[serde(default)]
    pub budgets: Vec<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BackupSummary {
    pub version: u32,
    pub timestamp: String,
    pub accounts: u32,
    pub transactions: u32,
    pub categories: u32,
    pub scheduled: u32,
    pub budgets: u32,
}

pub fn default_backup_file_name(today: NaiveDate) -> String {
    format!("dmxmoney_backup_{}.{BACKUP_EXTENSION}", today.format("%Y-%m-%d"))
}

pub fn summarize(file: &BackupFile) -> BackupSummary {
    BackupSummary {
        version: file.version,
        timestamp: file.timestamp.clone(),
        accounts: file.data.accounts.len() as u32,
        transactions: file.data.transactions.len() as u32,
        categories: file.data.categories.len() as u32,
        scheduled: file.data.scheduled.len() as u32,
        budgets: file.data.budgets.len() as u32,
    }
}

pub async fn build_backup(pool: &DbPool) -> CoreResult<BackupFile> {
    let settings = settings::load_app_settings(pool).await?;
    Ok(BackupFile {
        version: BACKUP_VERSION,
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        data: BackupData {
            accounts: repo::list_accounts(pool).await?,
            transactions: repo::list_transactions(pool).await?,
            categories: repo::list_categories(pool).await?,
            scheduled: repo::list_scheduled(pool).await?,
            budgets: repo::list_budgets(pool).await?,
            settings: Some(serde_json::to_value(settings).map_err(|error| CoreError::Io(error.to_string()))?),
        },
    })
}

pub fn encode_backup(file: &BackupFile) -> CoreResult<String> {
    let json = serde_json::to_string_pretty(file).map_err(|error| CoreError::Io(error.to_string()))?;
    Ok(STANDARD.encode(json.as_bytes()))
}

/// Contenu d'un fichier `.dmx` prêt à écrire.
pub async fn export_backup(pool: &DbPool) -> CoreResult<String> {
    encode_backup(&build_backup(pool).await?)
}

pub fn decode_backup(content: &str) -> CoreResult<BackupFile> {
    let invalid_format = || CoreError::import("Format de sauvegarde invalide.");
    let corrupted = || CoreError::import("Le fichier de sauvegarde est corrompu.");

    let value = match serde_json::from_str::<Value>(content) {
        Ok(value) => value,
        Err(_) => {
            let compact: String = content
                .chars()
                .filter(|character| !character.is_ascii_whitespace())
                .collect();
            let engine = GeneralPurpose::new(
                &alphabet::STANDARD,
                GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
            );
            let bytes = engine.decode(compact.as_bytes()).map_err(|_| corrupted())?;
            let json = String::from_utf8(bytes).map_err(|_| corrupted())?;
            serde_json::from_str::<Value>(&json).map_err(|_| corrupted())?
        }
    };

    if !value.get("data").is_some_and(Value::is_object) {
        return Err(invalid_format());
    }
    serde_json::from_value(value)
        .map_err(|error| CoreError::import(format!("Le fichier de sauvegarde est corrompu. ({error})")))
}

fn merge_by_id<T: Clone>(current: Vec<T>, incoming: &[T], id: impl Fn(&T) -> &str) -> Vec<T> {
    let mut merged = current;
    let mut positions: HashMap<String, usize> = merged
        .iter()
        .enumerate()
        .map(|(index, item)| (id(item).to_string(), index))
        .collect();
    for item in incoming {
        match positions.get(id(item)) {
            Some(index) => merged[*index] = item.clone(),
            None => {
                positions.insert(id(item).to_string(), merged.len());
                merged.push(item.clone());
            }
        }
    }
    merged
}

fn settings_json_field(settings: &Value, key: &str) -> Option<String> {
    match settings.get(key)? {
        Value::Null => None,
        Value::String(value) if value.is_empty() => None,
        Value::String(value) => Some(value.clone()),
        other => serde_json::to_string(other).ok(),
    }
}

/// Restaure une sauvegarde : remplacement complet ou fusion par identifiant.
pub async fn restore_backup(pool: &DbPool, content: &str, mode: RestoreMode) -> CoreResult<BackupSummary> {
    let file = decode_backup(content)?;
    let summary = summarize(&file);

    let data = match mode {
        RestoreMode::Replace => AppData {
            accounts: file.data.accounts.clone(),
            transactions: file.data.transactions.clone(),
            categories: file.data.categories.clone(),
            scheduled: file.data.scheduled.clone(),
            budgets: file.data.budgets.clone(),
        },
        RestoreMode::Merge => AppData {
            accounts: merge_by_id(repo::list_accounts(pool).await?, &file.data.accounts, |item| &item.id),
            transactions: merge_by_id(repo::list_transactions(pool).await?, &file.data.transactions, |item| {
                &item.id
            }),
            categories: merge_by_id(repo::list_categories(pool).await?, &file.data.categories, |item| {
                &item.id
            }),
            scheduled: merge_by_id(repo::list_scheduled(pool).await?, &file.data.scheduled, |item| &item.id),
            budgets: merge_by_id(repo::list_budgets(pool).await?, &file.data.budgets, |item| &item.id),
        },
    };

    let mut tx = pool.begin().await.ctx("restauration de la sauvegarde")?;
    repo::replace_all_data(&mut tx, &data).await?;
    tx.commit().await.ctx("restauration de la sauvegarde")?;

    if let (RestoreMode::Replace, Some(settings_value)) = (mode, file.data.settings.as_ref()) {
        let values = SettingsValuesPatch {
            account_groups: settings_json_field(settings_value, "accountGroups"),
            custom_groups: settings_json_field(settings_value, "customGroups"),
            custom_groups_order: settings_json_field(settings_value, "customGroupsOrder"),
            accounts_order: settings_json_field(settings_value, "accountsOrder"),
            ..SettingsValuesPatch::default()
        };
        if values.account_groups.is_some()
            || values.custom_groups.is_some()
            || values.custom_groups_order.is_some()
            || values.accounts_order.is_some()
        {
            settings::apply_settings_patch(
                pool,
                SettingsPatch {
                    schema_version: 2,
                    base_revision: i64::MAX,
                    values,
                    ..SettingsPatch::default()
                },
            )
            .await?;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_pool;
    use crate::models::TransactionType;
    use crate::ops::{self, AccountDraft, TransactionDraft};
    use crate::seed::ensure_initial_data;
    use crate::settings::SettingsChange;

    async fn populated_pool() -> DbPool {
        let pool = open_memory_pool().await.unwrap();
        ensure_initial_data(&pool).await.unwrap();
        let account = ops::save_account(
            &pool,
            AccountDraft {
                name: "Courant".into(),
                ..ops::new_account_draft()
            },
        )
        .await
        .unwrap();
        ops::save_transaction(
            &pool,
            TransactionDraft {
                id: None,
                kind: TransactionType::Expense,
                date: "2026-09-10".into(),
                amount: 12.5,
                description: "Café ☕".into(),
                category_id: "6".into(),
                account_id: account.clone(),
                to_account_id: None,
            },
        )
        .await
        .unwrap();
        settings::apply_change(&pool, SettingsChange::AddCustomGroup("Perso".into()))
            .await
            .unwrap();
        settings::apply_change(
            &pool,
            SettingsChange::AccountGroup {
                account_id: account,
                group: Some("Perso".into()),
            },
        )
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn export_is_base64_json_readable_by_v1() {
        let pool = populated_pool().await;
        let encoded = export_backup(&pool).await.unwrap();
        let json = String::from_utf8(STANDARD.decode(&encoded).unwrap()).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["version"], 1);
        assert_eq!(value["data"]["transactions"][0]["description"], "Café ☕");
        assert_eq!(
            value["data"]["transactions"][0]["accountId"],
            value["data"]["accounts"][0]["id"]
        );
        assert_eq!(value["data"]["settings"]["customGroups"][0], "Perso");
    }

    #[tokio::test]
    async fn replace_restores_data_and_account_groups() {
        let source = populated_pool().await;
        let encoded = export_backup(&source).await.unwrap();

        let target = open_memory_pool().await.unwrap();
        let summary = restore_backup(&target, &encoded, RestoreMode::Replace).await.unwrap();
        assert_eq!((summary.accounts, summary.transactions), (1, 1));

        let snapshot = crate::snapshot::load(&target).await.unwrap();
        assert_eq!(snapshot.transactions.len(), 1);
        assert_eq!(snapshot.settings.custom_groups, vec!["Perso"]);
        assert_eq!(snapshot.settings.account_groups.len(), 1);
    }

    #[tokio::test]
    async fn merge_keeps_local_records_and_accepts_raw_json() {
        let source = populated_pool().await;
        let raw = serde_json::to_string(&build_backup(&source).await.unwrap()).unwrap();

        let target = populated_pool().await;
        restore_backup(&target, &raw, RestoreMode::Merge).await.unwrap();
        let snapshot = crate::snapshot::load(&target).await.unwrap();
        assert_eq!(snapshot.accounts.len(), 2);
        assert_eq!(snapshot.transactions.len(), 2);
        assert_eq!(snapshot.categories.len(), crate::seed::DEFAULT_CATEGORIES.len());
    }

    #[test]
    fn rejects_invalid_content() {
        assert_eq!(
            decode_backup("{\"foo\":1}").unwrap_err().to_string(),
            "Format de sauvegarde invalide."
        );
        assert_eq!(
            decode_backup("%%%").unwrap_err().to_string(),
            "Le fichier de sauvegarde est corrompu."
        );
    }
}
