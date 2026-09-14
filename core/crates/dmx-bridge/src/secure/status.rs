use super::*;

pub async fn build_status(
    pool: &DbPool,
    data_dir: &Path,
    active: bool,
    port: Option<u16>,
    pairing_preview: Option<(String, String)>,
    // L'application embarque le client PWA : le mobile le charge alors depuis le pont local,
    // et non depuis la PWA publique (celle déployée sur le Worker).
    serves_local_pwa: bool,
) -> Result<SecureBridgeStatus, String> {
    let settings = load_settings(pool).await?;
    let managed_credential_ready = has_managed_device_secret();
    // Un certificat expiré n'est pas « prêt » : les fichiers sont là mais chaque mobile échoue la
    // négociation TLS. Une expiration inconnue reste acceptée pour ne pas bloquer une installation
    // réparée sans ce champ.
    let certificate_expired = settings.certificate_expires_at.as_deref().map(is_past).unwrap_or(false);
    let certificate_ready = !certificate_expired
        && certificate_paths(data_dir, settings.device_id.as_deref())
            .map(|paths| paths.cert.exists() && paths.key.exists())
            .unwrap_or(false);
    let non_empty = |value: Option<&String>| value.map(|value| !value.is_empty()).unwrap_or(false);
    let configured = non_empty(settings.domain.as_ref())
        && non_empty(settings.app_url.as_ref())
        && non_empty(settings.local_host.as_ref())
        && (managed_credential_ready || certificate_ready);
    let api_url = settings
        .local_host
        .as_ref()
        .zip(port)
        .map(|(host, port)| format!("https://{host}:{port}"));
    // Origine de la PWA : le pont local quand l'app embarque le client, sinon l'URL publique.
    let pwa_origin = if serves_local_pwa {
        api_url.clone().or_else(|| settings.app_url.clone())
    } else {
        settings.app_url.clone()
    };
    let pairing_url = pwa_origin
        .as_ref()
        .zip(api_url.as_ref())
        .zip(pairing_preview.as_ref())
        .map(|((app_url, api_url), (raw, _))| format!("{app_url}#pairing={raw}&api={api_url}"));

    let active = settings.enabled && active && certificate_ready;
    // Un renouvellement impossible mérite d'être affiché, mais pas comme une panne : le QR et les
    // mobiles appairés continuent de fonctionner avec le certificat actuel.
    let last_error = settings.last_error;
    let degraded = active
        && last_error
            .as_deref()
            .map(|error| error.contains("Le pont local continue de fonctionner"))
            .unwrap_or(false);

    Ok(SecureBridgeStatus {
        enabled: settings.enabled,
        configured,
        active,
        domain: settings.domain,
        app_url: pwa_origin,
        local_host: settings.local_host,
        device_id: settings.device_id,
        api_url,
        port,
        pairing_url,
        pairing_token_expires_at: pairing_preview.map(|(_, expires_at)| expires_at),
        certificate_expires_at: settings.certificate_expires_at,
        certificate_ready,
        dns_record_id: settings.dns_record_id,
        dns_last_updated_at: settings.dns_last_updated_at,
        managed: true,
        managed_service_url: managed_service_base_url(settings.managed_service_url.as_deref()),
        managed_credential_ready,
        passkeys: list_passkeys(pool).await?,
        last_error,
        degraded,
    })
}
