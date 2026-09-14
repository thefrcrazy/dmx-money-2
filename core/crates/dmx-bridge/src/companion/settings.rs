use super::*;

const DEFAULT_PORT: u16 = 8799;
const PORT_FALLBACK_COUNT: u16 = 30;

pub(super) fn bind_listener(preferred_port: u16) -> Result<(TcpListener, u16), String> {
    for offset in 0..=PORT_FALLBACK_COUNT {
        let port = preferred_port.saturating_add(offset);
        match TcpListener::bind(("0.0.0.0", port)) {
            Ok(listener) => return Ok((listener, port)),
            Err(error) if offset == PORT_FALLBACK_COUNT => {
                return Err(format!(
                    "Impossible de démarrer le serveur mobile sur le port {preferred_port}: {error}"
                ));
            }
            Err(_) => {}
        }
    }

    Err("Aucun port disponible pour l'accès mobile".to_string())
}

/// Adresse IP de la machine sur le réseau local (celle de la route par défaut).
pub fn detect_local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            let _ = socket.connect("8.8.8.8:80");
            socket.local_addr()
        })
        .map(|address| address.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

async fn ensure_settings_row(pool: &DbPool) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO settings (
            id, theme, \"primaryColor\", \"displayStyle\", \"componentSpacing\", \"componentPadding\",
            \"mobileAccessEnabled\", \"mobileAccessPort\"
        )
        VALUES (1, 'system', 'default', 'modern', 6, 6, 0, $1)",
    )
    .bind(i64::from(DEFAULT_PORT))
    .execute(pool)
    .await
    .map_err(|error| map_db_error(error, "initialisation des paramètres mobiles"))?;

    sqlx::query("UPDATE settings SET \"mobileAccessToken\" = NULL WHERE id = 1 AND \"mobileAccessToken\" IS NOT NULL")
        .execute(pool)
        .await
        .map_err(|error| map_db_error(error, "nettoyage du token mobile legacy"))?;

    Ok(())
}

pub(super) async fn load_mobile_settings(pool: &DbPool) -> Result<MobileSettings, String> {
    ensure_settings_row(pool).await?;

    let row = sqlx::query(
        "SELECT
            COALESCE(\"mobileAccessEnabled\", 0) AS enabled,
            COALESCE(\"mobileAccessPort\", 8799) AS port
         FROM settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| map_db_error(error, "lecture des paramètres mobiles"))?;

    let enabled = row.try_get::<i64, _>("enabled").unwrap_or(0) != 0;
    let port = row
        .try_get::<i64, _>("port")
        .unwrap_or(i64::from(DEFAULT_PORT))
        .clamp(1, i64::from(u16::MAX)) as u16;

    Ok(MobileSettings { enabled, port })
}

pub(super) async fn get_data_version(pool: &DbPool) -> Result<i64, String> {
    dmx_core::db::data_version(pool)
        .await
        .map_err(|error| error.to_string())
}

pub(super) fn map_db_error(error: sqlx::Error, context: &str) -> String {
    let message = error.to_string();
    log::error!("Database Error during {context}: {message}");

    if message.contains("FOREIGN KEY constraint failed") {
        return "Impossible de supprimer cet élément car il est utilisé ailleurs.".to_string();
    }
    if message.contains("UNIQUE constraint failed") {
        return "Un élément avec cet identifiant existe déjà.".to_string();
    }

    format!("Erreur BDD ({context}): {message}")
}
