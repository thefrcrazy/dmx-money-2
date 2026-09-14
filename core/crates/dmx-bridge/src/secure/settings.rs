use super::*;

pub async fn ensure_auto_configuration(pool: &DbPool, data_dir: &Path) -> Result<(), String> {
    let current = load_settings(pool).await?;
    let has_secret = has_managed_device_secret();
    let has_valid_certificate = current
        .certificate_expires_at
        .as_deref()
        .map(|value| !is_past(value))
        .unwrap_or(false);

    if has_bridge_identity(&current) && (has_secret || has_valid_certificate) {
        clear_last_error(pool).await;
        return Ok(());
    }

    // Un appareil enregistré garde son identité : le nom d'hôte local dérive de son identifiant,
    // donc DNS, TLS et appairage continuent de fonctionner même si le secret du trousseau a été
    // perdu. Se réenregistrer ici donnerait un nouvel identifiant et orphelinerait le certificat.
    if has_device_id(&current) {
        repair_managed_configuration(pool, &current).await?;
        return Ok(());
    }

    match provision_managed_device(pool, &current, true).await {
        Ok(()) => Ok(()),
        Err(error) if can_serve_locally(data_dir, &current) => {
            log::warn!("Secure bridge provisioning refused, keeping the local bridge: {error}");
            record_last_error(pool, &degraded_provisioning_message(&error)).await;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub(super) fn has_bridge_identity(settings: &SecureBridgeSettings) -> bool {
    [
        settings.domain.as_deref(),
        settings.app_url.as_deref(),
        settings.local_host.as_deref(),
    ]
    .into_iter()
    .all(|value| value.map(|value| !value.trim().is_empty()).unwrap_or(false))
}

pub(super) fn has_device_id(settings: &SecureBridgeSettings) -> bool {
    settings
        .device_id
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

/// Le desktop peut continuer à répondre seul aux mobiles appairés : le matériel TLS est sur disque
/// et l'hôte pour lequel il a été émis est connu.
pub(super) fn can_serve_locally(data_dir: &Path, settings: &SecureBridgeSettings) -> bool {
    has_device_id(settings)
        && settings
            .local_host
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        && certificate_paths(data_dir, settings.device_id.as_deref())
            .map(|paths| paths.cert.exists() && paths.key.exists())
            .unwrap_or(false)
}

pub(super) fn degraded_provisioning_message(error: &str) -> String {
    format!(
        "Renouvellement DmxMoney Bridge indisponible ({error}). \
         Le pont local continue de fonctionner avec le certificat actuel."
    )
}

pub(super) async fn clear_last_error(pool: &DbPool) {
    let _ = sqlx::query(
        "UPDATE settings SET \"secureBridgeLastError\" = NULL WHERE id = 1 AND \"secureBridgeLastError\" IS NOT NULL",
    )
    .execute(pool)
    .await;
}

pub(super) async fn record_last_error(pool: &DbPool, error: &str) {
    let _ = sqlx::query("UPDATE settings SET \"secureBridgeLastError\" = $1 WHERE id = 1")
        .bind(error)
        .execute(pool)
        .await;
}

async fn repair_managed_configuration(pool: &DbPool, current: &SecureBridgeSettings) -> Result<(), String> {
    let device_id = current
        .device_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Device ID DmxMoney Bridge manquant.".to_string())?;
    let domain = current
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_domain)
        .transpose()?
        .unwrap_or_else(|| DEFAULT_BRIDGE_DOMAIN.to_string());
    let app_url = current
        .app_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_url)
        .transpose()?
        .unwrap_or_else(|| {
            format!(
                "{}/mobile",
                managed_service_base_url(current.managed_service_url.as_deref())
            )
        });
    let local_host = current
        .local_host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_host)
        .transpose()?
        .unwrap_or_else(|| format!("{DEFAULT_DEVICE_PREFIX}-{device_id}.sync.{domain}"));

    sqlx::query(
        "UPDATE settings SET
            \"secureBridgeDomain\" = $1,
            \"secureBridgeAppUrl\" = $2,
            \"secureBridgeLocalHost\" = $3,
            \"secureBridgeManagedServiceUrl\" = $4,
            \"secureBridgeLastError\" = NULL
         WHERE id = 1",
    )
    .bind(domain)
    .bind(app_url)
    .bind(local_host)
    .bind(managed_service_base_url(current.managed_service_url.as_deref()))
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "réparation du pont sécurisé"))?;

    Ok(())
}

pub(super) async fn provision_managed_device(
    pool: &DbPool,
    current: &SecureBridgeSettings,
    preserve_existing_device: bool,
) -> Result<(), String> {
    let registration = managed_register_device(if preserve_existing_device {
        current.device_id.as_deref()
    } else {
        None
    })
    .await?;
    let domain = normalize_domain(&registration.domain)?;
    let app_url = normalize_url(&registration.app_url)?;
    let local_host = normalize_host(&registration.local_host)?;
    set_managed_device_secret(&registration.device_secret)
        .and_then(|_| get_managed_device_secret().map(|secret| !secret.is_empty()))
        .map_err(|error| format!("Secret DmxMoney Bridge non sauvegardé dans le trousseau: {error}"))?;

    // Le service conserve les DNS d'un appareil qui se réenregistre avec le même identifiant et le
    // certificat sur disque reste valable pour le même hôte : seule une nouvelle identité invalide
    // l'enregistrement DNS et la date d'expiration.
    let identity_changed = current.device_id.as_deref().map(str::trim) != Some(registration.device_id.as_str())
        || current.local_host.as_deref().map(str::trim) != Some(local_host.as_str());

    sqlx::query(
        "UPDATE settings SET
            \"secureBridgeDomain\" = $1,
            \"secureBridgeAppUrl\" = $2,
            \"secureBridgeLocalHost\" = $3,
            \"secureBridgeDeviceId\" = $4,
            \"secureBridgeManagedServiceUrl\" = $5,
            \"secureBridgeManagedRegisteredAt\" = $6,
            \"secureBridgeDnsRecordId\" = CASE WHEN $7 THEN NULL ELSE \"secureBridgeDnsRecordId\" END,
            \"secureBridgeCertificateExpiresAt\" = CASE WHEN $7 THEN NULL ELSE \"secureBridgeCertificateExpiresAt\" END,
            \"secureBridgeManagedDeviceSecret\" = NULL,
            \"secureBridgeLastError\" = NULL
         WHERE id = 1",
    )
    .bind(domain)
    .bind(app_url)
    .bind(local_host)
    .bind(registration.device_id)
    .bind(managed_service_base_url(current.managed_service_url.as_deref()))
    .bind(Utc::now().to_rfc3339())
    .bind(identity_changed)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "provisionnement automatique du pont sécurisé"))?;

    Ok(())
}

pub async fn set_enabled(pool: &DbPool, data_dir: &Path, enabled: bool) -> Result<(), String> {
    log::info!("Secure bridge set_enabled requested: enabled={enabled}");
    if enabled {
        ensure_auto_configuration(pool, data_dir).await?;
    }

    sqlx::query("UPDATE settings SET \"secureBridgeEnabled\" = $1 WHERE id = 1")
        .bind(enabled)
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "activation du pont sécurisé"))?;

    log::info!("Secure bridge enabled flag saved: enabled={enabled}");
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
            COALESCE(\"secureBridgeEnabled\", 0) AS enabled,
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
