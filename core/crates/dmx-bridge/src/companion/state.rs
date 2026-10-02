use super::*;

struct RelayRuntime {
    stop: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
    connected: Arc<AtomicBool>,
}

/// Compagnon Internet de l'application de bureau, sans serveur réseau entrant.
pub struct MobileCompanion {
    host: BridgeHost,
    closed: AtomicBool,
    secure_pairing: Mutex<Option<(String, String)>>,
    relay_runtime: Mutex<Option<RelayRuntime>>,
    configuration_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    configuration_lock: tokio::sync::Mutex<()>,
}

impl MobileCompanion {
    pub fn new(host: BridgeHost) -> Arc<Self> {
        Arc::new(Self {
            host,
            closed: AtomicBool::new(false),
            secure_pairing: Mutex::new(None),
            relay_runtime: Mutex::new(None),
            configuration_task: Mutex::new(None),
            configuration_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub fn host(&self) -> &BridgeHost {
        &self.host
    }

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

    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.stop_configuration();
        self.stop_relay();
    }

    /// Prépare l'accès Internet en arrière-plan ; une panne réseau ne bloque pas l'ouverture du bureau.
    pub async fn bootstrap_async(self: &Arc<Self>) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        if secure::load_settings(&self.host.pool).await?.enabled {
            let mut task = self.configuration_task.lock().map_err(|error| error.to_string())?;
            if task.as_ref().is_some_and(|task| !task.is_finished()) {
                return Ok(());
            }
            let weak = Arc::downgrade(self);
            *task = Some(self.host.runtime.spawn(async move {
                let mut delay = 2;
                loop {
                    let Some(companion) = weak.upgrade() else {
                        return;
                    };
                    let guard = companion.configuration_lock.lock().await;
                    if companion.closed.load(Ordering::SeqCst) {
                        return;
                    }
                    if !secure::load_settings(&companion.host.pool)
                        .await
                        .is_ok_and(|settings| settings.enabled)
                    {
                        return;
                    }
                    match relay::enable_internet(&companion.host.pool).await {
                        Ok(()) => {
                            if let Ok(Some(config)) = relay::load_config(&companion.host.pool).await {
                                if secure::load_settings(&companion.host.pool)
                                    .await
                                    .is_ok_and(|settings| settings.enabled)
                                {
                                    companion.start_relay(config);
                                }
                            }
                            companion.host.events.status_changed();
                            return;
                        }
                        Err(error) => {
                            let _ = sqlx::query("UPDATE settings SET \"secureBridgeLastError\"=$1 WHERE id=1")
                                .bind(&error)
                                .execute(&companion.host.pool)
                                .await;
                            companion.host.events.status_changed();
                        }
                    }
                    drop(guard);
                    drop(companion);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    delay = (delay * 2).min(30);
                }
            }));
        }
        Ok(())
    }

    pub async fn status_async(&self) -> Result<MobileCompanionStatus, String> {
        let _guard = self.configuration_lock.lock().await;
        let mut settings = secure::load_settings(&self.host.pool).await?;
        if self.closed.load(Ordering::SeqCst) {
            settings.enabled = false;
        }
        if !settings.enabled {
            self.stop_relay();
        }
        let config = if settings.enabled {
            relay::load_config(&self.host.pool).await?
        } else {
            None
        };
        let bridge = if let Some(config) = config {
            // Normalize an existing configuration locally. Only a new device needs a network enrollment.
            relay::prepare_existing(&self.host.pool, &config).await?;
            if settings.enabled {
                self.start_relay(config.clone());
            } else {
                self.stop_relay();
            }
            let active = settings.enabled
                && self
                    .relay_runtime
                    .lock()
                    .map_err(|e| e.to_string())?
                    .as_ref()
                    .is_some_and(|current| current.connected.load(Ordering::SeqCst));
            relay::build_status(
                &self.host.pool,
                &config,
                settings.enabled,
                active,
                if settings.enabled {
                    self.valid_pairing().await?
                } else {
                    None
                },
            )
            .await?
        } else {
            self.stop_relay();
            relay::unconfigured_status(&self.host.pool, &settings).await?
        };
        Ok(MobileCompanionStatus {
            enabled: bridge.enabled,
            active: bridge.active,
            host: None,
            port: None,
            url: bridge.app_url.clone(),
            data_version: get_data_version(&self.host.pool).await?,
            secure_bridge: Some(bridge),
        })
    }

    async fn valid_pairing(&self) -> Result<Option<(String, String)>, String> {
        let pairing = self.secure_pairing.lock().map_err(|error| error.to_string())?.clone();
        if let Some((token, _)) = pairing.as_ref() {
            let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mobile_pairing_tokens WHERE token_hash=$1 AND consumed_at IS NULL AND expires_at>$2)")
                .bind(secure::hash_secret(token)).bind(chrono::Utc::now().to_rfc3339())
                .fetch_one(&self.host.pool).await.map_err(|error| map_db_error(error, "vérification du QR"))?;
            if !valid {
                let mut current = self.secure_pairing.lock().map_err(|error| error.to_string())?;
                // A newly generated QR may have replaced the one checked above.
                if current.as_ref().map(|(token, _)| token) == pairing.as_ref().map(|(token, _)| token) {
                    *current = None;
                }
                return Ok(None);
            }
        }
        Ok(pairing)
    }

    pub async fn set_secure_bridge_enabled_async(&self, enabled: bool) -> Result<MobileCompanionStatus, String> {
        if !enabled {
            self.stop_configuration();
        }
        {
            let _guard = self.configuration_lock.lock().await;
            if self.closed.load(Ordering::SeqCst) {
                return Err("Compagnon arrêté.".into());
            }
            secure::set_enabled(&self.host.pool, enabled).await?;
            if !enabled {
                self.stop_relay();
                *self.secure_pairing.lock().map_err(|error| error.to_string())? = None;
            }
        }
        self.status_async().await
    }

    fn start_relay(&self, config: relay::RelayConfig) {
        let Ok(mut current) = self.relay_runtime.lock() else {
            return;
        };
        if self.closed.load(Ordering::SeqCst) || current.as_ref().is_some_and(|current| !current.task.is_finished()) {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(false));
        let task = self.host.runtime.spawn({
            let host = self.host.clone();
            let stop = stop.clone();
            let connected = connected.clone();
            async move {
                #[cfg(test)]
                std::future::pending::<()>().await;
                relay::run(host, config, stop, connected).await;
            }
        });
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

    fn stop_configuration(&self) {
        if let Ok(mut task) = self.configuration_task.lock() {
            if let Some(task) = task.take() {
                task.abort();
            }
        }
    }

    pub async fn regenerate_secure_pairing_token_async(&self) -> Result<MobileCompanionStatus, String> {
        {
            let _guard = self.configuration_lock.lock().await;
            if self.closed.load(Ordering::SeqCst) || !secure::load_settings(&self.host.pool).await?.enabled {
                return Err("Activez le compagnon Internet avant d’appairer un téléphone.".into());
            }
            relay::enable_internet(&self.host.pool).await?;
            let pairing = secure::regenerate_pairing_token(&self.host.pool).await?;
            *self.secure_pairing.lock().map_err(|error| error.to_string())? = Some(pairing);
        }
        self.status_async().await
    }

    pub async fn revoke_mobile_passkey_async(&self, passkey_id: String) -> Result<MobileCompanionStatus, String> {
        secure::revoke_passkey(&self.host.pool, passkey_id).await?;
        self.status_async().await
    }
}

impl Drop for MobileCompanion {
    fn drop(&mut self) {
        self.stop_configuration();
        self.stop_relay();
    }
}
