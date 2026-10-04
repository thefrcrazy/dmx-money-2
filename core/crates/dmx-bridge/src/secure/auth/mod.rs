use super::*;

mod challenges;
mod passkeys;
mod routes;
mod sessions;

pub(super) use self::passkeys::list_passkeys;
pub use self::routes::{authorize_api_request, handle_auth_request, regenerate_pairing_token, revoke_passkey};

use self::challenges::{delete_challenge, load_challenge, store_challenge};
use self::passkeys::{
    find_passkey_by_credential_id, insert_passkey, list_active_credentials_for_device, list_active_passkeys,
    update_passkey_usage,
};
use self::sessions::{authorize_session_for_auth, create_session, resume_session, revoke_session, session_response};

async fn housekeeping(pool: &DbPool) {
    use std::sync::atomic::{AtomicI64, Ordering};
    static LAST_CLEANUP: AtomicI64 = AtomicI64::new(0);
    let now = Utc::now().timestamp();
    let previous = LAST_CLEANUP.load(Ordering::Relaxed);
    if now - previous < 60
        || LAST_CLEANUP
            .compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        return;
    }
    if let Err(error) = dmx_core::db::purge_expired_companion_records(pool).await {
        log::warn!("Nettoyage des sessions du compagnon différé : {error}");
        LAST_CLEANUP.store(previous, Ordering::Relaxed);
    }
}
