//! Schéma SQLite. Les tables, colonnes et migrations de DmxMoney 1.x sont reprises à l'identique
//! (`src-tauri/src/db.rs`) pour ouvrir une base existante sans conversion ; la version 2 ajoute
//! uniquement les tables de synchronisation.
//! Dynamic SQL uses only internal whitelisted identifiers; all external values are bound.

use crate::error::{CoreResult, DbContext};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{SqliteConnection, SqlitePool};
use std::path::Path;
use std::time::Duration;

pub type DbPool = SqlitePool;

pub const DATABASE_FILE_NAME: &str = "dmxmoney2025.db";

/// Version du schéma propre à DmxMoney 2 (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 4;

/// Tables métier suivies par la synchronisation : (entité, table SQL).
pub const SYNC_ENTITY_TABLES: [(&str, &str); 5] = [
    ("accounts", "accounts"),
    ("categories", "categories"),
    ("budgets", "budgets"),
    ("scheduled", "scheduled_transactions"),
    ("transactions", "transactions"),
];

/// Colonnes de `settings` partagées entre appareils. La position de fenêtre et l'état du pont
/// restent propres à chaque machine.
pub const SYNCED_SETTINGS_COLUMNS: [&str; 24] = [
    "theme",
    "primaryColor",
    "displayStyle",
    "accountGroups",
    "customGroups",
    "customGroupsOrder",
    "accountsOrder",
    "componentSpacing",
    "componentPadding",
    "lastSeenVersion",
    "dismissedBudgetSuggestions",
    "dismissedScheduledSuggestions",
    "predictionTimeRange",
    "predictionCustomEndDate",
    "predictionAlertThreshold",
    "predictionMonthStartsOnFirst",
    "predictionFakeTransactions",
    "analyticsTimeRange",
    "analyticsCustomStartDate",
    "analyticsCustomEndDate",
    "analyticsMonthStartsOnFirst",
    "analyticsHiddenExpenseCategories",
    "analyticsHiddenIncomeCategories",
    "scheduledDueRange",
];

/// Restrict the application directory and SQLite sidecars, including upgrades of old installs.
pub fn protect_data_directory(path: &Path) -> CoreResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(path)?;
    Ok(())
}
fn protect_database_files(path: &Path) -> CoreResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        for file in [
            path.to_path_buf(),
            std::path::PathBuf::from(format!("{}-wal", path.display())),
            std::path::PathBuf::from(format!("{}-shm", path.display())),
        ] {
            match std::fs::symlink_metadata(&file) {
                Ok(metadata) if metadata.is_file() => {
                    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600))?
                }
                Ok(_) => {
                    return Err(crate::error::CoreError::Io(
                        "La base et ses journaux doivent être des fichiers ordinaires, sans lien symbolique.".into(),
                    ))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

pub(crate) async fn enable_defensive_mode(connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    let mut handle = connection.lock_handle().await?;
    let mut enabled: std::ffi::c_int = 0;
    // SAFETY: SQLx's lock owns the live handle for this call. DEFENSIVE takes an int and int*;
    // both driver and direct binding are pinned to the same libsqlite3-sys ABI in Cargo.toml.
    let result = unsafe {
        libsqlite3_sys::sqlite3_db_config(
            handle.as_raw_handle().as_ptr(),
            libsqlite3_sys::SQLITE_DBCONFIG_DEFENSIVE,
            1 as std::ffi::c_int,
            &mut enabled as *mut std::ffi::c_int,
        )
    };
    if result != libsqlite3_sys::SQLITE_OK || enabled != 1 {
        return Err(sqlx::Error::Protocol(
            "Impossible d’activer la protection SQLite DEFENSIVE.".into(),
        ));
    }
    Ok(())
}

pub async fn open_pool(path: &Path) -> CoreResult<DbPool> {
    protect_database_files(path)?;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .pragma("trusted_schema", "OFF")
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .after_connect(|connection, _| Box::pin(enable_defensive_mode(connection)))
        .connect_with(options)
        .await
        .ctx("ouverture de la base")?;

    create_tables(&pool).await?;
    protect_database_files(path)?;
    Ok(pool)
}

/// Bounded housekeeping. Old clients have no replay deadline, so their receipts are retained.
/// No financial row, passkey or pairing secret is removed by this operation.
pub async fn purge_expired_companion_records(pool: &DbPool) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("nettoyage du compagnon")?;
    for statement in [
        "DELETE FROM mobile_auth_challenges WHERE id IN (SELECT id FROM mobile_auth_challenges WHERE julianday(expires_at)<=julianday('now') LIMIT 1000)",
        "DELETE FROM mobile_pairing_tokens WHERE id IN (SELECT id FROM mobile_pairing_tokens WHERE consumed_at IS NOT NULL OR julianday(expires_at)<=julianday('now') LIMIT 1000)",
        "DELETE FROM mobile_sessions WHERE id IN (SELECT id FROM mobile_sessions WHERE revoked_at IS NOT NULL OR julianday(expires_at)<=julianday('now') OR julianday(created_at)<=julianday('now','-30 days') LIMIT 1000)",
        "DELETE FROM mobile_mutation_receipts WHERE id IN (SELECT id FROM mobile_mutation_receipts WHERE replay_until IS NOT NULL AND julianday(replay_until)<julianday('now') LIMIT 1000)",
    ] { sqlx::query(statement).execute(&mut *tx).await.ctx("nettoyage du compagnon")?; }
    tx.commit().await.ctx("nettoyage du compagnon")
}

/// Base en mémoire pour les tests : une seule connexion, jamais recyclée.
pub async fn open_memory_pool() -> CoreResult<DbPool> {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true)
        .pragma("trusted_schema", "OFF");

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .after_connect(|connection, _| Box::pin(enable_defensive_mode(connection)))
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await
        .ctx("ouverture de la base en mémoire")?;

    create_tables(&pool).await?;
    Ok(pool)
}

const BASE_TABLES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS accounts (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        \"type\" TEXT NOT NULL,
        \"initialBalance\" REAL NOT NULL,
        color TEXT,
        icon TEXT
    )",
    "CREATE TABLE IF NOT EXISTS transactions (
        id TEXT PRIMARY KEY,
        date TEXT NOT NULL,
        \"accountId\" TEXT NOT NULL,
        \"type\" TEXT NOT NULL,
        amount REAL NOT NULL,
        category TEXT NOT NULL,
        description TEXT,
        checked BOOLEAN DEFAULT 0,
        \"isTransfer\" BOOLEAN DEFAULT 0,
        \"linkedTransactionId\" TEXT,
        FOREIGN KEY(\"accountId\") REFERENCES accounts(id)
    )",
    "CREATE TABLE IF NOT EXISTS categories (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        icon TEXT NOT NULL,
        color TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS scheduled_transactions (
        id TEXT PRIMARY KEY,
        description TEXT NOT NULL,
        amount REAL NOT NULL,
        \"type\" TEXT NOT NULL,
        frequency TEXT NOT NULL,
        \"accountId\" TEXT NOT NULL,
        \"nextDate\" TEXT NOT NULL,
        category TEXT NOT NULL,
        FOREIGN KEY(\"accountId\") REFERENCES accounts(id)
    )",
    "CREATE TABLE IF NOT EXISTS budgets (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        amount REAL NOT NULL,
        category TEXT NOT NULL,
        \"accountId\" TEXT,
        FOREIGN KEY(\"accountId\") REFERENCES accounts(id)
    )",
    "CREATE TABLE IF NOT EXISTS settings (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        theme TEXT NOT NULL DEFAULT 'system',
        \"primaryColor\" TEXT NOT NULL DEFAULT '#6366f1',
        \"displayStyle\" TEXT NOT NULL DEFAULT 'modern',
        \"windowPositionX\" INTEGER,
        \"windowPositionY\" INTEGER,
        \"windowSizeWidth\" INTEGER,
        \"windowSizeHeight\" INTEGER,
        \"componentSpacing\" INTEGER NOT NULL DEFAULT 6,
        \"componentPadding\" INTEGER NOT NULL DEFAULT 6,
        \"mobileAccessEnabled\" BOOLEAN NOT NULL DEFAULT 0,
        \"mobileAccessToken\" TEXT,
        \"mobileAccessPort\" INTEGER NOT NULL DEFAULT 8799,
        \"secureBridgeEnabled\" BOOLEAN NOT NULL DEFAULT 0,
        \"secureBridgeDomain\" TEXT,
        \"secureBridgeAppUrl\" TEXT,
        \"secureBridgeLocalHost\" TEXT,
        \"secureBridgeDeviceId\" TEXT,
        \"secureBridgeCertificateExpiresAt\" TEXT,
        \"secureBridgeDnsRecordId\" TEXT,
        \"secureBridgeDnsLastUpdatedAt\" TEXT,
        \"secureBridgeLastError\" TEXT,
        \"secureBridgeManagedServiceUrl\" TEXT,
        \"secureBridgeManagedRegisteredAt\" TEXT,
        \"secureBridgeManagedDeviceSecret\" TEXT,
        \"dismissedBudgetSuggestions\" TEXT,
        \"dismissedScheduledSuggestions\" TEXT,
        \"predictionTimeRange\" TEXT NOT NULL DEFAULT 'year',
        \"predictionCustomEndDate\" TEXT,
        \"predictionAlertThreshold\" REAL NOT NULL DEFAULT 0,
        \"predictionMonthStartsOnFirst\" BOOLEAN NOT NULL DEFAULT 1,
        \"predictionFakeTransactions\" TEXT,
        \"analyticsTimeRange\" TEXT NOT NULL DEFAULT 'year',
        \"analyticsCustomStartDate\" TEXT,
        \"analyticsCustomEndDate\" TEXT,
        \"analyticsMonthStartsOnFirst\" BOOLEAN NOT NULL DEFAULT 1,
        \"analyticsHiddenExpenseCategories\" TEXT,
        \"analyticsHiddenIncomeCategories\" TEXT,
        \"scheduledDueRange\" TEXT NOT NULL DEFAULT 'all',
        \"settingsRevision\" INTEGER NOT NULL DEFAULT 0,
        \"settingsFieldVersions\" TEXT NOT NULL DEFAULT '{}'
    )",
    "CREATE INDEX IF NOT EXISTS idx_transactions_account_id ON transactions(\"accountId\")",
    "CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date)",
    "CREATE INDEX IF NOT EXISTS idx_scheduled_account_id ON scheduled_transactions(\"accountId\")",
    "CREATE INDEX IF NOT EXISTS idx_budgets_account_id ON budgets(\"accountId\")",
    "CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category)",
];

/// Migrations légères 1.x, dans leur ordre d'origine : (table, colonne, instruction).
const COLUMN_MIGRATIONS: &[(&str, &str, &str)] = &[
    (
        "transactions",
        "bankSource",
        "ALTER TABLE transactions ADD COLUMN \"bankSource\" TEXT",
    ),
    (
        "transactions",
        "bankTransactionId",
        "ALTER TABLE transactions ADD COLUMN \"bankTransactionId\" TEXT",
    ),
    (
        "settings",
        "displayStyle",
        "ALTER TABLE settings ADD COLUMN \"displayStyle\" TEXT NOT NULL DEFAULT 'modern'",
    ),
    (
        "settings",
        "componentSpacing",
        "ALTER TABLE settings ADD COLUMN \"componentSpacing\" INTEGER NOT NULL DEFAULT 6",
    ),
    (
        "settings",
        "componentPadding",
        "ALTER TABLE settings ADD COLUMN \"componentPadding\" INTEGER NOT NULL DEFAULT 6",
    ),
    (
        "settings",
        "accountGroups",
        "ALTER TABLE settings ADD COLUMN \"accountGroups\" TEXT",
    ),
    (
        "settings",
        "customGroups",
        "ALTER TABLE settings ADD COLUMN \"customGroups\" TEXT",
    ),
    (
        "settings",
        "customGroupsOrder",
        "ALTER TABLE settings ADD COLUMN \"customGroupsOrder\" TEXT",
    ),
    (
        "settings",
        "accountsOrder",
        "ALTER TABLE settings ADD COLUMN \"accountsOrder\" TEXT",
    ),
    (
        "scheduled_transactions",
        "toAccountId",
        "ALTER TABLE scheduled_transactions ADD COLUMN \"toAccountId\" TEXT",
    ),
    (
        "scheduled_transactions",
        "includeInForecast",
        "ALTER TABLE scheduled_transactions ADD COLUMN \"includeInForecast\" BOOLEAN DEFAULT 1",
    ),
    (
        "scheduled_transactions",
        "endDate",
        "ALTER TABLE scheduled_transactions ADD COLUMN \"endDate\" TEXT",
    ),
    (
        "scheduled_transactions",
        "budgetId",
        "ALTER TABLE scheduled_transactions ADD COLUMN \"budgetId\" TEXT",
    ),
    (
        "settings",
        "lastSeenVersion",
        "ALTER TABLE settings ADD COLUMN \"lastSeenVersion\" TEXT",
    ),
    (
        "settings",
        "mobileAccessEnabled",
        "ALTER TABLE settings ADD COLUMN \"mobileAccessEnabled\" BOOLEAN NOT NULL DEFAULT 0",
    ),
    (
        "settings",
        "mobileAccessToken",
        "ALTER TABLE settings ADD COLUMN \"mobileAccessToken\" TEXT",
    ),
    (
        "settings",
        "mobileAccessPort",
        "ALTER TABLE settings ADD COLUMN \"mobileAccessPort\" INTEGER NOT NULL DEFAULT 8799",
    ),
    (
        "settings",
        "dismissedBudgetSuggestions",
        "ALTER TABLE settings ADD COLUMN \"dismissedBudgetSuggestions\" TEXT",
    ),
    (
        "settings",
        "dismissedScheduledSuggestions",
        "ALTER TABLE settings ADD COLUMN \"dismissedScheduledSuggestions\" TEXT",
    ),
    (
        "settings",
        "predictionTimeRange",
        "ALTER TABLE settings ADD COLUMN \"predictionTimeRange\" TEXT NOT NULL DEFAULT 'year'",
    ),
    (
        "settings",
        "predictionCustomEndDate",
        "ALTER TABLE settings ADD COLUMN \"predictionCustomEndDate\" TEXT",
    ),
    (
        "settings",
        "predictionAlertThreshold",
        "ALTER TABLE settings ADD COLUMN \"predictionAlertThreshold\" REAL NOT NULL DEFAULT 0",
    ),
    (
        "settings",
        "predictionMonthStartsOnFirst",
        "ALTER TABLE settings ADD COLUMN \"predictionMonthStartsOnFirst\" BOOLEAN NOT NULL DEFAULT 1",
    ),
    (
        "settings",
        "predictionFakeTransactions",
        "ALTER TABLE settings ADD COLUMN \"predictionFakeTransactions\" TEXT",
    ),
    (
        "settings",
        "analyticsTimeRange",
        "ALTER TABLE settings ADD COLUMN \"analyticsTimeRange\" TEXT NOT NULL DEFAULT 'year'",
    ),
    (
        "settings",
        "analyticsCustomStartDate",
        "ALTER TABLE settings ADD COLUMN \"analyticsCustomStartDate\" TEXT",
    ),
    (
        "settings",
        "analyticsCustomEndDate",
        "ALTER TABLE settings ADD COLUMN \"analyticsCustomEndDate\" TEXT",
    ),
    (
        "settings",
        "analyticsMonthStartsOnFirst",
        "ALTER TABLE settings ADD COLUMN \"analyticsMonthStartsOnFirst\" BOOLEAN NOT NULL DEFAULT 1",
    ),
    (
        "settings",
        "analyticsHiddenExpenseCategories",
        "ALTER TABLE settings ADD COLUMN \"analyticsHiddenExpenseCategories\" TEXT",
    ),
    (
        "settings",
        "analyticsHiddenIncomeCategories",
        "ALTER TABLE settings ADD COLUMN \"analyticsHiddenIncomeCategories\" TEXT",
    ),
    (
        "settings",
        "scheduledDueRange",
        "ALTER TABLE settings ADD COLUMN \"scheduledDueRange\" TEXT NOT NULL DEFAULT 'all'",
    ),
    (
        "settings",
        "settingsRevision",
        "ALTER TABLE settings ADD COLUMN \"settingsRevision\" INTEGER NOT NULL DEFAULT 0",
    ),
    (
        "settings",
        "settingsFieldVersions",
        "ALTER TABLE settings ADD COLUMN \"settingsFieldVersions\" TEXT NOT NULL DEFAULT '{}'",
    ),
    (
        "settings",
        "secureBridgeEnabled",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeEnabled\" BOOLEAN NOT NULL DEFAULT 0",
    ),
    (
        "settings",
        "secureBridgeDomain",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeDomain\" TEXT",
    ),
    (
        "settings",
        "secureBridgeAppUrl",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeAppUrl\" TEXT",
    ),
    (
        "settings",
        "secureBridgeLocalHost",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeLocalHost\" TEXT",
    ),
    (
        "settings",
        "secureBridgeDeviceId",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeDeviceId\" TEXT",
    ),
    (
        "settings",
        "secureBridgeCertificateExpiresAt",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeCertificateExpiresAt\" TEXT",
    ),
    (
        "settings",
        "secureBridgeDnsRecordId",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeDnsRecordId\" TEXT",
    ),
    (
        "settings",
        "secureBridgeDnsLastUpdatedAt",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeDnsLastUpdatedAt\" TEXT",
    ),
    (
        "settings",
        "secureBridgeLastError",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeLastError\" TEXT",
    ),
    (
        "settings",
        "secureBridgeManagedServiceUrl",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeManagedServiceUrl\" TEXT",
    ),
    (
        "settings",
        "secureBridgeManagedRegisteredAt",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeManagedRegisteredAt\" TEXT",
    ),
    (
        "settings",
        "secureBridgeManagedDeviceSecret",
        "ALTER TABLE settings ADD COLUMN \"secureBridgeManagedDeviceSecret\" TEXT",
    ),
];

const COMPANION_TABLES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS mobile_passkeys (
        id TEXT PRIMARY KEY,
        credential_id TEXT NOT NULL UNIQUE,
        public_key TEXT NOT NULL,
        counter INTEGER NOT NULL DEFAULT 0,
        device_label TEXT,
        created_at TEXT NOT NULL,
        last_used_at TEXT,
        revoked_at TEXT
    )",
    "CREATE TABLE IF NOT EXISTS mobile_pairing_tokens (
        id TEXT PRIMARY KEY,
        token_hash TEXT NOT NULL UNIQUE,
        expires_at TEXT NOT NULL,
        consumed_at TEXT,
        created_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS mobile_sessions (
        id TEXT PRIMARY KEY,
        session_hash TEXT NOT NULL UNIQUE,
        csrf_hash TEXT NOT NULL,
        passkey_id TEXT,
        device_label TEXT,
        expires_at TEXT NOT NULL,
        created_at TEXT NOT NULL,
        last_used_at TEXT,
        revoked_at TEXT
    )",
    "CREATE TABLE IF NOT EXISTS mobile_auth_challenges (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        state_json TEXT NOT NULL,
        session_id TEXT,
        device_label TEXT,
        expires_at TEXT NOT NULL,
        created_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sync_state (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        version INTEGER NOT NULL DEFAULT 0
    )",
    "INSERT OR IGNORE INTO sync_state (id, version) VALUES (1, 0)",
];

const SYNC_TABLES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS sync_pending_remote (
        entity TEXT NOT NULL, record_id TEXT NOT NULL, change_json TEXT NOT NULL,
        PRIMARY KEY(entity, record_id)
    )",
    "CREATE TABLE IF NOT EXISTS mobile_mutation_receipts (
        id TEXT PRIMARY KEY,
        fingerprint TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        replay_until TEXT
    )",
    "CREATE TABLE IF NOT EXISTS sync_control (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        applying INTEGER NOT NULL DEFAULT 0
    )",
    "INSERT OR IGNORE INTO sync_control (id, applying) VALUES (1, 0)",
    "UPDATE sync_control SET applying = 0 WHERE id = 1",
    "CREATE TABLE IF NOT EXISTS sync_meta (
        entity TEXT NOT NULL,
        record_id TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (entity, record_id)
    )",
    "CREATE TABLE IF NOT EXISTS sync_outbox (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        entity TEXT NOT NULL,
        record_id TEXT NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_sync_outbox_record ON sync_outbox(entity, record_id)",
];

const NOW_ISO: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

fn outbox_trigger(table: &str, entity: &str, action: &str, columns: Option<&str>) -> String {
    let (reference, deleted) = if action == "DELETE" { ("OLD", 1) } else { ("NEW", 0) };
    let event = match columns {
        Some(columns) => format!("{action} OF {columns}"),
        None => action.to_string(),
    };
    let name = format!("trg_outbox_{table}_{}", action.to_lowercase());
    format!(
        "CREATE TRIGGER IF NOT EXISTS {name}
         AFTER {event} ON {table}
         WHEN (SELECT applying FROM sync_control WHERE id = 1) = 0
         BEGIN
            INSERT INTO sync_meta (entity, record_id, updated_at, deleted)
            VALUES ('{entity}', {reference}.id, {NOW_ISO}, {deleted})
            ON CONFLICT(entity, record_id) DO UPDATE
               SET updated_at = excluded.updated_at, deleted = excluded.deleted;
            INSERT INTO sync_outbox (entity, record_id, deleted, created_at)
            VALUES ('{entity}', {reference}.id, {deleted}, {NOW_ISO});
         END"
    )
}

async fn column_exists(connection: &mut SqliteConnection, table: &str, column: &str) -> CoreResult<bool> {
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM pragma_table_info('{table}') WHERE name = $1"
    )))
    .bind(column)
    .fetch_one(&mut *connection)
    .await
    .ctx("lecture du schéma")?;
    Ok(count > 0)
}

pub async fn create_tables(pool: &DbPool) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("initialisation de la base")?;

    for statement in BASE_TABLES {
        sqlx::query(*statement)
            .execute(&mut *tx)
            .await
            .ctx("création des tables")?;
    }

    for (table, column, definition) in COLUMN_MIGRATIONS {
        if !column_exists(&mut tx, table, column).await? {
            log::info!("Migrating {table} table: adding {column} column");
            sqlx::query(*definition)
                .execute(&mut *tx)
                .await
                .ctx("migration du schéma")?;
        }
    }

    // Les secrets ne sont plus stockés en base depuis 1.0.7 (trousseau système).
    sqlx::query(
        "UPDATE settings
         SET \"mobileAccessToken\" = NULL,
             \"secureBridgeManagedDeviceSecret\" = NULL
         WHERE \"mobileAccessToken\" IS NOT NULL
            OR \"secureBridgeManagedDeviceSecret\" IS NOT NULL",
    )
    .execute(&mut *tx)
    .await
    .ctx("nettoyage des secrets")?;

    for statement in COMPANION_TABLES.iter().chain(SYNC_TABLES) {
        sqlx::query(*statement)
            .execute(&mut *tx)
            .await
            .ctx("création des tables de synchronisation")?;
    }

    if !column_exists(&mut tx, "mobile_mutation_receipts", "created_at").await? {
        sqlx::query("ALTER TABLE mobile_mutation_receipts ADD COLUMN created_at TEXT NOT NULL DEFAULT ''")
            .execute(&mut *tx)
            .await
            .ctx("rétention des confirmations mobiles")?;
        // Pre-upgrade receipts retain a full retention window rather than disappearing immediately.
        sqlx::query("UPDATE mobile_mutation_receipts SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE created_at = ''")
            .execute(&mut *tx).await.ctx("rétention des confirmations mobiles")?;
    }
    if !column_exists(&mut tx, "mobile_mutation_receipts", "replay_until").await? {
        sqlx::query("ALTER TABLE mobile_mutation_receipts ADD COLUMN replay_until TEXT")
            .execute(&mut *tx)
            .await
            .ctx("rétention des confirmations mobiles")?;
    }
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_mobile_receipts_expiry ON mobile_mutation_receipts(replay_until)")
        .execute(&mut *tx)
        .await
        .ctx("rétention des confirmations mobiles")?;

    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_bank_identity ON transactions(\"accountId\", \"bankSource\", \"bankTransactionId\") WHERE \"bankSource\" IS NOT NULL AND \"bankTransactionId\" IS NOT NULL")
        .execute(&mut *tx).await.ctx("identité des imports bancaires")?;

    for table in [
        "accounts",
        "transactions",
        "categories",
        "budgets",
        "scheduled_transactions",
        "settings",
    ] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            let statement = format!(
                "CREATE TRIGGER IF NOT EXISTS trg_sync_{table}_{action}
                 AFTER {action} ON {table}
                 BEGIN
                    UPDATE sync_state SET version = version + 1 WHERE id = 1;
                 END"
            );
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&mut *tx)
                .await
                .ctx("création des déclencheurs")?;
        }
    }

    for (entity, table) in SYNC_ENTITY_TABLES {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            sqlx::query(sqlx::AssertSqlSafe(outbox_trigger(table, entity, action, None)))
                .execute(&mut *tx)
                .await
                .ctx("création des déclencheurs de synchronisation")?;
        }
    }

    let settings_columns = SYNCED_SETTINGS_COLUMNS
        .iter()
        .map(|column| format!("\"{column}\""))
        .collect::<Vec<_>>()
        .join(", ");
    for (action, columns) in [("INSERT", None), ("UPDATE", Some(settings_columns.as_str()))] {
        sqlx::query(sqlx::AssertSqlSafe(outbox_trigger(
            "settings", "settings", action, columns,
        )))
        .execute(&mut *tx)
        .await
        .ctx("création des déclencheurs de synchronisation")?;
    }

    sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA user_version = {SCHEMA_VERSION}")))
        .execute(&mut *tx)
        .await
        .ctx("version du schéma")?;

    tx.commit().await.ctx("initialisation de la base")?;
    Ok(())
}

/// Compteur incrémenté à chaque écriture (y compris celles du pont PWA).
pub async fn data_version<'e>(executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>) -> CoreResult<i64> {
    sqlx::query_scalar("SELECT version FROM sync_state WHERE id = 1")
        .fetch_one(executor)
        .await
        .ctx("lecture de la version des données")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn schema_is_idempotent_and_versioned() {
        let pool = open_memory_pool().await.unwrap();
        create_tables(&pool).await.unwrap();

        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(data_version(&pool).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn writes_bump_data_version_and_fill_outbox() {
        let pool = open_memory_pool().await.unwrap();
        sqlx::query("INSERT INTO categories (id, name, icon, color) VALUES ('c', 'Cat', 'Tag', '#000')")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(data_version(&pool).await.unwrap(), 1);
        let outbox: i64 = sqlx::query_scalar("SELECT count(*) FROM sync_outbox WHERE entity = 'categories'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(outbox, 1);
    }
}
