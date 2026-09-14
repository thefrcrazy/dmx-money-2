//! Pont compagnon mobile (PWA) : serveur HTTPS local du noyau, piloté depuis les réglages.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use dmx_bridge::{BridgeEvents, BridgeHost, MobileCompanion, MobileCompanionStatus};
use gtk::glib;

use crate::store::Store;

thread_local! {
    static COMPANION: RefCell<Option<Arc<MobileCompanion>>> = const { RefCell::new(None) };
}

/// Réveille l'interface quand la PWA écrit ou que l'état du pont change.
struct Events {
    sender: async_channel::Sender<()>,
}

impl BridgeEvents for Events {
    fn data_changed(&self, _data_version: i64) {
        let _ = self.sender.send_blocking(());
    }

    fn status_changed(&self) {
        let _ = self.sender.send_blocking(());
    }
}

/// Démarre le pont (il ne sert la PWA que si l'utilisateur a activé le mode compagnon).
pub fn start(store: &Rc<Store>, assets_dir: Option<PathBuf>) {
    if !store.bridge_available() || COMPANION.with(|slot| slot.borrow().is_some()) {
        return;
    }
    let (sender, receiver) = async_channel::unbounded::<()>();
    dmx_bridge::install_crypto_provider();
    let companion = MobileCompanion::new(BridgeHost {
        pool: store.engine().pool().clone(),
        runtime: store.engine().handle(),
        data_dir: store.engine().data_dir().to_path_buf(),
        assets_dir,
        events: Arc::new(Events { sender }),
    });
    if let Err(error) = companion.bootstrap() {
        log::warn!("Pont PWA indisponible : {error}");
        return;
    }
    COMPANION.with(|slot| *slot.borrow_mut() = Some(companion));

    let store_for_events = store.clone();
    glib::spawn_future_local(async move {
        while receiver.recv().await.is_ok() {
            store_for_events.reload();
        }
    });
}

pub fn companion() -> Option<Arc<MobileCompanion>> {
    COMPANION.with(|slot| slot.borrow().clone())
}

pub fn status() -> Option<MobileCompanionStatus> {
    companion().and_then(|companion| companion.status().ok())
}

pub fn set_enabled(enabled: bool) -> Result<MobileCompanionStatus, String> {
    companion()
        .ok_or_else(|| "Pont non démarré.".to_string())?
        .set_secure_bridge_enabled(enabled)
        .map_err(|error| error.to_string())
}

pub fn regenerate_pairing() -> Result<MobileCompanionStatus, String> {
    companion()
        .ok_or_else(|| "Pont non démarré.".to_string())?
        .regenerate_secure_pairing_token()
        .map_err(|error| error.to_string())
}

pub fn revoke_passkey(id: &str) -> Result<MobileCompanionStatus, String> {
    companion()
        .ok_or_else(|| "Pont non démarré.".to_string())?
        .revoke_mobile_passkey(id.to_string())
        .map_err(|error| error.to_string())
}

pub fn shutdown() {
    if let Some(companion) = COMPANION.with(|slot| slot.borrow_mut().take()) {
        companion.shutdown();
    }
}
