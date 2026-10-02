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
