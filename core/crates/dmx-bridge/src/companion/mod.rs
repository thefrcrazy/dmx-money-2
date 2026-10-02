//! Compagnon Internet : dispatch des requêtes vers le noyau par un relais chiffré sortant.

use crate::secure::{self, SecureBridgeStatus};
use crate::BridgeHost;
use dmx_core::db::DbPool;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

mod api;
mod bank_sync;
mod http;
pub(crate) mod relay;
mod response;
mod settings;
mod state;
mod types;
mod url;
mod validation;

use self::api::route_api_request;
use self::http::HttpRequest;
use self::response::{auth_response, empty_response, error_response, json_response, HttpResponse};
use self::settings::{get_data_version, map_db_error};
use self::url::{percent_decode, strip_query};

pub use self::state::MobileCompanion;
pub use self::types::MobileCompanionStatus;

#[cfg(test)]
mod tests;
