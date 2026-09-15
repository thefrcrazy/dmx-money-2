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
    sqlx::query("UPDATE mobile_passkeys SET revoked_at = $1 WHERE id = $2")
        .bind(now)
        .bind(passkey_id)
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "révocation de la passkey"))?;
    Ok(())
}

pub async fn authorize_api_request(
    pool: &DbPool,
    method: &str,
    path: &str,
    headers: &HashMap<String, String>,
) -> Result<(), String> {
    let session = extract_cookie(headers, SESSION_COOKIE).ok_or_else(|| "Session mobile manquante.".to_string())?;
    let row = sqlx::query(
        "SELECT id, csrf_hash, passkey_id, expires_at, revoked_at
         FROM mobile_sessions WHERE session_hash = $1",
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
    if revoked_at.is_some() || is_past(&expires_at) || passkey_id.is_none() {
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

    let now = Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE mobile_sessions SET last_used_at = $1 WHERE id = $2")
        .bind(now)
        .bind(row.try_get::<String, _>("id").unwrap_or_default())
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
    match (method, path) {
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

    sqlx::query("UPDATE mobile_pairing_tokens SET consumed_at = $1 WHERE id = $2")
        .bind(&now)
        .bind(token_id)
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "consommation du pairing mobile"))?;

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
    let passkey_id = insert_passkey(pool, &credential, Some(&label)).await?;
    revoke_session(pool, &session.id).await?;
    delete_challenge(pool, &payload.challenge_id).await?;
    let new_session = create_session(pool, Some(passkey_id), Some(label)).await?;
    Ok(session_response(new_session, false))
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
    update_passkey_usage(
        pool,
        &credential.id,
        i64::from(outcome.new_counter),
        device_label.as_deref(),
    )
    .await?;
    delete_challenge(pool, &payload.challenge_id).await?;
    let session = create_session(pool, Some(credential.id), device_label).await?;
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
            let now = Utc::now().to_rfc3339();
            let session_id = row.try_get::<String, _>("id").map_err(|error| error.to_string())?;
            let passkey_id = row.try_get::<Option<String>, _>("passkey_id").unwrap_or(None);

            let _ = sqlx::query("UPDATE mobile_sessions SET revoked_at = $1 WHERE id = $2")
                .bind(&now)
                .bind(session_id)
                .execute(pool)
                .await;

            if let Some(passkey_id) = passkey_id {
                let _ = sqlx::query("UPDATE mobile_passkeys SET revoked_at = $1 WHERE id = $2 AND revoked_at IS NULL")
                    .bind(&now)
                    .bind(passkey_id)
                    .execute(pool)
                    .await;
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

/// Origine attendue dans `clientDataJSON`, que `passkey-auth` compare telle quelle. La PWA vient
/// soit du Worker public, soit du pont lui-même quand l'application l'embarque : le QR d'appairage
/// ouvre alors `https://{hôte local}:{port}`. Les deux partagent le RP ID du domaine : l'origine de
/// la requête est retenue quand c'est l'une d'elles, l'origine publique sinon, comme en 1.x.
fn passkey_origin(settings: &SecureBridgeSettings, headers: &HashMap<String, String>) -> Result<String, String> {
    let public = secure_app_origin(settings).ok_or_else(|| "Origine PWA manquante.".to_string())?;
    match headers.get("origin").map(|value| value.trim()) {
        Some(origin) if origin == public || is_local_bridge_origin(settings, origin) => Ok(origin.to_string()),
        _ => Ok(public),
    }
}

/// Vrai pour l'origine exacte d'une page servie en HTTPS par ce pont, quel que soit le port : seul le
/// pont détient le certificat de son hôte local.
fn is_local_bridge_origin(settings: &SecureBridgeSettings, origin: &str) -> bool {
    let Some(local_host) = settings
        .local_host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
    else {
        return false;
    };
    url::Url::parse(origin).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some_and(|host| host.eq_ignore_ascii_case(local_host))
            && url.origin().ascii_serialization() == origin
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_ORIGIN: &str = "https://dmxmoney.develop-max.com";

    fn settings() -> SecureBridgeSettings {
        SecureBridgeSettings {
            enabled: true,
            domain: Some("develop-max.com".to_string()),
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
    fn passkeys_accept_the_pwa_served_by_the_local_bridge() {
        let origin = "https://mac-1234.sync.develop-max.com:8801";
        assert_eq!(passkey_origin(&settings(), &from_origin(origin)).unwrap(), origin);
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
