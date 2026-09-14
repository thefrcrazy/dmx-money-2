use super::*;

#[derive(Debug, Deserialize)]
struct DnsJsonResponse {
    #[serde(rename = "Answer")]
    answer: Option<Vec<DnsJsonAnswer>>,
}

#[derive(Debug, Deserialize)]
struct DnsJsonAnswer {
    data: String,
}

pub async fn load_tls_config(
    data_dir: &Path,
    settings: &SecureBridgeSettings,
) -> Result<Option<Arc<rustls::ServerConfig>>, String> {
    if !settings.enabled {
        return Ok(None);
    }

    let paths = match certificate_paths(data_dir, settings.device_id.as_deref()) {
        Some(paths) if paths.cert.exists() && paths.key.exists() => paths,
        _ => return Ok(None),
    };

    let cert_file = fs::File::open(&paths.cert).map_err(|error| format!("Certificat HTTPS illisible: {error}"))?;
    let key_file = fs::File::open(&paths.key).map_err(|error| format!("Clé HTTPS illisible: {error}"))?;
    let certs = rustls_pemfile::certs(&mut BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Certificat HTTPS invalide: {error}"))?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))
        .map_err(|error| format!("Clé HTTPS invalide: {error}"))?
        .ok_or_else(|| "Clé HTTPS manquante".to_string())?;
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|error| format!("Configuration TLS invalide: {error}"))?;

    Ok(Some(Arc::new(config)))
}

/// Identité légère du matériel TLS sur disque : elle change quand le certificat est renouvelé ou
/// réémis pour un autre hôte, ce qui indique au serveur qu'il doit redémarrer.
pub fn certificate_fingerprint(data_dir: &Path, settings: &SecureBridgeSettings) -> Option<String> {
    let paths = certificate_paths(data_dir, settings.device_id.as_deref())?;
    let metadata = fs::metadata(&paths.cert).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let host = settings.local_host.as_deref().unwrap_or_default();
    Some(format!("{host}:{}:{modified}", metadata.len()))
}

pub async fn refresh_infrastructure(pool: &DbPool, data_dir: &Path, force_certificate: bool) -> Result<(), String> {
    ensure_auto_configuration(pool, data_dir).await?;
    let mut settings = load_settings(pool).await?;
    if !settings.enabled {
        return Ok(());
    }

    // `Ok(Some(raison))` : le pont reste utilisable mais le renouvellement managé a été sauté ;
    // la raison reste visible au lieu d'être effacée.
    let result: Result<Option<String>, String> = async {
        let mut local_host = settings
            .local_host
            .clone()
            .ok_or_else(|| "Hôte local sécurisé manquant.".to_string())?;
        let local_ip = crate::companion::detect_local_ip();
        let mut degraded: Option<String> = None;

        let record_id = match managed_update_dns(&settings, &local_host, &local_ip).await {
            Ok(record_id) => Some(record_id),
            Err(error) if is_missing_managed_secret_error(&error) => {
                match provision_managed_device(pool, &settings, true).await {
                    Ok(_) => {
                        settings = load_settings(pool).await?;
                        local_host = settings
                            .local_host
                            .clone()
                            .ok_or_else(|| "Hôte local sécurisé manquant.".to_string())?;
                        Some(managed_update_dns(&settings, &local_host, &local_ip).await?)
                    }
                    Err(provision_error) if can_reuse_existing_bridge(data_dir, &settings, force_certificate) => {
                        log::warn!("Secure bridge managed refresh skipped; existing DNS/certificate are usable: {provision_error}");
                        degraded = Some(provision_error);
                        None
                    }
                    Err(provision_error) => return Err(provision_error),
                }
            }
            Err(error) if can_reuse_existing_bridge(data_dir, &settings, force_certificate) => {
                log::warn!("Secure bridge DNS refresh skipped; existing DNS/certificate are usable: {error}");
                degraded = Some(error);
                None
            }
            Err(error) => return Err(error),
        };

        if let Some(record_id) = record_id {
            sqlx::query(
                "UPDATE settings SET
                    \"secureBridgeDnsRecordId\" = $1,
                    \"secureBridgeDnsLastUpdatedAt\" = $2
                 WHERE id = 1",
            )
            .bind(record_id)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|error| map_db_error(error, "sauvegarde DNS sécurisé"))?;
        }

        ensure_certificate(pool, data_dir, &settings, &local_host, force_certificate).await?;
        Ok(degraded)
    }
    .await;

    match result {
        Ok(None) => {
            clear_last_error(pool).await;
            Ok(())
        }
        Ok(Some(reason)) => {
            record_last_error(pool, &degraded_provisioning_message(&reason)).await;
            Ok(())
        }
        // Un service managé indisponible ne coupe pas un pont que les mobiles appairés atteignent
        // encore : le certificat valide continue d'être servi et la raison est affichée.
        Err(error) if serves_valid_certificate(data_dir, &settings) => {
            log::warn!("Secure bridge refresh degraded, existing certificate still valid: {error}");
            record_last_error(pool, &degraded_provisioning_message(&error)).await;
            Ok(())
        }
        Err(error) => {
            record_last_error(pool, &error).await;
            Err(error)
        }
    }
}

fn serves_valid_certificate(data_dir: &Path, settings: &SecureBridgeSettings) -> bool {
    can_serve_locally(data_dir, settings)
        && settings
            .certificate_expires_at
            .as_deref()
            .map(|value| !is_past(value))
            .unwrap_or(false)
}

fn can_reuse_existing_bridge(data_dir: &Path, settings: &SecureBridgeSettings, force_certificate: bool) -> bool {
    if force_certificate {
        return false;
    }

    let non_empty = |value: Option<&str>| value.map(|value| !value.trim().is_empty()).unwrap_or(false);
    let certificate_fresh = settings
        .certificate_expires_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc) >= Utc::now() + ChronoDuration::days(7))
        .unwrap_or(false);
    let has_certificate_files = certificate_paths(data_dir, settings.device_id.as_deref())
        .map(|paths| paths.cert.exists() && paths.key.exists())
        .unwrap_or(false);

    non_empty(settings.dns_record_id.as_deref())
        && non_empty(settings.local_host.as_deref())
        && certificate_fresh
        && has_certificate_files
}

async fn ensure_certificate(
    pool: &DbPool,
    data_dir: &Path,
    settings: &SecureBridgeSettings,
    local_host: &str,
    force: bool,
) -> Result<(), String> {
    let expires_soon = settings
        .certificate_expires_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc) < Utc::now() + ChronoDuration::days(CERT_RENEW_WINDOW_DAYS))
        .unwrap_or(true);
    let paths = certificate_paths(data_dir, settings.device_id.as_deref())
        .ok_or_else(|| "Device ID du pont sécurisé manquant.".to_string())?;
    if !force && paths.cert.exists() && paths.key.exists() && !expires_soon {
        return Ok(());
    }

    fs::create_dir_all(&paths.dir).map_err(|error| format!("Création du dossier certificat impossible: {error}"))?;

    let contact: [&str; 0] = [];
    let new_account = NewAccount {
        contact: &contact,
        terms_of_service_agreed: true,
        only_return_existing: false,
    };
    let (account, _credentials) = Account::create(&new_account, LetsEncrypt::Production.url(), None)
        .await
        .map_err(|error| format!("Création du compte ACME impossible: {error}"))?;
    let identifiers = [Identifier::Dns(local_host.to_string())];
    let mut order = account
        .new_order(&NewOrder {
            identifiers: &identifiers,
        })
        .await
        .map_err(|error| format!("Création de commande ACME impossible: {error}"))?;

    let txt_name = format!("_acme-challenge.{local_host}");
    let mut txt_record_ids = Vec::new();
    for authz in order
        .authorizations()
        .await
        .map_err(|error| format!("Lecture des autorisations ACME impossible: {error}"))?
    {
        if authz.status == AuthorizationStatus::Valid {
            continue;
        }
        let challenge = authz
            .challenges
            .iter()
            .find(|challenge| challenge.r#type == ChallengeType::Dns01)
            .ok_or_else(|| "Challenge DNS-01 indisponible chez Let's Encrypt.".to_string())?;
        let dns_value = order.key_authorization(challenge).dns_value();
        let txt_id = managed_present_txt(settings, &txt_name, &dns_value).await?;
        txt_record_ids.push(txt_id);

        if let Err(error) = wait_for_dns_txt(&txt_name, &dns_value).await {
            cleanup_txt_records(settings, &mut txt_record_ids).await;
            return Err(error);
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
        if let Err(error) = order.set_challenge_ready(&challenge.url).await {
            cleanup_txt_records(settings, &mut txt_record_ids).await;
            return Err(format!("Validation DNS-01 impossible: {error}"));
        }

        for _ in 0..12 {
            let current = order
                .challenge(&challenge.url)
                .await
                .map_err(|error| format!("Suivi du challenge DNS-01 impossible: {error}"))?;
            if format!("{:?}", current.status) == "Valid" {
                break;
            }
            if format!("{:?}", current.status) == "Invalid" {
                cleanup_txt_records(settings, &mut txt_record_ids).await;
                return Err("Challenge DNS-01 refusé par Let's Encrypt.".to_string());
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    let key_pair = KeyPair::generate().map_err(|error| format!("Création clé TLS impossible: {error}"))?;
    let mut params = CertificateParams::new(vec![local_host.to_string()])
        .map_err(|error| format!("Création CSR impossible: {error}"))?;
    // Let's Encrypt refuse le Common Name par défaut de rcgen.
    params.distinguished_name = DistinguishedName::new();
    log::info!("Finalizing ACME order with SAN-only CSR for {local_host}");
    let csr = params
        .serialize_request(&key_pair)
        .map_err(|error| format!("Sérialisation CSR impossible: {error}"))?;

    for _ in 0..12 {
        let state = order
            .refresh()
            .await
            .map_err(|error| format!("Rafraîchissement commande ACME impossible: {error}"))?;
        if matches!(state.status, OrderStatus::Ready | OrderStatus::Valid) {
            break;
        }
        if matches!(state.status, OrderStatus::Invalid) {
            cleanup_txt_records(settings, &mut txt_record_ids).await;
            return Err("Commande ACME invalide.".to_string());
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    if order.state().status != OrderStatus::Valid {
        if let Err(error) = order.finalize(csr.der().as_ref()).await {
            cleanup_txt_records(settings, &mut txt_record_ids).await;
            return Err(format!("Finalisation certificat ACME impossible: {error}"));
        }
    }

    let mut cert_pem = None;
    for _ in 0..12 {
        if let Some(cert) = order
            .certificate()
            .await
            .map_err(|error| format!("Téléchargement certificat ACME impossible: {error}"))?
        {
            cert_pem = Some(cert);
            break;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    let cert_pem = cert_pem.ok_or_else(|| "Certificat ACME non disponible.".to_string())?;
    fs::write(&paths.cert, cert_pem).map_err(|error| format!("Sauvegarde certificat HTTPS impossible: {error}"))?;
    fs::write(&paths.key, key_pair.serialize_pem())
        .map_err(|error| format!("Sauvegarde clé HTTPS impossible: {error}"))?;

    cleanup_txt_records(settings, &mut txt_record_ids).await;

    let expires_at = (Utc::now() + ChronoDuration::days(90)).to_rfc3339();
    sqlx::query(
        "UPDATE settings SET
            \"secureBridgeCertificateExpiresAt\" = $1,
            \"secureBridgeLastError\" = NULL
         WHERE id = 1",
    )
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "sauvegarde expiration certificat"))?;

    Ok(())
}

#[derive(Debug)]
pub(super) struct CertificatePaths {
    pub(super) dir: PathBuf,
    pub(super) cert: PathBuf,
    pub(super) key: PathBuf,
}

pub(super) fn certificate_paths(data_dir: &Path, device_id: Option<&str>) -> Option<CertificatePaths> {
    let device_id = device_id.filter(|device_id| !device_id.trim().is_empty())?;
    let dir = data_dir.join(dmx_core::legacy::BRIDGE_DIRECTORY).join(device_id);
    Some(CertificatePaths {
        cert: dir.join("cert.pem"),
        key: dir.join("key.pem"),
        dir,
    })
}

async fn wait_for_dns_txt(name: &str, expected_value: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let mut last_seen = Vec::new();
    for attempt in 0..30 {
        match client
            .get("https://cloudflare-dns.com/dns-query")
            .header("accept", "application/dns-json")
            .query(&[("name", name), ("type", "TXT")])
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let body = response
                    .text()
                    .await
                    .map_err(|error| format!("Réponse DNS-01 illisible: {error}"))?;
                if let Ok(parsed) = serde_json::from_str::<DnsJsonResponse>(&body) {
                    last_seen = parsed
                        .answer
                        .unwrap_or_default()
                        .into_iter()
                        .map(|answer| normalize_txt_answer(&answer.data))
                        .collect();
                    if last_seen.iter().any(|value| value == expected_value) {
                        return Ok(());
                    }
                }
            }
            Ok(response) => last_seen = vec![format!("HTTP {}", response.status())],
            Err(error) => last_seen = vec![error.to_string()],
        }

        tokio::time::sleep(Duration::from_secs(if attempt < 6 { 5 } else { 10 })).await;
    }

    let detail = if last_seen.is_empty() {
        "aucun TXT visible".to_string()
    } else {
        format!("TXT visibles: {}", last_seen.join(", "))
    };
    Err(format!(
        "TXT DNS-01 non propagé pour {name} avant validation Let's Encrypt ({detail})."
    ))
}

fn normalize_txt_answer(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("\" \"", "")
        .replace("\\\"", "\"")
}

async fn cleanup_txt_records(settings: &SecureBridgeSettings, txt_record_ids: &mut Vec<String>) {
    for txt_id in std::mem::take(txt_record_ids) {
        let _ = managed_delete_txt(settings, &txt_id).await;
    }
}
