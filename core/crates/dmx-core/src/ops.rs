//! Opérations d'écriture validées et atomiques, partagées par toutes les interfaces.

use crate::dates::{format_date, parse_date};
use crate::db::DbPool;
use crate::error::{CoreError, CoreResult, DbContext};
use crate::models::{
    account_type_defaults, Account, Budget, Category, Periodicity, ScheduledTransaction, Transaction, TransactionType,
    ACCOUNT_TYPES, TRANSFER_CATEGORY_ID,
};
use crate::repo;
use crate::settings::{self, SettingsChange};
use crate::snapshot::{is_selected, Snapshot};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn round_cents(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn positive_amount(amount: f64) -> CoreResult<f64> {
    if amount.is_finite() && amount > 0.0 {
        Ok(round_cents(amount))
    } else {
        Err(CoreError::validation("Saisissez un montant valide"))
    }
}

fn valid_date(value: &str) -> CoreResult<String> {
    parse_date(value)
        .map(format_date)
        .ok_or_else(|| CoreError::validation("Sélectionnez une date valide"))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn account_exists(connection: &mut SqliteConnection, id: &str) -> CoreResult<bool> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE id = $1")
        .bind(id)
        .fetch_one(&mut *connection)
        .await
        .ctx("vérification du compte")?;
    Ok(count > 0)
}

async fn require_account(connection: &mut SqliteConnection, id: &str) -> CoreResult<()> {
    if id.is_empty() {
        return Err(CoreError::validation("Sélectionnez un compte"));
    }
    if !account_exists(connection, id).await? {
        return Err(CoreError::not_found("Compte introuvable."));
    }
    Ok(())
}

// --- Comptes ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountDraft {
    pub id: Option<String>,
    pub name: String,
    pub account_type: String,
    pub initial_balance: f64,
    pub color: String,
    pub icon: String,
    pub group: Option<String>,
}

pub fn new_account_draft() -> AccountDraft {
    let (icon, color) = account_type_defaults(ACCOUNT_TYPES[0]);
    AccountDraft {
        id: None,
        name: String::new(),
        account_type: ACCOUNT_TYPES[0].to_string(),
        initial_balance: 0.0,
        color: color.to_string(),
        icon: icon.to_string(),
        group: None,
    }
}

pub fn account_draft(snapshot: &Snapshot, id: &str) -> Option<AccountDraft> {
    let account = snapshot.account(id)?;
    Some(AccountDraft {
        id: Some(account.id.clone()),
        name: account.name.clone(),
        account_type: account.account_type.clone(),
        initial_balance: account.initial_balance,
        color: account.color.clone(),
        icon: account.icon.clone(),
        group: snapshot.settings.account_groups.get(id).cloned(),
    })
}

pub async fn save_account(pool: &DbPool, draft: AccountDraft) -> CoreResult<String> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::validation("Le nom du compte est requis."));
    }
    if !draft.initial_balance.is_finite() {
        return Err(CoreError::validation("Saisissez un solde initial valide"));
    }

    let account_type = if draft.account_type.trim().is_empty() {
        ACCOUNT_TYPES[0].to_string()
    } else {
        draft.account_type.trim().to_string()
    };
    let (default_icon, default_color) = account_type_defaults(&account_type);
    let account = Account {
        id: draft.id.clone().unwrap_or_else(new_id),
        name,
        account_type,
        initial_balance: round_cents(draft.initial_balance),
        color: non_empty(Some(draft.color)).unwrap_or_else(|| default_color.to_string()),
        icon: non_empty(Some(draft.icon)).unwrap_or_else(|| default_icon.to_string()),
    };

    let mut tx = pool.begin().await.ctx("enregistrement du compte")?;
    if draft.id.is_some() {
        if !account_exists(&mut tx, &account.id).await? {
            return Err(CoreError::not_found("Compte introuvable."));
        }
        repo::update_account(&mut tx, &account).await?;
    } else {
        repo::insert_account(&mut tx, &account).await?;
    }
    tx.commit().await.ctx("enregistrement du compte")?;

    let group = non_empty(draft.group);
    let current = settings::load_app_settings(pool).await?;
    if current.account_groups.get(&account.id) != group.as_ref() {
        settings::apply_change(
            pool,
            SettingsChange::AccountGroup {
                account_id: account.id.clone(),
                group,
            },
        )
        .await?;
    }

    Ok(account.id)
}

pub async fn delete_account(pool: &DbPool, id: &str) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("suppression du compte")?;
    repo::delete_account(&mut tx, id).await?;
    tx.commit().await.ctx("suppression du compte")?;

    let current = settings::load_app_settings(pool).await?;
    if current.account_groups.contains_key(id) {
        settings::apply_change(
            pool,
            SettingsChange::AccountGroup {
                account_id: id.to_string(),
                group: None,
            },
        )
        .await?;
    }
    if let Some(order) = current
        .accounts_order
        .filter(|order| order.iter().any(|item| item == id))
    {
        let order = order.into_iter().filter(|item| item != id).collect();
        settings::apply_change(pool, SettingsChange::AccountsOrder(order)).await?;
    }
    Ok(())
}

/// Cible d'un glisser-déposer dans la page Comptes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccountDropTarget {
    Account(String),
    /// `None` désigne « Non groupés ».
    Group(Option<String>),
}

fn move_item(items: &mut Vec<String>, from: usize, to: usize) {
    let item = items.remove(from);
    items.insert(to.min(items.len()), item);
}

/// Ordre effectif des comptes : ordre enregistré, puis comptes absents dans leur ordre de création.
pub fn effective_accounts_order(snapshot: &Snapshot) -> Vec<String> {
    let mut order: Vec<String> = snapshot
        .settings
        .accounts_order
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|id| snapshot.account(id).is_some())
        .collect();
    for account in &snapshot.accounts {
        if !order.contains(&account.id) {
            order.push(account.id.clone());
        }
    }
    order
}

pub async fn move_account(
    pool: &DbPool,
    snapshot: &Snapshot,
    account_id: &str,
    target: AccountDropTarget,
) -> CoreResult<()> {
    let current_group = snapshot.settings.account_groups.get(account_id).cloned();
    let target_group = match &target {
        AccountDropTarget::Account(over_id) => {
            if over_id == account_id {
                return Ok(());
            }
            let mut order = effective_accounts_order(snapshot);
            if let (Some(from), Some(to)) = (
                order.iter().position(|id| id == account_id),
                order.iter().position(|id| id == over_id),
            ) {
                move_item(&mut order, from, to);
                settings::apply_change(pool, SettingsChange::AccountsOrder(order)).await?;
            }
            snapshot.settings.account_groups.get(over_id).cloned()
        }
        AccountDropTarget::Group(group) => non_empty(group.clone()),
    };

    if target_group != current_group {
        settings::apply_change(
            pool,
            SettingsChange::AccountGroup {
                account_id: account_id.to_string(),
                group: target_group,
            },
        )
        .await?;
    }
    Ok(())
}

pub async fn move_group(pool: &DbPool, snapshot: &Snapshot, group: &str, over_group: &str) -> CoreResult<()> {
    let mut order = snapshot.settings.effective_group_order();
    if let (Some(from), Some(to)) = (
        order.iter().position(|item| item == group),
        order.iter().position(|item| item == over_group),
    ) {
        if from != to {
            move_item(&mut order, from, to);
            settings::apply_change(pool, SettingsChange::CustomGroupsOrder(order)).await?;
        }
    }
    Ok(())
}

// --- Catégories ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryDraft {
    pub id: Option<String>,
    pub name: String,
    pub icon: String,
    pub color: String,
}

pub const DEFAULT_CATEGORY_ICON: &str = "Tag";
pub const DEFAULT_CATEGORY_COLOR: &str = "#000000";
pub const IMPORTED_CATEGORY_COLOR: &str = "#9ca3af";

pub fn new_category_draft() -> CategoryDraft {
    CategoryDraft {
        id: None,
        name: String::new(),
        icon: DEFAULT_CATEGORY_ICON.to_string(),
        color: crate::palette::CATEGORY_COLORS
            .first()
            .copied()
            .unwrap_or(DEFAULT_CATEGORY_COLOR)
            .to_string(),
    }
}

pub fn category_draft(snapshot: &Snapshot, id: &str) -> Option<CategoryDraft> {
    let category = snapshot.categories.iter().find(|category| category.id == id)?;
    Some(CategoryDraft {
        id: Some(category.id.clone()),
        name: category.name.clone(),
        icon: category.icon.clone(),
        color: category.color.clone(),
    })
}

pub async fn save_category(pool: &DbPool, draft: CategoryDraft) -> CoreResult<String> {
    if draft.id.as_deref() == Some(TRANSFER_CATEGORY_ID) {
        return Err(CoreError::validation(
            "La catégorie Virement ne peut pas être modifiée.",
        ));
    }
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::validation("Le nom de la catégorie est requis."));
    }
    let category = Category {
        id: draft.id.clone().unwrap_or_else(new_id),
        name,
        icon: non_empty(Some(draft.icon)).unwrap_or_else(|| DEFAULT_CATEGORY_ICON.to_string()),
        color: non_empty(Some(draft.color)).unwrap_or_else(|| DEFAULT_CATEGORY_COLOR.to_string()),
    };

    let mut tx = pool.begin().await.ctx("enregistrement de la catégorie")?;
    if draft.id.is_some() {
        repo::update_category(&mut tx, &category).await?;
    } else {
        repo::insert_category(&mut tx, &category).await?;
    }
    tx.commit().await.ctx("enregistrement de la catégorie")?;
    Ok(category.id)
}

pub async fn delete_category(pool: &DbPool, id: &str) -> CoreResult<()> {
    if id == TRANSFER_CATEGORY_ID {
        return Err(CoreError::validation(
            "La catégorie Virement ne peut pas être supprimée.",
        ));
    }
    let mut tx = pool.begin().await.ctx("suppression de catégorie")?;
    repo::delete_category(&mut tx, id).await?;
    tx.commit().await.ctx("suppression de catégorie")
}

// --- Transactions ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionDraft {
    /// Identifiant d'une transaction existante (n'importe quel côté d'un virement).
    pub id: Option<String>,
    pub kind: TransactionType,
    pub date: String,
    pub amount: f64,
    pub description: String,
    pub category_id: String,
    /// Compte de l'opération, ou compte source d'un virement.
    pub account_id: String,
    pub to_account_id: Option<String>,
}

pub fn new_transaction_draft(snapshot: &Snapshot, filter: &[String], today: NaiveDate) -> TransactionDraft {
    let account_id = if filter.len() == 1 {
        filter[0].clone()
    } else {
        snapshot
            .accounts
            .first()
            .map(|account| account.id.clone())
            .unwrap_or_default()
    };
    TransactionDraft {
        id: None,
        kind: TransactionType::Expense,
        date: format_date(today),
        amount: 0.0,
        description: String::new(),
        category_id: String::new(),
        account_id,
        to_account_id: None,
    }
}

/// Préremplit le formulaire d'édition ; un virement est présenté depuis son compte source.
pub fn transaction_draft(snapshot: &Snapshot, id: &str) -> Option<TransactionDraft> {
    let transaction = snapshot.transaction(id)?;
    let linked = transaction
        .linked_transaction_id
        .as_deref()
        .and_then(|linked_id| snapshot.transaction(linked_id));

    if let (true, Some(linked)) = (transaction.category == TRANSFER_CATEGORY_ID, linked) {
        let (from, to) = if transaction.is_income() {
            (linked, transaction)
        } else {
            (transaction, linked)
        };
        return Some(TransactionDraft {
            id: Some(transaction.id.clone()),
            kind: TransactionType::Transfer,
            date: transaction.date.clone(),
            amount: transaction.amount,
            description: transaction.description.clone(),
            category_id: TRANSFER_CATEGORY_ID.to_string(),
            account_id: from.account_id.clone(),
            to_account_id: Some(to.account_id.clone()),
        });
    }

    Some(TransactionDraft {
        id: Some(transaction.id.clone()),
        kind: if transaction.is_income() {
            TransactionType::Income
        } else {
            TransactionType::Expense
        },
        date: transaction.date.clone(),
        amount: transaction.amount,
        description: transaction.description.clone(),
        category_id: transaction.category.clone(),
        account_id: transaction.account_id.clone(),
        to_account_id: None,
    })
}

struct TransferValues<'a> {
    date: &'a str,
    amount: f64,
    description: &'a str,
}

fn transfer_pair(
    (from_id, to_id): (String, String),
    (from_account, to_account): (&str, &str),
    values: &TransferValues,
    checked: (bool, bool),
) -> (Transaction, Transaction) {
    let base = |id: String, linked: String, account: &str, kind, checked| Transaction {
        id,
        date: values.date.to_string(),
        account_id: account.to_string(),
        transaction_type: kind,
        amount: values.amount,
        category: TRANSFER_CATEGORY_ID.to_string(),
        description: values.description.to_string(),
        checked,
        is_transfer: true,
        linked_transaction_id: Some(linked),
    };
    (
        base(
            from_id.clone(),
            to_id.clone(),
            from_account,
            TransactionType::Expense,
            checked.0,
        ),
        base(to_id, from_id, to_account, TransactionType::Income, checked.1),
    )
}

/// Crée ou modifie une opération, y compris les conversions simple ↔ virement.
/// Renvoie les identifiants écrits (deux pour un virement : source puis destination).
pub async fn save_transaction(pool: &DbPool, draft: TransactionDraft) -> CoreResult<Vec<String>> {
    let date = valid_date(&draft.date)?;
    let amount = positive_amount(draft.amount)?;
    let description = draft.description.trim().to_string();
    let is_transfer = draft.kind == TransactionType::Transfer;

    let mut tx = pool.begin().await.ctx("enregistrement de la transaction")?;
    require_account(&mut tx, &draft.account_id).await?;

    let to_account = if is_transfer {
        let to_account = non_empty(draft.to_account_id.clone())
            .filter(|to_account| *to_account != draft.account_id)
            .ok_or_else(|| CoreError::validation("Sélectionnez un compte destination différent"))?;
        require_account(&mut tx, &to_account).await?;
        Some(to_account)
    } else {
        None
    };
    if !is_transfer && draft.category_id.trim().is_empty() {
        return Err(CoreError::validation("Sélectionnez une catégorie"));
    }

    let existing = match draft.id.as_deref() {
        Some(id) => Some(
            repo::get_transaction(&mut tx, id)
                .await?
                .ok_or_else(|| CoreError::not_found("Transaction introuvable."))?,
        ),
        None => None,
    };
    let linked = match existing
        .as_ref()
        .and_then(|existing| existing.linked_transaction_id.clone())
    {
        Some(linked_id) => repo::get_transaction(&mut tx, &linked_id).await?,
        None => None,
    };

    let ids = match (existing, linked, to_account) {
        (Some(existing), Some(linked), Some(to_account)) => {
            let (from, to) = if existing.is_income() {
                (linked, existing)
            } else {
                (existing, linked)
            };
            let values = TransferValues {
                date: &date,
                amount,
                description: &description,
            };
            let (from, to) = transfer_pair(
                (from.id, to.id),
                (&draft.account_id, &to_account),
                &values,
                (from.checked, to.checked),
            );
            repo::update_transaction(&mut tx, &from).await?;
            repo::update_transaction(&mut tx, &to).await?;
            vec![from.id, to.id]
        }
        (existing, _, Some(to_account)) => {
            let checked = existing.as_ref().map(|existing| existing.checked).unwrap_or(false);
            if let Some(existing) = &existing {
                repo::delete_transaction(&mut tx, &existing.id).await?;
            }
            let values = TransferValues {
                date: &date,
                amount,
                description: &description,
            };
            let (from, to) = transfer_pair(
                (new_id(), new_id()),
                (&draft.account_id, &to_account),
                &values,
                (checked, checked),
            );
            repo::insert_transaction(&mut tx, &from).await?;
            repo::insert_transaction(&mut tx, &to).await?;
            vec![from.id, to.id]
        }
        (existing, linked, None) => {
            let transaction = Transaction {
                id: existing
                    .as_ref()
                    .map(|existing| existing.id.clone())
                    .unwrap_or_else(new_id),
                date,
                account_id: draft.account_id.clone(),
                transaction_type: if draft.kind == TransactionType::Income {
                    TransactionType::Income
                } else {
                    TransactionType::Expense
                },
                amount,
                category: draft.category_id.trim().to_string(),
                description,
                checked: existing.as_ref().map(|existing| existing.checked).unwrap_or(false),
                is_transfer: false,
                linked_transaction_id: None,
            };
            match (&existing, linked) {
                (Some(_), Some(_)) => {
                    repo::delete_transaction(&mut tx, &transaction.id).await?;
                    repo::insert_transaction(&mut tx, &transaction).await?;
                }
                (Some(_), None) => repo::update_transaction(&mut tx, &transaction).await?,
                (None, _) => repo::insert_transaction(&mut tx, &transaction).await?,
            }
            vec![transaction.id]
        }
    };

    tx.commit().await.ctx("enregistrement de la transaction")?;
    Ok(ids)
}

pub async fn delete_transactions(pool: &DbPool, ids: &[String]) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("suppression de transaction")?;
    for id in ids {
        repo::delete_transaction(&mut tx, id).await?;
    }
    tx.commit().await.ctx("suppression de transaction")
}

pub async fn set_transactions_checked(pool: &DbPool, ids: &[String], checked: bool) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("pointage de transaction")?;
    for id in ids {
        repo::set_transaction_checked(&mut tx, id, checked).await?;
    }
    tx.commit().await.ctx("pointage de transaction")
}

/// Pointage groupé : si tout est déjà pointé, tout est dépointé, sinon tout est pointé.
/// Renvoie le nouvel état.
///
/// Les deux jambes d'un virement sont pointées ensemble : pointer la sortie du Livret A pointe
/// aussi l'entrée sur le compte courant.
pub async fn toggle_transactions_checked(pool: &DbPool, snapshot: &Snapshot, ids: &[String]) -> CoreResult<bool> {
    let all_checked = ids
        .iter()
        .filter_map(|id| snapshot.transaction(id))
        .all(|transaction| transaction.checked);
    let checked = !all_checked;
    let mut targets: Vec<String> = ids.to_vec();
    for id in ids {
        if let Some(linked) = snapshot
            .transaction(id)
            .and_then(|transaction| transaction.linked_transaction_id.clone())
        {
            if !targets.contains(&linked) {
                targets.push(linked);
            }
        }
    }
    set_transactions_checked(pool, &targets, checked).await?;
    Ok(checked)
}

/// Édition en ligne d'une cellule du journal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InlineEdit {
    Description(String),
    Amount(f64),
}

/// Modifie une cellule ; les deux côtés d'un virement restent cohérents.
pub async fn update_transaction_inline(pool: &DbPool, id: &str, edit: InlineEdit) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("mise à jour de transaction")?;
    let transaction = repo::get_transaction(&mut tx, id)
        .await?
        .ok_or_else(|| CoreError::not_found("Transaction introuvable."))?;
    let linked = match transaction.linked_transaction_id.as_deref() {
        Some(linked_id) => repo::get_transaction(&mut tx, linked_id).await?,
        None => None,
    };

    let apply = |mut target: Transaction| -> CoreResult<Transaction> {
        match &edit {
            InlineEdit::Description(description) => target.description = description.trim().to_string(),
            InlineEdit::Amount(amount) => target.amount = positive_amount(*amount)?,
        }
        Ok(target)
    };

    repo::update_transaction(&mut tx, &apply(transaction)?).await?;
    if let Some(linked) = linked {
        repo::update_transaction(&mut tx, &apply(linked)?).await?;
    }
    tx.commit().await.ctx("mise à jour de transaction")
}

// --- Budgets ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetDraft {
    pub id: Option<String>,
    pub name: String,
    pub amount: f64,
    pub category_id: String,
    pub account_id: Option<String>,
}

pub fn new_budget_draft(snapshot: &Snapshot, filter: &[String]) -> BudgetDraft {
    BudgetDraft {
        id: None,
        name: String::new(),
        amount: 0.0,
        category_id: snapshot
            .selectable_categories()
            .first()
            .map(|category| category.id.clone())
            .unwrap_or_default(),
        account_id: (filter.len() == 1).then(|| filter[0].clone()),
    }
}

pub fn budget_draft(snapshot: &Snapshot, id: &str) -> Option<BudgetDraft> {
    let budget = snapshot.budget(id)?;
    Some(BudgetDraft {
        id: Some(budget.id.clone()),
        name: budget.name.clone(),
        amount: budget.amount,
        category_id: budget.category.clone(),
        account_id: budget.account_id.clone(),
    })
}

pub async fn save_budget(pool: &DbPool, draft: BudgetDraft) -> CoreResult<String> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::validation("Le nom du budget est requis."));
    }
    let amount = positive_amount(draft.amount)?;
    let category = draft.category_id.trim().to_string();
    if category.is_empty() || category == TRANSFER_CATEGORY_ID {
        return Err(CoreError::validation("Sélectionnez une catégorie"));
    }
    let account_id = non_empty(draft.account_id).filter(|account| account != "all");

    let mut tx = pool.begin().await.ctx("enregistrement du budget")?;
    if let Some(account_id) = &account_id {
        require_account(&mut tx, account_id).await?;
    }
    let budget = Budget {
        id: draft.id.clone().unwrap_or_else(new_id),
        name,
        amount,
        category,
        account_id,
    };
    if draft.id.is_some() {
        repo::update_budget(&mut tx, &budget).await?;
    } else {
        repo::insert_budget(&mut tx, &budget).await?;
    }
    tx.commit().await.ctx("enregistrement du budget")?;
    Ok(budget.id)
}

pub async fn delete_budget(pool: &DbPool, id: &str) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("suppression de budget")?;
    repo::delete_budget(&mut tx, id).await?;
    tx.commit().await.ctx("suppression de budget")
}

// --- Échéances ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledDraft {
    pub id: Option<String>,
    pub description: String,
    pub amount: f64,
    pub kind: TransactionType,
    pub category_id: String,
    pub account_id: String,
    pub to_account_id: Option<String>,
    pub frequency: Periodicity,
    pub next_date: String,
    pub end_date: Option<String>,
    pub budget_id: Option<String>,
}

pub fn new_scheduled_draft(snapshot: &Snapshot, today: NaiveDate) -> ScheduledDraft {
    ScheduledDraft {
        id: None,
        description: String::new(),
        amount: 0.0,
        kind: TransactionType::Expense,
        category_id: String::new(),
        account_id: snapshot
            .accounts
            .first()
            .map(|account| account.id.clone())
            .unwrap_or_default(),
        to_account_id: None,
        frequency: Periodicity::Monthly,
        next_date: format_date(today),
        end_date: None,
        budget_id: None,
    }
}

pub fn scheduled_draft(snapshot: &Snapshot, id: &str) -> Option<ScheduledDraft> {
    let item = snapshot.scheduled.iter().find(|item| item.id == id)?;
    Some(ScheduledDraft {
        id: Some(item.id.clone()),
        description: item.description.clone(),
        amount: item.amount,
        kind: item.transaction_type,
        category_id: item.category.clone(),
        account_id: item.account_id.clone(),
        to_account_id: item.to_account_id.clone(),
        frequency: item.frequency,
        next_date: item.next_date.clone(),
        end_date: item.end_date.clone(),
        budget_id: item.budget_id.clone(),
    })
}

/// Catégorie et compte imposés par un budget lié.
pub fn budget_link_defaults(snapshot: &Snapshot, budget_id: &str) -> Option<(String, Option<String>)> {
    snapshot
        .budget(budget_id)
        .map(|budget| (budget.category.clone(), budget.account_id.clone()))
}

pub async fn save_scheduled(pool: &DbPool, draft: ScheduledDraft) -> CoreResult<String> {
    let amount = positive_amount(draft.amount)?;
    let next_date = valid_date(&draft.next_date)?;
    let end_date = match non_empty(draft.end_date.clone()) {
        Some(end_date) => {
            let end_date = valid_date(&end_date)?;
            if end_date < next_date {
                return Err(CoreError::validation("La date de fin doit suivre la date de début."));
            }
            Some(end_date)
        }
        None => None,
    };

    let mut tx = pool.begin().await.ctx("enregistrement de l'échéance")?;
    let mut account_id = draft.account_id.clone();
    let mut category = draft.category_id.trim().to_string();
    let mut to_account_id = None;
    let mut budget_id = None;

    match draft.kind {
        TransactionType::Transfer => {
            require_account(&mut tx, &account_id).await?;
            let to_account = non_empty(draft.to_account_id.clone())
                .filter(|to_account| *to_account != account_id)
                .ok_or_else(|| CoreError::validation("Sélectionnez un compte destination différent"))?;
            require_account(&mut tx, &to_account).await?;
            to_account_id = Some(to_account);
            category = TRANSFER_CATEGORY_ID.to_string();
        }
        TransactionType::Expense if non_empty(draft.budget_id.clone()).is_some() => {
            let budget_row = sqlx::query("SELECT * FROM budgets WHERE id = $1")
                .bind(draft.budget_id.as_deref().unwrap_or_default().trim())
                .fetch_optional(&mut *tx)
                .await
                .ctx("lecture du budget")?
                .ok_or_else(|| CoreError::not_found("Budget introuvable."))?;
            let budget = repo::budget_from_row(&budget_row);
            category = budget.category.clone();
            if let Some(budget_account) = budget.account_id.clone() {
                account_id = budget_account;
            }
            require_account(&mut tx, &account_id).await?;
            budget_id = Some(budget.id);
        }
        _ => {
            require_account(&mut tx, &account_id).await?;
            if category.is_empty() {
                return Err(CoreError::validation("Sélectionnez une catégorie"));
            }
        }
    }

    let scheduled = ScheduledTransaction {
        id: draft.id.clone().unwrap_or_else(new_id),
        description: draft.description.trim().to_string(),
        amount,
        transaction_type: draft.kind,
        frequency: draft.frequency,
        account_id,
        next_date,
        category,
        to_account_id,
        include_in_forecast: Some(budget_id.is_some()),
        budget_id,
        end_date,
    };

    if draft.id.is_some() {
        repo::update_scheduled(&mut tx, &scheduled).await?;
    } else {
        repo::insert_scheduled(&mut tx, &scheduled).await?;
    }
    tx.commit().await.ctx("enregistrement de l'échéance")?;
    Ok(scheduled.id)
}

pub async fn delete_scheduled(pool: &DbPool, id: &str) -> CoreResult<()> {
    let mut tx = pool.begin().await.ctx("suppression d'échéance")?;
    repo::delete_scheduled(&mut tx, id).await?;
    tx.commit().await.ctx("suppression d'échéance")
}

/// Crée une échéance à partir d'une suggestion détectée dans le journal.
pub async fn add_scheduled(pool: &DbPool, scheduled: ScheduledTransaction) -> CoreResult<String> {
    let scheduled = ScheduledTransaction {
        id: if scheduled.id.is_empty() {
            new_id()
        } else {
            scheduled.id
        },
        ..scheduled
    };
    let mut tx = pool.begin().await.ctx("ajout d'échéance")?;
    repo::insert_scheduled(&mut tx, &scheduled).await?;
    tx.commit().await.ctx("ajout d'échéance")?;
    Ok(scheduled.id)
}

/// Comptes visibles pour un filtre donné, dans l'ordre de création.
pub fn filtered_account_ids(snapshot: &Snapshot, filter: &[String]) -> Vec<String> {
    snapshot
        .accounts
        .iter()
        .filter(|account| is_selected(filter, &account.id))
        .map(|account| account.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_pool;
    use crate::snapshot;

    async fn pool_with_accounts() -> DbPool {
        let pool = open_memory_pool().await.unwrap();
        for name in ["Courant", "Épargne"] {
            save_account(
                &pool,
                AccountDraft {
                    id: Some(name.to_string()),
                    name: name.to_string(),
                    ..new_account_draft()
                },
            )
            .await
            .unwrap_err();
        }
        pool
    }

    async fn create_account(pool: &DbPool, name: &str) -> String {
        save_account(
            pool,
            AccountDraft {
                name: name.into(),
                ..new_account_draft()
            },
        )
        .await
        .unwrap()
    }

    fn draft(kind: TransactionType, account: &str) -> TransactionDraft {
        TransactionDraft {
            id: None,
            kind,
            date: "2026-09-10".into(),
            amount: 42.0,
            description: "Test".into(),
            category_id: "5".into(),
            account_id: account.into(),
            to_account_id: None,
        }
    }

    #[tokio::test]
    async fn category_draft_reads_the_category_or_falls_back_to_defaults() {
        let pool = open_memory_pool().await.unwrap();
        let id = save_category(
            &pool,
            CategoryDraft {
                id: None,
                name: "Loisirs".into(),
                icon: "Gamepad2".into(),
                color: "#8b5cf6".into(),
            },
        )
        .await
        .unwrap();
        let snapshot = snapshot::load(&pool).await.unwrap();

        let existing = category_draft(&snapshot, &id).expect("catégorie existante");
        assert_eq!(existing.name, "Loisirs");
        assert_eq!(existing.icon, "Gamepad2");
        assert_eq!(existing.id.as_deref(), Some(id.as_str()));
        assert!(category_draft(&snapshot, "inconnue").is_none());

        let fresh = new_category_draft();
        assert!(fresh.id.is_none());
        assert_eq!(fresh.icon, DEFAULT_CATEGORY_ICON);
        assert_eq!(fresh.color, crate::palette::CATEGORY_COLORS[0]);
    }

    #[tokio::test]
    async fn the_transfer_category_cannot_be_edited_or_deleted() {
        let pool = open_memory_pool().await.unwrap();
        assert!(save_category(
            &pool,
            CategoryDraft {
                id: Some(crate::models::TRANSFER_CATEGORY_ID.to_string()),
                name: "Autre".into(),
                icon: "Tag".into(),
                color: "#000000".into(),
            },
        )
        .await
        .is_err());
        assert!(delete_category(&pool, crate::models::TRANSFER_CATEGORY_ID)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn editing_a_missing_account_fails_cleanly() {
        let pool = pool_with_accounts().await;
        assert!(repo::list_accounts(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn transfer_edit_from_income_side_keeps_both_accounts() {
        let pool = open_memory_pool().await.unwrap();
        let a1 = create_account(&pool, "Courant").await;
        let a2 = create_account(&pool, "Épargne").await;
        let a3 = create_account(&pool, "Livret").await;

        let ids = save_transaction(
            &pool,
            TransactionDraft {
                to_account_id: Some(a2.clone()),
                ..draft(TransactionType::Transfer, &a1)
            },
        )
        .await
        .unwrap();
        assert_eq!(ids.len(), 2);

        let snap = snapshot::load(&pool).await.unwrap();
        let mut edit = transaction_draft(&snap, &ids[1]).unwrap();
        assert_eq!(edit.account_id, a1);
        assert_eq!(edit.to_account_id.as_deref(), Some(a2.as_str()));

        edit.to_account_id = Some(a3.clone());
        edit.amount = 60.0;
        save_transaction(&pool, edit).await.unwrap();

        let snap = snapshot::load(&pool).await.unwrap();
        let from = snap.transaction(&ids[0]).unwrap();
        let to = snap.transaction(&ids[1]).unwrap();
        assert_eq!((from.account_id.as_str(), from.amount), (a1.as_str(), 60.0));
        assert_eq!((to.account_id.as_str(), to.amount), (a3.as_str(), 60.0));
    }

    #[tokio::test]
    async fn converting_between_single_and_transfer() {
        let pool = open_memory_pool().await.unwrap();
        let a1 = create_account(&pool, "Courant").await;
        let a2 = create_account(&pool, "Épargne").await;

        let single = save_transaction(&pool, draft(TransactionType::Expense, &a1))
            .await
            .unwrap();
        let pair = save_transaction(
            &pool,
            TransactionDraft {
                id: Some(single[0].clone()),
                to_account_id: Some(a2.clone()),
                ..draft(TransactionType::Transfer, &a1)
            },
        )
        .await
        .unwrap();
        let snap = snapshot::load(&pool).await.unwrap();
        assert_eq!(snap.transactions.len(), 2);
        assert!(snap.transaction(&single[0]).is_none());

        let back = save_transaction(
            &pool,
            TransactionDraft {
                id: Some(pair[1].clone()),
                ..draft(TransactionType::Income, &a2)
            },
        )
        .await
        .unwrap();
        let snap = snapshot::load(&pool).await.unwrap();
        assert_eq!(snap.transactions.len(), 1);
        assert_eq!(snap.transactions[0].id, back[0]);
        assert_eq!(snap.transactions[0].transaction_type, TransactionType::Income);
    }

    #[tokio::test]
    async fn validation_messages_are_french() {
        let pool = open_memory_pool().await.unwrap();
        let a1 = create_account(&pool, "Courant").await;
        let error = save_transaction(
            &pool,
            TransactionDraft {
                amount: 0.0,
                ..draft(TransactionType::Expense, &a1)
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "Saisissez un montant valide");

        let error = save_transaction(&pool, draft(TransactionType::Transfer, &a1))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "Sélectionnez un compte destination différent");
    }

    #[tokio::test]
    async fn group_check_toggles_like_v1() {
        let pool = open_memory_pool().await.unwrap();
        let a1 = create_account(&pool, "Courant").await;
        let first = save_transaction(&pool, draft(TransactionType::Expense, &a1))
            .await
            .unwrap()
            .remove(0);
        let second = save_transaction(&pool, draft(TransactionType::Expense, &a1))
            .await
            .unwrap()
            .remove(0);
        set_transactions_checked(&pool, std::slice::from_ref(&first), true)
            .await
            .unwrap();

        let ids = vec![first, second];
        let snap = snapshot::load(&pool).await.unwrap();
        assert!(toggle_transactions_checked(&pool, &snap, &ids).await.unwrap());
        let snap = snapshot::load(&pool).await.unwrap();
        assert!(snap.transactions.iter().all(|transaction| transaction.checked));
        assert!(!toggle_transactions_checked(&pool, &snap, &ids).await.unwrap());
    }

    #[tokio::test]
    async fn scheduled_linked_to_budget_takes_its_category_and_account() {
        let pool = open_memory_pool().await.unwrap();
        let a1 = create_account(&pool, "Courant").await;
        let a2 = create_account(&pool, "Joint").await;
        let budget = save_budget(
            &pool,
            BudgetDraft {
                id: None,
                name: "Courses".into(),
                amount: 300.0,
                category_id: "5".into(),
                account_id: Some(a2.clone()),
            },
        )
        .await
        .unwrap();

        let id = save_scheduled(
            &pool,
            ScheduledDraft {
                budget_id: Some(budget.clone()),
                category_id: "9".into(),
                amount: 80.0,
                ..new_scheduled_draft(
                    &Snapshot {
                        accounts: vec![],
                        ..Snapshot::default()
                    },
                    parse_date("2026-09-01").unwrap(),
                )
            }
            .with_account(&a1),
        )
        .await
        .unwrap();

        let snap = snapshot::load(&pool).await.unwrap();
        let item = snap.scheduled.iter().find(|item| item.id == id).unwrap();
        assert_eq!(item.category, "5");
        assert_eq!(item.account_id, a2);
        assert_eq!(item.include_in_forecast, Some(true));

        delete_budget(&pool, &budget).await.unwrap();
        let snap = snapshot::load(&pool).await.unwrap();
        assert_eq!(snap.scheduled[0].budget_id, None);
        assert_eq!(snap.scheduled[0].include_in_forecast, Some(false));
    }

    impl ScheduledDraft {
        fn with_account(mut self, account: &str) -> Self {
            self.account_id = account.to_string();
            self
        }
    }

    #[tokio::test]
    async fn moving_accounts_updates_order_and_group() {
        let pool = open_memory_pool().await.unwrap();
        let a1 = create_account(&pool, "Un").await;
        let a2 = create_account(&pool, "Deux").await;
        let a3 = create_account(&pool, "Trois").await;
        settings::apply_change(&pool, SettingsChange::AddCustomGroup("Perso".into()))
            .await
            .unwrap();
        settings::apply_change(
            &pool,
            SettingsChange::AccountGroup {
                account_id: a1.clone(),
                group: Some("Perso".into()),
            },
        )
        .await
        .unwrap();

        let snap = snapshot::load(&pool).await.unwrap();
        move_account(&pool, &snap, &a3, AccountDropTarget::Account(a1.clone()))
            .await
            .unwrap();

        let snap = snapshot::load(&pool).await.unwrap();
        assert_eq!(
            snap.settings.accounts_order,
            Some(vec![a3.clone(), a1.clone(), a2.clone()])
        );
        assert_eq!(snap.settings.account_groups.get(&a3).map(String::as_str), Some("Perso"));

        move_account(&pool, &snap, &a3, AccountDropTarget::Group(None))
            .await
            .unwrap();
        let snap = snapshot::load(&pool).await.unwrap();
        assert!(!snap.settings.account_groups.contains_key(&a3));

        delete_account(&pool, &a1).await.unwrap();
        let snap = snapshot::load(&pool).await.unwrap();
        assert_eq!(snap.settings.accounts_order, Some(vec![a3, a2]));
        assert!(snap.settings.account_groups.is_empty());
    }
}
