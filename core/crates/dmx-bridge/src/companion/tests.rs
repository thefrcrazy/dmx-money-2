//! Contrat du dispatch exécuté sur le bureau par le relais : routes, sessions et mutations.

use super::*;
use dmx_core::Engine;
use std::sync::atomic::AtomicI64;

struct CountingEvents(AtomicI64);

impl crate::BridgeEvents for CountingEvents {
    fn data_changed(&self, data_version: i64) {
        self.0.store(data_version, Ordering::SeqCst);
    }
    fn status_changed(&self) {}
}

struct Harness {
    engine: Engine,
    host: BridgeHost,
    cookie: String,
    csrf: String,
    events: Arc<CountingEvents>,
}

impl Harness {
    fn start() -> Self {
        let engine = Engine::open_in_memory().unwrap();
        let events = Arc::new(CountingEvents(AtomicI64::new(-1)));

        let (cookie, csrf) = ("session-brute".to_string(), "csrf-brut".to_string());
        engine.block_on(async {
            sqlx::query("INSERT OR IGNORE INTO settings (id) VALUES (1)").execute(engine.pool()).await.unwrap();
            sqlx::query("INSERT INTO mobile_passkeys (id, credential_id, public_key, created_at) VALUES ('pk1', 'test-credential', '{}', $1)")
                .bind(chrono::Utc::now().to_rfc3339()).execute(engine.pool()).await.unwrap();
            sqlx::query(
                "INSERT INTO mobile_sessions (id, session_hash, csrf_hash, passkey_id, device_label, expires_at, created_at)
                 VALUES ('s1', $1, $2, 'pk1', 'Test', $3, $4)",
            )
            .bind(secure::hash_secret(&cookie))
            .bind(secure::hash_secret(&csrf))
            .bind((chrono::Utc::now() + chrono::Duration::minutes(30)).to_rfc3339())
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(engine.pool())
            .await
            .unwrap();
        });

        let host = BridgeHost {
            pool: engine.pool().clone(),
            runtime: engine.handle(),
            data_dir: std::env::temp_dir().join(format!("dmx-bridge-test-{}", uuid::Uuid::new_v4())),
            assets_dir: None,
            events: events.clone(),
        };

        Self {
            engine,
            host,
            cookie,
            csrf,
            events,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&str>, authenticated: bool) -> (u16, String, String) {
        self.request_with_csrf(method, path, body, authenticated, true)
    }

    fn request_with_csrf(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
        authenticated: bool,
        csrf: bool,
    ) -> (u16, String, String) {
        let mut headers = HashMap::from([("content-type".into(), "application/json".into())]);
        if authenticated {
            headers.insert("cookie".into(), format!("dmxmoney_session={}", self.cookie));
            if csrf {
                headers.insert("x-dmx-csrf".into(), self.csrf.clone());
            }
        }
        let response = http::handle_request(
            HttpRequest {
                method: method.into(),
                path: path.into(),
                headers,
                body: body.unwrap_or_default().as_bytes().to_vec(),
            },
            &self.host,
        );
        (
            response.status,
            String::new(),
            String::from_utf8(response.body).unwrap(),
        )
    }
}

#[test]
fn api_requires_a_finalized_session() {
    let harness = Harness::start();
    let (status, _, body) = harness.request("GET", "/api/accounts", None, false);
    assert_eq!(status, 401);
    assert!(body.contains("Session mobile manquante."));
}

#[test]
fn unpaired_transfer_category_is_refused_before_companion_writes() {
    let harness = Harness::start();
    let account = json!({"id":"simple-account","name":"Compte fictif","type":"Courant","initialBalance":100});
    assert_eq!(
        harness
            .request("POST", "/api/accounts", Some(&account.to_string()), true)
            .0,
        201
    );
    let simple = json!({"id":"simple-row","date":"2026-10-03","accountId":"simple-account","type":"expense","amount":10,"category":"food","description":"Fictif","checked":false,"isTransfer":false});
    assert_eq!(
        harness
            .request("POST", "/api/transactions", Some(&simple.to_string()), true)
            .0,
        201
    );
    let before = harness.engine.snapshot().unwrap();
    let version = harness.engine.data_version().unwrap();
    let mut unpaired = simple.clone();
    unpaired["category"] = json!("transfer");
    unpaired["_base"] = simple;
    unpaired["_mutationId"] = json!("unpaired-existing");
    for method in ["PATCH", "PUT"] {
        let (status, _, body) = harness.request(method, "/api/transactions", Some(&unpaired.to_string()), true);
        assert_eq!(status, 400, "{method}: {body}");
    }
    unpaired["id"] = json!("new-unpaired");
    unpaired["_mutationId"] = json!("unpaired-new");
    let (status, _, body) = harness.request("POST", "/api/transactions", Some(&unpaired.to_string()), true);
    assert_eq!(status, 400, "{body}");
    assert_eq!(harness.engine.snapshot().unwrap().transactions, before.transactions);
    assert_eq!(harness.engine.data_version().unwrap(), version);
    let receipts: i64 = harness.engine.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM mobile_mutation_receipts WHERE id LIKE 'unpaired-%'")
            .fetch_one(harness.engine.pool())
            .await
            .unwrap()
    });
    assert_eq!(receipts, 0);
}

#[test]
fn revoking_a_mobile_invalidates_its_existing_sessions_immediately() {
    let harness = Harness::start();
    assert_eq!(harness.request("GET", "/api/accounts", None, true).0, 200);
    harness
        .engine
        .block_on(secure::revoke_passkey(harness.engine.pool(), "pk1".into()))
        .unwrap();
    assert_eq!(harness.request("GET", "/api/accounts", None, true).0, 401);
    assert_eq!(harness.request("POST", "/api/accounts", Some("{}"), true).0, 401);
}

#[test]
fn concurrent_pairing_requests_can_consume_a_qr_only_once() {
    let engine = Engine::open_in_memory().unwrap();
    let (token, _) = engine
        .block_on(secure::regenerate_pairing_token(engine.pool()))
        .unwrap();
    let results = engine.block_on(async {
        let payload = serde_json::to_vec(&json!({ "token": token, "deviceLabel": "Test" })).unwrap();
        let headers = HashMap::new();
        tokio::join!(
            secure::handle_auth_request(engine.pool(), "POST", "/auth/pairing/start", &headers, &payload),
            secure::handle_auth_request(engine.pool(), "POST", "/auth/pairing/start", &headers, &payload),
        )
    });
    assert_eq!(usize::from(results.0.is_ok()) + usize::from(results.1.is_ok()), 1);
}

#[test]
fn transaction_pages_are_bounded_and_reject_a_bank_changed_between_pages() {
    let harness = Harness::start();
    harness.engine.block_on(async {
        let mut transaction = harness.engine.pool().begin().await.unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, \"initialBalance\", color, icon) VALUES ('paged', 'Paged', 'Courant', 0, '#123456', 'Wallet')").execute(&mut *transaction).await.unwrap();
        for index in 0..2001 {
            sqlx::query("INSERT INTO transactions (id, date, \"accountId\", type, amount, category, description, checked, \"isTransfer\") VALUES ($1, '2026-10-01', 'paged', 'expense', 1, 'transfer', 'Test', 0, 0)")
                .bind(format!("page-{index}")).execute(&mut *transaction).await.unwrap();
        }
        transaction.commit().await.unwrap();
    });
    let (status, _, body) = harness.request("GET", "/api/transactions/page?limit=2000", None, true);
    assert_eq!(status, 200);
    let page: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(page["transactions"].as_array().unwrap().len(), 2000);
    assert_eq!(page["nextOffset"], 2000);
    let version = page["dataVersion"].as_i64().unwrap();
    let (status, _, body) = harness.request(
        "GET",
        &format!("/api/transactions/page?offset=2000&version={version}"),
        None,
        true,
    );
    assert_eq!(status, 200);
    let page: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(page["transactions"].as_array().unwrap().len(), 1);
    assert!(page["nextOffset"].is_null());
    harness
        .engine
        .block_on(sqlx::query("UPDATE transactions SET checked=1 WHERE id='page-0'").execute(harness.engine.pool()))
        .unwrap();
    assert_eq!(
        harness
            .request(
                "GET",
                &format!("/api/transactions/page?offset=2000&version={version}"),
                None,
                true
            )
            .0,
        409
    );
    for query in ["limit=2001", "offset=-1", "version=oops"] {
        assert_eq!(
            harness
                .request("GET", &format!("/api/transactions/page?{query}"), None, true)
                .0,
            400
        );
    }
}

#[test]
fn the_assistant_answers_a_sentence_from_the_pwa() {
    let harness = Harness::start();
    let account =
        r##"{"id":"acc-1","name":"Courant","type":"Courant","initialBalance":250,"color":"#3b82f6","icon":"Wallet"}"##;
    assert_eq!(harness.request("POST", "/api/accounts", Some(account), true).0, 201);

    // Lecture : la réponse vient du noyau, rien n'est écrit.
    let (status, _, body) = harness.request(
        "POST",
        "/api/assistant",
        Some(r#"{"text":"quel est mon solde ?"}"#),
        true,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"changed\":false"), "{body}");
    assert!(body.contains("250"), "{body}");

    // Écriture : l'opération dictée est enregistrée.
    let (status, _, body) = harness.request(
        "POST",
        "/api/assistant",
        Some(r#"{"text":"ajoute 12,50 en alimentation"}"#),
        true,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"changed\":true"), "{body}");
    let (_, _, journal) = harness.request("GET", "/api/transactions", None, true);
    assert!(journal.contains("12.5"), "{journal}");

    // Demande incomprise : signalée, sans écriture.
    let (status, _, body) = harness.request("POST", "/api/assistant", Some(r#"{"text":"bonjour"}"#), true);
    assert_eq!(status, 200);
    assert!(body.contains("\"understood\":false"), "{body}");

    // Sans session valide, la route reste fermée.
    assert_eq!(
        harness
            .request("POST", "/api/assistant", Some(r#"{"text":"solde"}"#), false)
            .0,
        401
    );
}

#[test]
fn crud_routes_match_the_v1_contract() {
    let harness = Harness::start();

    let (status, _, body) = harness.request("GET", "/api/status", None, true);
    assert_eq!(status, 200);
    assert!(body.contains("\"dataVersion\""));

    let account =
        r##"{"id":"acc-1","name":"Courant","type":"Courant","initialBalance":100,"color":"#3b82f6","icon":"Wallet"}"##;
    assert_eq!(harness.request("POST", "/api/accounts", Some(account), true).0, 201);
    assert_eq!(
        harness.request("POST", "/api/accounts", Some(account), true).0,
        201,
        "insertion idempotente"
    );

    let transaction = r#"{"id":"tx-1","date":"2026-09-10","accountId":"acc-1","type":"expense","amount":12.5,"category":"5","description":"Café","checked":false}"#;
    assert_eq!(
        harness.request("POST", "/api/transactions", Some(transaction), true).0,
        201
    );
    assert!(harness.events.0.load(Ordering::SeqCst) > 0, "le desktop est notifié");

    let (status, _, body) = harness.request("GET", "/api/transactions", None, true);
    assert_eq!(status, 200);
    let transactions: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(transactions[0]["accountId"], "acc-1");
    assert_eq!(transactions[0]["description"], "Café");

    let (status, _, body) = harness.request(
        "PATCH",
        "/api/settings",
        Some(r#"{"schemaVersion":2,"baseRevision":0,"values":{"theme":"dark"}}"#),
        true,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"revision\":1"));
    let (_, _, body) = harness.request("GET", "/api/settings", None, true);
    assert!(body.contains("\"theme\":\"dark\""));

    assert_eq!(harness.request("DELETE", "/api/transactions/tx-1", None, true).0, 200);
    let snapshot = harness.engine.snapshot().unwrap();
    assert!(snapshot.transactions.is_empty());
    assert_eq!(snapshot.settings.theme, dmx_core::models::Theme::Dark);

    let (status, _, _) = harness.request("GET", "/api/inconnu", None, true);
    assert_eq!(status, 404);
}

#[test]
fn mutations_require_the_csrf_token() {
    let harness = Harness::start();
    let body = r##"{"id":"c1","name":"Test","icon":"Tag","color":"#000"}"##;
    let (status, _, response) = harness.request_with_csrf("POST", "/api/categories", Some(body), true, false);
    assert_eq!(status, 401);
    assert!(response.contains("Jeton CSRF manquant."));
}

#[test]
fn the_relay_dispatch_never_serves_a_local_pwa() {
    let harness = Harness::start();
    let (status, _, _) = harness.request("GET", "/mobile", None, false);
    assert_eq!(status, 404);
}

#[test]
fn mobile_rejects_invalid_amounts_and_unbalanced_transfers_without_writes() {
    let harness = Harness::start();
    for amount in [-1.0, 0.001, 1e308] {
        let payload = json!({"id":"bad", "date":"2026-09-16", "accountId":"acc", "type":"expense", "amount":amount, "category":"5", "description":"test", "checked":false}).to_string();
        assert_eq!(
            harness.request("POST", "/api/transactions", Some(&payload), true).0,
            400
        );
    }
    let from = json!({"id":"from", "date":"2026-09-16", "accountId":"a", "type":"expense", "amount":10, "category":"transfer", "description":"test", "checked":false,"isTransfer":true,"linkedTransactionId":"to"});
    let to = json!({"id":"to", "date":"2026-09-16", "accountId":"b", "type":"income", "amount":11, "category":"transfer", "description":"test", "checked":false,"isTransfer":true,"linkedTransactionId":"from"});
    let payload = json!({"fromTransaction":from,"toTransaction":to}).to_string();
    assert_eq!(harness.request("POST", "/api/transfers", Some(&payload), true).0, 400);
    let (_, _, journal) = harness.request("GET", "/api/transactions", None, true);
    assert_eq!(journal, "[]");
}

#[test]
fn transfer_updates_are_atomic_and_cannot_retarget_unrelated_transactions() {
    let harness = Harness::start();
    for id in ["a", "b"] {
        let account =
            json!({"id":id,"name":id,"type":"Courant","initialBalance":100,"color":"#3b82f6","icon":"Wallet"})
                .to_string();
        assert_eq!(harness.request("POST", "/api/accounts", Some(&account), true).0, 201);
    }
    let from = json!({"id":"from", "date":"2026-09-16", "accountId":"a", "type":"expense", "amount":10, "category":"transfer", "description":"test", "checked":false,"isTransfer":true,"linkedTransactionId":"to"});
    let to = json!({"id":"to", "date":"2026-09-16", "accountId":"b", "type":"income", "amount":10, "category":"transfer", "description":"test", "checked":false,"isTransfer":true,"linkedTransactionId":"from"});
    let payload = json!({"fromTransaction":from,"toTransaction":to}).to_string();
    assert_eq!(harness.request("POST", "/api/transfers", Some(&payload), true).0, 201);
    assert_eq!(harness.request("POST", "/api/transfers", Some(&payload), true).0, 200);
    let mut edited = from.clone();
    edited["amount"] = json!(25);
    edited["checked"] = json!(true);
    assert_eq!(
        harness
            .request("PUT", "/api/transactions", Some(&edited.to_string()), true)
            .0,
        200
    );
    let (_, _, body) = harness.request("GET", "/api/transactions", None, true);
    let rows: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .all(|row| row["amount"].as_f64() == Some(25.0) && row["checked"] == true));
    edited["linkedTransactionId"] = json!("other");
    assert_eq!(
        harness
            .request("PUT", "/api/transactions", Some(&edited.to_string()), true)
            .0,
        400
    );
    // Stale retries must not produce a half-old, half-new transfer.
    assert_eq!(harness.request("POST", "/api/transfers", Some(&payload), true).0, 409);
}

#[test]
fn offline_field_edits_preserve_desktop_changes_and_replays_are_noops() {
    let harness = Harness::start();
    let account = r##"{"id":"sync-account","name":"Courant","type":"Courant","initialBalance":100,"color":"#3b82f6","icon":"Wallet"}"##;
    assert_eq!(harness.request("POST", "/api/accounts", Some(account), true).0, 201);
    let base = json!({"id":"sync-tx","date":"2026-09-17","accountId":"sync-account","type":"expense","amount":12.5,"category":"5","description":"Courses","checked":false});
    assert_eq!(
        harness
            .request("POST", "/api/transactions", Some(&base.to_string()), true)
            .0,
        201
    );
    let mut desktop = base.clone();
    desktop["amount"] = json!(20.0);
    assert_eq!(
        harness
            .request("PUT", "/api/transactions", Some(&desktop.to_string()), true)
            .0,
        200
    );
    let mut mobile = base.clone();
    mobile["checked"] = json!(true);
    mobile["_base"] = base.clone();
    mobile["_mutationId"] = json!("offline-check-1");
    assert_eq!(
        harness
            .request("PATCH", "/api/transactions", Some(&mobile.to_string()), true)
            .0,
        200
    );
    let tx = harness
        .engine
        .snapshot()
        .unwrap()
        .transactions
        .iter()
        .find(|tx| tx.id == "sync-tx")
        .unwrap()
        .clone();
    assert_eq!(tx.amount, 20.0);
    assert!(tx.checked);
    // The desktop unchecks it after the first send. A lost acknowledgement must not re-check it.
    assert_eq!(
        harness
            .request("PUT", "/api/transactions", Some(&desktop.to_string()), true)
            .0,
        200
    );
    assert_eq!(
        harness
            .request("PATCH", "/api/transactions", Some(&mobile.to_string()), true)
            .0,
        200
    );
    assert!(!harness.engine.snapshot().unwrap().transactions[0].checked);
    mobile["amount"] = json!(999.0);
    assert_eq!(
        harness
            .request("PATCH", "/api/transactions", Some(&mobile.to_string()), true)
            .0,
        409
    );
    // A delayed creation cannot resurrect a deleted transaction.
    assert_eq!(
        harness.request("DELETE", "/api/transactions/sync-tx", None, true).0,
        200
    );
    assert_eq!(
        harness
            .request("POST", "/api/transactions", Some(&base.to_string()), true)
            .0,
        201
    );
    assert!(harness.engine.snapshot().unwrap().transactions.is_empty());
}

#[test]
fn replaying_mobile_crud_never_duplicates_or_resurrects_bank_records() {
    let harness = Harness::start();
    let account =
        json!({"id":"a-sync","name":"Courant","type":"Courant","initialBalance":100,"color":"#007AFF","icon":"Wallet"});
    let category = json!({"id":"c-sync","name":"Test","color":"#007AFF","icon":"Tag"});
    let budget = json!({"id":"b-sync","name":"Budget","amount":100,"category":"5","accountId":"a-sync"});
    let scheduled = json!({"id":"s-sync","description":"Loyer","amount":10,"type":"expense","frequency":"monthly","accountId":"a-sync","nextDate":"2026-09-01","category":"5"});
    for (resource, item) in [
        ("accounts", account),
        ("categories", category),
        ("budgets", budget),
        ("scheduled", scheduled.clone()),
    ] {
        let path = format!("/api/{resource}");
        for _ in 0..3 {
            assert_eq!(harness.request("POST", &path, Some(&item.to_string()), true).0, 201);
        }
        let (_, _, body) = harness.request("GET", &path, None, true);
        let records: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(records.iter().filter(|row| row["id"] == item["id"]).count(), 1);
        let mut edited = item.clone();
        edited["_base"] = item.clone();
        edited["_mutationId"] = json!(format!("edit-{resource}"));
        if resource == "scheduled" {
            edited["description"] = json!("Nouveau libellé");
        } else {
            edited["name"] = json!("Nouveau libellé");
        }
        assert_eq!(harness.request("PATCH", &path, Some(&edited.to_string()), true).0, 200);
    }
    let today = dmx_core::dates::today_local();
    harness.engine.process_due_scheduled(today).unwrap();
    let count = harness.engine.snapshot().unwrap().transactions.len();
    assert!(count > 0);
    // Both the phone and desktop request processing of the same occurrence.
    for _ in 0..3 {
        assert_eq!(harness.request("POST", "/api/scheduled/process-due", None, true).0, 200);
        harness.engine.process_due_scheduled(today).unwrap();
    }
    let mut stale_edit = scheduled.clone();
    stale_edit["description"] = json!("Libellé hors ligne");
    stale_edit["_base"] = scheduled;
    stale_edit["_mutationId"] = json!("edit-after-due");
    assert_eq!(
        harness
            .request("PATCH", "/api/scheduled", Some(&stale_edit.to_string()), true)
            .0,
        200
    );
    harness.engine.process_due_scheduled(today).unwrap();
    assert_eq!(harness.engine.snapshot().unwrap().transactions.len(), count);
    assert!(harness
        .engine
        .snapshot()
        .unwrap()
        .scheduled
        .iter()
        .all(|item| item.next_date > today.to_string()));
    for (resource, id) in [
        ("scheduled", "s-sync"),
        ("budgets", "b-sync"),
        ("categories", "c-sync"),
        ("accounts", "a-sync"),
    ] {
        assert_eq!(
            harness
                .request("DELETE", &format!("/api/{resource}/{id}"), None, true)
                .0,
            200
        );
        assert_eq!(
            harness
                .request("DELETE", &format!("/api/{resource}/{id}"), None, true)
                .0,
            200
        );
    }
}

#[test]
fn a_receipted_transfer_retry_preserves_later_desktop_edits() {
    let harness = Harness::start();
    for id in ["from-account", "to-account"] {
        let account =
            json!({"id":id,"name":id,"type":"Courant","initialBalance":100,"color":"#007AFF","icon":"Wallet"});
        assert_eq!(
            harness
                .request("POST", "/api/accounts", Some(&account.to_string()), true)
                .0,
            201
        );
    }
    let from = json!({"id":"out","date":"2026-09-17","accountId":"from-account","type":"expense","amount":10,"category":"transfer","description":"Virement","checked":false,"isTransfer":true,"linkedTransactionId":"in"});
    let mut to = from.clone();
    to["id"] = json!("in");
    to["accountId"] = json!("to-account");
    to["type"] = json!("income");
    to["linkedTransactionId"] = json!("out");
    let payload = json!({"fromTransaction":from,"toTransaction":to,"_mutationId":"transfer-retry"});
    assert_eq!(
        harness
            .request("POST", "/api/transfers", Some(&payload.to_string()), true)
            .0,
        201
    );
    let mut edit = payload["fromTransaction"].clone();
    edit["amount"] = json!(20);
    edit["checked"] = json!(true);
    assert_eq!(
        harness
            .request("PUT", "/api/transactions", Some(&edit.to_string()), true)
            .0,
        200
    );
    for _ in 0..3 {
        assert_eq!(
            harness
                .request("POST", "/api/transfers", Some(&payload.to_string()), true)
                .0,
            200
        );
    }
    let snapshot = harness.engine.snapshot().unwrap();
    assert_eq!(snapshot.transactions.len(), 2);
    assert!(snapshot.transactions.iter().all(|tx| tx.amount == 20.0 && tx.checked));
}

#[test]
fn an_acknowledgement_lost_after_a_once_occurrence_cannot_recreate_it() {
    let harness = Harness::start();
    let account = json!({"id":"once-account","name":"Courant","type":"Courant","initialBalance":100,"color":"#007AFF","icon":"Wallet"});
    assert_eq!(
        harness
            .request("POST", "/api/accounts", Some(&account.to_string()), true)
            .0,
        201
    );
    let scheduled = json!({"id":"once","description":"Unique","amount":10,"type":"expense","frequency":"once","accountId":"once-account","nextDate":"2026-09-01","category":"5"});
    assert_eq!(
        harness
            .request("POST", "/api/scheduled", Some(&scheduled.to_string()), true)
            .0,
        201
    );
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            harness
                .engine
                .process_due_scheduled(dmx_core::dates::today_local())
                .unwrap()
        });
        let second = scope.spawn(|| {
            harness
                .engine
                .process_due_scheduled(dmx_core::dates::today_local())
                .unwrap()
        });
        first.join().unwrap();
        second.join().unwrap();
    });
    assert_eq!(
        harness
            .request("POST", "/api/scheduled", Some(&scheduled.to_string()), true)
            .0,
        201
    );
    assert_eq!(harness.request("POST", "/api/scheduled/process-due", None, true).0, 200);
    let snapshot = harness.engine.snapshot().unwrap();
    assert_eq!(snapshot.transactions.len(), 1);
    assert!(snapshot.scheduled.is_empty());
}

#[test]
fn mobile_transfer_patch_merges_both_accounts_once_and_rolls_back_on_conflict_or_second_write_failure() {
    let harness = Harness::start();
    for id in ["a", "b", "c", "d"] {
        let account =
            json!({"id":id,"name":id,"type":"Courant","initialBalance":100,"color":"#3b82f6","icon":"Wallet"})
                .to_string();
        assert_eq!(harness.request("POST", "/api/accounts", Some(&account), true).0, 201);
    }
    let from = json!({"id":"from", "date":"2026-10-03", "accountId":"a", "type":"expense", "amount":10, "category":"transfer", "description":"test", "checked":false,"isTransfer":true,"linkedTransactionId":"to"});
    let to = json!({"id":"to", "date":"2026-10-03", "accountId":"b", "type":"income", "amount":10, "category":"transfer", "description":"test", "checked":false,"isTransfer":true,"linkedTransactionId":"from"});
    let initial = json!({"fromTransaction":from,"toTransaction":to});
    assert_eq!(
        harness
            .request("POST", "/api/transfers", Some(&initial.to_string()), true)
            .0,
        201
    );
    let mut remote = from.clone();
    remote["amount"] = json!(25);
    assert_eq!(
        harness
            .request("PUT", "/api/transactions", Some(&remote.to_string()), true)
            .0,
        200
    );
    let mut desired_from = from.clone();
    let mut desired_to = to.clone();
    desired_from["accountId"] = json!("c");
    desired_to["accountId"] = json!("d");
    let patch = json!({"fromTransaction":desired_from,"toTransaction":desired_to,"_base":initial,"_mutationId":"pair-edit","_mutationCreatedAt":chrono::Utc::now().to_rfc3339()});
    assert_eq!(
        harness
            .request("PATCH", "/api/transfers", Some(&patch.to_string()), true)
            .0,
        200
    );
    let rows = harness.engine.snapshot().unwrap().transactions.clone();
    assert_eq!(rows.iter().find(|row| row.id == "from").unwrap().account_id, "c");
    assert_eq!(rows.iter().find(|row| row.id == "to").unwrap().account_id, "d");
    assert!(rows.iter().all(|row| row.amount == 25.0));
    let version = harness.engine.data_version().unwrap();
    assert_eq!(
        harness
            .request("PATCH", "/api/transfers", Some(&patch.to_string()), true)
            .0,
        200
    );
    assert_eq!(harness.engine.data_version().unwrap(), version);
    let mut conflicting = patch.clone();
    conflicting["_mutationId"] = json!("conflict");
    conflicting["fromTransaction"]["amount"] = json!(30);
    conflicting["toTransaction"]["amount"] = json!(30);
    assert_eq!(
        harness
            .request("PATCH", "/api/transfers", Some(&conflicting.to_string()), true)
            .0,
        409
    );
    assert_eq!(harness.engine.snapshot().unwrap().transactions, rows);
    let base_from = rows.iter().find(|row| row.id == "from").unwrap();
    let base_to = rows.iter().find(|row| row.id == "to").unwrap();
    let mut final_patch = json!({"fromTransaction":base_from,"toTransaction":base_to,"_base":{"fromTransaction":base_from,"toTransaction":base_to},"_mutationId":"second-failure"});
    final_patch["fromTransaction"]["description"] = json!("changed");
    final_patch["toTransaction"]["description"] = json!("changed");
    harness.engine.block_on(async { sqlx::query("CREATE TRIGGER fail_second_side BEFORE UPDATE ON transactions WHEN NEW.id='to' BEGIN SELECT RAISE(ABORT,'fixture second-side failure'); END").execute(harness.engine.pool()).await.unwrap(); });
    assert_eq!(
        harness
            .request("PATCH", "/api/transfers", Some(&final_patch.to_string()), true)
            .0,
        500
    );
    assert_eq!(harness.engine.snapshot().unwrap().transactions, rows);
    assert_eq!(harness.engine.data_version().unwrap(), version);
    let receipts: i64 = harness
        .engine
        .block_on(
            sqlx::query_scalar("SELECT count(*) FROM mobile_mutation_receipts WHERE id='second-failure'")
                .fetch_one(harness.engine.pool()),
        )
        .unwrap();
    assert_eq!(receipts, 0);
}
