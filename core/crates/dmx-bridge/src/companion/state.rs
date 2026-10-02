use super::*;

/// Serveur compagnon et pont sécurisé d'une application desktop.
///
/// Les méthodes synchrones bloquent le thread appelant ; elles ne doivent pas être appelées depuis
/// le runtime tokio fourni par l'hôte.
struct RelayRuntime {
    stop: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
    connected: Arc<AtomicBool>,
}

pub struct MobileCompanion {
    host: BridgeHost,
    runtime: Mutex<Option<ServerRuntime>>,
    secure_pairing: Mutex<Option<(String, String)>>,
    maintenance_started: AtomicBool,
    relay_runtime: Mutex<Option<RelayRuntime>>,
}

impl MobileCompanion {
    pub fn new(host: BridgeHost) -> Arc<Self> {
        Arc::new(Self {
            host,
            runtime: Mutex::new(None),
            secure_pairing: Mutex::new(None),
            maintenance_started: AtomicBool::new(false),
            relay_runtime: Mutex::new(None),
        })
    }

    pub fn host(&self) -> &BridgeHost {
        &self.host
    }

    /// Démarre le serveur si le mode compagnon est actif et lance la maintenance périodique.
    pub fn bootstrap(self: &Arc<Self>) -> Result<(), String> {
        self.host.runtime.block_on(self.bootstrap_async())
    }

    pub fn status(&self) -> Result<MobileCompanionStatus, String> {
        self.host.runtime.block_on(self.status_async())
    }

    pub fn set_secure_bridge_enabled(&self, enabled: bool) -> Result<MobileCompanionStatus, String> {
        self.host
            .runtime
            .block_on(self.set_secure_bridge_enabled_async(enabled))
    }

    pub fn regenerate_secure_pairing_token(&self) -> Result<MobileCompanionStatus, String> {
        self.host.runtime.block_on(self.regenerate_secure_pairing_token_async())
    }

    pub fn revoke_mobile_passkey(&self, passkey_id: String) -> Result<MobileCompanionStatus, String> {
        self.host.runtime.block_on(self.revoke_mobile_passkey_async(passkey_id))
    }

    /// Arrête le serveur local (fermeture de l'application).
    pub fn shutdown(&self) {
        self.stop_server();
        self.stop_relay();
    }

    pub async fn bootstrap_async(self: &Arc<Self>) -> Result<(), String> {
        let settings = load_mobile_settings(&self.host.pool).await?;
        let secure_enabled = secure::load_settings(&self.host.pool)
            .await
            .map(|settings| settings.enabled)
            .unwrap_or(false);
        if settings.enabled || secure_enabled {
            if let Some(config) = relay::load_config(&self.host.pool).await? {
                if secure_enabled {
                    self.start_relay(config);
                }
            } else {
                self.start_server(settings.port).await?;
            }
        }
        self.spawn_secure_bridge_maintenance();
        Ok(())
    }

    pub async fn status_async(&self) -> Result<MobileCompanionStatus, String> {
        let settings = load_mobile_settings(&self.host.pool).await?;
        let secure_enabled = secure::load_settings(&self.host.pool)
            .await
            .map(|settings| settings.enabled)
            .unwrap_or(false);
        if let Some(config) = relay::load_config(&self.host.pool).await? {
            if secure_enabled {
                self.start_relay(config.clone());
            } else {
                self.stop_relay();
            }
            self.stop_server();
            let active = secure_enabled
                && self
                    .relay_runtime
                    .lock()
                    .map_err(|e| e.to_string())?
                    .as_ref()
                    .is_some_and(|current| current.connected.load(Ordering::SeqCst));
            let pairing = self.secure_pairing.lock().map_err(|e| e.to_string())?.clone();
            let bridge = relay::build_status(&self.host.pool, &config, secure_enabled, active, pairing).await?;
            return Ok(MobileCompanionStatus {
                enabled: secure_enabled,
                active,
                host: None,
                port: None,
                url: bridge.app_url.clone(),
                data_version: get_data_version(&self.host.pool).await?,
                secure_bridge: Some(bridge),
            });
        }
        if secure_enabled {
            let _ = secure::ensure_auto_configuration(&self.host.pool, &self.host.data_dir).await;
        }

        if settings.enabled || secure_enabled {
            self.start_server(settings.port).await?;
        } else {
            self.stop_server();
        }

        let runtime = self.runtime.lock().map_err(|error| error.to_string())?.clone();
        let active = runtime
            .as_ref()
            .map(|server| !server.stop.load(Ordering::SeqCst))
            .unwrap_or(false);
        let secure_pairing = self.secure_pairing.lock().map_err(|error| error.to_string())?.clone();

        let secure_bridge = secure::build_status(
            &self.host.pool,
            &self.host.data_dir,
            active,
            runtime.as_ref().map(|server| server.port),
            secure_pairing,
            self.host.assets_dir.is_some(),
        )
        .await?;

        Ok(MobileCompanionStatus {
            enabled: settings.enabled,
            active,
            host: runtime.as_ref().map(|server| server.host.clone()),
            port: runtime.as_ref().map(|server| server.port),
            url: runtime.as_ref().map(|server| server.url.clone()),
            data_version: get_data_version(&self.host.pool).await?,
            secure_bridge: Some(secure_bridge),
        })
    }

    async fn start_server(&self, preferred_port: u16) -> Result<(), String> {
        let secure_settings = secure::load_settings(&self.host.pool).await.ok();
        let tls_config = match secure_settings.as_ref() {
            Some(settings) => secure::load_tls_config(&self.host.data_dir, settings).await?,
            None => None,
        };
        let secure = tls_config.is_some();
        let tls_fingerprint = secure
            .then(|| {
                secure_settings
                    .as_ref()
                    .and_then(|settings| secure::certificate_fingerprint(&self.host.data_dir, settings))
            })
            .flatten();

        if let Some(current) = self.runtime.lock().map_err(|error| error.to_string())?.clone() {
            if current.secure == secure
                && current.tls_fingerprint == tls_fingerprint
                && !current.stop.load(Ordering::SeqCst)
            {
                return Ok(());
            }
            current.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(("127.0.0.1", current.port));
        }

        let (listener, port) = bind_listener(preferred_port)?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("Configuration du serveur mobile impossible: {error}"))?;

        let host = if secure {
            secure_settings
                .as_ref()
                .and_then(|settings| settings.local_host.clone())
                .unwrap_or_else(detect_local_ip)
        } else {
            detect_local_ip()
        };
        let scheme = if secure { "https" } else { "http" };
        let url = format!("{scheme}://{host}:{port}/mobile");
        let stop = Arc::new(AtomicBool::new(false));

        sqlx::query("UPDATE settings SET \"mobileAccessPort\" = $1 WHERE id = 1")
            .bind(i64::from(port))
            .execute(&self.host.pool)
            .await
            .map_err(|error| map_db_error(error, "sauvegarde du port mobile"))?;

        *self.runtime.lock().map_err(|error| error.to_string())? = Some(ServerRuntime {
            host,
            port,
            url,
            secure,
            tls_fingerprint,
            stop: stop.clone(),
        });

        let security = ServerSecurity {
            secure_app_origin: secure_settings.as_ref().and_then(secure::secure_app_origin),
            app_url: secure_settings.as_ref().and_then(|settings| settings.app_url.clone()),
        };
        let host = self.host.clone();
        thread::spawn(move || server_loop(listener, host, security, tls_config, stop));

        Ok(())
    }

    fn stop_server(&self) {
        if let Ok(mut runtime) = self.runtime.lock() {
            if let Some(server) = runtime.take() {
                server.stop.store(true, Ordering::SeqCst);
                let _ = TcpStream::connect(("127.0.0.1", server.port));
            }
        }
    }

    pub async fn set_secure_bridge_enabled_async(&self, enabled: bool) -> Result<MobileCompanionStatus, String> {
        log::info!("Mobile companion secure bridge toggle requested: enabled={enabled}");
        secure::set_enabled(&self.host.pool, &self.host.data_dir, enabled).await?;
        if enabled {
            // Explicit activation can change the WebAuthn origin and consume the
            // old pairing tokens. Never render that stale token in a new Pages QR.
            *self.secure_pairing.lock().map_err(|error| error.to_string())? = None;
        }
        sqlx::query("UPDATE settings SET \"mobileAccessEnabled\" = $1 WHERE id = 1")
            .bind(enabled)
            .execute(&self.host.pool)
            .await
            .map_err(|error| map_db_error(error, "liaison du mode compagnon sécurisé"))?;

        if let Some(config) = relay::load_config(&self.host.pool).await? {
            self.stop_server();
            if enabled {
                self.start_relay(config);
            } else {
                self.stop_relay();
            }
            return self.status_async().await;
        }
        let settings = load_mobile_settings(&self.host.pool).await?;
        if settings.enabled || enabled {
            self.stop_server();
            self.start_server(settings.port).await?;
            if enabled {
                self.spawn_secure_bridge_refresh();
            }
        } else {
            self.stop_server();
        }
        self.status_async().await
    }

    fn start_relay(&self, config: relay::RelayConfig) {
        let Ok(mut current) = self.relay_runtime.lock() else {
            return;
        };
        if current.as_ref().is_some_and(|current| !current.task.is_finished()) {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(false));
        let task = self
            .host
            .runtime
            .spawn(relay::run(self.host.clone(), config, stop.clone(), connected.clone()));
        *current = Some(RelayRuntime { stop, task, connected });
    }

    fn stop_relay(&self) {
        if let Ok(mut current) = self.relay_runtime.lock() {
            if let Some(current) = current.take() {
                current.stop.store(true, Ordering::SeqCst);
                current.connected.store(false, Ordering::SeqCst);
                current.task.abort();
            }
        }
    }

    fn spawn_secure_bridge_refresh(&self) {
        let host = self.host.clone();
        self.host.runtime.spawn(async move {
            log::info!("Secure bridge infrastructure refresh started in background");
            match secure::refresh_infrastructure(&host.pool, &host.data_dir, false).await {
                Ok(_) => log::info!("Secure bridge infrastructure refresh completed"),
                Err(error) => log::error!("Secure bridge infrastructure refresh failed: {error}"),
            }
            host.events.status_changed();
        });
    }

    /// Garde l'enregistrement DNS aligné sur l'adresse locale et renouvelle le certificat avant
    /// expiration. Sans elle, un desktop fermé plusieurs semaines revient avec un DNS périmé et un
    /// certificat expiré, précisément quand l'appairage échouait en 1.x.
    fn spawn_secure_bridge_maintenance(self: &Arc<Self>) {
        if self.maintenance_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        self.host.runtime.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(MAINTENANCE_START_DELAY_SECS)).await;
                let Some(companion) = weak.upgrade() else {
                    return;
                };
                let host = companion.host.clone();
                drop(companion);

                let enabled = secure::load_settings(&host.pool)
                    .await
                    .map(|settings| settings.enabled)
                    .unwrap_or(false);
                if enabled {
                    match secure::refresh_infrastructure(&host.pool, &host.data_dir, false).await {
                        Ok(_) => log::info!("Secure bridge maintenance pass completed"),
                        Err(error) => log::warn!("Secure bridge maintenance pass failed: {error}"),
                    }
                    host.events.status_changed();
                }

                tokio::time::sleep(Duration::from_secs(
                    MAINTENANCE_INTERVAL_SECS - MAINTENANCE_START_DELAY_SECS,
                ))
                .await;
            }
        });
    }

    pub async fn regenerate_secure_pairing_token_async(&self) -> Result<MobileCompanionStatus, String> {
        // Un renouvellement managé refusé ne bloque pas l'appairage : tant que le pont HTTPS local
        // répond, le QR n'a besoin que d'un nouveau jeton local.
        if let Err(error) = secure::ensure_auto_configuration(&self.host.pool, &self.host.data_dir).await {
            let serving = self
                .runtime
                .lock()
                .map_err(|error| error.to_string())?
                .as_ref()
                .map(|server| server.secure && !server.stop.load(Ordering::SeqCst))
                .unwrap_or(false);
            if !serving {
                return Err(error);
            }
            log::warn!("Pairing token generated while the managed bridge is degraded: {error}");
        }
        let pairing = secure::regenerate_pairing_token(&self.host.pool).await?;
        *self.secure_pairing.lock().map_err(|error| error.to_string())? = Some(pairing);
        self.status_async().await
    }

    pub async fn revoke_mobile_passkey_async(&self, passkey_id: String) -> Result<MobileCompanionStatus, String> {
        secure::revoke_passkey(&self.host.pool, passkey_id).await?;
        self.status_async().await
    }
}

impl Drop for MobileCompanion {
    fn drop(&mut self) {
        self.stop_server();
        self.stop_relay();
    }
}
