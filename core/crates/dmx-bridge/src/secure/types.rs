use super::*;

#[derive(Debug, Clone)]
pub struct SecureBridgeSettings {
    pub enabled: bool,
    pub domain: Option<String>,
    pub app_url: Option<String>,
    pub local_host: Option<String>,
    pub device_id: Option<String>,
    pub certificate_expires_at: Option<String>,
    pub dns_record_id: Option<String>,
    pub dns_last_updated_at: Option<String>,
    pub last_error: Option<String>,
    pub managed_service_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobilePasskeyInfo {
    pub id: String,
    pub credential_id: String,
    pub device_label: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecureBridgeStatus {
    pub enabled: bool,
    pub configured: bool,
    pub active: bool,
    pub domain: Option<String>,
    pub app_url: Option<String>,
    pub local_host: Option<String>,
    pub device_id: Option<String>,
    pub api_url: Option<String>,
    pub port: Option<u16>,
    pub pairing_url: Option<String>,
    pub pairing_token_expires_at: Option<String>,
    pub certificate_expires_at: Option<String>,
    pub certificate_ready: bool,
    pub dns_record_id: Option<String>,
    pub dns_last_updated_at: Option<String>,
    pub managed: bool,
    pub managed_service_url: String,
    pub managed_credential_ready: bool,
    pub passkeys: Vec<MobilePasskeyInfo>,
    pub last_error: Option<String>,
    /// Le pont sert encore les mobiles appairés, mais le renouvellement managé échoue.
    pub degraded: bool,
}

#[derive(Debug)]
pub struct AuthRouteOutput {
    pub status: u16,
    pub body: Value,
    pub headers: Vec<(String, String)>,
}
