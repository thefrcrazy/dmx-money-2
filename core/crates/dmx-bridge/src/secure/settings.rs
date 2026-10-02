use super::*;

pub async fn set_enabled(pool: &DbPool, enabled: bool) -> Result<(), String> {
    if enabled {
        crate::companion::relay::enable_internet(pool).await?;
    }

    sqlx::query("UPDATE settings SET \"secureBridgeEnabled\" = $1, \"mobileAccessEnabled\" = $1, \"mobileAccessToken\" = NULL WHERE id = 1")
        .bind(enabled)
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "activation du pont sécurisé"))?;

    Ok(())
}

pub fn secure_app_origin(settings: &SecureBridgeSettings) -> Option<String> {
    settings
        .app_url
        .as_deref()
        .and_then(|url| url::Url::parse(url).ok())
        .map(|url| {
            let scheme = url.scheme();
            let host = url.host_str().unwrap_or_default();
            match url.port() {
                Some(port) => format!("{scheme}://{host}:{port}"),
                None => format!("{scheme}://{host}"),
            }
        })
}

pub async fn load_settings(pool: &DbPool) -> Result<SecureBridgeSettings, String> {
    let row = sqlx::query(
        "SELECT
            (COALESCE(\"secureBridgeEnabled\", 0) OR COALESCE(\"mobileAccessEnabled\", 0)) AS enabled,
            \"secureBridgeDomain\" AS domain,
            \"secureBridgeAppUrl\" AS app_url,
            \"secureBridgeLocalHost\" AS local_host,
            \"secureBridgeDeviceId\" AS device_id,
            \"secureBridgeCertificateExpiresAt\" AS certificate_expires_at,
            \"secureBridgeDnsRecordId\" AS dns_record_id,
            \"secureBridgeDnsLastUpdatedAt\" AS dns_last_updated_at,
            \"secureBridgeLastError\" AS last_error,
            \"secureBridgeManagedServiceUrl\" AS managed_service_url
         FROM settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture des paramètres du pont sécurisé"))?;

    let text = |column: &str| row.try_get::<Option<String>, _>(column).unwrap_or(None);
    Ok(SecureBridgeSettings {
        enabled: row.try_get::<i64, _>("enabled").unwrap_or(0) != 0,
        domain: text("domain"),
        app_url: text("app_url"),
        local_host: text("local_host"),
        device_id: text("device_id"),
        certificate_expires_at: text("certificate_expires_at"),
        dns_record_id: text("dns_record_id"),
        dns_last_updated_at: text("dns_last_updated_at"),
        last_error: text("last_error"),
        managed_service_url: text("managed_service_url"),
    })
}
