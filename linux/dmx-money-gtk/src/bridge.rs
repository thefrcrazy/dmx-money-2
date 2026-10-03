//! Compagnon mobile : relais Internet chiffré à connexion sortante.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};
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
    pending: Arc<AtomicU8>,
}

impl BridgeEvents for Events {
    fn data_changed(&self, _data_version: i64) {
        self.signal(1);
    }

    fn status_changed(&self) {
        self.signal(2);
    }
}

impl Events {
    fn signal(&self, event: u8) {
        self.pending.fetch_or(event, Ordering::AcqRel);
        // Un seul réveil suffit : les types d’événements restent dans le masque.
        let _ = self.sender.try_send(());
    }
}

/// Démarre le compagnon selon la configuration déjà activée par l'utilisateur.
pub fn start(store: &Rc<Store>, assets_dir: Option<PathBuf>) {
    if !store.bridge_available() || COMPANION.with(|slot| slot.borrow().is_some()) {
        return;
    }
    let (sender, receiver) = async_channel::bounded::<()>(1);
    let pending = Arc::new(AtomicU8::new(0));
    dmx_bridge::install_crypto_provider();
    let companion = MobileCompanion::new(BridgeHost {
        pool: store.engine().pool().clone(),
        runtime: store.engine().handle(),
        data_dir: store.engine().data_dir().to_path_buf(),
        assets_dir,
        events: Arc::new(Events {
            sender,
            pending: pending.clone(),
        }),
    });
    if let Err(error) = companion.bootstrap() {
        log::warn!("Pont PWA indisponible : {error}");
        return;
    }
    COMPANION.with(|slot| *slot.borrow_mut() = Some(companion));

    let weak_store = Rc::downgrade(store);
    glib::spawn_future_local(async move {
        while receiver.recv().await.is_ok() {
            let Some(store_for_events) = weak_store.upgrade() else {
                break;
            };
            let events = pending.swap(0, Ordering::AcqRel);
            if events & 1 != 0 {
                store_for_events.reload_if_changed();
            }
            if events & 2 != 0 {
                store_for_events.refresh_bridge_status();
            }
        }
    });
}

pub fn companion() -> Option<Arc<MobileCompanion>> {
    COMPANION.with(|slot| slot.borrow().clone())
}

pub fn status() -> Option<MobileCompanionStatus> {
    companion().and_then(|companion| companion.status().ok())
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_bursts_use_one_wakeup_and_preserve_data_and_status() {
        let (sender, receiver) = async_channel::bounded(1);
        let pending = Arc::new(AtomicU8::new(0));
        let events = Events {
            sender,
            pending: pending.clone(),
        };
        for _ in 0..100_000 {
            events.status_changed();
        }
        events.data_changed(12);
        assert_eq!(receiver.len(), 1);
        assert!(receiver.try_recv().is_ok());
        assert_eq!(pending.swap(0, Ordering::AcqRel), 3);
        events.status_changed();
        assert!(receiver.try_recv().is_ok());
        assert_eq!(pending.swap(0, Ordering::AcqRel), 2);
    }
}
