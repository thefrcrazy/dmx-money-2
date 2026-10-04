//! Journal de synchronisation entre appareils (iCloud).
//!
//! Chaque écriture locale alimente `sync_meta` et `sync_outbox` par déclencheurs. L'hôte envoie
//! les changements en attente, les acquitte, puis applique les changements distants : le plus
//! récent `updated_at` l'emporte par enregistrement. Pendant l'application, les déclencheurs
//! sont suspendus (`sync_control.applying`) pour ne pas renvoyer ce qui vient d'être reçu.
//! Dynamic SQL uses only internal whitelisted identifiers; all external values are bound.

use crate::db::{DbPool, SYNCED_SETTINGS_COLUMNS, SYNC_ENTITY_TABLES};
use crate::error::{CoreError, CoreResult, DbContext};
use crate::models::{Account, Budget, Category, ScheduledTransaction, Transaction};
use crate::repo;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Row, SqliteConnection};
use std::collections::HashMap;

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
    let Some(row) = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT * FROM {table} WHERE id = $1")))
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
    let mut connection = pool.begin().await.ctx("lecture des changements")?;
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
    connection.commit().await.ctx("lecture des changements")?;
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
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT OR IGNORE INTO sync_meta (entity, record_id, updated_at, deleted)
             SELECT '{entity}', CAST(id AS TEXT), {now}, 0 FROM {table}"
        )))
        .execute(&mut *tx)
        .await
        .ctx("préparation de la synchronisation")?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO sync_outbox (entity, record_id, deleted, created_at)
             SELECT '{entity}', CAST(id AS TEXT), 0, {now} FROM {table}"
        )))
        .execute(&mut *tx)
        .await
        .ctx("préparation de la synchronisation")?;
    }
    tx.commit().await.ctx("préparation de la synchronisation")?;
    pending_count(pool).await
}

async fn record_exists(connection: &mut SqliteConnection, table: &str, id: &str) -> CoreResult<bool> {
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM {table} WHERE id = $1"
    )))
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
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| CoreError::validation(format!("Changement distant illisible : {error}")))?;
    if change.entity != SETTINGS_ENTITY && value.get("id").and_then(Value::as_str) != Some(change.record_id.as_str()) {
        return Err(CoreError::validation("Identifiant du changement distant incohérent."));
    }
    serde_json::from_value(value)
        .map_err(|error| CoreError::validation(format!("Changement distant illisible : {error}")))
}

fn valid_remote_date(value: &str) -> bool {
    value.len() == 10 && crate::dates::parse_date(value).is_some()
}

fn validate_positive_money(value: f64) -> CoreResult<()> {
    if crate::metrics::is_valid_money(value) && value >= 0.0 {
        Ok(())
    } else {
        Err(CoreError::validation("Montant distant invalide."))
    }
}

async fn apply_upsert(connection: &mut SqliteConnection, change: &RemoteChange) -> CoreResult<()> {
    match change.entity.as_str() {
        SETTINGS_ENTITY => {
            let object: Map<String, Value> = parse_payload(change)?;
            // SQLite sync snapshots encode flags as integers; range validation needs only dates.
            let date = |name: &str| -> CoreResult<Option<String>> {
                match object.get(name) {
                    None | Some(Value::Null) => Ok(None),
                    Some(Value::String(value)) => Ok(Some(value.clone())),
                    _ => Err(CoreError::validation("Date distante invalide.")),
                }
            };
            let values = crate::settings::SettingsValuesPatch {
                prediction_custom_end_date: date("predictionCustomEndDate")?,
                analytics_custom_start_date: date("analyticsCustomStartDate")?,
                analytics_custom_end_date: date("analyticsCustomEndDate")?,
                ..Default::default()
            };
            if let Some(row) = sqlx::query("SELECT * FROM settings WHERE id=1")
                .fetch_optional(&mut *connection)
                .await
                .ctx("validation des paramètres distants")?
            {
                crate::settings::validate_custom_ranges(&row, &values)?;
            } else {
                // Seed the singleton inside the caller's savepoint before validating its defaults.
                sqlx::query("INSERT OR IGNORE INTO settings (id) VALUES (1)")
                    .execute(&mut *connection)
                    .await
                    .ctx("application des paramètres")?;
                let row = sqlx::query("SELECT * FROM settings WHERE id=1")
                    .fetch_one(&mut *connection)
                    .await
                    .ctx("validation des paramètres distants")?;
                crate::settings::validate_custom_ranges(&row, &values)?;
            }
            sqlx::query("INSERT OR IGNORE INTO settings (id) VALUES (1)")
                .execute(&mut *connection)
                .await
                .ctx("application des paramètres")?;
            for (column, value) in object {
                if !SYNCED_SETTINGS_COLUMNS.contains(&column.as_str()) {
                    continue;
                }
                let statement = format!("UPDATE settings SET \"{column}\" = $1 WHERE id = 1");
                let query = sqlx::query(sqlx::AssertSqlSafe(statement));
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
                query
                    .execute(&mut *connection)
                    .await
                    .ctx("application du paramètre distant")?;
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
            validate_positive_money(budget.amount)?;
            if record_exists(connection, "budgets", &budget.id).await? {
                repo::update_budget(connection, &budget).await?;
            } else {
                repo::insert_budget(connection, &budget).await?;
            }
        }
        "scheduled" => {
            let scheduled: ScheduledTransaction = parse_payload(change)?;
            validate_positive_money(scheduled.amount)?;
            if !valid_remote_date(&scheduled.next_date)
                || scheduled
                    .end_date
                    .as_deref()
                    .is_some_and(|date| !valid_remote_date(date))
            {
                return Err(CoreError::validation("Date de l'échéance distante invalide."));
            }
            if record_exists(connection, "scheduled_transactions", &scheduled.id).await? {
                repo::update_scheduled(connection, &scheduled).await?;
            } else {
                repo::insert_scheduled(connection, &scheduled).await?;
            }
        }
        "transactions" => {
            let transaction: Transaction = parse_payload(change)?;
            validate_positive_money(transaction.amount)?;
            if !valid_remote_date(&transaction.date) {
                return Err(CoreError::validation("Date de la transaction distante invalide."));
            }
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

/// The old partner must be removed or detach in the same batch before a link can change.
async fn old_partner_detaches(
    connection: &mut SqliteConnection,
    id: &str,
    linked: &str,
    candidates: &[RemoteChange],
) -> CoreResult<bool> {
    if !record_exists(connection, "transactions", linked).await? {
        return Ok(true);
    }
    let Some(change) = candidates
        .iter()
        .find(|other| other.entity == "transactions" && other.record_id == linked)
    else {
        return Ok(false);
    };
    if change.deleted {
        return Ok(true);
    }
    let partner: Transaction = parse_payload(change)?;
    crate::limits::transaction(&partner)?;
    validate_positive_money(partner.amount)?;
    if !valid_remote_date(&partner.date) {
        return Err(CoreError::validation("Date de la contrepartie distante invalide."));
    }
    Ok(partner.linked_transaction_id.as_deref() != Some(id))
}

async fn transfer_dependency_ready(
    connection: &mut SqliteConnection,
    change: &RemoteChange,
    candidates: &[RemoteChange],
) -> CoreResult<bool> {
    if change.entity != "transactions" {
        return Ok(true);
    }
    let existing = repo::get_transaction(connection, &change.record_id).await?;
    if change.deleted {
        // Never expose one surviving half while a tombstone for its counterpart is delayed.
        if let Some(linked) = existing.and_then(|item| item.linked_transaction_id) {
            return old_partner_detaches(connection, &change.record_id, &linked, candidates).await;
        }
        return Ok(true);
    }
    let item: Transaction = parse_payload(change)?;
    if let Some(linked) = existing.and_then(|item| item.linked_transaction_id) {
        if item.linked_transaction_id.as_deref() != Some(linked.as_str())
            && !old_partner_detaches(connection, &item.id, &linked, candidates).await?
        {
            return Ok(false);
        }
    }
    if !item.is_transfer && item.category != crate::models::TRANSFER_CATEGORY_ID {
        return Ok(item.linked_transaction_id.is_none());
    }
    let Some(linked) = item.linked_transaction_id.as_deref() else {
        return Ok(false);
    };
    let counterpart = match candidates
        .iter()
        .find(|other| other.entity == "transactions" && other.record_id == linked)
    {
        Some(other) if !other.deleted => Some(parse_payload::<Transaction>(other)?),
        Some(_) => None,
        None => repo::get_transaction(connection, linked).await?,
    };
    let Some(counterpart) = counterpart else {
        return Ok(false);
    };
    crate::limits::transaction(&item)?;
    crate::limits::transaction(&counterpart)?;
    validate_positive_money(item.amount)?;
    validate_positive_money(counterpart.amount)?;
    Ok(repo::are_transfer_counterparts(&item, &counterpart)
        && crate::metrics::cents(item.amount) == crate::metrics::cents(counterpart.amount)
        && item.date == counterpart.date
        && valid_remote_date(&counterpart.date)
        && crate::metrics::is_valid_money(counterpart.amount)
        && record_exists(connection, "accounts", &item.account_id).await?
        && record_exists(connection, "accounts", &counterpart.account_id).await?)
}

async fn apply_locked(connection: &mut SqliteConnection, incoming: Vec<RemoteChange>) -> CoreResult<ApplyReport> {
    let stored: Vec<String> = sqlx::query_scalar("SELECT change_json FROM sync_pending_remote")
        .fetch_all(&mut *connection)
        .await
        .ctx("lecture des changements différés")?;
    let mut changes: Vec<RemoteChange> = stored
        .into_iter()
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|error| CoreError::Database(format!("Changement différé illisible : {error}")))
        })
        .collect::<CoreResult<_>>()?;
    changes.extend(incoming);
    let mut report = ApplyReport::default();
    let mut newest: HashMap<(String, String), RemoteChange> = HashMap::new();
    for mut change in changes {
        let valid_entity = table_for(&change.entity).is_some()
            || (change.entity == SETTINGS_ENTITY && change.record_id == SETTINGS_RECORD_ID);
        let Ok(timestamp) = DateTime::parse_from_rfc3339(&change.updated_at) else {
            report.skipped += 1;
            continue;
        };
        if !valid_entity || change.record_id.is_empty() {
            report.skipped += 1;
            continue;
        }
        change.updated_at = timestamp
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let key = (change.entity.clone(), change.record_id.clone());
        if newest.get(&key).is_none_or(|old| {
            old.updated_at < change.updated_at
                || (old.updated_at == change.updated_at && (change.deleted || !old.deleted))
        }) {
            newest.insert(key, change);
        }
    }
    let mut changes = Vec::new();
    for (_, change) in newest {
        let local = sqlx::query("SELECT updated_at, deleted FROM sync_meta WHERE entity = $1 AND record_id = $2")
            .bind(&change.entity)
            .bind(&change.record_id)
            .fetch_optional(&mut *connection)
            .await
            .ctx("application des changements")?;
        let stale = local.as_ref().is_some_and(|row| {
            let timestamp = repo::row_string(row, "updated_at");
            timestamp > change.updated_at
                || (timestamp == change.updated_at && repo::row_bool(row, "deleted") && !change.deleted)
        });
        if stale {
            sqlx::query("DELETE FROM sync_pending_remote WHERE entity = $1 AND record_id = $2")
                .bind(&change.entity)
                .bind(&change.record_id)
                .execute(&mut *connection)
                .await
                .ctx("suppression du changement différé obsolète")?;
            report.skipped += 1;
        } else {
            changes.push(change);
        }
    }
    changes.sort_by_key(|change| {
        let rank = entity_rank(&change.entity);
        if change.deleted {
            (1, usize::MAX - rank)
        } else {
            (0, rank)
        }
    });
    let mut touched = std::collections::HashSet::new();
    for change in changes.iter().filter(|change| change.entity == "transactions") {
        touched.insert(change.record_id.clone());
        if let Some(existing) = repo::get_transaction(connection, &change.record_id).await? {
            if let Some(linked) = existing.linked_transaction_id {
                touched.insert(linked);
            }
        }
        if !change.deleted {
            if let Ok(item) = parse_payload::<Transaction>(change) {
                if let Some(linked) = item.linked_transaction_id {
                    touched.insert(linked);
                }
            }
        }
    }
    sqlx::query("UPDATE sync_control SET applying = 1 WHERE id = 1")
        .execute(&mut *connection)
        .await
        .ctx("application des changements")?;
    for change in &changes {
        sqlx::query("INSERT INTO sync_pending_remote(entity, record_id, change_json) VALUES ($1,$2,$3) ON CONFLICT(entity,record_id) DO UPDATE SET change_json=excluded.change_json")
            .bind(&change.entity).bind(&change.record_id).bind(to_json(change)?)
            .execute(&mut *connection).await.ctx("conservation du changement distant")?;
        match transfer_dependency_ready(connection, change, &changes).await {
            Ok(false) => {
                report.skipped += 1;
                continue;
            }
            Err(CoreError::Validation(_)) => {
                sqlx::query("DELETE FROM sync_pending_remote WHERE entity=$1 AND record_id=$2")
                    .bind(&change.entity)
                    .bind(&change.record_id)
                    .execute(&mut *connection)
                    .await
                    .ctx("rejet du changement distant invalide")?;
                report.skipped += 1;
                continue;
            }
            Err(error) => return Err(error),
            Ok(true) => {}
        }
        sqlx::query("SAVEPOINT remote_record")
            .execute(&mut *connection)
            .await
            .ctx("application atomique du changement")?;
        let result = if change.deleted {
            if let Some(table) = table_for(&change.entity) {
                sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table} WHERE id = $1")))
                    .bind(&change.record_id)
                    .execute(&mut *connection)
                    .await
                    .map(|_| ())
                    .ctx("application des suppressions")
            } else {
                Ok(())
            }
        } else {
            apply_upsert(connection, change).await
        };
        if let Err(error) = result {
            sqlx::query("ROLLBACK TO remote_record")
                .execute(&mut *connection)
                .await
                .ctx("annulation du changement distant")?;
            sqlx::query("RELEASE remote_record")
                .execute(&mut *connection)
                .await
                .ctx("annulation du changement distant")?;
            // FK failures retain the exact change durably: a future batch may provide its parent.
            // Invalid payloads are rejected; I/O/busy errors abort the batch rather than being hidden.
            match error {
                CoreError::Database(ref message)
                    if message == "Impossible de supprimer cet élément car il est utilisé ailleurs." => {}
                CoreError::Validation(_) => {
                    sqlx::query("DELETE FROM sync_pending_remote WHERE entity=$1 AND record_id=$2")
                        .bind(&change.entity)
                        .bind(&change.record_id)
                        .execute(&mut *connection)
                        .await
                        .ctx("rejet du changement distant invalide")?;
                }
                _ => return Err(error),
            }
            report.skipped += 1;
            continue;
        }
        sqlx::query("RELEASE remote_record")
            .execute(&mut *connection)
            .await
            .ctx("application atomique du changement")?;
        sqlx::query("INSERT INTO sync_meta (entity, record_id, updated_at, deleted) VALUES ($1,$2,$3,$4) ON CONFLICT(entity,record_id) DO UPDATE SET updated_at=excluded.updated_at, deleted=excluded.deleted")
            .bind(&change.entity).bind(&change.record_id).bind(&change.updated_at).bind(change.deleted)
            .execute(&mut *connection).await.ctx("application des changements")?;
        sqlx::query("DELETE FROM sync_pending_remote WHERE entity=$1 AND record_id=$2")
            .bind(&change.entity)
            .bind(&change.record_id)
            .execute(&mut *connection)
            .await
            .ctx("acquittement du changement différé")?;
        report.applied += 1;
    }
    // A rejected record must never leave a successful planned partner with a broken link.
    // Validate only affected pairs, preserving unrelated legacy rows while refusing partial writes.
    for id in touched {
        let Some(item) = repo::get_transaction(connection, &id).await? else {
            continue;
        };
        if item.is_transfer || item.category == crate::models::TRANSFER_CATEGORY_ID {
            let partner = match item.linked_transaction_id.as_deref() {
                Some(linked) => repo::get_transaction(connection, linked).await?,
                None => None,
            };
            if partner.as_ref().is_none_or(|partner| {
                !repo::are_transfer_counterparts(&item, partner)
                    || crate::metrics::cents(item.amount) != crate::metrics::cents(partner.amount)
                    || item.date != partner.date
            }) {
                return Err(CoreError::validation(
                    "Le lot distant laisserait un virement incomplet. Aucun changement n’a été enregistré.",
                ));
            }
        } else if item.linked_transaction_id.is_some() {
            return Err(CoreError::validation(
                "Le lot distant laisserait une liaison de virement invalide. Aucun changement n’a été enregistré.",
            ));
        }
    }
    let violation = sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut *connection)
        .await
        .ctx("vérification de l’intégrité synchronisée")?;
    if violation.is_some() {
        return Err(CoreError::validation(
            "La base contient une référence à un compte absent. La synchronisation a été annulée.",
        ));
    }
    sqlx::query("UPDATE sync_control SET applying = 0 WHERE id = 1")
        .execute(&mut *connection)
        .await
        .ctx("application des changements")?;
    Ok(report)
}

/// Applies valid records atomically while retaining out-of-order dependencies for the next batch.
/// Foreign keys stay enabled throughout; a failed transaction cannot leak its control flag.
pub async fn apply_remote_changes(pool: &DbPool, changes: Vec<RemoteChange>) -> CoreResult<ApplyReport> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .ctx("application des changements")?;
    let report = apply_locked(&mut transaction, changes).await?;
    transaction.commit().await.ctx("application des changements")?;
    Ok(report)
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

    #[tokio::test]
    async fn remote_payload_cannot_write_another_record_or_freeze_its_timestamp() {
        let pool = open_memory_pool().await.unwrap();
        let id = ops::save_account(
            &pool,
            AccountDraft {
                name: "Courant".into(),
                ..ops::new_account_draft()
            },
        )
        .await
        .unwrap();
        let mut account = crate::snapshot::load(&pool).await.unwrap().accounts.remove(0);
        account.name = "Modifié".into();
        let change = RemoteChange {
            entity: "accounts".into(),
            record_id: "another-id".into(),
            deleted: false,
            updated_at: "2026-12-01T00:00:00.000Z".into(),
            payload: Some(to_json(&account).unwrap()),
        };
        let report = apply_remote_changes(&pool, vec![change.clone()]).await.unwrap();
        assert_eq!((report.applied, report.skipped), (0, 1));
        let invalid = RemoteChange {
            record_id: id,
            updated_at: "not-a-timestamp".into(),
            ..change
        };
        let report = apply_remote_changes(&pool, vec![invalid]).await.unwrap();
        assert_eq!((report.applied, report.skipped), (0, 1));
        assert_eq!(repo::list_accounts(&pool).await.unwrap()[0].name, "Courant");
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(foreign_keys, 1);
    }

    #[tokio::test]
    async fn remote_timestamps_compare_instants_across_timezones() {
        let pool = open_memory_pool().await.unwrap();
        let id = ops::save_account(
            &pool,
            AccountDraft {
                name: "Local récent".into(),
                ..ops::new_account_draft()
            },
        )
        .await
        .unwrap();
        sqlx::query(
            "UPDATE sync_meta SET updated_at = '2026-09-10T10:00:00.000Z' WHERE entity = 'accounts' AND record_id = $1",
        )
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
        let mut account = repo::list_accounts(&pool).await.unwrap().remove(0);
        account.name = "Distant ancien".into();
        let change = RemoteChange {
            entity: "accounts".into(),
            record_id: id,
            deleted: false,
            updated_at: "2026-09-10T11:00:00.000+02:00".into(),
            payload: Some(to_json(&account).unwrap()),
        };
        let report = apply_remote_changes(&pool, vec![change]).await.unwrap();
        assert_eq!((report.applied, report.skipped), (0, 1));
        assert_eq!(repo::list_accounts(&pool).await.unwrap()[0].name, "Local récent");
    }

    #[tokio::test]
    async fn malformed_remote_transactions_do_not_reach_the_database() {
        let pool = open_memory_pool().await.unwrap();
        let account = ops::save_account(
            &pool,
            AccountDraft {
                name: "Courant".into(),
                ..ops::new_account_draft()
            },
        )
        .await
        .unwrap();
        for (date, amount) in [("2026-02-31", 10.0), ("2026-09-15", -10.0)] {
            let transaction = Transaction {
                id: "remote-tx".into(),
                date: date.into(),
                account_id: account.clone(),
                transaction_type: TransactionType::Expense,
                amount,
                category: "5".into(),
                description: "Test".into(),
                checked: false,
                is_transfer: false,
                linked_transaction_id: None,
                bank_source: None,
                bank_transaction_id: None,
            };
            let change = RemoteChange {
                entity: "transactions".into(),
                record_id: transaction.id.clone(),
                deleted: false,
                updated_at: "2026-12-01T00:00:00.000Z".into(),
                payload: Some(to_json(&transaction).unwrap()),
            };
            let report = apply_remote_changes(&pool, vec![change]).await.unwrap();
            assert_eq!((report.applied, report.skipped), (0, 1));
            assert!(repo::list_transactions(&pool).await.unwrap().is_empty());
        }
    }
}
