//! Routes `/api/*` consommées par la PWA. Les formes JSON et les codes de réponse sont ceux de 1.x ;
//! les écritures passent par `dmx-core` (insertions idempotentes pour rejouer la file hors ligne).

use super::*;
use dmx_core::models::{Account, Budget, Category, ScheduledTransaction, Transaction};
use dmx_core::settings::{
    apply_settings_patch, get_settings_record, legacy_mobile_settings_patch, SettingsPatch, SettingsRecord,
};
use dmx_core::{dates, ops, repo, scheduling, CoreError};
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

    match (method, resource) {
        ("GET", "status") => Ok((
            json_response(200, json!({ "ok": true, "dataVersion": get_data_version(pool).await? })),
            false,
        )),

        ("GET", "accounts") => Ok((
            json_response(200, repo::list_accounts(pool).await.map_err(core_error)?),
            false,
        )),
        ("POST", "accounts") => {
            let account: Account = parse_json(&request.body)?;
            in_transaction!(pool, "ajout du compte", |connection| repo::insert_account(
                connection, &account
            ));
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

        ("GET", "transactions") => Ok((
            json_response(200, repo::list_transactions(pool).await.map_err(core_error)?),
            false,
        )),
        ("POST", "transactions") => {
            let transaction: Transaction = parse_json(&request.body)?;
            in_transaction!(pool, "ajout de transaction", |connection| {
                repo::insert_transaction_if_absent(connection, &transaction)
            });
            Ok((ok(201), true))
        }
        ("PUT", "transactions") => {
            let transaction: Transaction = parse_json(&request.body)?;
            in_transaction!(pool, "mise à jour de transaction", |connection| {
                repo::update_transaction(connection, &transaction)
            });
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
            let mut tx = pool
                .begin()
                .await
                .map_err(|error| map_db_error(error, "ajout de virement"))?;
            repo::insert_transaction_if_absent(&mut tx, &payload.from_transaction)
                .await
                .map_err(core_error)?;
            repo::insert_transaction_if_absent(&mut tx, &payload.to_transaction)
                .await
                .map_err(core_error)?;
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
            in_transaction!(pool, "ajout de catégorie", |connection| repo::insert_category(
                connection, &category
            ));
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
            in_transaction!(pool, "ajout de budget", |connection| repo::insert_budget_if_absent(
                connection, &budget
            ));
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
            in_transaction!(pool, "ajout d'échéance", |connection| {
                repo::insert_scheduled_if_absent(connection, &scheduled)
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
