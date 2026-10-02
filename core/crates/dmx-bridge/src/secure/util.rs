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

pub(super) fn session_cookie(value: &str, expires_at: &str) -> (String, String) {
    let max_age = chrono::DateTime::parse_from_rfc3339(expires_at)
        .map(|expires| (expires.with_timezone(&Utc) - Utc::now()).num_seconds().max(0))
        .unwrap_or(0);
    (
        "Set-Cookie".to_string(),
        format!("{SESSION_COOKIE}={value}; Path=/; Max-Age={max_age}; Secure; HttpOnly; SameSite=Strict"),
    )
}

pub(super) fn clear_session_cookie() -> (String, String) {
    (
        "Set-Cookie".to_string(),
        format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; Secure; HttpOnly; SameSite=Strict"),
    )
}

pub(super) fn map_db_error(error: sqlx::Error, context: &str) -> String {
    let message = error.to_string();
    log::error!("Database Error during {context}: {message}");
    format!("Erreur BDD ({context}): {message}")
}
