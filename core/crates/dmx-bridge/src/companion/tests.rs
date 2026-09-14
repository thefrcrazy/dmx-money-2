//! Tests de contrat de l'API compagnon : routes, authentification par session et CORS, via un
//! vrai serveur HTTP local (sans TLS ni service managé).

use super::*;
use crate::NoopEvents;
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
    companion: Arc<MobileCompanion>,
    port: u16,
    cookie: String,
    csrf: String,
    events: Arc<CountingEvents>,
}

impl Harness {
    fn start() -> Self {
        let engine = Engine::open_in_memory().unwrap();
        let events = Arc::new(CountingEvents(AtomicI64::new(-1)));
        let preferred_port = 20000 + (std::process::id() % 20000) as u16;

        let (cookie, csrf) = ("session-brute".to_string(), "csrf-brut".to_string());
        engine.block_on(async {
            sqlx::query("INSERT OR IGNORE INTO settings (id) VALUES (1)").execute(engine.pool()).await.unwrap();
            sqlx::query("UPDATE settings SET \"mobileAccessEnabled\" = 1, \"mobileAccessPort\" = $1 WHERE id = 1")
                .bind(i64::from(preferred_port))
                .execute(engine.pool())
                .await
                .unwrap();
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

        let companion = MobileCompanion::new(BridgeHost {
            pool: engine.pool().clone(),
            runtime: engine.handle(),
            data_dir: std::env::temp_dir().join(format!("dmx-bridge-test-{}", uuid::Uuid::new_v4())),
            assets_dir: None,
            events: events.clone(),
        });
        companion.bootstrap().unwrap();
        let port = companion.status().unwrap().port.expect("serveur démarré");

        Self {
            engine,
            companion,
            port,
            cookie,
            csrf,
            events,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&str>, authenticated: bool) -> (u16, String, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let body = body.unwrap_or_default();
        let mut headers = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nOrigin: https://pwa.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        if authenticated {
            headers.push_str(&format!(
                "Cookie: dmxmoney_session={}\r\nX-Dmx-Csrf: {}\r\n",
                self.cookie, self.csrf
            ));
        }
        headers.push_str("\r\n");
        stream.write_all(headers.as_bytes()).unwrap();
        stream.write_all(body.as_bytes()).unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
        let status = head.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        (status, head.to_string(), body.to_string())
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.companion.shutdown();
    }
}

#[test]
fn api_requires_a_finalized_session() {
    let harness = Harness::start();
    let (status, _, body) = harness.request("GET", "/api/accounts", None, false);
    assert_eq!(status, 401);
    assert!(body.contains("Session mobile manquante."));

    let (status, head, _) = harness.request("OPTIONS", "/api/accounts", None, false);
    assert_eq!(status, 204);
    assert!(head.contains("Access-Control-Allow-Origin: https://pwa.example"));
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
    let mut stream = TcpStream::connect(("127.0.0.1", harness.port)).unwrap();
    let body = r##"{"id":"c1","name":"Test","icon":"Tag","color":"#000"}"##;
    let request = format!(
        "POST /api/categories HTTP/1.1\r\nHost: localhost\r\nCookie: dmxmoney_session={}\r\nContent-Length: {}\r\n\r\n{body}",
        harness.cookie,
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 401"));
    assert!(response.contains("Jeton CSRF manquant."));
}

#[test]
fn pages_redirect_or_404_without_local_assets() {
    let harness = Harness::start();
    let (status, _, _) = harness.request("GET", "/mobile", None, false);
    assert_eq!(status, 404);
    let _ = NoopEvents;
}
