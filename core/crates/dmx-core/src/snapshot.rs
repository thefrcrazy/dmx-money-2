//! Instantané en mémoire des données, point d'entrée de tous les calculs de vues.

use crate::db::{self, DbPool};
use crate::error::CoreResult;
use crate::models::{
    Account, Budget, Category, ScheduledTransaction, Transaction, TRANSFER_CATEGORY_ICON, TRANSFER_CATEGORY_ID,
    TRANSFER_CATEGORY_NAME, TRANSFER_COLOR, UNKNOWN_CATEGORY_COLOR, UNKNOWN_CATEGORY_ICON, UNKNOWN_CATEGORY_NAME,
};
use crate::repo;
use crate::settings::{self, AppSettings};
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub data_version: i64,
    pub accounts: Vec<Account>,
    /// Ordre du journal : date décroissante puis insertion décroissante.
    pub transactions: Vec<Transaction>,
    pub categories: Vec<Category>,
    pub scheduled: Vec<ScheduledTransaction>,
    /// Budgets du plus récent au plus ancien.
    pub budgets: Vec<Budget>,
    pub settings: AppSettings,
}

/// Présentation d'une catégorie, avec les replis « Virement » et « Inconnu » de 1.x.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CategoryDisplay {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
}

pub async fn load(pool: &DbPool) -> CoreResult<Snapshot> {
    let data_version = db::data_version(pool).await?;
    Ok(Snapshot {
        data_version,
        accounts: repo::list_accounts(pool).await?,
        transactions: repo::list_transactions(pool).await?,
        categories: repo::list_categories(pool).await?,
        scheduled: repo::list_scheduled(pool).await?,
        budgets: repo::list_budgets(pool).await?,
        settings: settings::load_app_settings(pool).await?,
    })
}

/// Un filtre vide sélectionne tous les comptes.
pub fn is_selected(filter: &[String], account_id: &str) -> bool {
    filter.is_empty() || filter.iter().any(|selected| selected == account_id)
}

impl Snapshot {
    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    pub fn budget(&self, id: &str) -> Option<&Budget> {
        self.budgets.iter().find(|budget| budget.id == id)
    }

    pub fn transaction(&self, id: &str) -> Option<&Transaction> {
        self.transactions.iter().find(|transaction| transaction.id == id)
    }

    pub fn account_name(&self, id: &str) -> String {
        self.account(id).map(|account| account.name.clone()).unwrap_or_default()
    }

    pub fn category_display(&self, id: &str) -> CategoryDisplay {
        if id == TRANSFER_CATEGORY_ID {
            return CategoryDisplay {
                id: id.to_string(),
                name: TRANSFER_CATEGORY_NAME.to_string(),
                icon: TRANSFER_CATEGORY_ICON.to_string(),
                color: TRANSFER_COLOR.to_string(),
            };
        }
        match self.category(id) {
            Some(category) => CategoryDisplay {
                id: category.id.clone(),
                name: category.name.clone(),
                icon: if category.icon.is_empty() {
                    UNKNOWN_CATEGORY_ICON.to_string()
                } else {
                    category.icon.clone()
                },
                color: if category.color.is_empty() {
                    UNKNOWN_CATEGORY_COLOR.to_string()
                } else {
                    category.color.clone()
                },
            },
            None => CategoryDisplay {
                id: id.to_string(),
                name: UNKNOWN_CATEGORY_NAME.to_string(),
                icon: UNKNOWN_CATEGORY_ICON.to_string(),
                color: UNKNOWN_CATEGORY_COLOR.to_string(),
            },
        }
    }

    /// Catégories sélectionnables dans les formulaires (sans « Virement »).
    pub fn selectable_categories(&self) -> Vec<&Category> {
        self.categories
            .iter()
            .filter(|category| category.id != TRANSFER_CATEGORY_ID)
            .collect()
    }
}
