use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingStartPayload {
    token: String,
    device_label: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterOptionsPayload {
    device_label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterVerifyPayload {
    challenge_id: String,
    device_label: Option<String>,
    response: RegistrationResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginVerifyPayload {
    challenge_id: String,
    device_label: Option<String>,
    response: AuthenticationResponse,
}

/// Crée un jeton d'appairage à usage unique. Renvoie (jeton brut, expiration).
pub async fn regenerate_pairing_token(pool: &DbPool) -> Result<(String, String), String> {
    housekeeping(pool).await;
    let raw = generate_token(32);
    let expires_at = (Utc::now() + ChronoDuration::minutes(PAIRING_TTL_MINUTES)).to_rfc3339();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO mobile_pairing_tokens (id, token_hash, expires_at, created_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(hash_secret(&raw))
    .bind(&expires_at)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "création du token de pairing"))?;

    Ok((raw, expires_at))
}

pub async fn revoke_passkey(pool: &DbPool, passkey_id: String) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| map_db_error(error, "révocation du mobile"))?;
    sqlx::query("UPDATE mobile_passkeys SET revoked_at = $1 WHERE id = $2")
        .bind(&now)
        .bind(&passkey_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| map_db_error(error, "révocation de la passkey"))?;
    sqlx::query("UPDATE mobile_sessions SET revoked_at = $1 WHERE passkey_id = $2")
        .bind(&now)
        .bind(&passkey_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| map_db_error(error, "révocation des sessions du mobile"))?;
    transaction
        .commit()
        .await
        .map_err(|error| map_db_error(error, "révocation du mobile"))?;
    Ok(())
}

pub async fn authorize_api_request(
    pool: &DbPool,
    method: &str,
    path: &str,
    headers: &HashMap<String, String>,
) -> Result<(), String> {
    housekeeping(pool).await;
    let session = extract_cookie(headers, SESSION_COOKIE).ok_or_else(|| "Session mobile manquante.".to_string())?;
    let row = sqlx::query(
        "SELECT s.id, s.csrf_hash, s.passkey_id, s.expires_at, s.revoked_at, s.created_at
         FROM mobile_sessions s JOIN mobile_passkeys p ON p.id=s.passkey_id
         WHERE s.session_hash = $1 AND p.revoked_at IS NULL",
    )
    .bind(hash_secret(&session))
    .fetch_optional(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture de session mobile"))?
    .ok_or_else(|| "Session mobile invalide.".to_string())?;

    let expires_at = row
        .try_get::<String, _>("expires_at")
        .map_err(|error| error.to_string())?;
    let revoked_at = row.try_get::<Option<String>, _>("revoked_at").unwrap_or(None);
    let passkey_id = row.try_get::<Option<String>, _>("passkey_id").unwrap_or(None);
    let created_at: String = row.try_get("created_at").map_err(|error| error.to_string())?;
    let created = chrono::DateTime::parse_from_rfc3339(&created_at)
        .map_err(|_| "Session mobile invalide.")?
        .with_timezone(&Utc);
    if revoked_at.is_some()
        || is_past(&expires_at)
        || created + ChronoDuration::days(SESSION_ABSOLUTE_TTL_DAYS) <= Utc::now()
        || passkey_id.is_none()
    {
        return Err("Session mobile expirée ou non finalisée.".to_string());
    }

    if method != "GET" && path != "/api/status" {
        let csrf = headers
            .get("x-dmx-csrf")
            .ok_or_else(|| "Jeton CSRF manquant.".to_string())?;
        let csrf_hash = row
            .try_get::<String, _>("csrf_hash")
            .map_err(|error| error.to_string())?;
        if !constant_time_eq(&hash_secret(csrf), &csrf_hash) {
            return Err("Jeton CSRF invalide.".to_string());
        }
    }

    let now = Utc::now();
    let renewed_expiry = (now + ChronoDuration::days(SESSION_IDLE_TTL_DAYS))
        .min(created + ChronoDuration::days(SESSION_ABSOLUTE_TTL_DAYS))
        .to_rfc3339();
    let _ = sqlx::query("UPDATE mobile_sessions SET last_used_at = $1, expires_at = MAX(expires_at, $3) WHERE id = $2 AND revoked_at IS NULL")
        .bind(now.to_rfc3339())
        .bind(row.try_get::<String, _>("id").unwrap_or_default())
        .bind(renewed_expiry)
        .execute(pool)
        .await;

    Ok(())
}

pub async fn handle_auth_request(
    pool: &DbPool,
    method: &str,
    path: &str,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> Result<AuthRouteOutput, String> {
    housekeeping(pool).await;
    match (method, path) {
        ("POST", "/auth/session") => resume_session(pool, headers).await,
        ("POST", "/auth/pairing/start") => pairing_start(pool, body).await,
        ("POST", "/auth/passkey/register/options") => register_options(pool, headers, body).await,
        ("POST", "/auth/passkey/register/verify") => register_verify(pool, headers, body).await,
        ("POST", "/auth/passkey/login/options") => login_options(pool, headers).await,
        ("POST", "/auth/passkey/login/verify") => login_verify(pool, headers, body).await,
        ("POST", "/auth/logout") => logout(pool, headers).await,
        ("POST", "/auth/unlink") => unlink(pool, headers).await,
        _ => Ok(AuthRouteOutput {
            status: 404,
            body: json!({ "error": "Route introuvable" }),
            headers: Vec::new(),
        }),
    }
}

async fn pairing_start(pool: &DbPool, body: &[u8]) -> Result<AuthRouteOutput, String> {
    let payload: PairingStartPayload = parse_json(body)?;
    let now = Utc::now().to_rfc3339();

    let row = sqlx::query(
        "SELECT id, expires_at, consumed_at
         FROM mobile_pairing_tokens WHERE token_hash = $1",
    )
    .bind(hash_secret(&payload.token))
    .fetch_optional(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture du pairing mobile"))?
    .ok_or_else(|| "Token de pairing invalide.".to_string())?;

    let token_id = row.try_get::<String, _>("id").map_err(|error| error.to_string())?;
    let expires_at = row
        .try_get::<String, _>("expires_at")
        .map_err(|error| error.to_string())?;
    let consumed_at = row.try_get::<Option<String>, _>("consumed_at").unwrap_or(None);
    if consumed_at.is_some() || is_past(&expires_at) {
        return Err("Token de pairing expiré ou déjà utilisé.".to_string());
    }

    let consumed =
        sqlx::query("UPDATE mobile_pairing_tokens SET consumed_at = $1 WHERE id = $2 AND consumed_at IS NULL")
            .bind(&now)
            .bind(token_id)
            .execute(pool)
            .await
            .map_err(|error| map_db_error(error, "consommation du pairing mobile"))?;
    if consumed.rows_affected() != 1 {
        return Err("Token de pairing déjà utilisé.".into());
    }

    let session = create_session(pool, None, payload.device_label).await?;
    Ok(session_response(session, true))
}

async fn register_options(
    pool: &DbPool,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> Result<AuthRouteOutput, String> {
    let payload: RegisterOptionsPayload = parse_json_or_default(body)?;
    let session = authorize_session_for_auth(pool, headers).await?;
    let settings = load_settings(pool).await?;
    let webauthn = build_webauthn(&settings, headers)?;
    let user_id = settings.device_id.as_deref().unwrap_or("dmxmoney").as_bytes().to_vec();
    let label = payload.device_label.unwrap_or_else(|| "Mobile".to_string());
    let existing = list_active_credentials_for_device(pool, &label).await?;
    let (challenge, state) = webauthn.start_registration(&user_id, "dmxmoney-mobile", &label, &existing);
    let challenge_id = store_challenge(pool, "register", &state, Some(&session.id), Some(&label)).await?;

    Ok(AuthRouteOutput {
        status: 200,
        body: json!({ "challengeId": challenge_id, "publicKey": challenge }),
        headers: Vec::new(),
    })
}

async fn register_verify(
    pool: &DbPool,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> Result<AuthRouteOutput, String> {
    let payload: RegisterVerifyPayload = parse_json(body)?;
    let session = authorize_session_for_auth(pool, headers).await?;
    let state: RegistrationState = load_challenge(pool, &payload.challenge_id, "register", Some(&session.id)).await?;
    let settings = load_settings(pool).await?;
    let webauthn = build_webauthn(&settings, headers)?;
    let credential = webauthn
        .finish_registration(&state, &payload.response)
        .map_err(|error| format!("Passkey refusée: {error}"))?;
    let label = payload.device_label.unwrap_or_else(|| "Mobile".to_string());
    let new_session = finalize_registration(pool, &session.id, &payload.challenge_id, &credential, &label).await?;
    Ok(session_response(new_session, false))
}

/// Called only after cryptographic verification. Claiming the pending session, challenge,
/// credential and final session in one transaction prevents parallel enrollment or partial state.
async fn finalize_registration(
    pool: &DbPool,
    pending_session: &str,
    challenge: &str,
    credential: &PasskeyCredential,
    label: &str,
) -> Result<sessions::SessionTokens, String> {
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;
    let claimed = sqlx::query("UPDATE mobile_sessions SET revoked_at=? WHERE id=? AND passkey_id IS NULL AND revoked_at IS NULL AND julianday(expires_at)>julianday('now')")
        .bind(Utc::now().to_rfc3339()).bind(pending_session).execute(&mut *tx).await.map_err(|error| error.to_string())?;
    if claimed.rows_affected() != 1 {
        return Err("Session de pairing expirée ou déjà finalisée.".into());
    }
    delete_challenge(&mut *tx, challenge).await?;
    let id = insert_passkey(&mut *tx, credential, Some(label)).await?;
    let session = create_session(&mut *tx, Some(id), Some(label.to_string())).await?;
    tx.commit().await.map_err(|error| error.to_string())?;
    Ok(session)
}

async fn login_options(pool: &DbPool, headers: &HashMap<String, String>) -> Result<AuthRouteOutput, String> {
    let settings = load_settings(pool).await?;
    let webauthn = build_webauthn(&settings, headers)?;
    let credentials = list_active_passkeys(pool).await?;
    if credentials.is_empty() {
        return Err("Aucune passkey mobile active. Scannez un QR de pairing.".to_string());
    }
    let (challenge, state) = webauthn.start_authentication_with_creds(&credentials);
    let challenge_id = store_challenge(pool, "login", &state, None, Some("Mobile")).await?;
    Ok(AuthRouteOutput {
        status: 200,
        body: json!({ "challengeId": challenge_id, "publicKey": challenge }),
        headers: Vec::new(),
    })
}

async fn login_verify(
    pool: &DbPool,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> Result<AuthRouteOutput, String> {
    let payload: LoginVerifyPayload = parse_json(body)?;
    let state: AuthenticationState = load_challenge(pool, &payload.challenge_id, "login", None).await?;
    let credential = find_passkey_by_credential_id(pool, &payload.response.id).await?;
    let settings = load_settings(pool).await?;
    let webauthn = build_webauthn(&settings, headers)?;
    let outcome = webauthn
        .finish_authentication(&state, &payload.response, &credential.passkey)
        .map_err(|error| format!("Passkey refusée: {error}"))?;
    let device_label = payload.device_label.clone();
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;
    delete_challenge(&mut *tx, &payload.challenge_id).await?;
    update_passkey_usage(
        &mut *tx,
        &credential.id,
        i64::from(outcome.new_counter),
        device_label.as_deref(),
    )
    .await?;
    let session = create_session(&mut *tx, Some(credential.id), device_label).await?;
    tx.commit().await.map_err(|error| error.to_string())?;
    Ok(session_response(session, false))
}

async fn logout(pool: &DbPool, headers: &HashMap<String, String>) -> Result<AuthRouteOutput, String> {
    if let Some(raw) = extract_cookie(headers, SESSION_COOKIE) {
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query("UPDATE mobile_sessions SET revoked_at = $1 WHERE session_hash = $2")
            .bind(now)
            .bind(hash_secret(&raw))
            .execute(pool)
            .await;
    }

    Ok(AuthRouteOutput {
        status: 200,
        body: json!({ "ok": true }),
        headers: vec![clear_session_cookie()],
    })
}

async fn unlink(pool: &DbPool, headers: &HashMap<String, String>) -> Result<AuthRouteOutput, String> {
    if let Some(raw) = extract_cookie(headers, SESSION_COOKIE) {
        let row = sqlx::query("SELECT id, passkey_id FROM mobile_sessions WHERE session_hash = $1")
            .bind(hash_secret(&raw))
            .fetch_optional(pool)
            .await
            .map_err(|error| map_db_error(error, "lecture de session mobile"))?;

        if let Some(row) = row {
            let session_id = row.try_get::<String, _>("id").map_err(|error| error.to_string())?;
            let passkey_id = row.try_get::<Option<String>, _>("passkey_id").unwrap_or(None);
            if let Some(passkey_id) = passkey_id {
                revoke_passkey(pool, passkey_id).await?;
            } else {
                revoke_session(pool, &session_id).await?;
            }
        }
    }

    Ok(AuthRouteOutput {
        status: 200,
        body: json!({ "ok": true }),
        headers: vec![clear_session_cookie()],
    })
}

fn build_webauthn(settings: &SecureBridgeSettings, headers: &HashMap<String, String>) -> Result<Webauthn, String> {
    let origin = passkey_origin(settings, headers)?;
    let rp_id = settings
        .domain
        .as_deref()
        .ok_or_else(|| "Domaine du pont sécurisé manquant.".to_string())?;
    Ok(Webauthn::new(rp_id, "DmxMoney", &origin).require_user_verification(true))
}

/// L'origine WebAuthn est exclusivement celle de la PWA HTTPS configurée.
fn passkey_origin(settings: &SecureBridgeSettings, _headers: &HashMap<String, String>) -> Result<String, String> {
    secure_app_origin(settings).ok_or_else(|| "Origine PWA manquante.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_ORIGIN: &str = "https://dmxmoney-companion.pages.dev";

    fn verified_credential(id: u8) -> PasskeyCredential {
        // These tests exercise the atomic persistence after WebAuthn verification,
        // not the authenticator's cryptographic verification itself.
        PasskeyCredential {
            id: CredentialId(vec![id]),
            public_key_cose: passkey_auth::CosePublicKey(vec![0xa0]),
            counter: 0,
            transports: vec![],
            aaguid: [0; 16],
        }
    }

    async fn pending_session(pool: &DbPool) -> String {
        create_session(pool, None, None).await.unwrap();
        sqlx::query_scalar("SELECT id FROM mobile_sessions WHERE passkey_id IS NULL AND revoked_at IS NULL")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_pending_pairing_session_can_finalize_only_one_credential() {
        let pool = dmx_core::db::open_memory_pool().await.unwrap();
        let session = pending_session(&pool).await;
        let mut challenges = Vec::new();
        for _ in 0..2 {
            challenges.push(
                store_challenge(&pool, "register", &json!({}), Some(&session), None)
                    .await
                    .unwrap(),
            );
        }
        let first_key = verified_credential(1);
        let second_key = verified_credential(2);
        let (first, second) = tokio::join!(
            finalize_registration(&pool, &session, &challenges[0], &first_key, "Phone"),
            finalize_registration(&pool, &session, &challenges[1], &second_key, "Phone")
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        let passkeys: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mobile_passkeys")
            .fetch_one(&pool)
            .await
            .unwrap();
        let finalized: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mobile_sessions WHERE passkey_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
        let revoked: Option<String> = sqlx::query_scalar("SELECT revoked_at FROM mobile_sessions WHERE id=?")
            .bind(&session)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!((passkeys, finalized), (1, 1));
        assert!(revoked.is_some());
    }

    #[tokio::test]
    async fn failed_credential_insert_rolls_back_pairing_claim_and_challenge_consumption() {
        let pool = dmx_core::db::open_memory_pool().await.unwrap();
        let session = pending_session(&pool).await;
        let challenge = store_challenge(&pool, "register", &json!({}), Some(&session), None)
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER fixture_fail_passkey BEFORE INSERT ON mobile_passkeys BEGIN SELECT RAISE(ABORT, 'fixture'); END")
            .execute(&pool).await.unwrap();
        assert!(
            finalize_registration(&pool, &session, &challenge, &verified_credential(1), "Phone")
                .await
                .is_err()
        );
        let revoked: Option<String> = sqlx::query_scalar("SELECT revoked_at FROM mobile_sessions WHERE id=?")
            .bind(&session)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(revoked.is_none());
        assert!(
            load_challenge::<serde_json::Value>(&pool, &challenge, "register", Some(&session))
                .await
                .is_ok()
        );
        let passkeys: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mobile_passkeys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(passkeys, 0);
    }

    fn settings() -> SecureBridgeSettings {
        SecureBridgeSettings {
            enabled: true,
            domain: Some("dmxmoney-companion.pages.dev".to_string()),
            app_url: Some(format!("{PUBLIC_ORIGIN}/mobile")),
            local_host: Some("mac-1234.sync.develop-max.com".to_string()),
            device_id: Some("mac-1234".to_string()),
            certificate_expires_at: None,
            dns_record_id: None,
            dns_last_updated_at: None,
            last_error: None,
            managed_service_url: None,
        }
    }

    fn from_origin(origin: &str) -> HashMap<String, String> {
        HashMap::from([("origin".to_string(), origin.to_string())])
    }

    #[test]
    fn passkeys_never_accept_the_previous_local_bridge_origin() {
        let origin = "https://mac-1234.sync.develop-max.com:8801";
        assert_eq!(
            passkey_origin(&settings(), &from_origin(origin)).unwrap(),
            PUBLIC_ORIGIN
        );
    }

    #[test]
    fn passkeys_keep_the_public_pwa_origin() {
        assert_eq!(
            passkey_origin(&settings(), &from_origin(PUBLIC_ORIGIN)).unwrap(),
            PUBLIC_ORIGIN
        );
        assert_eq!(passkey_origin(&settings(), &HashMap::new()).unwrap(), PUBLIC_ORIGIN);
    }

    #[test]
    fn passkeys_ignore_other_origins() {
        for origin in [
            "https://example.com",
            "http://mac-1234.sync.develop-max.com:8801",
            "https://other.sync.develop-max.com:8801",
            "https://mac-1234.sync.develop-max.com.example.com",
            "https://mac-1234.sync.develop-max.com:8801/mobile",
        ] {
            assert_eq!(
                passkey_origin(&settings(), &from_origin(origin)).unwrap(),
                PUBLIC_ORIGIN,
                "{origin}"
            );
        }
    }
}
