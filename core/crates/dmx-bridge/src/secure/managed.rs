use super::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedRegisterRequest {
    existing_device_id: Option<String>,
    local_ip: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedRegisterResponse {
    pub(super) device_id: String,
    pub(super) device_secret: String,
    pub(super) domain: String,
    pub(super) app_url: String,
    pub(super) local_host: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedDnsUpdateRequest<'a> {
    local_ip: &'a str,
    local_host: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedDnsUpdateResponse {
    record_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedTxtRequest<'a> {
    name: &'a str,
    value: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedTxtDeleteRequest<'a> {
    record_id: &'a str,
}

pub(super) fn managed_service_base_url(stored: Option<&str>) -> String {
    option_env!("DMXMONEY_MANAGED_BRIDGE_URL")
        .and_then(normalize_managed_service_url)
        .or_else(|| stored.and_then(normalize_managed_service_url))
        .unwrap_or(DEFAULT_MANAGED_SERVICE_URL)
        .to_string()
}

fn normalize_managed_service_url(value: &str) -> Option<&str> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed == LEGACY_MANAGED_SERVICE_URL {
        None
    } else {
        Some(trimmed)
    }
}

pub(super) async fn managed_register_device(
    existing_device_id: Option<&str>,
) -> Result<ManagedRegisterResponse, String> {
    let base_url = managed_service_base_url(None);
    let existing_device_id = existing_device_id.filter(|value| !value.trim().is_empty());

    // Un appareil déjà enregistré prouve qu'il en est propriétaire avec son propre secret : le
    // secret d'enregistrement partagé est renouvelé de temps en temps et une installation qui en
    // porterait un ancien serait sinon bloquée hors de son propre pont.
    let mut credentials: Vec<String> = Vec::new();
    if existing_device_id.is_some() {
        if let Ok(secret) = get_available_managed_device_secret() {
            credentials.push(secret);
        }
    }
    if let Some(secret) = get_managed_registration_secret() {
        if !credentials.contains(&secret) {
            credentials.push(secret);
        }
    }
    if credentials.is_empty() {
        credentials.push(String::new());
    }

    let mut last_error = None;
    for credential in credentials {
        let payload = ManagedRegisterRequest {
            existing_device_id: existing_device_id.map(str::to_string),
            local_ip: crate::companion::detect_local_ip(),
        };
        let mut request = reqwest::Client::new()
            .post(format!("{base_url}/v1/devices/register"))
            .json(&payload);
        if !credential.is_empty() {
            request = request.bearer_auth(&credential);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Service DmxMoney Bridge indisponible ({base_url}): {error}"))?;

        let rejected = matches!(response.status().as_u16(), 401 | 403);
        match parse_managed_response::<ManagedRegisterResponse>(response, "Provisionnement DmxMoney Bridge").await {
            Ok(registration) => return validate_registration(registration),
            // Seul un refus d'authentification justifie d'essayer un autre secret.
            Err(error) if rejected => last_error = Some(error),
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| "Provisionnement DmxMoney Bridge refusé.".to_string()))
}

fn validate_registration(registration: ManagedRegisterResponse) -> Result<ManagedRegisterResponse, String> {
    if [
        &registration.device_id,
        &registration.device_secret,
        &registration.domain,
        &registration.app_url,
        &registration.local_host,
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err("Provisionnement DmxMoney Bridge incomplet.".to_string());
    }
    Ok(registration)
}

pub(super) async fn managed_update_dns(
    settings: &SecureBridgeSettings,
    local_host: &str,
    local_ip: &str,
) -> Result<String, String> {
    let (base_url, device_id, secret) = managed_credentials(settings)?;
    let response = reqwest::Client::new()
        .post(format!("{base_url}/v1/devices/{device_id}/dns"))
        .bearer_auth(secret)
        .json(&ManagedDnsUpdateRequest { local_ip, local_host })
        .send()
        .await
        .map_err(|error| format!("Mise à jour DNS DmxMoney Bridge impossible: {error}"))?;
    parse_managed_response::<ManagedDnsUpdateResponse>(response, "Mise à jour DNS DmxMoney Bridge")
        .await
        .map(|record| record.record_id)
}

pub(super) async fn managed_present_txt(
    settings: &SecureBridgeSettings,
    name: &str,
    value: &str,
) -> Result<String, String> {
    let (base_url, device_id, secret) = managed_credentials(settings)?;
    let response = reqwest::Client::new()
        .post(format!("{base_url}/v1/devices/{device_id}/acme/txt"))
        .bearer_auth(secret)
        .json(&ManagedTxtRequest { name, value })
        .send()
        .await
        .map_err(|error| format!("Création TXT ACME DmxMoney Bridge impossible: {error}"))?;
    parse_managed_response::<ManagedDnsUpdateResponse>(response, "Création TXT ACME DmxMoney Bridge")
        .await
        .map(|record| record.record_id)
}

pub(super) async fn managed_delete_txt(settings: &SecureBridgeSettings, record_id: &str) -> Result<(), String> {
    let (base_url, device_id, secret) = managed_credentials(settings)?;
    let response = reqwest::Client::new()
        .post(format!("{base_url}/v1/devices/{device_id}/acme/txt/delete"))
        .bearer_auth(secret)
        .json(&ManagedTxtDeleteRequest { record_id })
        .send()
        .await
        .map_err(|error| format!("Suppression TXT ACME DmxMoney Bridge impossible: {error}"))?;
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Suppression TXT ACME DmxMoney Bridge refusée ({status}): {body}"
        ))
    }
}

fn managed_credentials(settings: &SecureBridgeSettings) -> Result<(String, String, String), String> {
    let base_url = managed_service_base_url(settings.managed_service_url.as_deref());
    let device_id = settings
        .device_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Device ID DmxMoney Bridge manquant.".to_string())?
        .to_string();
    let secret = get_available_managed_device_secret()?;
    Ok((base_url, device_id, secret))
}

pub(super) fn has_managed_device_secret() -> bool {
    get_available_managed_device_secret()
        .map(|secret| !secret.is_empty())
        .unwrap_or(false)
}

fn get_available_managed_device_secret() -> Result<String, String> {
    get_managed_device_secret().and_then(|secret| {
        if secret.is_empty() {
            Err("Secret DmxMoney Bridge vide.".to_string())
        } else {
            Ok(secret)
        }
    })
}

pub(super) fn is_missing_managed_secret_error(error: &str) -> bool {
    error.contains("Secret DmxMoney Bridge absent")
        || error.contains("Secret DmxMoney Bridge vide")
        || error.contains("Trousseau système indisponible")
        || error.contains("No matching entry found in secure storage")
}

async fn parse_managed_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    context: &str,
) -> Result<T, String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Réponse DmxMoney Bridge illisible: {error}"))?;
    if !status.is_success() {
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(format!(
                "{context} refusé ({status}): cette installation n’est plus autorisée par le service \
                 DmxMoney Bridge. Mets à jour l’application desktop pour récupérer un accès valide."
            ));
        }
        return Err(format!("{context} refusé ({status}): {body}"));
    }
    serde_json::from_str(&body).map_err(|error| format!("Réponse DmxMoney Bridge invalide: {error}"))
}
