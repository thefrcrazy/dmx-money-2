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
const DEFAULT_COMPANION_URL: &str = "https://dmxmoney-companion.pages.dev/mobile/";

fn companion_url(_config: &RelayConfig) -> Result<String, String> {
    let value = option_env!("DMXMONEY_COMPANION_URL")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| DEFAULT_COMPANION_URL.into());
    let url = Url::parse(&value).map_err(|_| "Adresse du compagnon invalide")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Le compagnon doit être une adresse HTTPS sans identifiants".into());
    }
    Ok(value)
}

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
fn save_secret(config: &RelayConfig) -> Result<(), String> {
    TEST_SECRETS.lock().unwrap().insert(config.id.clone(), config.clone());
    Ok(())
}
#[cfg(test)]
fn read_secret(id: &str) -> Result<RelayConfig, String> {
    TEST_SECRETS
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(|| "Clé du relais absente des tests".into())
}

#[cfg(test)]
static TEST_SECRETS: std::sync::LazyLock<std::sync::Mutex<HashMap<String, RelayConfig>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

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
        prepare_existing(pool, &config).await?;
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

pub(crate) async fn enable_internet(pool: &DbPool) -> Result<(), String> {
    if !try_provision(pool).await? {
        return Err("Service d’accès Internet indisponible. Réessayez après reconnexion ; aucun DNS individuel n’est nécessaire.".into());
    }
    let config = load_config(pool).await?.ok_or("Configuration du relais absente")?;
    activate(pool, &config).await
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
    let app = companion_url(config)?;
    let domain = Url::parse(&app)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .ok_or("URL du relais invalide")?;
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| map_db_error(e, "activation du relais"))?;
    let previous = sqlx::query("SELECT \"secureBridgeDeviceId\", \"secureBridgeAppUrl\" FROM settings WHERE id=1")
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| map_db_error(e, "lecture de l’identité du pont"))?;
    let previous_id: Option<String> = previous.get("secureBridgeDeviceId");
    let previous_app: Option<String> = previous.get("secureBridgeAppUrl");
    let origin_changed = previous_app
        .as_deref()
        .and_then(|app| Url::parse(app).ok())
        .is_some_and(|previous| previous.origin() != Url::parse(&app).unwrap().origin());
    if previous_id.as_deref().is_some_and(|previous| previous != config.id) || origin_changed {
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
                .map_err(|e| map_db_error(e, "mise à jour de l’appairage"))?;
        }
    }
    sqlx::query("INSERT OR REPLACE INTO remote_relay (id, device_id, service) VALUES (1, $1, $2)")
        .bind(&config.id)
        .bind(&config.service)
        .execute(&mut *transaction)
        .await
        .map_err(|e| map_db_error(e, "enregistrement du relais"))?;
    sqlx::query("UPDATE settings SET \"secureBridgeDomain\"=$1, \"secureBridgeAppUrl\"=$2, \"secureBridgeLocalHost\"=NULL, \"secureBridgeDeviceId\"=$3, \"secureBridgeManagedServiceUrl\"=$4, \"secureBridgeLastError\"=NULL, \"secureBridgeManagedDeviceSecret\"=NULL, \"secureBridgeManagedRegisteredAt\"=NULL, \"secureBridgeDnsRecordId\"=NULL, \"secureBridgeDnsLastUpdatedAt\"=NULL, \"secureBridgeCertificateExpiresAt\"=NULL, \"mobileAccessToken\"=NULL WHERE id=1")
        .bind(domain).bind(app).bind(&config.id).bind(&config.service)
        .execute(&mut *transaction).await.map_err(|e| map_db_error(e, "activation du relais"))?;
    transaction
        .commit()
        .await
        .map_err(|e| map_db_error(e, "activation du relais"))?;
    Ok(())
}

pub(super) async fn prepare_existing(pool: &DbPool, config: &RelayConfig) -> Result<(), String> {
    let app = companion_url(config)?;
    let current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM settings WHERE id=1 AND \"secureBridgeDeviceId\"=$1 AND \"secureBridgeAppUrl\"=$2 AND \"secureBridgeDomain\"=$3 AND \"secureBridgeManagedServiceUrl\"=$4 AND \"secureBridgeLocalHost\" IS NULL AND \"secureBridgeDnsRecordId\" IS NULL AND \"secureBridgeDnsLastUpdatedAt\" IS NULL AND \"secureBridgeCertificateExpiresAt\" IS NULL AND \"secureBridgeManagedDeviceSecret\" IS NULL AND \"mobileAccessToken\" IS NULL)")
        .bind(&config.id).bind(&app).bind(Url::parse(&app).unwrap().host_str()).bind(&config.service)
        .fetch_one(pool).await.map_err(|e| map_db_error(e, "lecture de la configuration Internet"))?;
    if !current {
        activate(pool, config).await?;
    }
    Ok(())
}

pub(super) async fn unconfigured_status(
    pool: &DbPool,
    settings: &secure::SecureBridgeSettings,
) -> Result<SecureBridgeStatus, String> {
    Ok(SecureBridgeStatus {
        enabled: settings.enabled,
        configured: false,
        active: false,
        domain: None,
        app_url: Some(DEFAULT_COMPANION_URL.into()),
        local_host: None,
        device_id: None,
        api_url: None,
        port: None,
        pairing_url: None,
        pairing_token_expires_at: None,
        certificate_expires_at: None,
        certificate_ready: false,
        dns_record_id: None,
        dns_last_updated_at: None,
        managed: true,
        managed_service_url: DEFAULT_REMOTE_RELAY_URL.into(),
        managed_credential_ready: false,
        passkeys: secure::relay_passkeys(pool).await?,
        last_error: if settings.enabled {
            Some(settings.last_error.clone().unwrap_or_else(|| {
                "Préparation du compagnon Internet en cours ; vérifiez la connexion de l’ordinateur.".into()
            }))
        } else {
            None
        },
        degraded: false,
    })
}

pub(super) async fn build_status(
    pool: &DbPool,
    config: &RelayConfig,
    enabled: bool,
    active: bool,
    pairing: Option<(String, String)>,
) -> Result<SecureBridgeStatus, String> {
    let api = format!("{}/relay/{}", config.service, config.id);
    let app = secure::load_settings(pool)
        .await?
        .app_url
        .unwrap_or(companion_url(config)?);
    let pairing = pairing.filter(|_| enabled);
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
    let settings = host.runtime.block_on(secure::load_settings(&host.pool));
    if !settings.as_ref().is_ok_and(|settings| settings.enabled) {
        return error_response(401, "Le compagnon distant est désactivé");
    }
    let origin = settings
        .as_ref()
        .ok()
        .and_then(secure::secure_app_origin)
        .unwrap_or_else(|| origin.into());
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
    headers.insert("origin".into(), origin.clone());
    http::handle_request(
        HttpRequest {
            method: request.method,
            path: request.path,
            headers,
            body: request.body.into_bytes(),
        },
        host,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MutationEvents(std::sync::Mutex<Vec<i64>>);

    impl crate::BridgeEvents for MutationEvents {
        fn data_changed(&self, version: i64) {
            self.0.lock().unwrap().push(version);
        }
        fn status_changed(&self) {}
    }

    fn finalized_session(engine: &dmx_core::Engine) -> (String, String) {
        let cookie = random_token();
        let csrf = random_token();
        // Test-only fixture for the state after a successful passkey ceremony. The
        // production handler still validates the session, CSRF and revocation.
        engine.block_on(async {
            sqlx::query("INSERT OR IGNORE INTO mobile_passkeys (id, credential_id, public_key, created_at) VALUES ('relay-mutation-passkey', 'test-credential', '{}', $1)")
                .bind(chrono::Utc::now().to_rfc3339()).execute(engine.pool()).await.unwrap();
            sqlx::query(
                "INSERT INTO mobile_sessions (id, session_hash, csrf_hash, passkey_id, device_label, expires_at, created_at)
                 VALUES ('relay-mutation-session', $1, $2, 'relay-mutation-passkey', 'Relay mutation test', $3, $4)",
            )
            .bind(secure::hash_secret(&cookie))
            .bind(secure::hash_secret(&csrf))
            .bind((chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339())
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(engine.pool())
            .await
            .unwrap();
        });
        (cookie, csrf)
    }

    fn request_packet(
        config: &RelayConfig,
        method: &str,
        path: &str,
        body: &str,
        headers: HashMap<String, String>,
    ) -> Envelope {
        let id = Uuid::new_v4().to_string();
        let clear = serde_json::to_vec(&json!({
            "id": id, "issuedAt": chrono::Utc::now().timestamp_millis(),
            "method": method, "path": path, "headers": headers, "body": body,
        }))
        .unwrap();
        let mut nonce = [0; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher(config, b"request")
            .unwrap()
            .encrypt(Nonce::from_slice(&nonce), clear.as_ref())
            .unwrap();
        Envelope {
            id,
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        }
    }

    fn response_payload(config: &RelayConfig, request_id: &str, encrypted: Envelope) -> serde_json::Value {
        assert_eq!(encrypted.id, request_id);
        let nonce = URL_SAFE_NO_PAD.decode(encrypted.nonce).unwrap();
        let ciphertext = URL_SAFE_NO_PAD.decode(encrypted.ciphertext).unwrap();
        let bytes = cipher(config, b"response")
            .unwrap()
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(result["id"], request_id);
        result
    }

    fn session_headers(cookie: &str, csrf: &str) -> HashMap<String, String> {
        HashMap::from([
            ("cookie".into(), format!("dmxmoney_session={cookie}")),
            ("x-dmx-csrf".into(), csrf.into()),
            ("content-type".into(), "application/json".into()),
        ])
    }

    async fn remote_request(client: &reqwest::Client, config: &RelayConfig, packet: &Envelope) -> serde_json::Value {
        let origin = std::env::var("DMXMONEY_RELAY_SMOKE_ORIGIN").unwrap_or_else(|_| config.service.clone());
        let response = client
            .post(format!("{}/relay/{}/request", config.service, config.id))
            .header("origin", &origin)
            .bearer_auth(hash(&config.key))
            .json(packet)
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            200,
            "relay must return an encrypted desktop response"
        );
        if std::env::var("DMXMONEY_RELAY_SMOKE_ORIGIN").is_ok() {
            assert_eq!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .and_then(|value| value.to_str().ok()),
                Some(origin.as_str())
            );
        }
        response_payload(config, &packet.id, response.json().await.unwrap())
    }

    #[test]
    fn encrypted_mutations_update_desktop_snapshot_and_emit_notifications() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        engine.block_on(schema(engine.pool())).unwrap();
        engine
            .block_on(
                sqlx::query("UPDATE settings SET \"secureBridgeEnabled\"=1, \"secureBridgeAppUrl\"='https://dmxmoney-companion.pages.dev/mobile/' WHERE id=1")
                    .execute(engine.pool()),
            )
            .unwrap();
        let (cookie, csrf) = finalized_session(&engine);
        let events = Arc::new(MutationEvents::default());
        let host = BridgeHost {
            pool: engine.pool().clone(),
            runtime: engine.handle(),
            data_dir: std::env::temp_dir(),
            assets_dir: None,
            events: events.clone(),
        };
        let config = RelayConfig {
            id: Uuid::new_v4().simple().to_string(),
            service: "https://example.com".into(),
            key: random_token(),
            desktop_token: random_token(),
        };
        let request = |method, path, body: &str, headers| {
            let packet = request_packet(&config, method, path, body, headers);
            let clear = decrypt(&config, &packet).unwrap();
            let response = dispatch(&host, clear, &config.service);
            response_payload(
                &config,
                &packet.id,
                serde_json::from_str(&encrypt(&config, &packet.id, response).unwrap()).unwrap(),
            )
        };
        let account = r##"{"id":"relay-account","name":"Remote","type":"Courant","initialBalance":100,"color":"#3b82f6","icon":"Wallet"}"##;
        let initial = engine.snapshot().unwrap();
        assert_eq!(
            request("POST", "/api/accounts", account, session_headers(&cookie, "wrong"))["status"],
            401
        );
        assert_eq!(engine.snapshot().unwrap().data_version, initial.data_version);
        assert!(events.0.lock().unwrap().is_empty());
        assert_eq!(
            request("POST", "/api/accounts", account, session_headers(&cookie, &csrf))["status"],
            201
        );
        assert!(engine
            .snapshot()
            .unwrap()
            .accounts
            .iter()
            .any(|account| account.id == "relay-account"));
        let transaction = r#"{"id":"relay-transaction","date":"2026-10-02","accountId":"relay-account","type":"expense","amount":12.5,"category":"5","description":"Sent from phone","checked":false}"#;
        assert_eq!(
            request(
                "POST",
                "/api/transactions",
                transaction,
                session_headers(&cookie, &csrf)
            )["status"],
            201
        );
        let after_create = engine.snapshot().unwrap();
        assert_eq!(
            after_create
                .transactions
                .iter()
                .find(|tx| tx.id == "relay-transaction")
                .unwrap()
                .description,
            "Sent from phone"
        );
        let edited = transaction.replace("Sent from phone", "Edited from phone");
        assert_eq!(
            request("PUT", "/api/transactions", &edited, session_headers(&cookie, &csrf))["status"],
            200
        );
        assert_eq!(
            engine
                .snapshot()
                .unwrap()
                .transactions
                .iter()
                .find(|tx| tx.id == "relay-transaction")
                .unwrap()
                .description,
            "Edited from phone"
        );
        assert_eq!(
            request(
                "DELETE",
                "/api/transactions/relay-transaction",
                "",
                session_headers(&cookie, &csrf)
            )["status"],
            200
        );
        assert!(engine
            .snapshot()
            .unwrap()
            .transactions
            .iter()
            .all(|tx| tx.id != "relay-transaction"));
        let versions = events.0.lock().unwrap();
        assert_eq!(versions.len(), 4, "each accepted mutation notifies the native desktop");
        assert!(versions.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(versions.last().copied(), Some(engine.snapshot().unwrap().data_version));
    }
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
    fn moving_the_same_device_to_pages_revokes_old_credentials_but_preserves_new_qr() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        let _ = finalized_session(&engine);
        let config = RelayConfig {
            id: Uuid::new_v4().simple().to_string(),
            service: DEFAULT_REMOTE_RELAY_URL.into(),
            key: random_token(),
            desktop_token: random_token(),
        };
        engine.block_on(async {
            schema(engine.pool()).await.unwrap();
            sqlx::query("UPDATE settings SET \"secureBridgeDeviceId\"=$1, \"secureBridgeAppUrl\"=$2 WHERE id=1")
                .bind(&config.id).bind(format!("{}/mobile/", config.service)).execute(engine.pool()).await.unwrap();
            sqlx::query("INSERT OR REPLACE INTO mobile_passkeys (id, credential_id, public_key, created_at) VALUES ('relay-mutation-passkey', 'old-worker-credential', '{}', $1)")
                .bind(chrono::Utc::now().to_rfc3339()).execute(engine.pool()).await.unwrap();
            let (old_qr, _) = secure::regenerate_pairing_token(engine.pool()).await.unwrap();
            activate(engine.pool(), &config).await.unwrap();
            let settings = secure::load_settings(engine.pool()).await.unwrap();
            assert_eq!(settings.device_id.as_deref(), Some(config.id.as_str()), "device identity is retained");
            assert_eq!(settings.app_url.as_deref(), Some(DEFAULT_COMPANION_URL));
            for statement in [
                "SELECT revoked_at FROM mobile_passkeys WHERE id='relay-mutation-passkey'",
                "SELECT revoked_at FROM mobile_sessions WHERE id='relay-mutation-session'",
            ] {
                let revoked: Option<String> = sqlx::query_scalar(statement).fetch_one(engine.pool()).await.unwrap();
                assert!(revoked.is_some(), "credentials for the old WebAuthn origin must be revoked");
            }
            let consumed: Option<String> = sqlx::query_scalar("SELECT consumed_at FROM mobile_pairing_tokens WHERE token_hash=$1")
                .bind(secure::hash_secret(&old_qr)).fetch_one(engine.pool()).await.unwrap();
            assert!(consumed.is_some());
            let (new_qr, expires) = secure::regenerate_pairing_token(engine.pool()).await.unwrap();
            activate(engine.pool(), &config).await.unwrap();
            let consumed: Option<String> = sqlx::query_scalar("SELECT consumed_at FROM mobile_pairing_tokens WHERE token_hash=$1")
                .bind(secure::hash_secret(&new_qr)).fetch_one(engine.pool()).await.unwrap();
            assert!(consumed.is_none(), "activation at the same Pages origin keeps the new QR usable");
            let status = build_status(engine.pool(), &config, true, true, Some((new_qr, expires))).await.unwrap();
            assert_eq!(status.app_url.as_deref(), Some(DEFAULT_COMPANION_URL));
            assert_eq!(status.api_url.as_deref(), Some(format!("{}/relay/{}", config.service, config.id).as_str()));
            assert!(status.pairing_url.as_ref().unwrap().starts_with(&format!("{DEFAULT_COMPANION_URL}#pairing=")));
        });
    }

    #[test]
    fn unavailable_internet_activation_preserves_legacy_settings_and_pairing() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        engine.block_on(async {
            sqlx::query("UPDATE settings SET \"secureBridgeEnabled\"=0, \"secureBridgeDeviceId\"='legacy-device', \"secureBridgeDomain\"='legacy.example.com', \"secureBridgeAppUrl\"='https://legacy.example.com/mobile/', \"secureBridgeLocalHost\"='pc.legacy.example.com', \"secureBridgeDnsRecordId\"='old-dns', \"secureBridgeLastError\"='previous-error' WHERE id=1")
                .execute(engine.pool()).await.unwrap();
            let (old_qr, _) = secure::regenerate_pairing_token(engine.pool()).await.unwrap();
            let before = secure::load_settings(engine.pool()).await.unwrap();
            let error = secure::set_enabled(engine.pool(), true).await.unwrap_err();
            assert!(error.contains("Service d’accès Internet indisponible"));
            let after = secure::load_settings(engine.pool()).await.unwrap();
            assert_eq!(after.enabled, before.enabled);
            assert_eq!(after.device_id, before.device_id);
            assert_eq!(after.app_url, before.app_url);
            assert_eq!(after.domain, before.domain);
            assert_eq!(after.local_host, before.local_host);
            assert_eq!(after.dns_record_id, before.dns_record_id);
            assert_eq!(after.last_error, before.last_error);
            let consumed: Option<String> = sqlx::query_scalar("SELECT consumed_at FROM mobile_pairing_tokens WHERE token_hash=$1")
                .bind(secure::hash_secret(&old_qr)).fetch_one(engine.pool()).await.unwrap();
            assert!(consumed.is_none(), "an unavailable relay must not invalidate existing pairing");
            let relays: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM remote_relay").fetch_one(engine.pool()).await.unwrap();
            assert_eq!(relays, 0);
        });
    }

    fn companion_for(engine: &dmx_core::Engine) -> Arc<MobileCompanion> {
        MobileCompanion::new(BridgeHost {
            pool: engine.pool().clone(),
            runtime: engine.handle(),
            data_dir: std::env::temp_dir().join("dmx-online-only-test"),
            assets_dir: None,
            events: Arc::new(crate::NoopEvents),
        })
    }

    fn configured_companion(engine: &dmx_core::Engine) -> Arc<MobileCompanion> {
        let config = RelayConfig {
            id: Uuid::new_v4().simple().to_string(),
            service: DEFAULT_REMOTE_RELAY_URL.into(),
            key: random_token(),
            desktop_token: random_token(),
        };
        save_secret(&config).unwrap();
        engine.block_on(async {
            schema(engine.pool()).await.unwrap();
            activate(engine.pool(), &config).await.unwrap();
            secure::set_enabled(engine.pool(), true).await.unwrap();
        });
        companion_for(engine)
    }

    #[test]
    fn old_local_settings_never_start_a_listener_and_financial_data_stays_intact() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        engine.block_on(async {
            sqlx::query("INSERT INTO accounts (id,name,type,\"initialBalance\",color,icon) VALUES ('kept','Kept','Courant',123.45,'#007AFF','Wallet')").execute(engine.pool()).await.unwrap();
            sqlx::query("UPDATE settings SET \"mobileAccessEnabled\"=1, \"secureBridgeEnabled\"=1, \"secureBridgeLocalHost\"='old.sync.example.com', \"secureBridgeAppUrl\"='https://old.example.com/mobile', \"secureBridgeDeviceId\"='old-device'").execute(engine.pool()).await.unwrap();
        });
        let companion = companion_for(&engine);
        companion.bootstrap().unwrap();
        let status = companion.status().unwrap();
        assert!(status.enabled);
        assert!(!status.active);
        assert!(status.host.is_none() && status.port.is_none());
        assert_eq!(status.url.as_deref(), Some(DEFAULT_COMPANION_URL));
        let bridge = status.secure_bridge.unwrap();
        assert!(!bridge.configured);
        assert!(bridge.local_host.is_none() && bridge.dns_record_id.is_none() && bridge.pairing_url.is_none());
        let snapshot = engine.snapshot().unwrap();
        let account = snapshot.accounts.iter().find(|account| account.id == "kept").unwrap();
        assert_eq!(account.name, "Kept");
        assert_eq!(account.initial_balance, 123.45);
        companion.shutdown();
    }

    #[test]
    fn desktop_restarts_preserve_the_pages_pairing_and_the_mobile_session() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        let companion = configured_companion(&engine);
        let (cookie, csrf) = finalized_session(&engine);
        let qr = companion
            .regenerate_secure_pairing_token()
            .unwrap()
            .secure_bridge
            .unwrap()
            .pairing_url
            .unwrap();
        assert_eq!(
            companion
                .status()
                .unwrap()
                .secure_bridge
                .unwrap()
                .pairing_url
                .as_deref(),
            Some(qr.as_str())
        );
        companion.shutdown();
        let restarted = companion_for(&engine);
        restarted.bootstrap().unwrap();
        assert_eq!(restarted.status().unwrap().url.as_deref(), Some(DEFAULT_COMPANION_URL));
        engine
            .block_on(secure::authorize_api_request(
                engine.pool(),
                "GET",
                "/api/accounts",
                &session_headers(&cookie, &csrf),
            ))
            .unwrap();
        restarted.shutdown();
    }

    #[test]
    fn a_consumed_cached_qr_disappears_and_disabled_or_closed_companions_cannot_restart() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        let companion = configured_companion(&engine);
        assert!(companion
            .regenerate_secure_pairing_token()
            .unwrap()
            .secure_bridge
            .unwrap()
            .pairing_url
            .is_some());
        engine
            .block_on(
                sqlx::query("UPDATE mobile_pairing_tokens SET consumed_at=$1")
                    .bind(chrono::Utc::now().to_rfc3339())
                    .execute(engine.pool()),
            )
            .unwrap();
        assert!(companion.status().unwrap().secure_bridge.unwrap().pairing_url.is_none());
        companion.regenerate_secure_pairing_token().unwrap();
        engine.block_on(async {
            let (_, disabled) = tokio::join!(
                companion.status_async(),
                companion.set_secure_bridge_enabled_async(false)
            );
            assert!(!disabled.unwrap().enabled);
        });
        let disabled = companion.status().unwrap();
        assert!(!disabled.enabled && !disabled.active);
        assert!(disabled.secure_bridge.unwrap().pairing_url.is_none());
        companion.set_secure_bridge_enabled(true).unwrap();
        companion.shutdown();
        let closed = companion.status().unwrap();
        assert!(!closed.enabled && !closed.active);
        assert!(closed.secure_bridge.unwrap().pairing_url.is_none());
        assert!(companion.set_secure_bridge_enabled(true).is_err());
    }

    #[test]
    fn disabling_does_not_require_the_relay_secret_to_remain_readable() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        let companion = configured_companion(&engine);
        companion.regenerate_secure_pairing_token().unwrap();
        let config = engine.block_on(load_config(engine.pool())).unwrap().unwrap();
        TEST_SECRETS.lock().unwrap().remove(&config.id);
        let disabled = companion.set_secure_bridge_enabled(false).unwrap();
        assert!(!disabled.enabled && !disabled.active);
        assert!(disabled.secure_bridge.unwrap().pairing_url.is_none());
        companion.shutdown();
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
        let events = Arc::new(MutationEvents::default());
        let config = RelayConfig {
            id: Uuid::new_v4().simple().to_string(),
            service: std::env::var("DMXMONEY_RELAY_SMOKE_URL").unwrap_or_else(|_| "http://127.0.0.1:8789".into()),
            key: random_token(),
            desktop_token: random_token(),
        };
        if config.service.starts_with("https://") {
            engine.block_on(activate(engine.pool(), &config)).unwrap();
        }
        if let Ok(origin) = std::env::var("DMXMONEY_RELAY_SMOKE_ORIGIN") {
            let origin = Url::parse(&origin).unwrap();
            assert_eq!(origin.scheme(), "https");
            assert_eq!(origin.path(), "/");
            engine
                .block_on(
                    sqlx::query("UPDATE settings SET \"secureBridgeAppUrl\"=$1 WHERE id=1")
                        .bind(format!("{}/mobile/", origin.origin().ascii_serialization()))
                        .execute(engine.pool()),
                )
                .unwrap();
        }
        let (pairing, _) = engine
            .block_on(secure::regenerate_pairing_token(engine.pool()))
            .unwrap();
        let (cookie, mut csrf) = finalized_session(&engine);
        let host = BridgeHost {
            pool: engine.pool().clone(),
            runtime: engine.handle(),
            data_dir: std::env::temp_dir(),
            assets_dir: None,
            events: events.clone(),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(false));
        engine.block_on(async {
            use futures_util::FutureExt;
            enroll(&config).await.unwrap();
            let task_host = host.clone(); let task_config = config.clone(); let task_stop = stop.clone(); let task_connected = connected.clone();
            let task = tokio::spawn(async move { run_connection(&task_host, &task_config, &task_stop, &task_connected).await });
            let client = reqwest::Client::builder().timeout(Duration::from_secs(35)).build().unwrap();
            // Clean up the test-only relay even if a network or mutation assertion fails.
            let outcome = std::panic::AssertUnwindSafe(async {
            for _ in 0..100 { if connected.load(Ordering::SeqCst) { break; } tokio::time::sleep(Duration::from_millis(50)).await; }
            assert!(connected.load(Ordering::SeqCst));
            let body = serde_json::to_string(&json!({ "token": pairing, "deviceLabel": "Live encrypted mutation test" })).unwrap();
            let packet = request_packet(&config, "POST", "/auth/pairing/start", &body, HashMap::new());
            let origin = std::env::var("DMXMONEY_RELAY_SMOKE_ORIGIN").unwrap_or_else(|_| config.service.clone());
            if std::env::var("DMXMONEY_RELAY_SMOKE_ORIGIN").is_ok() {
                let preflight = client.request(reqwest::Method::OPTIONS, format!("{}/relay/{}/request", config.service, config.id))
                    .header("origin", &origin).header("access-control-request-method", "POST")
                    .header("access-control-request-headers", "authorization, content-type").send().await.unwrap();
                assert_eq!(preflight.status().as_u16(), 204);
                assert_eq!(preflight.headers().get("access-control-allow-origin").and_then(|value| value.to_str().ok()), Some(origin.as_str()));
                assert_eq!(preflight.headers().get("access-control-allow-methods").and_then(|value| value.to_str().ok()), Some("POST"));
            }
            let unauthorized = client.post(format!("{}/relay/{}/request", config.service, config.id))
                .header("origin", origin).bearer_auth(random_token()).json(&packet).send().await.unwrap();
            assert_eq!(unauthorized.status().as_u16(), 401, "routing credential is checked before forwarding to desktop");
            let mut pending_cookie = String::new();
            for expected in [200, 409] {
                let result = remote_request(&client, &config, &packet).await;
                assert_eq!(result["status"], expected);
                if expected == 200 {
                    pending_cookie = result["headers"]["set-cookie"].as_str().unwrap().split(';').next().unwrap().trim_start_matches("dmxmoney_session=").into();
                    assert!(!pending_cookie.is_empty());
                }
            }
            let account = r##"{"id":"live-relay-account","name":"Live remote","type":"Courant","initialBalance":100,"color":"#3b82f6","icon":"Wallet"}"##;
            for headers in [session_headers(&pending_cookie, &csrf), session_headers(&cookie, "wrong-csrf")] {
                let denied = request_packet(&config, "POST", "/api/accounts", account, headers);
                assert_eq!(remote_request(&client, &config, &denied).await["status"], 401);
            }
            assert!(events.0.lock().unwrap().is_empty(), "denied mutations must not notify the desktop");
            for _ in 0..2 {
                let resumed=request_packet(&config,"POST","/auth/session","{}",session_headers(&cookie,&csrf));
                let response=remote_request(&client,&config,&resumed).await;
                assert_eq!(response["status"],200,"refresh restores the finalized session without a passkey ceremony");
                let body: serde_json::Value=serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
                assert_eq!(body["passkeyRequired"],false);
                csrf=body["csrfToken"].as_str().unwrap().into();
            }
            let created = request_packet(&config, "POST", "/api/accounts", account, session_headers(&cookie, &csrf));
            assert_eq!(remote_request(&client, &config, &created).await["status"], 201);
            let transaction = r#"{"id":"live-relay-transaction","date":"2026-10-02","accountId":"live-relay-account","type":"expense","amount":12.5,"category":"5","description":"Written over public relay","checked":false}"#;
            let created = request_packet(&config, "POST", "/api/transactions", transaction, session_headers(&cookie, &csrf));
            assert_eq!(remote_request(&client, &config, &created).await["status"], 201);
            let snapshot = tokio::task::block_in_place(|| engine.snapshot().unwrap());
            assert_eq!(snapshot.transactions.iter().find(|tx| tx.id == "live-relay-transaction").unwrap().description, "Written over public relay");
            let edited = transaction.replace("Written over public relay", "Edited over public relay");
            let updated = request_packet(&config, "PUT", "/api/transactions", &edited, session_headers(&cookie, &csrf));
            assert_eq!(remote_request(&client, &config, &updated).await["status"], 200);
            let read = request_packet(&config, "GET", "/api/transactions", "", session_headers(&cookie, &csrf));
            let result = remote_request(&client, &config, &read).await;
            assert_eq!(result["status"], 200);
            let remote_transactions: serde_json::Value = serde_json::from_str(result["body"].as_str().unwrap()).unwrap();
            assert_eq!(remote_transactions[0]["description"], "Edited over public relay");
            let snapshot = tokio::task::block_in_place(|| engine.snapshot().unwrap());
            assert_eq!(snapshot.transactions.iter().find(|tx| tx.id == "live-relay-transaction").unwrap().description, "Edited over public relay");
            let removed = request_packet(&config, "DELETE", "/api/transactions/live-relay-transaction", "", session_headers(&cookie, &csrf));
            assert_eq!(remote_request(&client, &config, &removed).await["status"], 200);
            let snapshot = tokio::task::block_in_place(|| engine.snapshot().unwrap());
            assert!(snapshot.transactions.iter().all(|tx| tx.id != "live-relay-transaction"));
            let versions = events.0.lock().unwrap().clone();
            assert_eq!(versions.len(), 4);
            assert!(versions.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(versions.last().copied(), Some(snapshot.data_version));
            secure::revoke_passkey(engine.pool(), "relay-mutation-passkey".into()).await.unwrap();
            let denied = request_packet(&config, "POST", "/api/transactions", transaction, session_headers(&cookie, &csrf));
            assert_eq!(remote_request(&client, &config, &denied).await["status"], 401);
            let revoked_resume=request_packet(&config,"POST","/auth/session","{}",session_headers(&cookie,&csrf));
            assert_eq!(remote_request(&client,&config,&revoked_resume).await["status"],401);
            assert_eq!(events.0.lock().unwrap().len(), 4);
            }).catch_unwind().await;
            stop.store(true, Ordering::SeqCst);
            task.abort();
            let _ = task.await;
            let cleanup = client.delete(format!("{}/relay/{}/enroll", config.service, config.id))
                .bearer_auth(&config.desktop_token).send().await.unwrap();
            assert_eq!(cleanup.status().as_u16(), 204);
            let removed = client.post(format!("{}/relay/{}/request", config.service, config.id))
                .bearer_auth(hash(&config.key)).json(&request_packet(&config, "GET", "/api/status", "", session_headers(&cookie, &csrf)))
                .send().await.unwrap();
            assert_eq!(removed.status().as_u16(), 404, "test enrollment is deleted from the public Worker");
            if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
        });
    }
}
