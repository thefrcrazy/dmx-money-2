//! Routes `/api/*` consommées par la PWA. Les formes JSON et les codes de réponse sont ceux de 1.x ;
//! les écritures passent par `dmx-core` (insertions idempotentes pour rejouer la file hors ligne).

use super::*;
use dmx_core::models::{Account, Budget, Category, ScheduledTransaction, Transaction};
use dmx_core::settings::{
    apply_settings_patch, get_settings_record, legacy_mobile_settings_patch, SettingsPatch, SettingsRecord,
};
use dmx_core::{dates, ops, repo, scheduling, CoreError};
use futures_util::TryStreamExt;
use sqlx::SqliteConnection;

fn core_error(error: CoreError) -> String {
    error.to_string()
}

macro_rules! in_transaction {
    ($pool:expr, $context:literal, |$connection:ident| $body:expr) => {{
        let mut tx = $pool.begin().await.map_err(|error| map_db_error(error, $context))?;
        let $connection: &mut SqliteConnection = &mut tx;
        let result = $body.await.map_err(core_error)?;
        tx.commit().await.map_err(|error| map_db_error(error, $context))?;
        result
    }};
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransferPayload {
    from_transaction: Transaction,
    to_transaction: Transaction,
    #[serde(rename = "_mutationId", default)]
    mutation_id: Option<String>,
    #[serde(rename = "_mutationCreatedAt", default)]
    mutation_created_at: Option<String>,
}

fn ok(status: u16) -> HttpResponse {
    json_response(status, json!({ "ok": true }))
}

pub(super) async fn route_api_request(
    pool: &DbPool,
    request: HttpRequest,
    path: &str,
) -> Result<(HttpResponse, bool), String> {
    let parts = path
        .trim_start_matches('/')
        .split('/')
        .map(percent_decode)
        .collect::<Vec<_>>();

    if parts.len() < 2 || parts[0] != "api" {
        return Ok((error_response(404, "Route introuvable"), false));
    }

    let resource = parts[1].as_str();
    let id = parts.get(2).map(String::as_str);
    let method = request.method.as_str();

    if matches!(method, "POST" | "PUT" | "PATCH") && matches!(resource, "transactions" | "transfers") {
        let payload: serde_json::Value = parse_json(&request.body)?;
        if let Err(error) = bank_sync::replay_deadline(&payload) {
            return Ok((error_response(409, &error), false));
        }
    }

    if matches!(method, "POST" | "PUT") && !(resource == "scheduled" && id == Some("process-due")) {
        if let Err(error) = super::validation::validate(resource, &request.body) {
            return Ok((error_response(400, &error), false));
        }
    }

    if method == "PATCH" && resource == "transfers" {
        return super::bank_sync::patch_transfer(pool, &request.body).await;
    }

    if method == "PATCH" && resource != "settings" {
        return super::bank_sync::patch(pool, resource, &request.body).await;
    }

    match (method, resource) {
        ("GET", "status") => Ok((
            json_response(
                200,
                json!({ "ok": true, "dataVersion": get_data_version(pool).await?, "bankSyncVersion": 1 }),
            ),
            false,
        )),

        ("GET", "accounts") => Ok((
            json_response(200, repo::list_accounts(pool).await.map_err(core_error)?),
            false,
        )),
        ("POST", "accounts") => {
            let account: Account = parse_json(&request.body)?;
            in_transaction!(pool, "ajout mobile", |connection| async {
                if !is_deleted(connection, "accounts", &account.id).await? {
                    repo::insert_account(connection, &account).await?;
                }
                Ok(())
            });
            Ok((ok(201), true))
        }
        ("PUT", "accounts") => {
            let account: Account = parse_json(&request.body)?;
            in_transaction!(pool, "mise à jour du compte", |connection| repo::update_account(
                connection, &account
            ));
            Ok((ok(200), true))
        }
        ("DELETE", "accounts") => {
            let id = extract_id(id, &request.body)?;
            ops::delete_account(pool, &id).await.map_err(core_error)?;
            Ok((ok(200), true))
        }

        ("GET", "transactions") if id == Some("page") => transaction_page(pool, &request.path).await,
        ("GET", "transactions") => Ok((
            json_response(200, repo::list_transactions(pool).await.map_err(core_error)?),
            false,
        )),
        ("POST", "transactions") => {
            let transaction: Transaction = parse_json(&request.body)?;
            if transaction.is_transfer {
                return Ok((
                    error_response(
                        400,
                        "Utilisez la route de virement pour créer les deux contreparties ensemble.",
                    ),
                    false,
                ));
            }
            in_transaction!(pool, "ajout mobile", |connection| async {
                if !is_deleted(connection, "transactions", &transaction.id).await? {
                    repo::insert_transaction_if_absent(connection, &transaction).await?;
                }
                Ok(())
            });
            Ok((ok(201), true))
        }
        ("PUT", "transactions") => {
            let transaction: Transaction = parse_json(&request.body)?;
            let mut tx = pool
                .begin()
                .await
                .map_err(|error| map_db_error(error, "mise à jour de transaction"))?;
            let Some(previous) = load_transaction(&mut tx, &transaction.id).await? else {
                return Ok((error_response(404, "Transaction introuvable."), false));
            };
            if previous.is_transfer != transaction.is_transfer
                || previous.linked_transaction_id != transaction.linked_transaction_id
            {
                return Ok((
                    error_response(400, "La liaison d'un virement ne peut pas être remplacée."),
                    false,
                ));
            }
            if let Some(linked) = transaction.linked_transaction_id.as_deref() {
                let Some(mut counterpart) = load_transaction(&mut tx, linked).await? else {
                    return Ok((error_response(400, "Contrepartie introuvable."), false));
                };
                if !counterpart.is_transfer
                    || counterpart.linked_transaction_id.as_deref() != Some(transaction.id.as_str())
                    || counterpart.account_id == transaction.account_id
                    || counterpart.transaction_type == transaction.transaction_type
                {
                    return Ok((error_response(400, "Contrepartie invalide."), false));
                }
                counterpart.amount = transaction.amount;
                counterpart.date = transaction.date.clone();
                counterpart.description = transaction.description.clone();
                counterpart.checked = transaction.checked;
                repo::update_transaction(&mut tx, &counterpart)
                    .await
                    .map_err(core_error)?;
            }
            repo::update_transaction(&mut tx, &transaction)
                .await
                .map_err(core_error)?;
            tx.commit()
                .await
                .map_err(|error| map_db_error(error, "mise à jour de transaction"))?;
            Ok((ok(200), true))
        }
        ("DELETE", "transactions") => {
            let id = extract_id(id, &request.body)?;
            in_transaction!(pool, "suppression de transaction", |connection| {
                repo::delete_transaction(connection, &id)
            });
            Ok((ok(200), true))
        }
        ("POST", "transfers") => {
            let payload: TransferPayload = parse_json(&request.body)?;
            let stamp = payload
                .mutation_created_at
                .as_ref()
                .map(|value| json!({"_mutationCreatedAt": value}))
                .unwrap_or_else(|| json!({}));
            let replay_until = match bank_sync::replay_deadline(&stamp) {
                Ok(value) => value,
                Err(error) => return Ok((error_response(409, &error), false)),
            };
            let mut tx = pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(|error| map_db_error(error, "ajout de virement"))?;
            let fingerprint = secure::hash_secret(&format!("transfers:{}", String::from_utf8_lossy(&request.body)));
            if let Some(id) = &payload.mutation_id {
                if id.is_empty() || id.len() > 128 {
                    return Ok((error_response(400, "Identifiant de modification invalide."), false));
                }
                let receipt: Option<String> =
                    sqlx::query_scalar("SELECT fingerprint FROM mobile_mutation_receipts WHERE id = ?")
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                if let Some(previous) = receipt {
                    return Ok(if previous == fingerprint {
                        (ok(200), false)
                    } else {
                        (error_response(409, "Identifiant de modification déjà utilisé."), false)
                    });
                }
            }
            if is_deleted(&mut tx, "transactions", &payload.from_transaction.id)
                .await
                .map_err(core_error)?
                || is_deleted(&mut tx, "transactions", &payload.to_transaction.id)
                    .await
                    .map_err(core_error)?
            {
                return Ok((ok(200), false));
            }
            let from = load_transaction(&mut tx, &payload.from_transaction.id).await?;
            let to = load_transaction(&mut tx, &payload.to_transaction.id).await?;
            if from.is_some() || to.is_some() {
                if from.as_ref() == Some(&payload.from_transaction) && to.as_ref() == Some(&payload.to_transaction) {
                    return Ok((ok(200), false));
                }
                return Ok((error_response(409, "Identifiants de virement déjà utilisés."), false));
            }
            repo::insert_transaction_if_absent(&mut tx, &payload.from_transaction)
                .await
                .map_err(core_error)?;
            repo::insert_transaction_if_absent(&mut tx, &payload.to_transaction)
                .await
                .map_err(core_error)?;
            if let Some(id) = &payload.mutation_id {
                sqlx::query("INSERT INTO mobile_mutation_receipts (id, fingerprint, replay_until) VALUES (?, ?, ?)")
                    .bind(id)
                    .bind(fingerprint)
                    .bind(replay_until)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            tx.commit()
                .await
                .map_err(|error| map_db_error(error, "ajout de virement"))?;
            Ok((ok(201), true))
        }

        ("GET", "categories") => Ok((
            json_response(200, repo::list_categories(pool).await.map_err(core_error)?),
            false,
        )),
        ("POST", "categories") => {
            let category: Category = parse_json(&request.body)?;
            in_transaction!(pool, "ajout mobile", |connection| async {
                if !is_deleted(connection, "categories", &category.id).await? {
                    repo::insert_category(connection, &category).await?;
                }
                Ok(())
            });
            Ok((ok(201), true))
        }
        ("PUT", "categories") => {
            let category: Category = parse_json(&request.body)?;
            in_transaction!(pool, "mise à jour de catégorie", |connection| repo::update_category(
                connection, &category
            ));
            Ok((ok(200), true))
        }
        ("DELETE", "categories") => {
            let id = extract_id(id, &request.body)?;
            in_transaction!(pool, "suppression de catégorie", |connection| repo::delete_category(
                connection, &id
            ));
            Ok((ok(200), true))
        }

        ("GET", "budgets") => Ok((
            json_response(200, repo::list_budgets(pool).await.map_err(core_error)?),
            false,
        )),
        ("POST", "budgets") => {
            let budget: Budget = parse_json(&request.body)?;
            in_transaction!(pool, "ajout mobile", |connection| async {
                if !is_deleted(connection, "budgets", &budget.id).await? {
                    repo::insert_budget_if_absent(connection, &budget).await?;
                }
                Ok(())
            });
            Ok((ok(201), true))
        }
        ("PUT", "budgets") => {
            let budget: Budget = parse_json(&request.body)?;
            in_transaction!(pool, "mise à jour de budget", |connection| repo::update_budget(
                connection, &budget
            ));
            Ok((ok(200), true))
        }
        ("DELETE", "budgets") => {
            let id = extract_id(id, &request.body)?;
            in_transaction!(pool, "suppression de budget", |connection| repo::delete_budget(
                connection, &id
            ));
            Ok((ok(200), true))
        }

        ("GET", "scheduled") => Ok((
            json_response(200, repo::list_scheduled(pool).await.map_err(core_error)?),
            false,
        )),
        ("POST", "scheduled") if id == Some("process-due") => {
            let processed = scheduling::process_due(pool, dates::today_local())
                .await
                .map_err(core_error)?
                .created_transactions;
            Ok((json_response(200, json!({ "processed": processed })), processed > 0))
        }
        ("POST", "scheduled") => {
            let scheduled: ScheduledTransaction = parse_json(&request.body)?;
            in_transaction!(pool, "ajout mobile", |connection| async {
                if !is_deleted(connection, "scheduled", &scheduled.id).await? {
                    repo::insert_scheduled_if_absent(connection, &scheduled).await?;
                }
                Ok(())
            });
            Ok((ok(201), true))
        }
        ("PUT", "scheduled") => {
            let scheduled: ScheduledTransaction = parse_json(&request.body)?;
            in_transaction!(pool, "mise à jour d'échéance", |connection| repo::update_scheduled(
                connection, &scheduled
            ));
            Ok((ok(200), true))
        }
        ("DELETE", "scheduled") => {
            let id = extract_id(id, &request.body)?;
            in_transaction!(pool, "suppression d'échéance", |connection| repo::delete_scheduled(
                connection, &id
            ));
            Ok((ok(200), true))
        }

        ("GET", "settings") => Ok((
            json_response(200, get_settings_record(pool).await.map_err(core_error)?),
            false,
        )),
        ("PUT", "settings") => {
            let record: SettingsRecord = parse_json(&request.body)?;
            let result = apply_settings_patch(pool, legacy_mobile_settings_patch(record))
                .await
                .map_err(core_error)?;
            Ok((json_response(200, result), true))
        }
        ("PATCH", "settings") => {
            let patch: SettingsPatch = parse_json(&request.body)?;
            let result = apply_settings_patch(pool, patch).await.map_err(core_error)?;
            Ok((json_response(200, result), true))
        }

        _ => Ok((error_response(404, "Route introuvable"), false)),
    }
}

/// Each page is one SQLite read snapshot; a changed bank version rejects the next page
/// rather than returning an inconsistent mix of two different journals.
async fn transaction_page(pool: &DbPool, path: &str) -> Result<(HttpResponse, bool), String> {
    let query = path.split_once('?').map(|(_, query)| query).unwrap_or_default();
    let parameters: HashMap<_, _> = ::url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
    let parse = |key: &str, default: i64| -> Result<i64, ()> {
        parameters
            .get(key)
            .map(|value| value.parse::<i64>().map_err(|_| ()))
            .unwrap_or(Ok(default))
    };
    let (Ok(offset), Ok(limit), Ok(version)) = (parse("offset", 0), parse("limit", 2000), parse("version", -1)) else {
        return Ok((error_response(400, "Pagination invalide"), false));
    };
    if !(0..=i32::MAX as i64).contains(&offset) || !(1..=2000).contains(&limit) || version < -1 {
        return Ok((error_response(400, "Pagination invalide"), false));
    }
    let mut read = pool
        .begin()
        .await
        .map_err(|error| map_db_error(error, "lecture du journal paginé"))?;
    let data_version = dmx_core::db::data_version(&mut *read).await.map_err(core_error)?;
    if version != -1 && version != data_version {
        return Ok((
            error_response(
                409,
                "Le journal a changé pendant sa lecture ; recommencez la synchronisation",
            ),
            false,
        ));
    }
    // JSON is embedded as a string in the encrypted response. 3 MiB leaves margin
    // for a worst-case 2x escape expansion followed by base64 (relay cap: 12 MiB).
    const PAGE_JSON_BUDGET: usize = 3 * 1024 * 1024;
    let mut rows = sqlx::query("SELECT * FROM transactions ORDER BY date DESC, rowid DESC LIMIT $1 OFFSET $2")
        .bind(limit + 1)
        .bind(offset)
        .fetch(&mut *read);
    let mut transactions = Vec::new();
    let mut bytes = 256;
    let mut has_more = false;
    while let Some(row) = rows
        .try_next()
        .await
        .map_err(|error| map_db_error(error, "lecture du journal paginé"))?
    {
        if transactions.len() >= limit as usize {
            has_more = true;
            break;
        }
        let item = repo::transaction_from_row(&row);
        let item_bytes = serde_json::to_vec(&item).map_err(|error| error.to_string())?.len() + 1;
        if bytes + item_bytes > PAGE_JSON_BUDGET {
            if transactions.is_empty() {
                return Ok((error_response(413, "Une opération dépasse la taille de synchronisation autorisée. Réduisez son libellé depuis l’application de bureau."), false));
            }
            has_more = true;
            break;
        }
        bytes += item_bytes;
        transactions.push(item);
    }
    drop(rows);
    let next_offset = has_more.then_some(offset + transactions.len() as i64);
    read.commit()
        .await
        .map_err(|error| map_db_error(error, "lecture du journal paginé"))?;
    Ok((
        json_response(
            200,
            json!({ "transactions": transactions, "dataVersion": data_version, "nextOffset": next_offset }),
        ),
        false,
    ))
}

fn parse_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("JSON invalide: {error}"))
}

#[derive(Deserialize)]
struct DeletePayload {
    id: String,
}

fn extract_id(path_id: Option<&str>, body: &[u8]) -> Result<String, String> {
    if let Some(id) = path_id {
        return Ok(id.to_string());
    }
    parse_json::<DeletePayload>(body).map(|payload| payload.id)
}

async fn load_transaction(connection: &mut SqliteConnection, id: &str) -> Result<Option<Transaction>, String> {
    let row = sqlx::query("SELECT * FROM transactions WHERE id = $1")
        .bind(id)
        .fetch_optional(connection)
        .await
        .map_err(|error| map_db_error(error, "lecture de transaction"))?;
    Ok(row.as_ref().map(repo::transaction_from_row))
}

async fn is_deleted(connection: &mut SqliteConnection, entity: &str, id: &str) -> Result<bool, CoreError> {
    let deleted: Option<bool> = sqlx::query_scalar("SELECT deleted FROM sync_meta WHERE entity = ? AND record_id = ?")
        .bind(entity)
        .bind(id)
        .fetch_optional(connection)
        .await
        .map_err(|error| CoreError::Database(map_db_error(error, "lecture de suppression")))?;
    Ok(deleted.unwrap_or(false))
}

#[cfg(test)]
mod byte_page_tests {
    use super::*;
    #[tokio::test]
    async fn escaped_descriptions_split_pages_and_preserve_every_record_within_relay_cap() {
        let pool = dmx_core::db::open_memory_pool().await.unwrap();
        let account = ops::save_account(
            &pool,
            ops::AccountDraft {
                name: "Fictif".into(),
                ..ops::new_account_draft()
            },
        )
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        for index in 0..450 {
            repo::insert_transaction(
                &mut tx,
                &Transaction {
                    id: format!("fixture-{index}"),
                    date: "2026-10-03".into(),
                    account_id: account.clone(),
                    transaction_type: dmx_core::models::TransactionType::Expense,
                    amount: 1.0,
                    category: "test".into(),
                    description: "\\\"".repeat(2048),
                    checked: false,
                    is_transfer: false,
                    linked_transaction_id: None,
                    bank_source: None,
                    bank_transaction_id: None,
                },
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();
        let mut offset = 0;
        let mut ids = std::collections::HashSet::new();
        let mut pages = 0;
        loop {
            let (response, changed) =
                transaction_page(&pool, &format!("/api/transactions/page?limit=2000&offset={offset}"))
                    .await
                    .unwrap();
            assert_eq!(response.status, 200);
            assert!(!changed);
            assert!(response.body.len() <= 3 * 1024 * 1024);
            let clear = serde_json::to_vec(&json!({"id":"fixture", "status":200, "headers":{"content-type":response.content_type}, "body":String::from_utf8(response.body.clone()).unwrap()})).unwrap();
            let encoded = (clear.len() + 16).div_ceil(3) * 4 + 256;
            assert!(encoded < 12 * 1024 * 1024);
            let page: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
            for row in page["transactions"].as_array().unwrap() {
                assert!(ids.insert(row["id"].as_str().unwrap().to_string()));
            }
            pages += 1;
            let Some(next) = page["nextOffset"].as_i64() else {
                break;
            };
            assert!(next > offset);
            offset = next;
        }
        assert_eq!(ids.len(), 450);
        assert!(pages >= 2);
    }
}
