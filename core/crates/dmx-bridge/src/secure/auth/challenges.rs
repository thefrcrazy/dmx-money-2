use super::*;

pub(super) async fn store_challenge<T: Serialize>(
    pool: &DbPool,
    kind: &str,
    state: &T,
    session_id: Option<&str>,
    device_label: Option<&str>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + ChronoDuration::minutes(CHALLENGE_TTL_MINUTES)).to_rfc3339();
    let state_json = serde_json::to_string(state).map_err(|error| error.to_string())?;
    sqlx::query(
        "INSERT INTO mobile_auth_challenges
            (id, kind, state_json, session_id, device_label, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&id)
    .bind(kind)
    .bind(state_json)
    .bind(session_id)
    .bind(device_label)
    .bind(expires_at)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "création du challenge passkey"))?;
    Ok(id)
}

pub(super) async fn load_challenge<T: for<'de> Deserialize<'de>>(
    pool: &DbPool,
    id: &str,
    kind: &str,
    session_id: Option<&str>,
) -> Result<T, String> {
    let row = sqlx::query(
        "SELECT state_json, expires_at, session_id
         FROM mobile_auth_challenges WHERE id = $1 AND kind = $2",
    )
    .bind(id)
    .bind(kind)
    .fetch_optional(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture du challenge passkey"))?
    .ok_or_else(|| "Challenge passkey introuvable.".to_string())?;

    let expires_at = row
        .try_get::<String, _>("expires_at")
        .map_err(|error| error.to_string())?;
    if is_past(&expires_at) {
        return Err("Challenge passkey expiré.".to_string());
    }
    let stored_session = row.try_get::<Option<String>, _>("session_id").unwrap_or(None);
    if stored_session.as_deref() != session_id {
        return Err("Challenge passkey non autorisé.".to_string());
    }
    let state_json = row
        .try_get::<String, _>("state_json")
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&state_json).map_err(|error| error.to_string())
}

pub(super) async fn delete_challenge(pool: &DbPool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM mobile_auth_challenges WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "suppression du challenge passkey"))?;
    Ok(())
}
