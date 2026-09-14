//! Pont compagnon mobile de DmxMoney : serveur HTTPS local servant l'API de la PWA,
//! authentification par clé d'accès et provisionnement DNS/ACME via le service managé.
//!
//! Port de `src-tauri/src/mobile_companion` et `src-tauri/src/secure_bridge` (1.x). Le contrat
//! HTTP est inchangé pour que la PWA existante fonctionne sans modification ; les dépendances à
//! Tauri sont remplacées par [`BridgeHost`].

pub mod companion;
pub mod secure;

use dmx_core::db::DbPool;
use std::path::PathBuf;
use std::sync::Arc;

pub use companion::{MobileCompanion, MobileCompanionStatus};
pub use secure::{MobilePasskeyInfo, SecureBridgeStatus};

/// Notifications envoyées à l'application hôte.
pub trait BridgeEvents: Send + Sync + 'static {
    /// Des données ont été modifiées depuis la PWA.
    fn data_changed(&self, data_version: i64);
    /// L'état du pont a changé (certificat renouvelé, provisionnement terminé…).
    fn status_changed(&self);
    /// Reformulation d'une demande de l'assistant par l'hôte (modèle sur l'appareil).
    ///
    /// La PWA envoie du texte libre ; l'application de bureau peut le normaliser avec son modèle
    /// local avant que le noyau ne l'analyse. Sans modèle, `None` : la phrase est analysée telle
    /// quelle et la réponse reste la même.
    fn rephrase_assistant_request(&self, _text: &str) -> Option<String> {
        None
    }
}

/// Événements ignorés (tests, outils en ligne de commande).
pub struct NoopEvents;

impl BridgeEvents for NoopEvents {
    fn data_changed(&self, _data_version: i64) {}
    fn status_changed(&self) {}
}

/// Ce que l'application hôte fournit au pont.
#[derive(Clone)]
pub struct BridgeHost {
    pub pool: DbPool,
    /// Runtime tokio du moteur ; les appels synchrones du pont ne doivent pas venir de ses threads.
    pub runtime: tokio::runtime::Handle,
    /// Dossier de données de l'application (certificats dans `secure-bridge/<appareil>`).
    pub data_dir: PathBuf,
    /// Build de la PWA servi localement ; sans lui, les pages redirigent vers la PWA publique.
    pub assets_dir: Option<PathBuf>,
    pub events: Arc<dyn BridgeEvents>,
}

/// Installe le fournisseur cryptographique rustls ; à appeler une fois au démarrage.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
