//! Noyau métier de DmxMoney 2 : base SQLite compatible 1.x, règles de calcul,
//! imports bancaires, sauvegardes `.dmx` et synchronisation.
//!
//! Les interfaces natives n'effectuent aucun calcul : elles appellent ce crate
//! (directement en Rust, ou via `dmx-ffi` depuis Swift et C#).

pub mod accounts_view;
pub mod analytics;
pub mod assistant;
pub mod backup;
pub mod budget;
pub mod dashboard;
pub mod dates;
pub mod db;
pub mod engine;
pub mod error;
pub mod format;
pub mod import;
pub mod journal;
pub mod legacy;
pub mod metrics;
pub mod models;
pub mod ops;
pub mod palette;
pub mod predictions;
pub mod repo;
pub mod scheduled_view;
pub mod scheduling;
pub mod seed;
pub mod settings;
pub mod snapshot;
pub mod sync;
pub mod text;

pub use engine::{Engine, EngineConfig, OpenReport};
pub use error::{CoreError, CoreResult};
