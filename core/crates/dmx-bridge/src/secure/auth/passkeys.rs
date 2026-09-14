use super::*;

#[derive(Debug)]
pub(super) struct StoredCredential {
    pub(super) id: String,
    pub(super) passkey: PasskeyCredential,
}

pub(super) async fn insert_passkey(
    pool: &DbPool,
    passkey: &PasskeyCredential,
    device_label: Option<&str>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let public_key = serde_json::to_string(passkey).map_err(|error| error.to_string())?;
    sqlx::query(
        "INSERT INTO mobile_passkeys
            (id, credential_id, public_key, counter, device_label, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&id)
    .bind(passkey.id.to_b64url())
    .bind(public_key)
    .bind(i64::from(passkey.counter))
    .bind(device_label)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "enregistrement de la passkey mobile"))?;
    Ok(id)
}

/// Identifiants à exclure lors de l'enregistrement d'un mobile : seulement ceux du même appareil,
/// pour qu'un second téléphone partageant un trousseau (iCloud, Google) enregistre sa propre clé.
pub(super) async fn list_active_credentials_for_device(
    pool: &DbPool,
    device_label: &str,
) -> Result<Vec<CredentialId>, String> {
    let rows = sqlx::query(
        "SELECT public_key FROM mobile_passkeys
         WHERE revoked_at IS NULL AND device_label = $1
         ORDER BY created_at DESC",
    )
    .bind(device_label)
    .fetch_all(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture des passkeys de l’appareil"))?;

    rows.into_iter()
        .map(|row| {
            let value = row
                .try_get::<String, _>("public_key")
                .map_err(|error| error.to_string())?;
            serde_json::from_str::<PasskeyCredential>(&value)
                .map(|passkey| passkey.id)
                .map_err(|error| error.to_string())
        })
        .collect()
}

pub(super) async fn list_active_passkeys(pool: &DbPool) -> Result<Vec<PasskeyCredential>, String> {
    let rows = sqlx::query("SELECT public_key FROM mobile_passkeys WHERE revoked_at IS NULL ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|error| map_db_error(error, "lecture des passkeys mobiles"))?;

    rows.into_iter()
        .map(|row| {
            let value = row
                .try_get::<String, _>("public_key")
                .map_err(|error| error.to_string())?;
            serde_json::from_str::<PasskeyCredential>(&value).map_err(|error| error.to_string())
        })
        .collect()
}

pub(super) async fn find_passkey_by_credential_id(
    pool: &DbPool,
    credential_id: &str,
) -> Result<StoredCredential, String> {
    let row = sqlx::query(
        "SELECT id, public_key FROM mobile_passkeys
         WHERE credential_id = $1 AND revoked_at IS NULL",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture de passkey mobile"))?
    .ok_or_else(|| "Passkey mobile inconnue.".to_string())?;

    let id = row.try_get::<String, _>("id").map_err(|error| error.to_string())?;
    let public_key = row
        .try_get::<String, _>("public_key")
        .map_err(|error| error.to_string())?;
    let passkey = serde_json::from_str::<PasskeyCredential>(&public_key)
        .map_err(|error| format!("Passkey stockée invalide: {error}"))?;
    Ok(StoredCredential { id, passkey })
}

pub(super) async fn update_passkey_usage(
    pool: &DbPool,
    id: &str,
    counter: i64,
    device_label: Option<&str>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE mobile_passkeys
         SET counter = $1,
             last_used_at = $2,
             device_label = CASE
                WHEN $3 IS NOT NULL AND length($3) > 0
                     AND (device_label IS NULL OR length(device_label) = 0 OR device_label = 'Mobile')
                THEN $3
                ELSE device_label
             END
         WHERE id = $4",
    )
    .bind(counter)
    .bind(now)
    .bind(device_label)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "mise à jour de passkey mobile"))?;
    Ok(())
}

pub(in crate::secure) async fn list_passkeys(pool: &DbPool) -> Result<Vec<MobilePasskeyInfo>, String> {
    let rows = sqlx::query(
        "SELECT id, credential_id, device_label, created_at, last_used_at, revoked_at
         FROM mobile_passkeys ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture des passkeys mobiles"))?;

    Ok(rows
        .into_iter()
        .map(|row| MobilePasskeyInfo {
            id: row.try_get::<String, _>("id").unwrap_or_default(),
            credential_id: row.try_get::<String, _>("credential_id").unwrap_or_default(),
            device_label: row.try_get::<Option<String>, _>("device_label").unwrap_or(None),
            created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
            last_used_at: row.try_get::<Option<String>, _>("last_used_at").unwrap_or(None),
            revoked_at: row.try_get::<Option<String>, _>("revoked_at").unwrap_or(None),
        })
        .collect())
}
