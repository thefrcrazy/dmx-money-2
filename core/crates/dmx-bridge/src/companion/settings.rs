use super::*;

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
