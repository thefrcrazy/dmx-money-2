//! Pont HTTPS sécurisé : passkeys, sessions, certificats ACME et service managé Cloudflare
//! (port de `src-tauri/src/secure_bridge`).

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration as ChronoDuration, Utc};
use dmx_core::db::DbPool;
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewAccount, NewOrder, OrderStatus,
};
use passkey_auth::{
    AuthenticationResponse, AuthenticationState, CredentialId, PasskeyCredential, RegistrationResponse,
    RegistrationState, Webauthn,
};
use rand::{rngs::OsRng, RngCore};
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Service du trousseau conservé depuis 1.x : l'appareil et son sous-domaine sont repris tels quels.
#[cfg_attr(test, allow(dead_code))]
const KEYCHAIN_SERVICE: &str = "DmxMoney Secure Bridge";
#[cfg_attr(test, allow(dead_code))]
const KEYCHAIN_MANAGED_DEVICE_SECRET: &str = "managed-device-secret";
#[cfg_attr(test, allow(dead_code))]
const KEYCHAIN_MANAGED_REGISTRATION_SECRET: &str = "managed-registration-secret";
const SESSION_COOKIE: &str = "dmxmoney_session";
const PAIRING_TTL_MINUTES: i64 = 10;
const SESSION_TTL_MINUTES: i64 = 45;
const CHALLENGE_TTL_MINUTES: i64 = 5;
const CERT_RENEW_WINDOW_DAYS: i64 = 30;
const DEFAULT_MANAGED_SERVICE_URL: &str = "https://dmxmoney.develop-max.com";
const LEGACY_MANAGED_SERVICE_URL: &str = "https://bridge.dmxmoney.app";
const DEFAULT_BRIDGE_DOMAIN: &str = "develop-max.com";
const DEFAULT_DEVICE_PREFIX: &str = "dmx";

mod auth;
mod certificates;
mod managed;
mod settings;
mod status;
mod types;
mod util;

pub use self::auth::{authorize_api_request, handle_auth_request, regenerate_pairing_token, revoke_passkey};
pub use self::certificates::{certificate_fingerprint, load_tls_config, refresh_infrastructure};
pub use self::settings::{ensure_auto_configuration, load_settings, secure_app_origin, set_enabled};
pub use self::status::build_status;
pub use self::types::{AuthRouteOutput, MobilePasskeyInfo, SecureBridgeSettings, SecureBridgeStatus};

#[cfg(test)]
pub(crate) use self::util::hash_secret;

use self::auth::list_passkeys;
use self::certificates::certificate_paths;
use self::managed::{
    has_managed_device_secret, is_missing_managed_secret_error, managed_delete_txt, managed_present_txt,
    managed_register_device, managed_service_base_url, managed_update_dns,
};
use self::settings::{
    can_serve_locally, clear_last_error, degraded_provisioning_message, provision_managed_device, record_last_error,
};
#[cfg(not(test))]
use self::util::hash_secret;
use self::util::{
    clear_session_cookie, constant_time_eq, extract_cookie, generate_token, get_managed_device_secret,
    get_managed_registration_secret, is_past, map_db_error, normalize_domain, normalize_host, normalize_url,
    parse_json, parse_json_or_default, session_cookie, set_managed_device_secret,
};
