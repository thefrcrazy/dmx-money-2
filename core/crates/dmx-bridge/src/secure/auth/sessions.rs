use super::*;

#[derive(Debug)]
pub(super) struct SessionTokens {
    raw_session: String,
    raw_csrf: String,
    expires_at: String,
}

#[derive(Debug)]
pub(super) struct AuthorizedSession {
    pub(super) id: String,
}

pub(super) async fn create_session(
    pool: &DbPool,
    passkey_id: Option<String>,
    device_label: Option<String>,
) -> Result<SessionTokens, String> {
    let raw_session = generate_token(32);
    let raw_csrf = generate_token(32);
    let now = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + ChronoDuration::minutes(SESSION_TTL_MINUTES)).to_rfc3339();
    sqlx::query(
        "INSERT INTO mobile_sessions
            (id, session_hash, csrf_hash, passkey_id, device_label, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(hash_secret(&raw_session))
    .bind(hash_secret(&raw_csrf))
    .bind(passkey_id)
    .bind(device_label)
    .bind(&expires_at)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "création de session mobile"))?;

    Ok(SessionTokens {
        raw_session,
        raw_csrf,
        expires_at,
    })
}

pub(super) fn session_response(session: SessionTokens, passkey_required: bool) -> AuthRouteOutput {
    AuthRouteOutput {
        status: 200,
        body: json!({
            "ok": true,
            "csrfToken": session.raw_csrf,
            "expiresAt": session.expires_at,
            "passkeyRequired": passkey_required,
        }),
        headers: vec![session_cookie(&session.raw_session)],
    }
}

pub(super) async fn authorize_session_for_auth(
    pool: &DbPool,
    headers: &HashMap<String, String>,
) -> Result<AuthorizedSession, String> {
    let session = extract_cookie(headers, SESSION_COOKIE).ok_or_else(|| "Session de pairing manquante.".to_string())?;
    let row = sqlx::query("SELECT id, expires_at, revoked_at FROM mobile_sessions WHERE session_hash = $1")
        .bind(hash_secret(&session))
        .fetch_optional(pool)
        .await
        .map_err(|error| map_db_error(error, "lecture de session mobile"))?
        .ok_or_else(|| "Session de pairing invalide.".to_string())?;

    let expires_at = row
        .try_get::<String, _>("expires_at")
        .map_err(|error| error.to_string())?;
    let revoked_at = row.try_get::<Option<String>, _>("revoked_at").unwrap_or(None);
    if revoked_at.is_some() || is_past(&expires_at) {
        return Err("Session de pairing expirée.".to_string());
    }

    Ok(AuthorizedSession {
        id: row.try_get::<String, _>("id").map_err(|error| error.to_string())?,
    })
}

pub(super) async fn revoke_session(pool: &DbPool, id: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE mobile_sessions SET revoked_at = $1 WHERE id = $2")
        .bind(now)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "révocation de session mobile"))?;
    Ok(())
}
