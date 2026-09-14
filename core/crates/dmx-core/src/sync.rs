//! Journal de synchronisation entre appareils (iCloud).
//!
//! Chaque écriture locale alimente `sync_meta` et `sync_outbox` par déclencheurs. L'hôte envoie
//! les changements en attente, les acquitte, puis applique les changements distants : le plus
//! récent `updated_at` l'emporte par enregistrement. Pendant l'application, les déclencheurs
//! sont suspendus (`sync_control.applying`) pour ne pas renvoyer ce qui vient d'être reçu.

use crate::db::{DbPool, SYNCED_SETTINGS_COLUMNS, SYNC_ENTITY_TABLES};
use crate::error::{CoreError, CoreResult, DbContext};
use crate::models::{Account, Budget, Category, ScheduledTransaction, Transaction};
use crate::repo;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Row, SqliteConnection};

pub const SETTINGS_ENTITY: &str = "settings";
pub const SETTINGS_RECORD_ID: &str = "1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncChange {
    pub seq: i64,
    pub entity: String,
    pub record_id: String,
    pub deleted: bool,
    pub updated_at: String,
    /// Enregistrement sérialisé en JSON (absent pour une suppression).
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteChange {
    pub entity: String,
    pub record_id: String,
    pub deleted: bool,
    pub updated_at: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ApplyReport {
    pub applied: u32,
    pub skipped: u32,
}

fn table_for(entity: &str) -> Option<&'static str> {
    SYNC_ENTITY_TABLES
        .iter()
        .find(|(name, _)| *name == entity)
        .map(|(_, table)| *table)
}

fn entity_rank(entity: &str) -> usize {
    SYNC_ENTITY_TABLES
        .iter()
        .position(|(name, _)| *name == entity)
        .unwrap_or(SYNC_ENTITY_TABLES.len())
}

fn to_json<T: Serialize>(value: &T) -> CoreResult<String> {
    serde_json::to_string(value).map_err(|error| CoreError::Database(error.to_string()))
}

async fn record_payload(
    connection: &mut SqliteConnection,
    entity: &str,
    record_id: &str,
) -> CoreResult<Option<String>> {
    if entity == SETTINGS_ENTITY {
        let Some(row) = sqlx::query("SELECT * FROM settings WHERE id = 1")
            .fetch_optional(&mut *connection)
            .await
            .ctx("lecture des paramètres à synchroniser")?
        else {
            return Ok(None);
        };
        let mut object = Map::new();
        for column in SYNCED_SETTINGS_COLUMNS {
            let value = if let Ok(Some(text)) = row.try_get::<Option<String>, _>(column) {
                Value::String(text)
            } else if let Ok(Some(number)) = row.try_get::<Option<i64>, _>(column) {
                Value::from(number)
            } else if let Ok(Some(number)) = row.try_get::<Option<f64>, _>(column) {
                serde_json::Number::from_f64(number)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            object.insert(column.to_string(), value);
        }
        return Ok(Some(to_json(&object)?));
    }

    let Some(table) = table_for(entity) else {
        return Ok(None);
    };
    let Some(row) = sqlx::query(&format!("SELECT * FROM {table} WHERE id = $1"))
        .bind(record_id)
        .fetch_optional(&mut *connection)
        .await
        .ctx("lecture de l'enregistrement à synchroniser")?
    else {
        return Ok(None);
    };

    let payload = match entity {
        "accounts" => to_json(&repo::account_from_row(&row))?,
        "categories" => to_json(&repo::category_from_row(&row))?,
        "budgets" => to_json(&repo::budget_from_row(&row))?,
        "scheduled" => to_json(&repo::scheduled_from_row(&row))?,
        _ => to_json(&repo::transaction_from_row(&row))?,
    };
    Ok(Some(payload))
}

/// Changements locaux à envoyer, le plus ancien d'abord, un seul par enregistrement.
pub async fn pending_changes(pool: &DbPool, limit: u32) -> CoreResult<Vec<SyncChange>> {
    let mut connection = pool.acquire().await.ctx("lecture des changements")?;
    let rows = sqlx::query(
        "SELECT o.entity AS entity, o.record_id AS record_id, MAX(o.seq) AS seq,
                m.updated_at AS updated_at, m.deleted AS deleted
         FROM sync_outbox o
         LEFT JOIN sync_meta m ON m.entity = o.entity AND m.record_id = o.record_id
         GROUP BY o.entity, o.record_id
         ORDER BY seq ASC
         LIMIT $1",
    )
    .bind(i64::from(limit))
    .fetch_all(&mut *connection)
    .await
    .ctx("lecture des changements")?;

    let mut changes = Vec::with_capacity(rows.len());
    for row in rows {
        let entity: String = row.try_get("entity").unwrap_or_default();
        let record_id: String = row.try_get("record_id").unwrap_or_default();
        let mut deleted = repo::row_bool(&row, "deleted");
        let payload = if deleted {
            None
        } else {
            let payload = record_payload(&mut connection, &entity, &record_id).await?;
            deleted = payload.is_none();
            payload
        };
        changes.push(SyncChange {
            seq: repo::row_i64(&row, "seq").unwrap_or_default(),
            updated_at: repo::row_string(&row, "updated_at"),
            entity,
            record_id,
            deleted,
            payload,
        });
    }
    Ok(changes)
}

pub async fn pending_count(pool: &DbPool) -> CoreResult<i64> {
    sqlx::query_scalar("SELECT count(DISTINCT entity || ':' || record_id) FROM sync_outbox")
        .fetch_one(pool)
        .await
        .ctx("lecture des changements")
}

/// Retire de la file les changements envoyés (sans effacer une modification plus récente).
pub async fn acknowledge_changes(pool: &DbPool, changes: &[SyncChange]) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("acquittement des changements")?;
    for change in changes {
        sqlx::query("DELETE FROM sync_outbox WHERE entity = $1 AND record_id = $2 AND seq <= $3")
            .bind(&change.entity)
            .bind(&change.record_id)
            .bind(change.seq)
            .execute(&mut *tx)
            .await
            .ctx("acquittement des changements")?;
    }
    tx.commit().await.ctx("acquittement des changements")
}

/// Met en file toutes les données existantes (première activation de la synchronisation).
pub async fn enqueue_all(pool: &DbPool) -> CoreResult<i64> {
    let mut tx = pool.begin().await.ctx("préparation de la synchronisation")?;
    let now = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
    for (entity, table) in SYNC_ENTITY_TABLES
        .iter()
        .map(|(entity, table)| (*entity, *table))
        .chain(std::iter::once((SETTINGS_ENTITY, "settings")))
    {
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO sync_meta (entity, record_id, updated_at, deleted)
             SELECT '{entity}', CAST(id AS TEXT), {now}, 0 FROM {table}"
        ))
        .execute(&mut *tx)
        .await
        .ctx("préparation de la synchronisation")?;
        sqlx::query(&format!(
            "INSERT INTO sync_outbox (entity, record_id, deleted, created_at)
             SELECT '{entity}', CAST(id AS TEXT), 0, {now} FROM {table}"
        ))
        .execute(&mut *tx)
        .await
        .ctx("préparation de la synchronisation")?;
    }
    tx.commit().await.ctx("préparation de la synchronisation")?;
    pending_count(pool).await
}

async fn record_exists(connection: &mut SqliteConnection, table: &str, id: &str) -> CoreResult<bool> {
    let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE id = $1"))
        .bind(id)
        .fetch_one(&mut *connection)
        .await
        .ctx("application des changements")?;
    Ok(count > 0)
}

fn parse_payload<T: for<'de> Deserialize<'de>>(change: &RemoteChange) -> CoreResult<T> {
    let payload = change
        .payload
        .as_deref()
        .ok_or_else(|| CoreError::validation("Changement distant sans contenu."))?;
    serde_json::from_str(payload)
        .map_err(|error| CoreError::validation(format!("Changement distant illisible : {error}")))
}

async fn apply_upsert(connection: &mut SqliteConnection, change: &RemoteChange) -> CoreResult<()> {
    match change.entity.as_str() {
        SETTINGS_ENTITY => {
            let object: Map<String, Value> = parse_payload(change)?;
            sqlx::query("INSERT OR IGNORE INTO settings (id) VALUES (1)")
                .execute(&mut *connection)
                .await
                .ctx("application des paramètres")?;
            for (column, value) in object {
                if !SYNCED_SETTINGS_COLUMNS.contains(&column.as_str()) {
                    continue;
                }
                let statement = format!("UPDATE settings SET \"{column}\" = $1 WHERE id = 1");
                let query = sqlx::query(&statement);
                let query = match value {
                    Value::String(text) => query.bind(text),
                    Value::Bool(flag) => query.bind(flag),
                    Value::Number(number) => match number.as_i64() {
                        Some(integer) => query.bind(integer),
                        None => query.bind(number.as_f64().unwrap_or_default()),
                    },
                    Value::Null => query.bind(Option::<String>::None),
                    other => query.bind(other.to_string()),
                };
                if let Err(error) = query.execute(&mut *connection).await {
                    log::warn!("Paramètre distant {column} ignoré : {error}");
                }
            }
        }
        "accounts" => {
            let account: Account = parse_payload(change)?;
            if record_exists(connection, "accounts", &account.id).await? {
                repo::update_account(connection, &account).await?;
            } else {
                repo::insert_account(connection, &account).await?;
            }
        }
        "categories" => {
            let category: Category = parse_payload(change)?;
            if record_exists(connection, "categories", &category.id).await? {
                repo::update_category(connection, &category).await?;
            } else {
                repo::insert_category(connection, &category).await?;
            }
        }
        "budgets" => {
            let budget: Budget = parse_payload(change)?;
            if record_exists(connection, "budgets", &budget.id).await? {
                repo::update_budget(connection, &budget).await?;
            } else {
                repo::insert_budget(connection, &budget).await?;
            }
        }
        "scheduled" => {
            let scheduled: ScheduledTransaction = parse_payload(change)?;
            if record_exists(connection, "scheduled_transactions", &scheduled.id).await? {
                repo::update_scheduled(connection, &scheduled).await?;
            } else {
                repo::insert_scheduled(connection, &scheduled).await?;
            }
        }
        "transactions" => {
            let transaction: Transaction = parse_payload(change)?;
            if record_exists(connection, "transactions", &transaction.id).await? {
                repo::update_transaction(connection, &transaction).await?;
            } else {
                repo::insert_transaction(connection, &transaction).await?;
            }
        }
        other => return Err(CoreError::validation(format!("Entité inconnue : {other}"))),
    }
    Ok(())
}

async fn apply_locked(connection: &mut SqliteConnection, mut changes: Vec<RemoteChange>) -> CoreResult<ApplyReport> {
    changes.sort_by_key(|change| {
        let rank = entity_rank(&change.entity);
        if change.deleted {
            (1, usize::MAX - rank)
        } else {
            (0, rank)
        }
    });

    sqlx::query("UPDATE sync_control SET applying = 1 WHERE id = 1")
        .execute(&mut *connection)
        .await
        .ctx("application des changements")?;

    let mut report = ApplyReport::default();
    for change in &changes {
        let local: Option<String> =
            sqlx::query_scalar("SELECT updated_at FROM sync_meta WHERE entity = $1 AND record_id = $2")
                .bind(&change.entity)
                .bind(&change.record_id)
                .fetch_optional(&mut *connection)
                .await
                .ctx("application des changements")?;
        if local.as_deref().is_some_and(|local| local > change.updated_at.as_str()) {
            report.skipped += 1;
            continue;
        }

        if change.deleted {
            if let Some(table) = table_for(&change.entity) {
                sqlx::query(&format!("DELETE FROM {table} WHERE id = $1"))
                    .bind(&change.record_id)
                    .execute(&mut *connection)
                    .await
                    .ctx("application des suppressions")?;
            }
        } else if let Err(error) = apply_upsert(connection, change).await {
            log::warn!(
                "Changement distant {}:{} ignoré : {error}",
                change.entity,
                change.record_id
            );
            report.skipped += 1;
            continue;
        }

        sqlx::query(
            "INSERT INTO sync_meta (entity, record_id, updated_at, deleted) VALUES ($1, $2, $3, $4)
             ON CONFLICT(entity, record_id) DO UPDATE SET updated_at = excluded.updated_at, deleted = excluded.deleted",
        )
        .bind(&change.entity)
        .bind(&change.record_id)
        .bind(&change.updated_at)
        .bind(change.deleted)
        .execute(&mut *connection)
        .await
        .ctx("application des changements")?;
        report.applied += 1;
    }

    sqlx::query("UPDATE sync_control SET applying = 0 WHERE id = 1")
        .execute(&mut *connection)
        .await
        .ctx("application des changements")?;
    Ok(report)
}

/// Applique les changements reçus d'un autre appareil.
pub async fn apply_remote_changes(pool: &DbPool, changes: Vec<RemoteChange>) -> CoreResult<ApplyReport> {
    if changes.is_empty() {
        return Ok(ApplyReport::default());
    }

    let mut connection = pool.acquire().await.ctx("application des changements")?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .ctx("application des changements")?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await
        .ctx("application des changements")?;

    let result = apply_locked(&mut connection, changes).await;
    let finish = match &result {
        Ok(_) => sqlx::query("COMMIT").execute(&mut *connection).await,
        Err(_) => sqlx::query("ROLLBACK").execute(&mut *connection).await,
    };
    let _ = sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *connection).await;
    finish.ctx("application des changements")?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_pool;
    use crate::models::TransactionType;
    use crate::ops::{self, AccountDraft, TransactionDraft};
    use crate::settings::{self, SettingsChange};
    use std::time::Duration;

    fn as_remote(changes: &[SyncChange]) -> Vec<RemoteChange> {
        changes
            .iter()
            .map(|change| RemoteChange {
                entity: change.entity.clone(),
                record_id: change.record_id.clone(),
                deleted: change.deleted,
                updated_at: change.updated_at.clone(),
                payload: change.payload.clone(),
            })
            .collect()
    }

    async fn push(from: &DbPool, to: &DbPool) -> ApplyReport {
        let changes = pending_changes(from, 500).await.unwrap();
        let report = apply_remote_changes(to, as_remote(&changes)).await.unwrap();
        acknowledge_changes(from, &changes).await.unwrap();
        report
    }

    #[tokio::test]
    async fn replicates_creations_updates_and_deletions_without_echo() {
        let device_a = open_memory_pool().await.unwrap();
        let device_b = open_memory_pool().await.unwrap();

        let account = ops::save_account(
            &device_a,
            AccountDraft {
                name: "Courant".into(),
                ..ops::new_account_draft()
            },
        )
        .await
        .unwrap();
        ops::save_transaction(
            &device_a,
            TransactionDraft {
                id: None,
                kind: TransactionType::Expense,
                date: "2026-09-10".into(),
                amount: 20.0,
                description: "Test".into(),
                category_id: "5".into(),
                account_id: account.clone(),
                to_account_id: None,
            },
        )
        .await
        .unwrap();
        settings::apply_change(&device_a, SettingsChange::PredictionAlertThreshold(150.0))
            .await
            .unwrap();

        let report = push(&device_a, &device_b).await;
        assert_eq!(report.skipped, 0);
        assert_eq!(pending_count(&device_a).await.unwrap(), 0);
        assert_eq!(
            pending_count(&device_b).await.unwrap(),
            0,
            "les changements appliqués ne sont pas renvoyés"
        );

        let snapshot = crate::snapshot::load(&device_b).await.unwrap();
        assert_eq!(snapshot.accounts.len(), 1);
        assert_eq!(snapshot.transactions.len(), 1);
        assert_eq!(snapshot.settings.prediction_alert_threshold, 150.0);

        tokio::time::sleep(Duration::from_millis(5)).await;
        ops::delete_account(&device_a, &account).await.unwrap();
        push(&device_a, &device_b).await;
        let snapshot = crate::snapshot::load(&device_b).await.unwrap();
        assert!(snapshot.accounts.is_empty());
        assert!(snapshot.transactions.is_empty());
    }

    #[tokio::test]
    async fn newer_local_change_wins_and_enqueue_all_exports_everything() {
        let device_a = open_memory_pool().await.unwrap();
        let device_b = open_memory_pool().await.unwrap();

        let account = ops::save_account(
            &device_a,
            AccountDraft {
                name: "Ancien".into(),
                ..ops::new_account_draft()
            },
        )
        .await
        .unwrap();
        let stale = pending_changes(&device_a, 10).await.unwrap();
        push(&device_a, &device_b).await;

        tokio::time::sleep(Duration::from_millis(5)).await;
        let mut draft = ops::account_draft(&crate::snapshot::load(&device_b).await.unwrap(), &account).unwrap();
        draft.name = "Récent".into();
        ops::save_account(&device_b, draft).await.unwrap();

        let report = apply_remote_changes(&device_b, as_remote(&stale)).await.unwrap();
        assert_eq!(report.skipped, stale.len() as u32);
        let snapshot = crate::snapshot::load(&device_b).await.unwrap();
        assert_eq!(snapshot.accounts[0].name, "Récent");

        acknowledge_changes(&device_b, &pending_changes(&device_b, 10).await.unwrap())
            .await
            .unwrap();
        assert!(enqueue_all(&device_b).await.unwrap() >= 1);
    }
}
