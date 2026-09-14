//! Accès bas niveau aux tables (port de `src-tauri/src/commands.rs`).
//!
//! Les écritures prennent une connexion (souvent une transaction SQL ouverte par l'appelant)
//! afin de composer des opérations atomiques.

use crate::db::DbPool;
use crate::error::{CoreResult, DbContext};
use crate::models::{
    Account, AppData, Budget, Category, Periodicity, ScheduledTransaction, Transaction, TransactionType,
    DEFAULT_ACCOUNT_COLOR, DEFAULT_ACCOUNT_ICON,
};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

pub(crate) fn row_string(row: &SqliteRow, column: &str) -> String {
    row_opt_string(row, column).unwrap_or_default()
}

pub(crate) fn row_opt_string(row: &SqliteRow, column: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(column).ok().flatten().or_else(|| {
        row.try_get::<Option<i64>, _>(column)
            .ok()
            .flatten()
            .map(|value| value.to_string())
    })
}

/// Chaîne optionnelle servant de référence : une valeur vide équivaut à `NULL`.
pub(crate) fn row_opt_reference(row: &SqliteRow, column: &str) -> Option<String> {
    row_opt_string(row, column).filter(|value| !value.is_empty())
}

pub(crate) fn row_f64(row: &SqliteRow, column: &str) -> f64 {
    row.try_get::<Option<f64>, _>(column)
        .ok()
        .flatten()
        .or_else(|| {
            row.try_get::<Option<i64>, _>(column)
                .ok()
                .flatten()
                .map(|value| value as f64)
        })
        .unwrap_or_default()
}

pub(crate) fn row_i64(row: &SqliteRow, column: &str) -> Option<i64> {
    row.try_get::<Option<i64>, _>(column).ok().flatten()
}

pub(crate) fn row_opt_bool(row: &SqliteRow, column: &str) -> Option<bool> {
    row.try_get::<Option<bool>, _>(column)
        .ok()
        .flatten()
        .or_else(|| row_i64(row, column).map(|value| value != 0))
}

pub(crate) fn row_bool(row: &SqliteRow, column: &str) -> bool {
    row_opt_bool(row, column).unwrap_or(false)
}

pub fn account_from_row(row: &SqliteRow) -> Account {
    Account {
        id: row_string(row, "id"),
        name: row_string(row, "name"),
        account_type: row_string(row, "type"),
        initial_balance: row_f64(row, "initialBalance"),
        color: row_opt_reference(row, "color").unwrap_or_else(|| DEFAULT_ACCOUNT_COLOR.to_string()),
        icon: row_opt_reference(row, "icon").unwrap_or_else(|| DEFAULT_ACCOUNT_ICON.to_string()),
    }
}

pub fn transaction_from_row(row: &SqliteRow) -> Transaction {
    Transaction {
        id: row_string(row, "id"),
        date: row_string(row, "date"),
        account_id: row_string(row, "accountId"),
        transaction_type: TransactionType::parse(&row_string(row, "type")),
        amount: row_f64(row, "amount"),
        category: row_string(row, "category"),
        description: row_string(row, "description"),
        checked: row_bool(row, "checked"),
        is_transfer: row_bool(row, "isTransfer"),
        linked_transaction_id: row_opt_reference(row, "linkedTransactionId"),
    }
}

pub fn category_from_row(row: &SqliteRow) -> Category {
    Category {
        id: row_string(row, "id"),
        name: row_string(row, "name"),
        icon: row_string(row, "icon"),
        color: row_string(row, "color"),
    }
}

pub fn budget_from_row(row: &SqliteRow) -> Budget {
    Budget {
        id: row_string(row, "id"),
        name: row_string(row, "name"),
        amount: row_f64(row, "amount"),
        category: row_string(row, "category"),
        account_id: row_opt_reference(row, "accountId"),
    }
}

pub fn scheduled_from_row(row: &SqliteRow) -> ScheduledTransaction {
    ScheduledTransaction {
        id: row_string(row, "id"),
        description: row_string(row, "description"),
        amount: row_f64(row, "amount"),
        transaction_type: TransactionType::parse(&row_string(row, "type")),
        frequency: Periodicity::parse(&row_string(row, "frequency")),
        account_id: row_string(row, "accountId"),
        next_date: row_string(row, "nextDate"),
        category: row_string(row, "category"),
        to_account_id: row_opt_reference(row, "toAccountId"),
        include_in_forecast: row_opt_bool(row, "includeInForecast"),
        budget_id: row_opt_reference(row, "budgetId"),
        end_date: row_opt_reference(row, "endDate"),
    }
}

// --- Comptes ---

pub async fn list_accounts(pool: &DbPool) -> CoreResult<Vec<Account>> {
    let rows = sqlx::query("SELECT * FROM accounts ORDER BY rowid ASC")
        .fetch_all(pool)
        .await
        .ctx("récupération des comptes")?;
    Ok(rows.iter().map(account_from_row).collect())
}

pub async fn insert_account(connection: &mut SqliteConnection, account: &Account) -> CoreResult<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO accounts (id, name, \"type\", \"initialBalance\", color, icon) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&account.id)
    .bind(&account.name)
    .bind(&account.account_type)
    .bind(account.initial_balance)
    .bind(&account.color)
    .bind(&account.icon)
    .execute(&mut *connection)
    .await
    .ctx("ajout du compte")?;
    Ok(())
}

pub async fn update_account(connection: &mut SqliteConnection, account: &Account) -> CoreResult<()> {
    sqlx::query(
        "UPDATE accounts SET name = $1, \"type\" = $2, \"initialBalance\" = $3, color = $4, icon = $5 WHERE id = $6",
    )
    .bind(&account.name)
    .bind(&account.account_type)
    .bind(account.initial_balance)
    .bind(&account.color)
    .bind(&account.icon)
    .bind(&account.id)
    .execute(&mut *connection)
    .await
    .ctx("mise à jour du compte")?;
    Ok(())
}

/// Supprime le compte, ses transactions et ses échéances ; les budgets liés sont déliés.
pub async fn delete_account(connection: &mut SqliteConnection, id: &str) -> CoreResult<()> {
    sqlx::query("DELETE FROM transactions WHERE \"accountId\" = $1")
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("suppression des transactions liées")?;
    sqlx::query("DELETE FROM scheduled_transactions WHERE \"accountId\" = $1 OR \"toAccountId\" = $1")
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("suppression des échéances liées")?;
    sqlx::query("UPDATE budgets SET \"accountId\" = NULL WHERE \"accountId\" = $1")
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("déliaison des budgets liés")?;
    sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("suppression du compte")?;
    Ok(())
}

// --- Transactions ---

pub async fn list_transactions(pool: &DbPool) -> CoreResult<Vec<Transaction>> {
    let rows = sqlx::query("SELECT * FROM transactions ORDER BY date DESC, rowid DESC")
        .fetch_all(pool)
        .await
        .ctx("récupération des transactions")?;
    Ok(rows.iter().map(transaction_from_row).collect())
}

pub async fn get_transaction(connection: &mut SqliteConnection, id: &str) -> CoreResult<Option<Transaction>> {
    let row = sqlx::query("SELECT * FROM transactions WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *connection)
        .await
        .ctx("récupération de la transaction")?;
    Ok(row.as_ref().map(transaction_from_row))
}

const INSERT_TRANSACTION: &str = "INTO transactions (id, date, \"accountId\", \"type\", amount, category, description, checked, \"isTransfer\", \"linkedTransactionId\") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)";

async fn execute_insert_transaction(
    connection: &mut SqliteConnection,
    verb: &str,
    transaction: &Transaction,
) -> CoreResult<bool> {
    let statement = format!("{verb} {INSERT_TRANSACTION}");
    let result = sqlx::query(&statement)
        .bind(&transaction.id)
        .bind(&transaction.date)
        .bind(&transaction.account_id)
        .bind(transaction.transaction_type.as_str())
        .bind(transaction.amount)
        .bind(&transaction.category)
        .bind(&transaction.description)
        .bind(transaction.checked)
        .bind(transaction.is_transfer)
        .bind(&transaction.linked_transaction_id)
        .execute(&mut *connection)
        .await
        .ctx("ajout de transaction")?;
    Ok(result.rows_affected() > 0)
}

pub async fn insert_transaction(connection: &mut SqliteConnection, transaction: &Transaction) -> CoreResult<()> {
    execute_insert_transaction(connection, "INSERT", transaction)
        .await
        .map(|_| ())
}

/// Insère seulement si l'identifiant n'existe pas encore (échéances idempotentes).
pub async fn insert_transaction_if_absent(
    connection: &mut SqliteConnection,
    transaction: &Transaction,
) -> CoreResult<bool> {
    execute_insert_transaction(connection, "INSERT OR IGNORE", transaction).await
}

pub async fn update_transaction(connection: &mut SqliteConnection, transaction: &Transaction) -> CoreResult<()> {
    sqlx::query(
        "UPDATE transactions SET date = $1, \"accountId\" = $2, \"type\" = $3, amount = $4, category = $5, description = $6, checked = $7, \"isTransfer\" = $8, \"linkedTransactionId\" = $9 WHERE id = $10",
    )
    .bind(&transaction.date)
    .bind(&transaction.account_id)
    .bind(transaction.transaction_type.as_str())
    .bind(transaction.amount)
    .bind(&transaction.category)
    .bind(&transaction.description)
    .bind(transaction.checked)
    .bind(transaction.is_transfer)
    .bind(&transaction.linked_transaction_id)
    .bind(&transaction.id)
    .execute(&mut *connection)
    .await
    .ctx("mise à jour de transaction")?;
    Ok(())
}

/// Supprime une transaction et, pour un virement, sa contrepartie liée.
pub async fn delete_transaction(connection: &mut SqliteConnection, id: &str) -> CoreResult<()> {
    let linked: Option<String> = sqlx::query_scalar("SELECT \"linkedTransactionId\" FROM transactions WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *connection)
        .await
        .ctx("récupération du virement lié")?
        .flatten();

    match linked.filter(|linked| !linked.is_empty()) {
        Some(linked) => {
            sqlx::query("DELETE FROM transactions WHERE id = $1 OR id = $2")
                .bind(id)
                .bind(linked)
                .execute(&mut *connection)
                .await
                .ctx("suppression du virement lié")?;
        }
        None => {
            sqlx::query("DELETE FROM transactions WHERE id = $1")
                .bind(id)
                .execute(&mut *connection)
                .await
                .ctx("suppression de transaction")?;
        }
    }
    Ok(())
}

pub async fn set_transaction_checked(connection: &mut SqliteConnection, id: &str, checked: bool) -> CoreResult<()> {
    sqlx::query("UPDATE transactions SET checked = $1 WHERE id = $2")
        .bind(checked)
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("pointage de transaction")?;
    Ok(())
}

// --- Catégories ---

pub async fn list_categories(pool: &DbPool) -> CoreResult<Vec<Category>> {
    let rows = sqlx::query("SELECT * FROM categories ORDER BY rowid ASC")
        .fetch_all(pool)
        .await
        .ctx("récupération des catégories")?;
    Ok(rows.iter().map(category_from_row).collect())
}

pub async fn insert_category(connection: &mut SqliteConnection, category: &Category) -> CoreResult<()> {
    sqlx::query("INSERT OR IGNORE INTO categories (id, name, icon, color) VALUES ($1, $2, $3, $4)")
        .bind(&category.id)
        .bind(&category.name)
        .bind(&category.icon)
        .bind(&category.color)
        .execute(&mut *connection)
        .await
        .ctx("ajout de catégorie")?;
    Ok(())
}

pub async fn update_category(connection: &mut SqliteConnection, category: &Category) -> CoreResult<()> {
    sqlx::query("UPDATE categories SET name = $1, icon = $2, color = $3 WHERE id = $4")
        .bind(&category.name)
        .bind(&category.icon)
        .bind(&category.color)
        .bind(&category.id)
        .execute(&mut *connection)
        .await
        .ctx("mise à jour de catégorie")?;
    Ok(())
}

pub async fn delete_category(connection: &mut SqliteConnection, id: &str) -> CoreResult<()> {
    sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("suppression de catégorie")?;
    Ok(())
}

// --- Budgets ---

pub async fn list_budgets(pool: &DbPool) -> CoreResult<Vec<Budget>> {
    let rows = sqlx::query("SELECT * FROM budgets ORDER BY rowid DESC")
        .fetch_all(pool)
        .await
        .ctx("récupération des budgets")?;
    Ok(rows.iter().map(budget_from_row).collect())
}

async fn execute_insert_budget(connection: &mut SqliteConnection, verb: &str, budget: &Budget) -> CoreResult<bool> {
    let statement =
        format!("{verb} INTO budgets (id, name, amount, category, \"accountId\") VALUES ($1, $2, $3, $4, $5)");
    let result = sqlx::query(&statement)
        .bind(&budget.id)
        .bind(&budget.name)
        .bind(budget.amount)
        .bind(&budget.category)
        .bind(&budget.account_id)
        .execute(&mut *connection)
        .await
        .ctx("ajout de budget")?;
    Ok(result.rows_affected() > 0)
}

pub async fn insert_budget(connection: &mut SqliteConnection, budget: &Budget) -> CoreResult<()> {
    execute_insert_budget(connection, "INSERT", budget).await.map(|_| ())
}

/// Variante idempotente utilisée par l'API compagnon (renvoi d'une mutation hors ligne).
pub async fn insert_budget_if_absent(connection: &mut SqliteConnection, budget: &Budget) -> CoreResult<bool> {
    execute_insert_budget(connection, "INSERT OR IGNORE", budget).await
}

pub async fn update_budget(connection: &mut SqliteConnection, budget: &Budget) -> CoreResult<()> {
    sqlx::query("UPDATE budgets SET name = $1, amount = $2, category = $3, \"accountId\" = $4 WHERE id = $5")
        .bind(&budget.name)
        .bind(budget.amount)
        .bind(&budget.category)
        .bind(&budget.account_id)
        .bind(&budget.id)
        .execute(&mut *connection)
        .await
        .ctx("mise à jour de budget")?;
    Ok(())
}

/// Supprime un budget ; les échéances liées sont déliées et sortent des prévisions.
pub async fn delete_budget(connection: &mut SqliteConnection, id: &str) -> CoreResult<()> {
    sqlx::query(
        "UPDATE scheduled_transactions SET \"budgetId\" = NULL, \"includeInForecast\" = 0 WHERE \"budgetId\" = $1",
    )
    .bind(id)
    .execute(&mut *connection)
    .await
    .ctx("déliaison des échéances du budget")?;
    sqlx::query("DELETE FROM budgets WHERE id = $1")
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("suppression de budget")?;
    Ok(())
}

// --- Échéances ---

pub async fn list_scheduled(pool: &DbPool) -> CoreResult<Vec<ScheduledTransaction>> {
    let rows = sqlx::query("SELECT * FROM scheduled_transactions ORDER BY rowid ASC")
        .fetch_all(pool)
        .await
        .ctx("récupération des échéances")?;
    Ok(rows.iter().map(scheduled_from_row).collect())
}

pub async fn insert_scheduled(connection: &mut SqliteConnection, scheduled: &ScheduledTransaction) -> CoreResult<()> {
    execute_insert_scheduled(connection, "INSERT", scheduled)
        .await
        .map(|_| ())
}

/// Variante idempotente utilisée par l'API compagnon.
pub async fn insert_scheduled_if_absent(
    connection: &mut SqliteConnection,
    scheduled: &ScheduledTransaction,
) -> CoreResult<bool> {
    execute_insert_scheduled(connection, "INSERT OR IGNORE", scheduled).await
}

async fn execute_insert_scheduled(
    connection: &mut SqliteConnection,
    verb: &str,
    scheduled: &ScheduledTransaction,
) -> CoreResult<bool> {
    let statement = format!(
        "{verb} INTO scheduled_transactions (id, description, amount, \"type\", frequency, \"accountId\", \"nextDate\", category, \"toAccountId\", \"includeInForecast\", \"budgetId\", \"endDate\") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
    );
    let result = sqlx::query(&statement)
        .bind(&scheduled.id)
        .bind(&scheduled.description)
        .bind(scheduled.amount)
        .bind(scheduled.transaction_type.as_str())
        .bind(scheduled.frequency.as_str())
        .bind(&scheduled.account_id)
        .bind(&scheduled.next_date)
        .bind(&scheduled.category)
        .bind(&scheduled.to_account_id)
        .bind(scheduled.include_in_forecast)
        .bind(&scheduled.budget_id)
        .bind(&scheduled.end_date)
        .execute(&mut *connection)
        .await
        .ctx("ajout d'échéance")?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_scheduled(connection: &mut SqliteConnection, scheduled: &ScheduledTransaction) -> CoreResult<()> {
    sqlx::query(
        "UPDATE scheduled_transactions SET description = $1, amount = $2, \"type\" = $3, frequency = $4, \"accountId\" = $5, \"nextDate\" = $6, category = $7, \"toAccountId\" = $8, \"includeInForecast\" = $9, \"budgetId\" = $10, \"endDate\" = $11 WHERE id = $12",
    )
    .bind(&scheduled.description)
    .bind(scheduled.amount)
    .bind(scheduled.transaction_type.as_str())
    .bind(scheduled.frequency.as_str())
    .bind(&scheduled.account_id)
    .bind(&scheduled.next_date)
    .bind(&scheduled.category)
    .bind(&scheduled.to_account_id)
    .bind(scheduled.include_in_forecast)
    .bind(&scheduled.budget_id)
    .bind(&scheduled.end_date)
    .bind(&scheduled.id)
    .execute(&mut *connection)
    .await
    .ctx("mise à jour d'échéance")?;
    Ok(())
}

pub async fn delete_scheduled(connection: &mut SqliteConnection, id: &str) -> CoreResult<()> {
    sqlx::query("DELETE FROM scheduled_transactions WHERE id = $1")
        .bind(id)
        .execute(&mut *connection)
        .await
        .ctx("suppression d'échéance")?;
    Ok(())
}

// --- Import complet ---

/// Remplace toutes les données métier (import `.dmx`), dans l'ordre imposé par les clés étrangères.
pub async fn replace_all_data(connection: &mut SqliteConnection, data: &AppData) -> CoreResult<()> {
    for (table, context) in [
        ("transactions", "nettoyage des transactions"),
        ("scheduled_transactions", "nettoyage des échéances"),
        ("budgets", "nettoyage des budgets"),
        ("accounts", "nettoyage des comptes"),
        ("categories", "nettoyage des catégories"),
    ] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *connection)
            .await
            .ctx(context)?;
    }

    for account in &data.accounts {
        sqlx::query(
            "INSERT INTO accounts (id, name, \"type\", \"initialBalance\", color, icon) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&account.id)
        .bind(&account.name)
        .bind(&account.account_type)
        .bind(account.initial_balance)
        .bind(&account.color)
        .bind(&account.icon)
        .execute(&mut *connection)
        .await
        .ctx("import de compte")?;
    }

    for category in &data.categories {
        sqlx::query("INSERT INTO categories (id, name, icon, color) VALUES ($1, $2, $3, $4)")
            .bind(&category.id)
            .bind(&category.name)
            .bind(&category.icon)
            .bind(&category.color)
            .execute(&mut *connection)
            .await
            .ctx("import de catégorie")?;
    }

    for budget in &data.budgets {
        insert_budget(connection, budget).await?;
    }
    for transaction in &data.transactions {
        insert_transaction(connection, transaction).await?;
    }
    for scheduled in &data.scheduled {
        insert_scheduled(connection, scheduled).await?;
    }
    Ok(())
}
