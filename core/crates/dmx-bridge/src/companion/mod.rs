//! Serveur local de l'API compagnon (port de `src-tauri/src/mobile_companion`).

use crate::secure::{self, SecureBridgeStatus};
use crate::BridgeHost;
use dmx_core::db::DbPool;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

mod api;
mod assets;
mod bank_sync;
mod http;
mod response;
mod settings;
mod state;
mod types;
mod url;
mod validation;

use self::api::route_api_request;
use self::assets::serve_static_asset;
use self::http::{server_loop, HttpRequest};
use self::response::{auth_response, empty_response, error_response, json_response, write_response, HttpResponse};
pub use self::settings::detect_local_ip;
use self::settings::{bind_listener, get_data_version, load_mobile_settings, map_db_error};
use self::types::{MobileSettings, ServerRuntime, ServerSecurity};
use self::url::{percent_decode, strip_query};

/// Délai avant la première passe de maintenance, pour ne jamais ralentir le démarrage.
const MAINTENANCE_START_DELAY_SECS: u64 = 45;
/// Six heures : assez pour suivre un changement d'adresse locale, négligeable sinon.
const MAINTENANCE_INTERVAL_SECS: u64 = 6 * 60 * 60;

pub use self::state::MobileCompanion;
pub use self::types::MobileCompanionStatus;

#[cfg(test)]
mod tests;
