//! Remplit un dossier de données avec un jeu de démonstration déterministe.
//!
//! Sert aux captures d'écran (`DMXMONEY_SNAPSHOT_DIR` sur macOS et Linux) et aux essais
//! manuels, sans jamais toucher aux données réelles :
//!
//! ```bash
//! cargo run -p seed-demo -- /tmp/dmx-demo
//! DMXMONEY_DATA_DIR=/tmp/dmx-demo cargo run -p dmx-money-gtk
//! ```

use chrono::{Datelike, Duration, NaiveDate};
use dmx_core::models::{Periodicity, TransactionType};
use dmx_core::ops::{AccountDraft, BudgetDraft, ScheduledDraft, TransactionDraft};
use dmx_core::predictions::FakeTransactionDraft;
use dmx_core::settings::SettingsChange;
use dmx_core::{Engine, EngineConfig};

/// Générateur congruentiel : même jeu de données à chaque exécution.
struct Random(u64);

impl Random {
    fn next(&mut self, modulo: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % modulo
    }

    /// Montant en euros avec deux décimales, entre `low` et `high`.
    fn amount(&mut self, low: u64, high: u64) -> f64 {
        let cents = low * 100 + self.next((high - low) * 100);
        cents as f64 / 100.0
    }
}

fn main() {
    let Some(directory) = std::env::args().nth(1) else {
        eprintln!("usage: seed-demo <dossier de données>");
        std::process::exit(2);
    };
    let engine = Engine::open(EngineConfig {
        data_dir: directory.clone().into(),
        legacy_database_paths: Vec::new(),
    })
    .expect("ouverture de la base");

    let today = dmx_core::dates::today_local();
    let mut random = Random(20260912);

    // --- Comptes, dans deux groupes ---
    let accounts = [
        (
            "Compte Courant",
            "Courant",
            1_420.55,
            "Wallet",
            "#3b82f6",
            Some("Quotidien"),
        ),
        ("Livret A", "Épargne", 8_600.00, "PiggyBank", "#10b981", Some("Épargne")),
        (
            "Assurance Vie",
            "Investissement",
            12_750.00,
            "TrendingUp",
            "#8b5cf6",
            Some("Épargne"),
        ),
        ("Espèces", "Espèces", 120.00, "Banknote", "#f59e0b", None),
    ];
    let mut ids = Vec::new();
    for (name, kind, balance, icon, color, group) in accounts {
        if let Some(group) = group {
            // Le groupe peut déjà exister (deux comptes dans « Épargne »).
            let _ = engine.apply_settings_change(SettingsChange::AddCustomGroup(group.to_string()));
        }
        let id = engine
            .save_account(AccountDraft {
                id: None,
                name: name.to_string(),
                account_type: kind.to_string(),
                initial_balance: balance,
                color: color.to_string(),
                icon: icon.to_string(),
                group: group.map(str::to_string),
            })
            .expect("compte");
        ids.push(id);
    }

    let categories = engine.snapshot().expect("instantané").categories.clone();
    let category = |name: &str| {
        categories
            .iter()
            .find(|category| category.name.eq_ignore_ascii_case(name))
            .map(|category| category.id.clone())
            .unwrap_or_else(|| {
                categories
                    .first()
                    .map(|category| category.id.clone())
                    .unwrap_or_default()
            })
    };

    // --- Quatre mois d'opérations ---
    let depenses: [(&str, &str, u64, u64); 8] = [
        ("Courses", "Alimentation", 25, 95),
        ("Essence", "Carburant", 40, 80),
        ("Restaurant", "Restaurants / Cafés", 15, 60),
        ("Pharmacie", "Pharmacie", 8, 40),
        ("Abonnement streaming", "Abonnements (VOD/Musique)", 9, 18),
        ("Électricité", "Charges / Énergie", 60, 110),
        ("Internet", "Téléphonie / Internet", 30, 45),
        ("Vêtements", "Shopping / Vêtements", 20, 120),
    ];
    let start = today - Duration::days(120);
    let mut day = start;
    while day <= today {
        // Retrait d'espèces au début du mois, pour alimenter le compte « Espèces ».
        if day.day() == 2 {
            save(
                &engine,
                TransactionDraft {
                    id: None,
                    kind: TransactionType::Transfer,
                    date: day.format("%Y-%m-%d").to_string(),
                    amount: 80.0,
                    description: "Retrait d'espèces".to_string(),
                    category_id: String::new(),
                    account_id: ids[0].clone(),
                    to_account_id: Some(ids[3].clone()),
                },
            );
        }
        // Salaire le 28 de chaque mois.
        if day.day() == 28 {
            save(
                &engine,
                TransactionDraft {
                    id: None,
                    kind: TransactionType::Income,
                    date: day.format("%Y-%m-%d").to_string(),
                    amount: 2_450.0,
                    description: "Salaire".to_string(),
                    category_id: category("Salaire"),
                    account_id: ids[0].clone(),
                    to_account_id: None,
                },
            );
            // Virement d'épargne juste après le salaire.
            save(
                &engine,
                TransactionDraft {
                    id: None,
                    kind: TransactionType::Transfer,
                    date: day.format("%Y-%m-%d").to_string(),
                    amount: 300.0,
                    description: "Épargne mensuelle".to_string(),
                    category_id: String::new(),
                    account_id: ids[0].clone(),
                    to_account_id: Some(ids[1].clone()),
                },
            );
        }
        // Deux à trois dépenses par jour ouvré.
        let count = if day.weekday().number_from_monday() <= 5 {
            random.next(3)
        } else {
            random.next(2)
        };
        for _ in 0..count {
            let (description, category_name, low, high) = depenses[random.next(depenses.len() as u64) as usize];
            let cash = random.next(10) == 0 && high <= 60;
            let account = if cash { ids[3].clone() } else { ids[0].clone() };
            save(
                &engine,
                TransactionDraft {
                    id: None,
                    kind: TransactionType::Expense,
                    date: day.format("%Y-%m-%d").to_string(),
                    amount: random.amount(low, high),
                    description: description.to_string(),
                    category_id: category(category_name),
                    account_id: account,
                    to_account_id: None,
                },
            );
        }
        day += Duration::days(1);
    }

    // --- Budgets mensuels ---
    for (name, category_name, amount) in [
        ("Courses", "Alimentation", 450.0),
        ("Carburant", "Carburant", 180.0),
        ("Restaurants", "Restaurants / Cafés", 160.0),
        ("Énergie", "Charges / Énergie", 200.0),
    ] {
        engine
            .save_budget(BudgetDraft {
                id: None,
                name: name.to_string(),
                amount,
                category_id: category(category_name),
                account_id: None,
            })
            .expect("budget");
    }

    // --- Échéances ---
    let next_month = first_of_next_month(today);
    engine
        .save_scheduled(ScheduledDraft {
            id: None,
            description: "Salaire".to_string(),
            amount: 2_450.0,
            kind: TransactionType::Income,
            category_id: category("Salaire"),
            account_id: ids[0].clone(),
            to_account_id: None,
            frequency: Periodicity::Monthly,
            next_date: next_month
                .with_day(28)
                .unwrap_or(next_month)
                .format("%Y-%m-%d")
                .to_string(),
            end_date: None,
            budget_id: None,
        })
        .expect("salaire récurrent");
    for (description, category_name, amount, frequency, day_of_month) in [
        ("Loyer", "Loyer / Prêt", 780.0, Periodicity::Monthly, 5),
        ("Assurance auto", "Entretien Voiture", 62.5, Periodicity::Monthly, 12),
        ("Mutuelle", "Médecin / Santé", 48.0, Periodicity::Monthly, 8),
        ("Impôts", "Impôts / Taxes", 1_260.0, Periodicity::Annual, 15),
    ] {
        let date = next_month.with_day(day_of_month).unwrap_or(next_month);
        engine
            .save_scheduled(ScheduledDraft {
                id: None,
                description: description.to_string(),
                amount,
                kind: TransactionType::Expense,
                category_id: category(category_name),
                account_id: ids[0].clone(),
                to_account_id: None,
                frequency,
                next_date: date.format("%Y-%m-%d").to_string(),
                end_date: None,
                budget_id: None,
            })
            .expect("échéance");
    }

    // --- Prédictions : seuil d'alerte et deux transactions fictives ---
    engine
        .apply_settings_change(SettingsChange::PredictionAlertThreshold(200.0))
        .expect("seuil");
    for (description, amount, kind, offset) in [
        ("Réparation voiture", 640.0, TransactionType::Expense, 18),
        ("Prime", 850.0, TransactionType::Income, 34),
    ] {
        engine
            .save_fake_transaction(
                FakeTransactionDraft {
                    id: None,
                    date: (today + Duration::days(offset)).format("%Y-%m-%d").to_string(),
                    description: description.to_string(),
                    amount,
                    kind,
                    account_id: ids[0].clone(),
                    to_account_id: None,
                    category_id: category("Divers"),
                },
                today,
            )
            .expect("transaction fictive");
    }

    // Version vue : le jeu de démonstration ne déclenche pas la modale « Nouveautés ».
    engine
        .apply_settings_change(SettingsChange::LastSeenVersion(env!("CARGO_PKG_VERSION").to_string()))
        .expect("version vue");

    let snapshot = engine.snapshot().expect("instantané");
    println!(
        "==> {} : {} comptes, {} opérations, {} budgets, {} échéances",
        directory,
        snapshot.accounts.len(),
        snapshot.transactions.len(),
        snapshot.budgets.len(),
        snapshot.scheduled.len()
    );
}

/// Une opération sur deux est pointée, comme un compte suivi régulièrement.
fn save(engine: &Engine, draft: TransactionDraft) {
    let checked = draft.date < dmx_core::dates::today_local().format("%Y-%m-%d").to_string();
    let ids = engine.save_transaction(draft).expect("opération");
    if checked {
        engine.set_transactions_checked(&ids, true).expect("pointage");
    }
}

fn first_of_next_month(today: NaiveDate) -> NaiveDate {
    let (year, month) = if today.month() == 12 {
        (today.year() + 1, 1)
    } else {
        (today.year(), today.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today)
}
