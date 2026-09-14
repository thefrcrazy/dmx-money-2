use super::*;

pub(super) fn generate_token(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn hash_secret(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

pub(super) fn constant_time_eq(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}

pub(super) fn normalize_domain(value: &str) -> Result<String, String> {
    let trimmed = value
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_matches('/');
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains(':') {
        return Err("Domaine invalide.".to_string());
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub(super) fn normalize_host(value: &str) -> Result<String, String> {
    let host = normalize_domain(value)?;
    if host.split('.').count() < 2 {
        return Err("Hôte local invalide.".to_string());
    }
    Ok(host)
}

pub(super) fn normalize_url(value: &str) -> Result<String, String> {
    let url = url::Url::parse(value.trim()).map_err(|_| "URL PWA invalide.".to_string())?;
    if url.scheme() != "https" {
        return Err("L’URL PWA doit être en HTTPS.".to_string());
    }
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub(super) fn parse_json<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("JSON invalide: {error}"))
}

pub(super) fn parse_json_or_default<T: for<'de> Deserialize<'de> + Default>(body: &[u8]) -> Result<T, String> {
    if body.is_empty() {
        return Ok(T::default());
    }
    parse_json(body)
}

pub(super) fn is_past(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc) <= Utc::now())
        .unwrap_or(true)
}

pub(super) fn extract_cookie(headers: &HashMap<String, String>, name: &str) -> Option<String> {
    headers.get("cookie").and_then(|cookie| {
        cookie.split(';').find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_string())
        })
    })
}

pub(super) fn session_cookie(value: &str) -> (String, String) {
    (
        "Set-Cookie".to_string(),
        format!(
            "{SESSION_COOKIE}={value}; Path=/; Max-Age={}; Secure; HttpOnly; SameSite=Strict",
            SESSION_TTL_MINUTES * 60
        ),
    )
}

pub(super) fn clear_session_cookie() -> (String, String) {
    (
        "Set-Cookie".to_string(),
        format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; Secure; HttpOnly; SameSite=Strict"),
    )
}

#[cfg(not(test))]
fn managed_device_secret_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_MANAGED_DEVICE_SECRET)
        .map_err(|error| format!("Trousseau système indisponible: {error}"))
}

#[cfg(not(test))]
pub(super) fn set_managed_device_secret(secret: &str) -> Result<(), String> {
    managed_device_secret_entry()?
        .set_password(secret)
        .map_err(|error| format!("Sauvegarde secret DmxMoney Bridge impossible: {error}"))
}

#[cfg(not(test))]
pub(super) fn get_managed_device_secret() -> Result<String, String> {
    managed_device_secret_entry()?.get_password().map_err(|error| {
        // Utile à journaliser : un trousseau verrouillé ou une application re-signée ressemble à
        // une entrée absente, et les deux se diagnostiquent très différemment.
        log::warn!("Managed bridge device secret unavailable from the keychain: {error}");
        format!("Secret DmxMoney Bridge absent du trousseau: {error}")
    })
}

// Les tests ne touchent jamais au trousseau réel : un binaire de test déclencherait une demande
// d'autorisation système et lirait le secret de l'appareil de l'utilisateur.
#[cfg(test)]
static TEST_DEVICE_SECRET: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
pub(super) fn set_managed_device_secret(secret: &str) -> Result<(), String> {
    *TEST_DEVICE_SECRET.lock().map_err(|error| error.to_string())? = Some(secret.to_string());
    Ok(())
}

#[cfg(test)]
pub(super) fn get_managed_device_secret() -> Result<String, String> {
    TEST_DEVICE_SECRET
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .ok_or_else(|| "Secret DmxMoney Bridge absent du trousseau: test".to_string())
}

#[cfg(test)]
pub(super) fn get_managed_registration_secret() -> Option<String> {
    None
}

#[cfg(not(test))]
fn managed_registration_secret_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_MANAGED_REGISTRATION_SECRET)
        .map_err(|error| format!("Trousseau système indisponible: {error}"))
}

#[cfg(not(test))]
pub(super) fn get_managed_registration_secret() -> Option<String> {
    option_env!("DMXMONEY_MANAGED_BRIDGE_REGISTRATION_SECRET")
        .map(str::trim)
        .filter(|secret| !secret.is_empty())
        .map(str::to_string)
        .or_else(|| {
            managed_registration_secret_entry()
                .ok()
                .and_then(|entry| entry.get_password().ok())
                .map(|secret| secret.trim().to_string())
                .filter(|secret| !secret.is_empty())
        })
}

pub(super) fn map_db_error(error: sqlx::Error, context: &str) -> String {
    let message = error.to_string();
    log::error!("Database Error during {context}: {message}");
    format!("Erreur BDD ({context}): {message}")
}
