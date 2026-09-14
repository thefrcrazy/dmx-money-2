use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileCompanionStatus {
    pub enabled: bool,
    pub active: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub url: Option<String>,
    pub data_version: i64,
    pub secure_bridge: Option<SecureBridgeStatus>,
}

#[derive(Debug, Clone)]
pub(super) struct MobileSettings {
    pub(super) enabled: bool,
    pub(super) port: u16,
}

#[derive(Clone)]
pub(super) struct ServerRuntime {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) url: String,
    pub(super) secure: bool,
    /// Identité du matériel TLS au démarrage du serveur : un certificat renouvelé ou émis pour un
    /// autre hôte redémarre le serveur au lieu de servir une chaîne expirée.
    pub(super) tls_fingerprint: Option<String>,
    pub(super) stop: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(super) struct ServerSecurity {
    pub(super) secure_app_origin: Option<String>,
    pub(super) app_url: Option<String>,
}
