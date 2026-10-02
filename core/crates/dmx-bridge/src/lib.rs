//! Compagnon mobile DmxMoney : PWA Cloudflare Pages, relais chiffré sortant et passkeys.
//! Les requêtes authentifiées sont exécutées par le noyau sur l’ordinateur.

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
    /// La connexion au relais ou l’appairage mobile a changé.
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
    /// Dossier de données de l’application fourni par l’hôte.
    pub data_dir: PathBuf,
    /// Champ conservé pour les bindings existants ; la PWA est hébergée sur Cloudflare Pages.
    pub assets_dir: Option<PathBuf>,
    pub events: Arc<dyn BridgeEvents>,
}

/// Installe le fournisseur cryptographique rustls ; à appeler une fois au démarrage.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
