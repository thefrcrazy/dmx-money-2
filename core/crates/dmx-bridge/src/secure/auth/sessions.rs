use super::*;

#[derive(Debug)]
pub(super) struct SessionTokens {
    raw_session: String,
    raw_csrf: String,
    expires_at: String,
}

#[derive(Debug)]
pub(super) struct AuthorizedSession {
    pub(super) id: String,
}

pub(super) async fn create_session<'e>(
    pool: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    passkey_id: Option<String>,
    device_label: Option<String>,
) -> Result<SessionTokens, String> {
    let raw_session = generate_token(32);
    let raw_csrf = csrf_token(&raw_session);
    let now = Utc::now().to_rfc3339();
    let expires_at = if passkey_id.is_some() {
        Utc::now() + ChronoDuration::days(SESSION_IDLE_TTL_DAYS)
    } else {
        Utc::now() + ChronoDuration::minutes(PENDING_SESSION_TTL_MINUTES)
    }
    .to_rfc3339();
    sqlx::query(
        "INSERT INTO mobile_sessions
            (id, session_hash, csrf_hash, passkey_id, device_label, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(hash_secret(&raw_session))
    .bind(hash_secret(&raw_csrf))
    .bind(passkey_id)
    .bind(device_label)
    .bind(&expires_at)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "création de session mobile"))?;

    Ok(SessionTokens {
        raw_session,
        raw_csrf,
        expires_at,
    })
}

pub(super) async fn revoke_session(pool: &DbPool, id: &str) -> Result<(), String> {
    sqlx::query("UPDATE mobile_sessions SET revoked_at=$1 WHERE id=$2")
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "révocation de session mobile"))?;
    Ok(())
}

pub(super) fn session_response(session: SessionTokens, passkey_required: bool) -> AuthRouteOutput {
    AuthRouteOutput {
        status: 200,
        body: json!({
            "ok": true,
            "csrfToken": session.raw_csrf,
            "expiresAt": session.expires_at,
            "passkeyRequired": passkey_required,
        }),
        headers: vec![session_cookie(&session.raw_session, &session.expires_at)],
    }
}

pub(super) async fn authorize_session_for_auth(
    pool: &DbPool,
    headers: &HashMap<String, String>,
) -> Result<AuthorizedSession, String> {
    let session = extract_cookie(headers, SESSION_COOKIE).ok_or_else(|| "Session de pairing manquante.".to_string())?;
    // Registration is authorized only by the short-lived session issued by a fresh QR.
    let row = sqlx::query(
        "SELECT id, expires_at, revoked_at FROM mobile_sessions WHERE session_hash = $1 AND passkey_id IS NULL",
    )
    .bind(hash_secret(&session))
    .fetch_optional(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture de session mobile"))?
    .ok_or_else(|| "Session de pairing invalide.".to_string())?;

    let expires_at = row
        .try_get::<String, _>("expires_at")
        .map_err(|error| error.to_string())?;
    let revoked_at = row.try_get::<Option<String>, _>("revoked_at").unwrap_or(None);
    if revoked_at.is_some() || is_past(&expires_at) {
        return Err("Session de pairing expirée.".to_string());
    }

    Ok(AuthorizedSession {
        id: row.try_get::<String, _>("id").map_err(|error| error.to_string())?,
    })
}

/// Stable across reloads and tabs; the high-entropy session remains the only bearer secret.
pub(super) fn csrf_token(session: &str) -> String {
    hash_secret(&format!("dmxmoney-csrf-v1:{session}"))
}

pub(super) async fn resume_session(
    pool: &DbPool,
    headers: &HashMap<String, String>,
) -> Result<AuthRouteOutput, String> {
    let raw_session = extract_cookie(headers, SESSION_COOKIE).ok_or("Session mobile manquante.")?;
    let raw_csrf = csrf_token(&raw_session);
    let row = sqlx::query("SELECT s.id, s.expires_at, s.created_at FROM mobile_sessions s JOIN mobile_passkeys p ON p.id=s.passkey_id WHERE s.session_hash=$1 AND s.revoked_at IS NULL AND p.revoked_at IS NULL")
        .bind(hash_secret(&raw_session)).fetch_optional(pool).await
        .map_err(|error| map_db_error(error, "restauration de session mobile"))?
        .ok_or("Session mobile expirée ou révoquée.")?;
    let expires_at: String = row.try_get("expires_at").map_err(|error| error.to_string())?;
    let created_at: String = row.try_get("created_at").map_err(|error| error.to_string())?;
    let created = chrono::DateTime::parse_from_rfc3339(&created_at)
        .map_err(|_| "Session mobile invalide.")?
        .with_timezone(&Utc);
    let now = Utc::now();
    let absolute = created + ChronoDuration::days(SESSION_ABSOLUTE_TTL_DAYS);
    if is_past(&expires_at) || absolute <= now {
        return Err("Session mobile expirée.".into());
    }
    let expires_at = (now + ChronoDuration::days(SESSION_IDLE_TTL_DAYS))
        .min(absolute)
        .to_rfc3339();
    // Re-check revocation in the same write that renews the session; no revoked credential can be revived.
    let updated = sqlx::query("UPDATE mobile_sessions SET csrf_hash=$1, expires_at=$2, last_used_at=$3 WHERE id=$4 AND revoked_at IS NULL AND expires_at>$3 AND EXISTS(SELECT 1 FROM mobile_passkeys p WHERE p.id=mobile_sessions.passkey_id AND p.revoked_at IS NULL)")
        .bind(hash_secret(&raw_csrf)).bind(&expires_at).bind(now.to_rfc3339())
        .bind(row.try_get::<String,_>("id").map_err(|error| error.to_string())?)
        .execute(pool).await.map_err(|error| map_db_error(error, "renouvellement de session mobile"))?;
    if updated.rows_affected() != 1 {
        return Err("Session mobile expirée ou révoquée.".into());
    }
    Ok(session_response(
        SessionTokens {
            raw_session,
            raw_csrf,
            expires_at,
        },
        false,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (dmx_core::Engine, SessionTokens, HashMap<String, String>) {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        let tokens = engine.block_on(async {
            sqlx::query("INSERT INTO mobile_passkeys (id, credential_id, public_key, created_at) VALUES ('pk', 'credential', '{}', $1)")
                .bind(Utc::now().to_rfc3339()).execute(engine.pool()).await.unwrap();
            create_session(engine.pool(), Some("pk".into()), Some("Phone".into())).await.unwrap()
        });
        let headers = HashMap::from([("cookie".into(), format!("{SESSION_COOKIE}={}", tokens.raw_session))]);
        (engine, tokens, headers)
    }

    #[test]
    fn a_finalized_session_resumes_without_webauthn_and_keeps_the_same_csrf_across_tabs() {
        let (engine, tokens, mut headers) = fixture();
        engine.block_on(async {
            for _ in 0..2 {
                let resumed = resume_session(engine.pool(), &headers).await.unwrap();
                assert_eq!(resumed.status, 200);
                assert_eq!(resumed.body["csrfToken"], tokens.raw_csrf);
                assert_eq!(resumed.body["passkeyRequired"], false);
                let expiry = chrono::DateTime::parse_from_rfc3339(resumed.body["expiresAt"].as_str().unwrap()).unwrap();
                assert!((expiry.with_timezone(&Utc) - Utc::now()).num_days() >= 6);
            }
            headers.insert("x-dmx-csrf".into(), tokens.raw_csrf);
            crate::secure::authorize_api_request(engine.pool(), "POST", "/api/accounts", &headers)
                .await
                .unwrap();
        });
    }

    #[test]
    fn pending_pairing_sessions_cannot_be_restored_as_authenticated_sessions() {
        let engine = dmx_core::Engine::open_in_memory().unwrap();
        engine.block_on(async {
            let tokens = create_session(engine.pool(), None, None).await.unwrap();
            let headers = HashMap::from([("cookie".into(), format!("{SESSION_COOKIE}={}", tokens.raw_session))]);
            assert!(resume_session(engine.pool(), &headers).await.is_err());
            assert!(authorize_session_for_auth(engine.pool(), &headers).await.is_ok());
        });
    }

    #[test]
    fn finalized_or_unlinked_sessions_cannot_register_another_passkey() {
        let (engine, tokens, headers) = fixture();
        engine.block_on(async {
            let other = create_session(engine.pool(), Some("pk".into()), None).await.unwrap();
            let other_headers = HashMap::from([("cookie".into(), format!("{SESSION_COOKIE}={}", other.raw_session))]);
            assert!(authorize_session_for_auth(engine.pool(), &headers).await.is_err());
            assert!(authorize_session_for_auth(engine.pool(), &other_headers).await.is_err());
            crate::secure::handle_auth_request(engine.pool(), "POST", "/auth/unlink", &headers, b"{}")
                .await
                .unwrap();
            for (cookie, headers) in [(&tokens.raw_session, &headers), (&other.raw_session, &other_headers)] {
                assert!(authorize_session_for_auth(engine.pool(), headers).await.is_err());
                assert!(resume_session(engine.pool(), headers).await.is_err());
                let revoked: Option<String> =
                    sqlx::query_scalar("SELECT revoked_at FROM mobile_sessions WHERE session_hash=$1")
                        .bind(hash_secret(cookie))
                        .fetch_one(engine.pool())
                        .await
                        .unwrap();
                assert!(revoked.is_some());
            }
        });
    }

    #[test]
    fn revoked_credentials_block_every_existing_session_and_cannot_be_revived() {
        let (engine, tokens, headers) = fixture();
        engine.block_on(async {
            let other = create_session(engine.pool(), Some("pk".into()), None).await.unwrap();
            sqlx::query("UPDATE mobile_passkeys SET revoked_at=$1 WHERE id='pk'")
                .bind(Utc::now().to_rfc3339())
                .execute(engine.pool())
                .await
                .unwrap();
            for cookie in [&tokens.raw_session, &other.raw_session] {
                let headers = HashMap::from([("cookie".into(), format!("{SESSION_COOKIE}={cookie}"))]);
                assert!(resume_session(engine.pool(), &headers).await.is_err());
                assert!(
                    crate::secure::authorize_api_request(engine.pool(), "GET", "/api/accounts", &headers)
                        .await
                        .is_err()
                );
            }
            assert!(resume_session(engine.pool(), &headers).await.is_err());
        });
    }

    #[test]
    fn expired_sessions_are_not_renewed_and_absolute_lifetime_is_enforced() {
        for (created, expires) in [(-1, -1), (-31, 1)] {
            let (engine, _, headers) = fixture();
            engine.block_on(async {
                sqlx::query("UPDATE mobile_sessions SET created_at=$1, expires_at=$2")
                    .bind((Utc::now() + ChronoDuration::days(created)).to_rfc3339())
                    .bind((Utc::now() + ChronoDuration::days(expires)).to_rfc3339())
                    .execute(engine.pool())
                    .await
                    .unwrap();
                assert!(resume_session(engine.pool(), &headers).await.is_err());
                assert!(
                    crate::secure::authorize_api_request(engine.pool(), "GET", "/api/accounts", &headers)
                        .await
                        .is_err()
                );
            });
        }
    }

    #[test]
    fn renewal_is_capped_at_thirty_days_even_with_regular_activity() {
        let (engine, _, headers) = fixture();
        engine.block_on(async {
            sqlx::query("UPDATE mobile_sessions SET created_at=$1")
                .bind((Utc::now() - ChronoDuration::days(29)).to_rfc3339())
                .execute(engine.pool())
                .await
                .unwrap();
            let resumed = resume_session(engine.pool(), &headers).await.unwrap();
            let expiry = chrono::DateTime::parse_from_rfc3339(resumed.body["expiresAt"].as_str().unwrap()).unwrap();
            assert!((expiry.with_timezone(&Utc) - Utc::now()).num_hours() <= 24);
        });
    }

    #[test]
    fn explicit_logout_invalidates_the_saved_mobile_session() {
        let (engine, _, headers) = fixture();
        engine.block_on(async {
            crate::secure::handle_auth_request(engine.pool(), "POST", "/auth/logout", &headers, b"{}")
                .await
                .unwrap();
            assert!(resume_session(engine.pool(), &headers).await.is_err());
        });
    }
}
