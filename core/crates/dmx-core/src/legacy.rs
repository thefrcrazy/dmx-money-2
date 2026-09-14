//! Reprise des données de DmxMoney 1.x (application Tauri `com.dmxmoney.desktop`).
//!
//! La base d'origine est ouverte en lecture seule et copiée par `VACUUM INTO`, ce qui inclut
//! les écritures encore dans le journal WAL si l'ancienne application tourne toujours.

use crate::db::DATABASE_FILE_NAME;
use crate::error::{CoreError, CoreResult, DbContext};
use serde::Serialize;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};
use std::path::{Path, PathBuf};

pub const LEGACY_APP_IDENTIFIER: &str = "com.dmxmoney.desktop";

/// Emplacement de la base 1.x sur la plateforme courante (dossier de données de Tauri).
pub fn default_legacy_database_paths() -> Vec<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join(LEGACY_APP_IDENTIFIER).join(DATABASE_FILE_NAME))
        .into_iter()
        .collect()
}

/// Dossier des certificats du pont sécurisé, à côté de la base.
pub const BRIDGE_DIRECTORY: &str = "secure-bridge";

fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Reprend les certificats du pont 1.x pour conserver l'appareil et le sous-domaine existants.
pub fn copy_legacy_bridge_files(legacy_database: &Path, data_dir: &Path) -> CoreResult<bool> {
    let Some(source) = legacy_database.parent().map(|dir| dir.join(BRIDGE_DIRECTORY)) else {
        return Ok(false);
    };
    let destination = data_dir.join(BRIDGE_DIRECTORY);
    if !source.is_dir() || destination.exists() {
        return Ok(false);
    }
    copy_dir_recursive(&source, &destination)?;
    Ok(true)
}

/// Inventaire d'une base DmxMoney (lecture seule) : sert à comparer le dossier de la 2.x
/// avec une base 1.x trouvée à côté, sans jamais l'ouvrir en écriture.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DatabaseInventory {
    pub path: String,
    pub accounts: u32,
    pub transactions: u32,
    pub categories: u32,
    pub budgets: u32,
    pub scheduled: u32,
    /// Date de la dernière opération (`YYYY-MM-DD`), absente si la base est vide.
    pub last_transaction_date: Option<String>,
}

impl DatabaseInventory {
    /// Base « plus complète » : opération plus récente, ou autant de dates mais plus de contenu.
    pub fn is_richer_than(&self, other: &DatabaseInventory) -> bool {
        match (&self.last_transaction_date, &other.last_transaction_date) {
            (Some(mine), Some(theirs)) if mine != theirs => mine > theirs,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            _ => {
                (self.transactions, self.budgets, self.scheduled) > (other.transactions, other.budgets, other.scheduled)
            }
        }
    }
}

pub async fn inventory(path: &Path) -> CoreResult<DatabaseInventory> {
    let mut connection = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .foreign_keys(false)
        .connect()
        .await
        .ctx("lecture de la base DmxMoney")?;

    async fn count(connection: &mut sqlx::SqliteConnection, table: &str) -> u32 {
        sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&mut *connection)
            .await
            .unwrap_or(0) as u32
    }

    let inventory = DatabaseInventory {
        path: path.to_string_lossy().to_string(),
        accounts: count(&mut connection, "accounts").await,
        transactions: count(&mut connection, "transactions").await,
        categories: count(&mut connection, "categories").await,
        budgets: count(&mut connection, "budgets").await,
        scheduled: count(&mut connection, "scheduled_transactions").await,
        last_transaction_date: sqlx::query_scalar::<_, Option<String>>("SELECT max(date) FROM transactions")
            .fetch_one(&mut connection)
            .await
            .ok()
            .flatten(),
    };
    connection.close().await.ctx("lecture de la base DmxMoney")?;
    Ok(inventory)
}

/// Toutes les bases 1.x lisibles parmi les candidates (hors base de destination).
pub fn existing_legacy_databases(candidates: &[PathBuf], destination: &Path) -> Vec<PathBuf> {
    candidates
        .iter()
        .filter(|candidate| candidate.as_path() != destination)
        .filter(|candidate| std::fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0))
        .cloned()
        .collect()
}

pub fn find_legacy_database(candidates: &[PathBuf], destination: &Path) -> Option<PathBuf> {
    candidates
        .iter()
        .filter(|candidate| candidate.as_path() != destination)
        .find(|candidate| std::fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0))
        .cloned()
}

pub async fn copy_legacy_database(source: &Path, destination: &Path) -> CoreResult<()> {
    if destination.exists() {
        return Err(CoreError::Io("La base de destination existe déjà.".to_string()));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let vacuum = async {
        let mut connection = SqliteConnectOptions::new()
            .filename(source)
            .read_only(true)
            .foreign_keys(false)
            .connect()
            .await
            .ctx("ouverture de la base DmxMoney 1.x")?;
        sqlx::query("VACUUM INTO $1")
            .bind(destination.to_string_lossy().to_string())
            .execute(&mut connection)
            .await
            .ctx("copie de la base DmxMoney 1.x")?;
        connection.close().await.ctx("copie de la base DmxMoney 1.x")?;
        CoreResult::Ok(())
    };

    match vacuum.await {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(destination);
            let wal = PathBuf::from(format!("{}-wal", source.to_string_lossy()));
            if wal.exists() {
                return Err(error);
            }
            log::warn!("VACUUM INTO impossible, copie simple de la base 1.x : {error}");
            std::fs::copy(source, destination)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_pool;

    #[tokio::test]
    async fn copies_a_live_wal_database_without_touching_it() {
        let dir = std::env::temp_dir().join(format!("dmx-legacy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("legacy.db");
        let destination = dir.join("copy").join(DATABASE_FILE_NAME);

        let pool = open_pool(&source).await.unwrap();
        sqlx::query("INSERT INTO categories (id, name, icon, color) VALUES ('x', 'Héritée', 'Tag', '#000')")
            .execute(&pool)
            .await
            .unwrap();
        let version_before = crate::db::data_version(&pool).await.unwrap();

        copy_legacy_database(&source, &destination).await.unwrap();

        assert_eq!(crate::db::data_version(&pool).await.unwrap(), version_before);
        pool.close().await;

        let copy = open_pool(&destination).await.unwrap();
        let name: String = sqlx::query_scalar("SELECT name FROM categories WHERE id = 'x'")
            .fetch_one(&copy)
            .await
            .unwrap();
        assert_eq!(name, "Héritée");
        copy.close().await;

        assert!(copy_legacy_database(&source, &destination).await.is_err());
        assert_eq!(
            find_legacy_database(&[dir.join("absent.db"), source.clone()], &destination),
            Some(source)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
