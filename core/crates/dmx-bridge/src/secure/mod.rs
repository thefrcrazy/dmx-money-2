//! Authentification du compagnon Internet : passkeys, appairage et sessions vérifiées sur le bureau.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration as ChronoDuration, Utc};
use dmx_core::db::DbPool;
use passkey_auth::{
    AuthenticationResponse, AuthenticationState, CredentialId, PasskeyCredential, RegistrationResponse,
    RegistrationState, Webauthn,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::HashMap;
use subtle::ConstantTimeEq;
use uuid::Uuid;

const SESSION_COOKIE: &str = "dmxmoney_session";
const PAIRING_TTL_MINUTES: i64 = 10;
const PENDING_SESSION_TTL_MINUTES: i64 = 10;
const SESSION_IDLE_TTL_DAYS: i64 = 7;
const SESSION_ABSOLUTE_TTL_DAYS: i64 = 30;
const CHALLENGE_TTL_MINUTES: i64 = 5;
mod auth;
mod settings;
mod types;
mod util;

pub use self::auth::{authorize_api_request, handle_auth_request, regenerate_pairing_token, revoke_passkey};
pub use self::settings::{load_settings, secure_app_origin, set_enabled};
pub use self::types::{AuthRouteOutput, MobilePasskeyInfo, SecureBridgeSettings, SecureBridgeStatus};

pub(crate) use self::util::hash_secret;

use self::auth::list_passkeys;

pub(crate) async fn relay_passkeys(pool: &DbPool) -> Result<Vec<MobilePasskeyInfo>, String> {
    list_passkeys(pool).await
}
use self::util::{
    clear_session_cookie, constant_time_eq, extract_cookie, generate_token, is_past, map_db_error, parse_json,
    parse_json_or_default, session_cookie,
};
