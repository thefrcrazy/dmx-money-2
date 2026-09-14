use thiserror::Error;

/// Erreurs remontées aux interfaces. Les messages sont destinés à l'utilisateur, en français.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum CoreError {
    #[error("{0}")]
    Database(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Import(String),
    #[error("{0}")]
    Io(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

impl CoreError {
    pub fn validation(message: impl Into<String>) -> Self {
        CoreError::Validation(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        CoreError::NotFound(message.into())
    }

    pub fn import(message: impl Into<String>) -> Self {
        CoreError::Import(message.into())
    }
}

impl From<std::io::Error> for CoreError {
    fn from(error: std::io::Error) -> Self {
        CoreError::Io(error.to_string())
    }
}

/// Traduit une erreur SQLx comme le faisait `map_db_error` dans DmxMoney 1.x.
pub(crate) fn db_error(error: sqlx::Error, context: &str) -> CoreError {
    let message = error.to_string();
    log::error!("Database Error during {context}: {message}");

    if message.contains("FOREIGN KEY constraint failed") {
        return CoreError::Database("Impossible de supprimer cet élément car il est utilisé ailleurs.".to_string());
    }
    if message.contains("UNIQUE constraint failed") {
        return CoreError::Database("Un élément avec cet identifiant existe déjà.".to_string());
    }

    CoreError::Database(format!("Erreur BDD ({context}): {message}"))
}

pub(crate) trait DbContext<T> {
    fn ctx(self, context: &str) -> CoreResult<T>;
}

impl<T> DbContext<T> for Result<T, sqlx::Error> {
    fn ctx(self, context: &str) -> CoreResult<T> {
        self.map_err(|error| db_error(error, context))
    }
}
