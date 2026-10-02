//! Outbound relay. Cloudflare receives ciphertext and routing identifiers, never the master key.
use super::*;
use ::url::Url;
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures_util::{SinkExt, StreamExt};
use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{client::IntoClientRequest, Message},
};
use uuid::Uuid;

#[cfg(not(test))]
const KEYCHAIN_SERVICE: &str = "DmxMoney Remote Relay";
const MAX_FRAME: usize = 12 * 1024 * 1024;
const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_AGE_MS: i64 = 120_000;
const DEFAULT_REMOTE_RELAY_URL: &str = "https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev";

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct RelayConfig {
    pub(super) id: String,
    pub(super) service: String,
    key: String,
    desktop_token: String,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    id: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayRequest {
    id: String,
    issued_at: i64,
    method: String,
    path: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: String,
}

fn random_token() -> String {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn hash(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}

fn cipher(config: &RelayConfig, direction: &[u8]) -> Result<Aes256Gcm, String> {
    let master = URL_SAFE_NO_PAD
        .decode(&config.key)
        .map_err(|_| "Clé du relais invalide")?;
    if master.len() != 32 {
        return Err("Clé du relais invalide".into());
    }
    let hkdf = Hkdf::<Sha256>::new(Some(b"dmx-relay-v1"), &master);
    let mut key = [0; 32];
    hkdf.expand(direction, &mut key)
        .map_err(|_| "Dérivation de clé impossible")?;
    Ok(Aes256Gcm::new((&key).into()))
}

fn decrypt(config: &RelayConfig, envelope: &Envelope) -> Result<RelayRequest, String> {
    let nonce = URL_SAFE_NO_PAD.decode(&envelope.nonce).map_err(|_| "Nonce invalide")?;
    if nonce.len() != 12 {
        return Err("Nonce invalide".into());
    }
    let ciphertext = URL_SAFE_NO_PAD
        .decode(&envelope.ciphertext)
        .map_err(|_| "Message invalide")?;
    if ciphertext.len() > MAX_BODY + 32 * 1024 {
        return Err("Message trop volumineux".into());
    }
    let clear = cipher(config, b"request")?
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "Authentification du message impossible")?;
    let request: RelayRequest = serde_json::from_slice(&clear).map_err(|_| "Message invalide")?;
    validate_request(&request, &envelope.id, chrono::Utc::now().timestamp_millis())?;
    Ok(request)
}

fn validate_request(request: &RelayRequest, id: &str, now: i64) -> Result<(), String> {
    if request.id != id
        || uuid::Uuid::parse_str(id).is_err()
        || now.abs_diff(request.issued_at) > MAX_AGE_MS as u64
        || !matches!(request.method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE")
        || !(request.path.starts_with("/api/") || request.path.starts_with("/auth/"))
        || request.path.len() > 2048
        || request.path.contains(['\r', '\n', '#'])
        || request.body.len() > MAX_BODY
        || request.headers.len() > 16
        || request
            .headers
            .iter()
            .any(|(key, value)| key.len() > 64 || value.len() > 8192 || value.contains(['\r', '\n']))
    {
        return Err("Requête du relais invalide ou expirée".into());
    }
    Ok(())
}

fn encrypt(config: &RelayConfig, id: &str, response: HttpResponse) -> Result<String, String> {
    let mut headers = HashMap::from([("content-type".to_string(), response.content_type)]);
    for (key, value) in response.headers {
        headers.insert(key.to_ascii_lowercase(), value);
    }
    let clear = serde_json::to_vec(&json!({
        "id": id, "status": response.status, "headers": headers,
        "body": String::from_utf8(response.body).map_err(|_| "Réponse non textuelle")?,
    }))
    .map_err(|_| "Réponse invalide")?;
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher(config, b"response")?
        .encrypt(Nonce::from_slice(&nonce), clear.as_ref())
        .map_err(|_| "Chiffrement impossible")?;
    let frame = serde_json::to_string(&Envelope {
        id: id.into(),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
    .map_err(|_| "Réponse invalide")?;
    if frame.len() > MAX_FRAME {
        return Err("Réponse trop volumineuse pour le relais".into());
    }
    Ok(frame)
}

async fn schema(pool: &DbPool) -> Result<(), String> {
    sqlx::query("CREATE TABLE IF NOT EXISTS remote_relay (id INTEGER PRIMARY KEY CHECK(id=1), device_id TEXT NOT NULL, service TEXT NOT NULL)")
        .execute(pool).await.map_err(|e| map_db_error(e, "initialisation du relais"))?;
    sqlx::query("CREATE TABLE IF NOT EXISTS remote_relay_requests (id TEXT PRIMARY KEY, expires_at INTEGER NOT NULL)")
        .execute(pool)
        .await
        .map_err(|e| map_db_error(e, "initialisation anti-rejeu"))?;
    Ok(())
}

#[cfg(not(test))]
fn save_secret(config: &RelayConfig) -> Result<(), String> {
    let value = serde_json::to_string(config).map_err(|_| "Configuration du relais invalide")?;
    keyring::Entry::new(KEYCHAIN_SERVICE, &config.id)
        .and_then(|entry| entry.set_password(&value))
        .map_err(|_| "Impossible de conserver la clé du relais dans le trousseau système".into())
}

#[cfg(not(test))]
fn read_secret(id: &str) -> Result<RelayConfig, String> {
    let value = keyring::Entry::new(KEYCHAIN_SERVICE, id)
        .and_then(|entry| entry.get_password())
        .map_err(|_| "Clé du relais absente ou trousseau verrouillé ; réappairer depuis cet ordinateur")?;
    serde_json::from_str(&value).map_err(|_| "Configuration du relais invalide".into())
}

#[cfg(test)]
fn save_secret(_: &RelayConfig) -> Result<(), String> {
    Ok(())
}
#[cfg(test)]
fn read_secret(_: &str) -> Result<RelayConfig, String> {
    Err("Trousseau absent des tests".into())
}

pub(super) async fn load_config(pool: &DbPool) -> Result<Option<RelayConfig>, String> {
    schema(pool).await?;
    let row = sqlx::query("SELECT device_id, service FROM remote_relay WHERE id=1")
        .fetch_optional(pool)
        .await
        .map_err(|e| map_db_error(e, "lecture du relais"))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let id: String = row.get("device_id");
    let service: String = row.get("service");
    let config = read_secret(&id)?;
    if config.id != id || config.service != service {
        return Err("Identité du relais incohérente".into());
    }
    Ok(Some(config))
}

/// No network access in unit tests; production discovers the relay before opting into it.
pub(crate) async fn try_provision(pool: &DbPool) -> Result<bool, String> {
    if let Some(config) = load_config(pool).await? {
        activate(pool, &config).await?;
        return Ok(true);
    }
    if cfg!(test) {
        return Ok(false);
    }
    let service = option_env!("DMXMONEY_REMOTE_RELAY_URL")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_REMOTE_RELAY_URL)
        .trim_end_matches('/');
    let url = Url::parse(service).map_err(|_| "URL du relais invalide")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Le service de relais doit être une origine HTTPS".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Client du relais indisponible")?;
    let response = match client.get(format!("{service}/relay/capabilities")).send().await {
        Ok(response) => response,
        Err(_) => return Ok(false),
    };
    if !response.status().is_success() || response.content_length().unwrap_or(0) > 1024 {
        return Ok(false);
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Réponse du relais illisible")? {
        if bytes.len() + chunk.len() > 1024 {
            return Ok(false);
        }
        bytes.extend_from_slice(&chunk);
    }
    if serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|v| v.get("protocol").and_then(|v| v.as_str()).map(str::to_owned))
        .as_deref()
        != Some("dmx-relay-v1")
    {
        return Ok(false);
    }
    let config = RelayConfig {
        id: Uuid::new_v4().simple().to_string(),
        service: service.into(),
        key: random_token(),
        desktop_token: random_token(),
    };
    save_secret(&config)?;
    enroll(&config).await?;
    activate(pool, &config).await?;
    Ok(true)
}

async fn enroll(config: &RelayConfig) -> Result<(), String> {
    // Mobile sends SHA256(master key string) as bearer. The service stores its hash again.
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Client du relais indisponible")?
        .post(format!("{}/relay/{}/enroll", config.service, config.id))
        .bearer_auth(&config.desktop_token)
        .json(&json!({ "desktopHash": hash(&config.desktop_token), "mobileHash": hash(&hash(&config.key)) }))
        .send()
        .await
        .map_err(|_| "Service de relais indisponible")?;
    if !response.status().is_success() {
        return Err(format!(
            "Enregistrement du relais refusé ({})",
            response.status().as_u16()
        ));
    }
    Ok(())
}

async fn activate(pool: &DbPool, config: &RelayConfig) -> Result<(), String> {
    let domain = Url::parse(&config.service)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .ok_or("URL du relais invalide")?;
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| map_db_error(e, "activation du relais"))?;
    let previous: Option<String> = sqlx::query_scalar("SELECT \"secureBridgeDeviceId\" FROM settings WHERE id=1")
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| map_db_error(e, "lecture de l’identité du pont"))?;
    if previous.as_deref().is_some_and(|previous| previous != config.id) {
        // Changing the WebAuthn origin requires re-pairing. Keep revocation records but
        // invalidate all old credentials, sessions and unused QRs atomically.
        let now = chrono::Utc::now().to_rfc3339();
        for statement in [
            "UPDATE mobile_passkeys SET revoked_at=$1 WHERE revoked_at IS NULL",
            "UPDATE mobile_sessions SET revoked_at=$1 WHERE revoked_at IS NULL",
            "UPDATE mobile_pairing_tokens SET consumed_at=$1 WHERE consumed_at IS NULL",
        ] {
            sqlx::query(statement)
                .bind(&now)
                .execute(&mut *transaction)
                .await
                .map_err(|e| map_db_error(e, "migration de l’appairage"))?;
        }
    }
    sqlx::query("INSERT OR REPLACE INTO remote_relay (id, device_id, service) VALUES (1, $1, $2)")
        .bind(&config.id)
        .bind(&config.service)
        .execute(&mut *transaction)
        .await
        .map_err(|e| map_db_error(e, "enregistrement du relais"))?;
    sqlx::query("UPDATE settings SET \"secureBridgeDomain\"=$1, \"secureBridgeAppUrl\"=$2, \"secureBridgeLocalHost\"=NULL, \"secureBridgeDeviceId\"=$3, \"secureBridgeManagedServiceUrl\"=$4, \"secureBridgeLastError\"=NULL WHERE id=1")
        .bind(domain).bind(format!("{}/mobile", config.service)).bind(&config.id).bind(&config.service)
        .execute(&mut *transaction).await.map_err(|e| map_db_error(e, "activation du relais"))?;
    transaction
        .commit()
        .await
        .map_err(|e| map_db_error(e, "activation du relais"))?;
    Ok(())
}

pub(super) async fn build_status(
    pool: &DbPool,
    config: &RelayConfig,
    enabled: bool,
    active: bool,
    pairing: Option<(String, String)>,
) -> Result<SecureBridgeStatus, String> {
    let api = format!("{}/relay/{}", config.service, config.id);
    let app = format!("{}/mobile", config.service);
    let pairing_url = pairing
        .as_ref()
        .map(|(token, _)| format!("{app}#pairing={token}&relay={api}&key={}", config.key));
    Ok(SecureBridgeStatus {
        enabled,
        configured: true,
        active: enabled && active,
        domain: Url::parse(&config.service)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned)),
        app_url: Some(app),
        local_host: None,
        device_id: Some(config.id.clone()),
        api_url: Some(api),
        port: None,
        pairing_url,
        pairing_token_expires_at: pairing.map(|(_, expires)| expires),
        certificate_expires_at: None,
        certificate_ready: false,
        dns_record_id: None,
        dns_last_updated_at: None,
        managed: true,
        managed_service_url: config.service.clone(),
        managed_credential_ready: true,
        passkeys: secure::relay_passkeys(pool).await?,
        last_error: if enabled && !active {
            Some("Connexion au relais en cours ; l’ordinateur doit rester allumé et connecté à Internet.".into())
        } else {
            None
        },
        degraded: false,
    })
}

/// Reconnect with exponential backoff. A stop flag is checked on every frame/heartbeat.
pub(super) async fn run(host: BridgeHost, config: RelayConfig, stop: Arc<AtomicBool>, connected: Arc<AtomicBool>) {
    let mut delay = 1u64;
    while !stop.load(Ordering::SeqCst) {
        if let Err(error) = run_connection(&host, &config, &stop, &connected).await {
            log::warn!("Remote relay disconnected: {error}");
        }
        connected.store(false, Ordering::SeqCst);
        host.events.status_changed();
        tokio::time::sleep(Duration::from_secs(delay)).await;
        delay = (delay * 2).min(30);
    }
}

async fn run_connection(
    host: &BridgeHost,
    config: &RelayConfig,
    stop: &Arc<AtomicBool>,
    connected: &Arc<AtomicBool>,
) -> Result<(), String> {
    let socket_url = format!(
        "{}/relay/{}/connect",
        config
            .service
            .replacen("https://", "wss://", 1)
            .replacen("http://", "ws://", 1),
        config.id
    );
    let mut request = socket_url.into_client_request().map_err(|_| "URL du relais invalide")?;
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", config.desktop_token)
            .parse()
            .map_err(|_| "Identifiant du relais invalide")?,
    );
    let socket_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME));
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async_with_config(request, Some(socket_config), false),
    )
    .await
    .map_err(|_| "Connexion au relais expirée")?
    .map_err(|_| "Connexion au relais impossible")?;
    if stop.load(Ordering::SeqCst) {
        let _ = socket.close(None).await;
        return Ok(());
    }
    connected.store(true, Ordering::SeqCst);
    host.events.status_changed();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    let mut last_message = std::time::Instant::now();
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if stop.load(Ordering::SeqCst) { let _ = socket.close(None).await; return Ok(()); }
                if last_message.elapsed() > Duration::from_secs(45) { return Err("Relais silencieux".into()); }
                socket.send(Message::Text("ping".into())).await.map_err(|_| "Relais indisponible")?;
            }
            message = socket.next() => {
                last_message = std::time::Instant::now();
                let Some(Ok(message)) = message else { return Err("Connexion au relais fermée".into()); };
                let Message::Text(text) = message else { continue; };
                if text == "pong" { continue; }
                if stop.load(Ordering::SeqCst) { return Ok(()); }
                if text.len() > MAX_FRAME { return Err("Message du relais trop volumineux".into()); }
                let packet: Envelope = serde_json::from_str(&text).map_err(|_| "Message du relais invalide")?;
                let clear = match decrypt(config, &packet) { Ok(clear) => clear, Err(_) => continue };
                let host = host.clone();
                let origin = config.service.clone();
                // Existing handlers are synchronous and call Handle::block_on. Keep them off
                // Tokio executor threads and reuse the exact same authorization/business code.
                let id = clear.id.clone();
                let response = tokio::task::spawn_blocking(move || dispatch(&host, clear, &origin)).await.map_err(|_| "Traitement du relais interrompu")?;
                let frame = match encrypt(config, &id, response) {
                    Ok(frame) => frame,
                    Err(_) => encrypt(config, &id, error_response(413, "Données trop volumineuses pour le relais"))?,
                };
                socket.send(Message::Text(frame.into())).await.map_err(|_| "Relais indisponible")?;
            }
        }
    }
}

fn dispatch(host: &BridgeHost, request: RelayRequest, origin: &str) -> HttpResponse {
    let enabled = host
        .runtime
        .block_on(secure::load_settings(&host.pool))
        .is_ok_and(|settings| settings.enabled);
    if !enabled {
        return error_response(401, "Le compagnon distant est désactivé");
    }
    let now = chrono::Utc::now().timestamp_millis();
    let accepted = host.runtime.block_on(async {
        sqlx::query("DELETE FROM remote_relay_requests WHERE expires_at < $1")
            .bind(now)
            .execute(&host.pool)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO remote_relay_requests (id, expires_at) VALUES ($1, $2)")
            .bind(&request.id)
            .bind(now + 300_000)
            .execute(&host.pool)
            .await
    });
    if !accepted.is_ok_and(|result| result.rows_affected() == 1) {
        return error_response(409, "Requête expirée ou déjà traitée");
    }
    let mut headers: HashMap<_, _> = request
        .headers
        .into_iter()
        .filter(|(key, _)| {
            matches!(
                key.to_ascii_lowercase().as_str(),
                "cookie" | "x-dmx-csrf" | "content-type"
            )
        })
        .map(|(key, value)| (key.to_ascii_lowercase(), value))
        .collect();
    headers.insert("origin".into(), origin.into());
    http::handle_request(
        HttpRequest {
            method: request.method,
            path: request.path,
            headers,
            body: request.body.into_bytes(),
        },
        host,
        &ServerSecurity {
            secure_app_origin: Some(origin.into()),
            app_url: Some(format!("{origin}/mobile")),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_encryption_separates_directions_and_detects_tampering() {
        let config = RelayConfig {
            id: "test".into(),
            service: "https://example.com".into(),
            key: URL_SAFE_NO_PAD.encode([7; 32]),
            desktop_token: random_token(),
        };
        let nonce = [3; 12];
        let clear = b"financial data";
        let encrypted = cipher(&config, b"request")
            .unwrap()
            .encrypt(Nonce::from_slice(&nonce), clear.as_ref())
            .unwrap();
        assert_eq!(
            cipher(&config, b"request")
                .unwrap()
                .decrypt(Nonce::from_slice(&nonce), encrypted.as_ref())
                .unwrap(),
            clear
        );
        assert!(cipher(&config, b"response")
            .unwrap()
            .decrypt(Nonce::from_slice(&nonce), encrypted.as_ref())
            .is_err());
        let mut tampered = encrypted;
        tampered[0] ^= 1;
        assert!(cipher(&config, b"request")
            .unwrap()
            .decrypt(Nonce::from_slice(&nonce), tampered.as_ref())
            .is_err());
        // Interoperability fixture checked against Web Crypto in the PWA tests.
        assert_eq!(
            URL_SAFE_NO_PAD.encode(
                cipher(&config, b"request")
                    .unwrap()
                    .encrypt(Nonce::from_slice(&nonce), clear.as_ref())
                    .unwrap()
            ),
            "0XCKWd-tiNRKR9oBcNZiQKc63zee7z56JyadPQ9E"
        );
    }

    #[test]
    fn rejects_old_forged_and_non_api_requests() {
        let mut request = RelayRequest {
            id: Uuid::new_v4().to_string(),
            issued_at: 1000,
            method: "POST".into(),
            path: "/api/accounts".into(),
            headers: HashMap::new(),
            body: "{}".into(),
        };
        assert!(validate_request(&request, &request.id, 1000).is_ok());
        assert!(validate_request(&request, &request.id, 122_000).is_err());
        assert!(validate_request(&request, &Uuid::new_v4().to_string(), 1000).is_err());
        request.path = "https://attacker.invalid/api/accounts".into();
        assert!(validate_request(&request, &request.id, 1000).is_err());
    }

    #[test]
    fn encrypted_requests_use_existing_auth_and_reject_replays_after_restart() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        engine.block_on(schema(engine.pool())).unwrap();
        engine
            .block_on(sqlx::query("UPDATE settings SET \"secureBridgeEnabled\"=1 WHERE id=1").execute(engine.pool()))
            .unwrap();
        let host = BridgeHost {
            pool: engine.pool().clone(),
            runtime: engine.handle(),
            data_dir: std::env::temp_dir(),
            assets_dir: None,
            events: Arc::new(crate::NoopEvents),
        };
        let id = Uuid::new_v4().to_string();
        let request = || RelayRequest {
            id: id.clone(),
            issued_at: chrono::Utc::now().timestamp_millis(),
            method: "GET".into(),
            path: "/api/accounts".into(),
            headers: HashMap::new(),
            body: String::new(),
        };
        assert_eq!(dispatch(&host, request(), "https://example.com").status, 401);
        // Replay protection is in SQLite, so replacing the host/in-memory state cannot reset it.
        let restarted = host.clone();
        assert_eq!(dispatch(&restarted, request(), "https://example.com").status, 409);
    }

    #[test]
    fn switching_to_the_relay_revokes_old_sessions_and_preserves_new_pairing() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        engine.block_on(async {
            schema(engine.pool()).await.unwrap();
            sqlx::query("UPDATE settings SET \"secureBridgeDeviceId\"='old-local' WHERE id=1")
                .execute(engine.pool())
                .await
                .unwrap();
            let (old_token, _) = secure::regenerate_pairing_token(engine.pool()).await.unwrap();
            let config = RelayConfig {
                id: Uuid::new_v4().simple().to_string(),
                service: "https://example.com".into(),
                key: random_token(),
                desktop_token: random_token(),
            };
            activate(engine.pool(), &config).await.unwrap();
            let used: Option<String> =
                sqlx::query_scalar("SELECT consumed_at FROM mobile_pairing_tokens WHERE token_hash=$1")
                    .bind(secure::hash_secret(&old_token))
                    .fetch_one(engine.pool())
                    .await
                    .unwrap();
            assert!(used.is_some());
            let (new_token, _) = secure::regenerate_pairing_token(engine.pool()).await.unwrap();
            activate(engine.pool(), &config).await.unwrap();
            let used: Option<String> =
                sqlx::query_scalar("SELECT consumed_at FROM mobile_pairing_tokens WHERE token_hash=$1")
                    .bind(secure::hash_secret(&new_token))
                    .fetch_one(engine.pool())
                    .await
                    .unwrap();
            assert!(used.is_none());
            let columns: Vec<sqlx::sqlite::SqliteRow> = sqlx::query("PRAGMA table_info(remote_relay)")
                .fetch_all(engine.pool())
                .await
                .unwrap();
            assert_eq!(
                columns
                    .iter()
                    .map(|column| column.get::<String, _>("name"))
                    .collect::<Vec<_>>(),
                vec!["id", "device_id", "service"]
            );
        });
    }

    #[test]
    #[ignore = "requires local Worker or DMXMONEY_RELAY_SMOKE_URL"]
    fn local_cloudflare_worker_round_trip_uses_encryption_auth_and_replay_protection() {
        // Native entry points install the same provider before creating the bridge.
        crate::install_crypto_provider();
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        engine.block_on(schema(engine.pool())).unwrap();
        engine
            .block_on(sqlx::query("UPDATE settings SET \"secureBridgeEnabled\"=1 WHERE id=1").execute(engine.pool()))
            .unwrap();
        let (pairing, _) = engine
            .block_on(secure::regenerate_pairing_token(engine.pool()))
            .unwrap();
        let config = RelayConfig {
            id: Uuid::new_v4().simple().to_string(),
            service: std::env::var("DMXMONEY_RELAY_SMOKE_URL").unwrap_or_else(|_| "http://127.0.0.1:8789".into()),
            key: random_token(),
            desktop_token: random_token(),
        };
        let host = BridgeHost {
            pool: engine.pool().clone(),
            runtime: engine.handle(),
            data_dir: std::env::temp_dir(),
            assets_dir: None,
            events: Arc::new(crate::NoopEvents),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(false));
        engine.block_on(async {
            enroll(&config).await.unwrap();
            let task_host = host.clone(); let task_config = config.clone(); let task_stop = stop.clone(); let task_connected = connected.clone();
            let task = tokio::spawn(async move { run_connection(&task_host, &task_config, &task_stop, &task_connected).await });
            for _ in 0..100 { if connected.load(Ordering::SeqCst) { break; } tokio::time::sleep(Duration::from_millis(50)).await; }
            assert!(connected.load(Ordering::SeqCst));
            let id = Uuid::new_v4().to_string();
            let clear = serde_json::to_vec(&json!({ "id": id, "issuedAt": chrono::Utc::now().timestamp_millis(), "method": "POST", "path": "/auth/pairing/start", "body": serde_json::to_string(&json!({ "token": pairing, "deviceLabel": "Local smoke" })).unwrap() })).unwrap();
            let mut nonce = [0; 12]; OsRng.fill_bytes(&mut nonce);
            let ciphertext = cipher(&config, b"request").unwrap().encrypt(Nonce::from_slice(&nonce), clear.as_ref()).unwrap();
            let packet = Envelope { id: id.clone(), nonce: URL_SAFE_NO_PAD.encode(nonce), ciphertext: URL_SAFE_NO_PAD.encode(ciphertext) };
            let client = reqwest::Client::new();
            for expected in [200, 409] {
                let response = client.post(format!("{}/relay/{}/request", config.service, config.id)).bearer_auth(hash(&config.key)).json(&packet).send().await.unwrap();
                assert_eq!(response.status().as_u16(), 200);
                let encrypted: Envelope = response.json().await.unwrap();
                assert_eq!(encrypted.id, id);
                let nonce = URL_SAFE_NO_PAD.decode(encrypted.nonce).unwrap();
                let ciphertext = URL_SAFE_NO_PAD.decode(encrypted.ciphertext).unwrap();
                let bytes = cipher(&config, b"response").unwrap().decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref()).unwrap();
                let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(result["status"], expected);
                if expected == 200 { assert!(result["headers"]["set-cookie"].as_str().unwrap().contains("dmxmoney_session=")); }
            }
            stop.store(true, Ordering::SeqCst);
            task.abort();
            let cleanup = client.delete(format!("{}/relay/{}/enroll", config.service, config.id))
                .bearer_auth(&config.desktop_token).send().await.unwrap();
            assert_eq!(cleanup.status().as_u16(), 204);
        });
    }
}
