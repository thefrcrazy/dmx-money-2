//! Données initiales et migrations applicatives exécutées au démarrage
//! (port de `loadBankData` dans `src/context/BankContext.tsx`).

use crate::db::DbPool;
use crate::error::{CoreResult, DbContext};
use crate::models::{
    Budget, Category, TransactionType, TRANSFER_CATEGORY_ICON, TRANSFER_CATEGORY_ID, TRANSFER_CATEGORY_NAME,
    TRANSFER_COLOR,
};
use crate::repo;

/// Catégories créées sur une base vide : (id, nom, icône, couleur).
pub const DEFAULT_CATEGORIES: [(&str, &str, &str, &str); 29] = [
    ("1", "Loyer / Prêt", "Home", "#1e3a8a"),
    ("2", "Charges / Énergie", "Zap", "#f59e0b"),
    ("3", "Eau", "Droplets", "#0ea5e9"),
    ("4", "Assurance Habitation", "Shield", "#4b5563"),
    ("5", "Alimentation", "ShoppingBag", "#ef4444"),
    ("6", "Restaurants / Cafés", "Utensils", "#ea580c"),
    ("7", "Shopping / Vêtements", "Tag", "#ec4899"),
    ("8", "Hygiène / Beauté", "Smile", "#f472b6"),
    ("9", "Carburant", "Fuel", "#b45309"),
    ("10", "Transport en commun", "Bus", "#d97706"),
    ("11", "Entretien Voiture", "Hammer", "#6b7280"),
    ("12", "Parking / Péage", "MapPin", "#4b5563"),
    ("13", "Médecin / Santé", "Heart", "#dc2626"),
    ("14", "Pharmacie", "Pill", "#f87171"),
    ("15", "Loisirs / Cinéma", "Gamepad2", "#8b5cf6"),
    ("16", "Abonnements (VOD/Musique)", "Tv", "#6366f1"),
    ("17", "Sport / Bien-être", "Dumbbell", "#06b6d4"),
    ("18", "Voyages / Vacances", "Plane", "#2563eb"),
    ("19", "Téléphonie / Internet", "Wifi", "#3b82f6"),
    ("20", "High-Tech / Logiciels", "Monitor", "#1e40af"),
    ("21", "Salaire", "Banknote", "#16a34a"),
    ("22", "Primes / Bonus", "Award", "#d9f99d"),
    ("23", "Cadeaux reçus", "Gift", "#db2777"),
    ("24", "Remboursements", "TrendingUp", "#4ade80"),
    ("25", "Cadeaux offerts", "Gift", "#fca5a5"),
    ("26", "Frais Bancaires", "Landmark", "#1f2937"),
    ("27", "Impôts / Taxes", "Briefcase", "#7c2d12"),
    ("28", "Divers", "MoreHorizontal", "#6b7280"),
    (
        TRANSFER_CATEGORY_ID,
        TRANSFER_CATEGORY_NAME,
        TRANSFER_CATEGORY_ICON,
        TRANSFER_COLOR,
    ),
];

/// Catégorie « Divers » utilisée par défaut lors d'un import sans catégorie.
pub const FALLBACK_CATEGORY_ID: &str = "28";

fn category(entry: &(&str, &str, &str, &str)) -> Category {
    Category {
        id: entry.0.to_string(),
        name: entry.1.to_string(),
        icon: entry.2.to_string(),
        color: entry.3.to_string(),
    }
}

pub fn transfer_category() -> Category {
    category(DEFAULT_CATEGORIES.last().expect("la catégorie virement existe"))
}

/// Base vide : catégories par défaut. Base existante : garantit la catégorie virement.
pub async fn ensure_initial_data(pool: &DbPool) -> CoreResult<()> {
    let total: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM accounts)
              + (SELECT count(*) FROM transactions)
              + (SELECT count(*) FROM categories)
              + (SELECT count(*) FROM scheduled_transactions)
              + (SELECT count(*) FROM budgets)",
    )
    .fetch_one(pool)
    .await
    .ctx("initialisation des données")?;

    let mut tx = pool.begin().await.ctx("initialisation des données")?;
    if total == 0 {
        for entry in DEFAULT_CATEGORIES.iter() {
            repo::insert_category(&mut tx, &category(entry)).await?;
        }
        // Base neuve : la projection part d'aujourd'hui et non du 1er du mois. Le défaut de
        // colonne reste celui de la 1.x (le schéma doit rester identique), seule la ligne
        // créée ici diffère.
        sqlx::query(
            "INSERT OR IGNORE INTO settings (
                id, theme, \"primaryColor\", \"displayStyle\", \"componentSpacing\",
                \"componentPadding\", \"predictionMonthStartsOnFirst\"
            ) VALUES (1, 'system', '#6366f1', 'modern', 6, 6, 0)",
        )
        .execute(&mut *tx)
        .await
        .ctx("initialisation des données")?;
    } else {
        repo::insert_category(&mut tx, &transfer_category()).await?;
    }
    tx.commit().await.ctx("initialisation des données")?;
    Ok(())
}

/// Les anciennes échéances « incluses dans les prévisions » deviennent des budgets liés.
pub async fn migrate_legacy_scheduled_budgets(pool: &DbPool) -> CoreResult<usize> {
    let legacy = repo::list_scheduled(pool)
        .await?
        .into_iter()
        .filter(|item| {
            item.include_in_forecast == Some(true)
                && item.budget_id.is_none()
                && item.transaction_type == TransactionType::Expense
                && item.category != TRANSFER_CATEGORY_ID
        })
        .collect::<Vec<_>>();

    if legacy.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await.ctx("migration des échéances")?;
    for mut item in legacy.iter().cloned() {
        let budget = Budget {
            id: uuid::Uuid::new_v4().to_string(),
            name: item.description.clone(),
            amount: item.amount,
            category: item.category.clone(),
            account_id: Some(item.account_id.clone()),
        };
        repo::insert_budget(&mut tx, &budget).await?;
        item.budget_id = Some(budget.id);
        item.include_in_forecast = Some(true);
        repo::update_scheduled(&mut tx, &item).await?;
    }
    tx.commit().await.ctx("migration des échéances")?;
    Ok(legacy.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_pool;
    use crate::models::{Account, Periodicity, ScheduledTransaction};

    #[tokio::test]
    async fn empty_database_receives_default_categories_once() {
        let pool = open_memory_pool().await.unwrap();
        ensure_initial_data(&pool).await.unwrap();
        ensure_initial_data(&pool).await.unwrap();

        let categories = repo::list_categories(&pool).await.unwrap();
        assert_eq!(categories.len(), DEFAULT_CATEGORIES.len());
        assert_eq!(categories.last().unwrap().id, TRANSFER_CATEGORY_ID);
    }

    #[tokio::test]
    async fn legacy_forecast_scheduled_items_become_budgets() {
        let pool = open_memory_pool().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        repo::insert_account(
            &mut tx,
            &Account {
                id: "a1".into(),
                name: "Courant".into(),
                account_type: "Courant".into(),
                initial_balance: 0.0,
                color: "#000".into(),
                icon: "Wallet".into(),
            },
        )
        .await
        .unwrap();
        repo::insert_scheduled(
            &mut tx,
            &ScheduledTransaction {
                id: "s1".into(),
                description: "Courses".into(),
                amount: 300.0,
                transaction_type: TransactionType::Expense,
                frequency: Periodicity::Monthly,
                account_id: "a1".into(),
                next_date: "2026-10-01".into(),
                category: "5".into(),
                to_account_id: None,
                include_in_forecast: Some(true),
                budget_id: None,
                end_date: None,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(migrate_legacy_scheduled_budgets(&pool).await.unwrap(), 1);
        assert_eq!(migrate_legacy_scheduled_budgets(&pool).await.unwrap(), 0);

        let budgets = repo::list_budgets(&pool).await.unwrap();
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].account_id.as_deref(), Some("a1"));
        let scheduled = repo::list_scheduled(&pool).await.unwrap();
        assert_eq!(scheduled[0].budget_id.as_deref(), Some(budgets[0].id.as_str()));
    }
}
