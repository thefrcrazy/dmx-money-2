//! Offline edits change only fields edited on the mobile, in one transaction with their receipt.
use super::*;
use dmx_core::{models::*, repo};
use serde_json::Value;
// Dynamic SQL uses only internal whitelisted identifiers; all external values are bound.

fn canonical(resource: &str, value: Value) -> Result<Value, String> {
    macro_rules! convert {
        ($ty:ty) => {
            serde_json::from_value::<$ty>(value)
                .and_then(serde_json::to_value)
                .map_err(|error| error.to_string())
        };
    }
    match resource {
        "accounts" => convert!(Account),
        "transactions" => convert!(Transaction),
        "categories" => convert!(Category),
        "budgets" => convert!(Budget),
        "scheduled" => convert!(ScheduledTransaction),
        _ => Err("Collection inconnue.".into()),
    }
}

/// New clients bind a mutation to a finite replay window. Legacy receipts keep no expiry.
pub(super) fn replay_deadline(payload: &Value) -> Result<Option<String>, String> {
    let Some(value) = payload.get("_mutationCreatedAt") else {
        return Ok(None);
    };
    let date = value
        .as_str()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .ok_or("Date de modification invalide.")?
        .with_timezone(&chrono::Utc);
    let now = chrono::Utc::now();
    if date > now + chrono::Duration::minutes(2) || date < now - chrono::Duration::days(90) {
        return Err("Cette modification est trop ancienne (plus de 90 jours) ou sa date est invalide. Vérifiez-la avant de la recréer.".into());
    }
    Ok(Some(
        (date + chrono::Duration::days(90)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    ))
}

pub(super) async fn patch(pool: &DbPool, resource: &str, body: &[u8]) -> Result<(HttpResponse, bool), String> {
    let table = match resource {
        "accounts" => "accounts",
        "transactions" => "transactions",
        "categories" => "categories",
        "budgets" => "budgets",
        "scheduled" => "scheduled_transactions",
        _ => return Ok((error_response(404, "Collection inconnue."), false)),
    };
    let payload: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return Ok((error_response(400, "Modification invalide."), false)),
    };
    let Some(mutation_id) = payload
        .get("_mutationId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= 128)
    else {
        return Ok((error_response(400, "Identifiant de modification manquant."), false));
    };
    let (desired, base) = match (
        canonical(resource, payload.clone()),
        canonical(resource, payload["_base"].clone()),
    ) {
        (Ok(desired), Ok(base)) if desired["id"] == base["id"] => (desired, base),
        _ => return Ok((error_response(400, "Version de départ invalide."), false)),
    };
    if let Err(error) = super::validation::validate(resource, body) {
        return Ok((error_response(400, &error), false));
    }
    let replay_until = match replay_deadline(&payload) {
        Ok(value) => value,
        Err(error) => return Ok((error_response(409, &error), false)),
    };
    let fingerprint = secure::hash_secret(&format!("{resource}:{}", String::from_utf8_lossy(body)));
    // Acquire the SQLite write lock before reading: desktop edits cannot intervene between merge and write.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.map_err(|e| e.to_string())?;
    let receipt: Option<String> = sqlx::query_scalar("SELECT fingerprint FROM mobile_mutation_receipts WHERE id = ?")
        .bind(mutation_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(previous) = receipt {
        return Ok(if previous == fingerprint {
            (json_response(200, json!({"ok": true})), false)
        } else {
            (error_response(409, "Identifiant de modification déjà utilisé."), false)
        });
    }
    let row = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT * FROM {table} WHERE id = ?")))
        .bind(desired["id"].as_str().unwrap_or_default())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let mut changed = false;
    if let Some(row) = row {
        let current = match resource {
            "accounts" => serde_json::to_value(repo::account_from_row(&row)),
            "transactions" => serde_json::to_value(repo::transaction_from_row(&row)),
            "categories" => serde_json::to_value(repo::category_from_row(&row)),
            "budgets" => serde_json::to_value(repo::budget_from_row(&row)),
            "scheduled" => serde_json::to_value(repo::scheduled_from_row(&row)),
            _ => unreachable!(),
        }
        .map_err(|e| e.to_string())?;
        let mut merged = current.clone();
        for (key, value) in desired.as_object().ok_or("Modification invalide")? {
            if key != "id" && base.get(key) != Some(value) {
                merged[key] = value.clone();
            }
        }
        let bytes = serde_json::to_vec(&merged).map_err(|e| e.to_string())?;
        if let Err(error) = super::validation::validate(resource, &bytes) {
            return Ok((error_response(400, &error), false));
        }
        macro_rules! update {
            ($ty:ty, $func:path) => {{
                let item: $ty = serde_json::from_value(merged).map_err(|e| e.to_string())?;
                $func(&mut tx, &item).await.map_err(|e| e.to_string())?;
            }};
        }
        match resource {
            "accounts" => update!(Account, repo::update_account),
            "categories" => update!(Category, repo::update_category),
            "budgets" => update!(Budget, repo::update_budget),
            "scheduled" => update!(ScheduledTransaction, repo::update_scheduled),
            "transactions" => {
                let item: Transaction = serde_json::from_value(merged).map_err(|e| e.to_string())?;
                let previous: Transaction = serde_json::from_value(current).map_err(|e| e.to_string())?;
                if item.is_transfer != previous.is_transfer
                    || item.linked_transaction_id != previous.linked_transaction_id
                {
                    return Ok((
                        error_response(400, "La liaison du virement ne peut pas changer."),
                        false,
                    ));
                }
                if let Some(linked) = &item.linked_transaction_id {
                    let row = sqlx::query("SELECT * FROM transactions WHERE id = ?")
                        .bind(linked)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                    let Some(row) = row else {
                        return Ok((error_response(409, "Contrepartie introuvable."), false));
                    };
                    let mut counterpart = repo::transaction_from_row(&row);
                    if !counterpart.is_transfer
                        || counterpart.linked_transaction_id.as_deref() != Some(item.id.as_str())
                        || counterpart.account_id == item.account_id
                        || counterpart.transaction_type == item.transaction_type
                    {
                        return Ok((error_response(400, "Contrepartie invalide."), false));
                    }
                    counterpart.amount = item.amount;
                    counterpart.date = item.date.clone();
                    counterpart.description = item.description.clone();
                    counterpart.checked = item.checked;
                    repo::update_transaction(&mut tx, &counterpart)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                repo::update_transaction(&mut tx, &item)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            _ => unreachable!(),
        }
        changed = true;
    }
    // Deletion wins over a stale edit. Never recreate a record deleted on the desktop.
    sqlx::query("INSERT INTO mobile_mutation_receipts (id, fingerprint, replay_until) VALUES (?, ?, ?)")
        .bind(mutation_id)
        .bind(fingerprint)
        .bind(replay_until)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok((json_response(200, json!({"ok": true})), changed))
}

/// Update both accounts of a transfer under one SQLite lock and one replay receipt.
pub(super) async fn patch_transfer(pool: &DbPool, body: &[u8]) -> Result<(HttpResponse, bool), String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Pair {
        from_transaction: Transaction,
        to_transaction: Transaction,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Patch {
        from_transaction: Transaction,
        to_transaction: Transaction,
        #[serde(rename = "_base")]
        base: Pair,
        #[serde(rename = "_mutationId")]
        mutation_id: String,
    }
    let payload: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return Ok((error_response(400, "Modification invalide."), false)),
    };
    let patch: Patch = match serde_json::from_value(payload.clone()) {
        Ok(value) => value,
        Err(_) => return Ok((error_response(400, "Version de départ invalide."), false)),
    };
    if patch.mutation_id.is_empty() || patch.mutation_id.len() > 128 {
        return Ok((error_response(400, "Identifiant de modification manquant."), false));
    }
    if let Err(error) = super::validation::validate("transfers", body) {
        return Ok((error_response(400, &error), false));
    }
    let replay_until = match replay_deadline(&payload) {
        Ok(value) => value,
        Err(error) => return Ok((error_response(409, &error), false)),
    };
    let fingerprint = secure::hash_secret(&format!("transfers:{}", String::from_utf8_lossy(body)));
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.map_err(|e| e.to_string())?;
    let receipt: Option<String> = sqlx::query_scalar("SELECT fingerprint FROM mobile_mutation_receipts WHERE id=?")
        .bind(&patch.mutation_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(previous) = receipt {
        return Ok(if previous == fingerprint {
            (json_response(200, json!({"ok":true})), false)
        } else {
            (error_response(409, "Identifiant de modification déjà utilisé."), false)
        });
    }
    let from = repo::get_transaction(&mut tx, &patch.from_transaction.id)
        .await
        .map_err(|e| e.to_string())?;
    let to = repo::get_transaction(&mut tx, &patch.to_transaction.id)
        .await
        .map_err(|e| e.to_string())?;
    let changed = match (from, to) {
        (Some(from), Some(to)) => {
            let (merged_from, merged_to) = match dmx_core::ops::merge_transfer_transactions(
                &from,
                &to,
                &patch.base.from_transaction,
                &patch.base.to_transaction,
                &patch.from_transaction,
                &patch.to_transaction,
            ) {
                Ok(pair) => pair,
                Err(error) => return Ok((error_response(409, &error.to_string()), false)),
            };
            let changed = merged_from != from || merged_to != to;
            if changed {
                repo::update_transaction(&mut tx, &merged_from)
                    .await
                    .map_err(|e| e.to_string())?;
                repo::update_transaction(&mut tx, &merged_to)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            changed
        }
        // A stale mobile edit never recreates a transfer deleted on the desktop.
        (None, None) => false,
        _ => {
            return Ok((
                error_response(409, "Une contrepartie du virement est introuvable."),
                false,
            ))
        }
    };
    sqlx::query("INSERT INTO mobile_mutation_receipts (id,fingerprint,replay_until) VALUES (?,?,?)")
        .bind(patch.mutation_id)
        .bind(fingerprint)
        .bind(replay_until)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok((json_response(200, json!({"ok":true})), changed))
}
