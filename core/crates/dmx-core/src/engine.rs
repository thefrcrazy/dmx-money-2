//! Moteur synchrone : ouvre la base, garde un instantané en cache et expose toutes les vues
//! et opérations aux interfaces (Rust direct pour Linux, FFI pour Swift et C#).
//!
//! Les méthodes bloquent le thread appelant : les hôtes les appellent hors du thread principal.
//! Elles ne doivent pas être appelées depuis le runtime tokio du moteur.

use crate::accounts_view::{self, AccountsQuery, AccountsView, TraySummary};
use crate::analytics::{self, AnalyticsQuery, AnalyticsView};
use crate::assistant;
use crate::backup::{self, BackupSummary, RestoreMode};
use crate::budget::{self, BudgetOverview, BudgetQuery};
use crate::dashboard::{self, DashboardView};
use crate::db::{self, DbPool, DATABASE_FILE_NAME};
use crate::error::{CoreError, CoreResult};
use crate::import::{
    self, CsvColumnMapping, CsvOptions, CsvPreview, ParsedStatementTransaction, StatementFormat,
    StatementImportRequest, StatementImportResult,
};
use crate::journal::{self, JournalQuery, JournalView};
use crate::legacy;
use crate::models::{PredictionFakeTransaction, TransactionType};
use crate::ops::{
    self, AccountDraft, AccountDropTarget, BudgetDraft, CategoryDraft, InlineEdit, ScheduledDraft, TransactionDraft,
};
use crate::predictions::{self, FakeTransactionDraft, PredictionQuery, PredictionView};
use crate::scheduled_view::{self, ScheduledQuery, ScheduledView};
use crate::scheduling::{self, ProcessDueResult};
use crate::seed;
use crate::settings::{self, AppSettings, SettingsChange, SettingsPatch, SettingsPatchResult, SettingsRecord};
use crate::snapshot::{self, Snapshot};
use crate::sync::{self, ApplyReport, RemoteChange, SyncChange};
use chrono::NaiveDate;
use serde::Serialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::runtime::{Builder, Handle, Runtime};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub data_dir: PathBuf,
    /// Bases DmxMoney 1.x à reprendre si la nouvelle base n'existe pas encore.
    pub legacy_database_paths: Vec<PathBuf>,
}

impl EngineConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            legacy_database_paths: legacy::default_legacy_database_paths(),
        }
    }
}

/// Marqueur des bases 1.x que l'utilisateur ne veut plus voir proposées.
const LEGACY_IGNORED_FILE: &str = ".legacy-ignored";

/// Identité d'une base pour ce marqueur : chemin et dernière opération connue.
fn legacy_marker(inventory: &legacy::DatabaseInventory) -> String {
    format!(
        "{}|{}",
        inventory.path,
        inventory.last_transaction_date.clone().unwrap_or_default()
    )
}

/// Résultat d'une reprise de base 1.x : ce qui a été repris et où se trouve la sauvegarde.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyAdoption {
    pub inventory: legacy::DatabaseInventory,
    pub summary: backup::BackupSummary,
    /// Sauvegarde `.dmx` de la base remplacée.
    pub backup_file: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenReport {
    pub database_path: String,
    pub created: bool,
    pub imported_legacy_database: Option<String>,
    pub legacy_import_error: Option<String>,
    /// Base DmxMoney 1.x plus complète trouvée à côté : l'interface propose de la reprendre.
    /// Arrive quand le dossier de la 2.x contenait déjà une base (copie ancienne, autre build).
    pub legacy_candidate: Option<legacy::DatabaseInventory>,
}

pub struct Engine {
    runtime: Runtime,
    pool: DbPool,
    data_dir: PathBuf,
    open_report: OpenReport,
    cache: Mutex<Option<Arc<Snapshot>>>,
}

fn build_runtime() -> CoreResult<Runtime> {
    Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("dmx-core")
        .enable_all()
        .build()
        .map_err(CoreError::from)
}

impl Engine {
    pub fn open(config: EngineConfig) -> CoreResult<Self> {
        std::fs::create_dir_all(&config.data_dir)?;
        let runtime = build_runtime()?;
        let database_path = config.data_dir.join(DATABASE_FILE_NAME);

        let mut report = OpenReport {
            database_path: database_path.to_string_lossy().to_string(),
            created: !database_path.exists(),
            imported_legacy_database: None,
            legacy_import_error: None,
            legacy_candidate: None,
        };

        if !database_path.exists() {
            if let Some(source) = legacy::find_legacy_database(&config.legacy_database_paths, &database_path) {
                match runtime.block_on(legacy::copy_legacy_database(&source, &database_path)) {
                    Ok(()) => {
                        report.created = false;
                        report.imported_legacy_database = Some(source.to_string_lossy().to_string());
                        if let Err(error) = legacy::copy_legacy_bridge_files(&source, &config.data_dir) {
                            log::warn!("Certificats du pont 1.x non repris : {error}");
                        }
                    }
                    Err(error) => {
                        log::error!("Reprise de la base DmxMoney 1.x impossible : {error}");
                        report.legacy_import_error = Some(error.to_string());
                    }
                }
            }
        }


        let pool = runtime.block_on(db::open_pool(&database_path))?;
        let engine = Self {
            runtime,
            pool,
            data_dir: config.data_dir,
            open_report: report,
            cache: Mutex::new(None),
        };
        engine.run_startup()?;
        Ok(engine)
    }

    /// Moteur sur une base en mémoire, pour les tests et les aperçus.
    pub fn open_in_memory() -> CoreResult<Self> {
        let runtime = build_runtime()?;
        let pool = runtime.block_on(db::open_memory_pool())?;
        let engine = Self {
            runtime,
            pool,
            data_dir: PathBuf::new(),
            open_report: OpenReport {
                database_path: ":memory:".to_string(),
                created: true,
                imported_legacy_database: None,
                legacy_import_error: None,
                legacy_candidate: None,
            },
            cache: Mutex::new(None),
        };
        engine.run_startup()?;
        Ok(engine)
    }

    fn run_startup(&self) -> CoreResult<()> {
        self.write(async {
            seed::ensure_initial_data(&self.pool).await?;
            seed::migrate_legacy_scheduled_budgets(&self.pool).await?;
            Ok(())
        })
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    pub fn handle(&self) -> Handle {
        self.runtime.handle().clone()
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn open_report(&self) -> &OpenReport {
        &self.open_report
    }

    fn write<T>(&self, future: impl Future<Output = CoreResult<T>>) -> CoreResult<T> {
        let result = self.runtime.block_on(future);
        self.invalidate();
        result
    }

    pub fn invalidate(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            *cache = None;
        }
    }

    pub fn data_version(&self) -> CoreResult<i64> {
        self.runtime.block_on(db::data_version(&self.pool))
    }

    /// Instantané courant, rechargé seulement si les données ont changé (y compris via le pont).
    pub fn snapshot(&self) -> CoreResult<Arc<Snapshot>> {
        let version = self.data_version()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| CoreError::Database("Cache indisponible.".to_string()))?;
        if let Some(snapshot) = cache.as_ref().filter(|snapshot| snapshot.data_version == version) {
            return Ok(Arc::clone(snapshot));
        }
        let snapshot = Arc::new(self.runtime.block_on(snapshot::load(&self.pool))?);
        *cache = Some(Arc::clone(&snapshot));
        Ok(snapshot)
    }

    // --- Vues ---

    pub fn settings(&self) -> CoreResult<AppSettings> {
        Ok(self.snapshot()?.settings.clone())
    }

    pub fn settings_record(&self) -> CoreResult<Option<SettingsRecord>> {
        self.runtime.block_on(settings::get_settings_record(&self.pool))
    }

    pub fn dashboard(&self, accounts: &[String], today: NaiveDate) -> CoreResult<DashboardView> {
        Ok(dashboard::dashboard(&*self.snapshot()?, accounts, today))
    }

    pub fn journal(&self, query: &JournalQuery) -> CoreResult<JournalView> {
        Ok(journal::journal(&*self.snapshot()?, query))
    }

    pub fn budget(&self, query: &BudgetQuery, today: NaiveDate) -> CoreResult<BudgetOverview> {
        Ok(budget::budget_overview(&*self.snapshot()?, query, today))
    }

    pub fn scheduled(&self, query: &ScheduledQuery, today: NaiveDate) -> CoreResult<ScheduledView> {
        Ok(scheduled_view::scheduled_view(&*self.snapshot()?, query, today))
    }

    pub fn analytics(&self, query: &AnalyticsQuery, today: NaiveDate) -> CoreResult<AnalyticsView> {
        Ok(analytics::analytics(&*self.snapshot()?, query, today))
    }

    pub fn predictions(&self, query: &PredictionQuery, today: NaiveDate) -> CoreResult<PredictionView> {
        Ok(predictions::predictions(&*self.snapshot()?, query, today))
    }

    pub fn accounts(&self, query: &AccountsQuery) -> CoreResult<AccountsView> {
        Ok(accounts_view::accounts_view(&*self.snapshot()?, query))
    }

    pub fn tray_summary(&self) -> CoreResult<TraySummary> {
        Ok(accounts_view::tray_summary(&*self.snapshot()?))
    }

    // --- Brouillons de formulaires ---

    pub fn account_draft(&self, id: Option<&str>) -> CoreResult<AccountDraft> {
        match id {
            Some(id) => {
                ops::account_draft(&*self.snapshot()?, id).ok_or_else(|| CoreError::not_found("Compte introuvable."))
            }
            None => Ok(ops::new_account_draft()),
        }
    }

    pub fn category_draft(&self, id: Option<&str>) -> CoreResult<CategoryDraft> {
        match id {
            Some(id) => ops::category_draft(&*self.snapshot()?, id)
                .ok_or_else(|| CoreError::not_found("Catégorie introuvable.")),
            None => Ok(ops::new_category_draft()),
        }
    }

    pub fn transaction_draft(
        &self,
        id: Option<&str>,
        accounts: &[String],
        today: NaiveDate,
    ) -> CoreResult<TransactionDraft> {
        let snapshot = self.snapshot()?;
        match id {
            Some(id) => {
                ops::transaction_draft(&snapshot, id).ok_or_else(|| CoreError::not_found("Transaction introuvable."))
            }
            None => Ok(ops::new_transaction_draft(&snapshot, accounts, today)),
        }
    }

    pub fn budget_draft(&self, id: Option<&str>, accounts: &[String]) -> CoreResult<BudgetDraft> {
        let snapshot = self.snapshot()?;
        match id {
            Some(id) => ops::budget_draft(&snapshot, id).ok_or_else(|| CoreError::not_found("Budget introuvable.")),
            None => Ok(ops::new_budget_draft(&snapshot, accounts)),
        }
    }

    pub fn scheduled_draft(&self, id: Option<&str>, today: NaiveDate) -> CoreResult<ScheduledDraft> {
        let snapshot = self.snapshot()?;
        match id {
            Some(id) => {
                ops::scheduled_draft(&snapshot, id).ok_or_else(|| CoreError::not_found("Échéance introuvable."))
            }
            None => Ok(ops::new_scheduled_draft(&snapshot, today)),
        }
    }

    pub fn fake_transaction_draft(
        &self,
        id: Option<&str>,
        accounts: &[String],
        today: NaiveDate,
    ) -> CoreResult<FakeTransactionDraft> {
        let snapshot = self.snapshot()?;
        match id {
            Some(id) => predictions::fake_transaction_draft(&snapshot, id)
                .ok_or_else(|| CoreError::not_found("Transaction fictive introuvable.")),
            None => Ok(predictions::new_fake_transaction_draft(&snapshot, accounts, today)),
        }
    }

    // --- Écritures ---

    pub fn process_due_scheduled(&self, today: NaiveDate) -> CoreResult<ProcessDueResult> {
        self.write(scheduling::process_due(&self.pool, today))
    }

    // --- Assistant (Siri, PWA) ---

    /// Répond à une phrase et applique ce qu'elle demande quand `apply` est vrai.
    pub fn assistant(&self, text: &str, today: NaiveDate, apply: bool) -> CoreResult<assistant::AssistantReply> {
        let snapshot = self.snapshot()?;
        let Some(intent) = assistant::interpret(&snapshot, text, today) else {
            return Ok(assistant::AssistantReply::not_understood());
        };
        if apply {
            self.assistant_execute(intent, today)
        } else {
            Ok(assistant::answer(&snapshot, &intent, today))
        }
    }

    /// Exécute une intention déjà décidée (par l'analyse du noyau ou par un modèle de l'hôte).
    pub fn assistant_execute(
        &self,
        intent: assistant::AssistantIntent,
        today: NaiveDate,
    ) -> CoreResult<assistant::AssistantReply> {
        match intent {
            assistant::AssistantIntent::AddTransaction(draft) => {
                self.write(ops::save_transaction(&self.pool, draft.clone()))?;
                Ok(assistant::saved_reply(&*self.snapshot()?, &draft))
            }
            assistant::AssistantIntent::ProcessDue => {
                let result = self.process_due_scheduled(today)?;
                Ok(assistant::due_reply(result.created_transactions as usize))
            }
            other => Ok(assistant::answer(&*self.snapshot()?, &other, today)),
        }
    }

    /// Intention déduite d'une phrase, sans rien appliquer.
    pub fn assistant_intent(&self, text: &str, today: NaiveDate) -> CoreResult<Option<assistant::AssistantIntent>> {
        Ok(assistant::interpret(&*self.snapshot()?, text, today))
    }

    /// Solde, pour Siri : le nom du compte est résolu par le noyau.
    pub fn assistant_balance(&self, account: Option<&str>, today: NaiveDate) -> CoreResult<assistant::AssistantReply> {
        let snapshot = self.snapshot()?;
        let intent = assistant::balance_intent(&snapshot, account);
        Ok(assistant::answer(&snapshot, &intent, today))
    }

    /// Reste à dépenser, pour Siri.
    pub fn assistant_budget(&self, category: Option<&str>, today: NaiveDate) -> CoreResult<assistant::AssistantReply> {
        let snapshot = self.snapshot()?;
        let intent = assistant::budget_intent(&snapshot, category);
        Ok(assistant::answer(&snapshot, &intent, today))
    }

    /// Prochaines échéances, pour Siri.
    pub fn assistant_upcoming(&self, today: NaiveDate) -> CoreResult<assistant::AssistantReply> {
        let snapshot = self.snapshot()?;
        Ok(assistant::answer(
            &snapshot,
            &assistant::AssistantIntent::Upcoming,
            today,
        ))
    }

    /// Résumé du mois, pour Siri.
    pub fn assistant_month(&self, today: NaiveDate) -> CoreResult<assistant::AssistantReply> {
        let snapshot = self.snapshot()?;
        Ok(assistant::answer(
            &snapshot,
            &assistant::AssistantIntent::MonthSummary,
            today,
        ))
    }

    /// Brouillon d'opération pour l'intention Siri « ajoute … » (paramètres déjà structurés).
    pub fn assistant_draft(
        &self,
        amount: f64,
        kind: TransactionType,
        category: Option<&str>,
        account: Option<&str>,
        description: Option<&str>,
        today: NaiveDate,
    ) -> CoreResult<assistant::AssistantIntent> {
        let snapshot = self.snapshot()?;
        Ok(assistant::draft_from_parts(
            &snapshot,
            amount,
            kind,
            category,
            account,
            description,
            today,
        ))
    }

    pub fn save_account(&self, draft: AccountDraft) -> CoreResult<String> {
        self.write(ops::save_account(&self.pool, draft))
    }

    pub fn delete_account(&self, id: &str) -> CoreResult<()> {
        self.write(ops::delete_account(&self.pool, id))
    }

    pub fn move_account(&self, account_id: &str, target: AccountDropTarget) -> CoreResult<()> {
        let snapshot = self.snapshot()?;
        self.write(ops::move_account(&self.pool, &snapshot, account_id, target))
    }

    pub fn move_group(&self, group: &str, over_group: &str) -> CoreResult<()> {
        let snapshot = self.snapshot()?;
        self.write(ops::move_group(&self.pool, &snapshot, group, over_group))
    }

    pub fn save_category(&self, draft: CategoryDraft) -> CoreResult<String> {
        self.write(ops::save_category(&self.pool, draft))
    }

    pub fn delete_category(&self, id: &str) -> CoreResult<()> {
        self.write(ops::delete_category(&self.pool, id))
    }

    pub fn save_transaction(&self, draft: TransactionDraft) -> CoreResult<Vec<String>> {
        self.write(ops::save_transaction(&self.pool, draft))
    }

    pub fn delete_transactions(&self, ids: &[String]) -> CoreResult<()> {
        self.write(ops::delete_transactions(&self.pool, ids))
    }

    pub fn set_transactions_checked(&self, ids: &[String], checked: bool) -> CoreResult<()> {
        self.write(ops::set_transactions_checked(&self.pool, ids, checked))
    }

    pub fn toggle_transactions_checked(&self, ids: &[String]) -> CoreResult<bool> {
        let snapshot = self.snapshot()?;
        self.write(ops::toggle_transactions_checked(&self.pool, &snapshot, ids))
    }

    pub fn update_transaction_inline(&self, id: &str, edit: InlineEdit) -> CoreResult<()> {
        self.write(ops::update_transaction_inline(&self.pool, id, edit))
    }

    pub fn save_budget(&self, draft: BudgetDraft) -> CoreResult<String> {
        self.write(ops::save_budget(&self.pool, draft))
    }

    pub fn delete_budget(&self, id: &str) -> CoreResult<()> {
        self.write(ops::delete_budget(&self.pool, id))
    }

    pub fn accept_budget_suggestion(&self, key: &str, accounts: &[String], today: NaiveDate) -> CoreResult<String> {
        let snapshot = self.snapshot()?;
        let suggestion = budget::budget_suggestions(&snapshot, accounts, today)
            .into_iter()
            .find(|suggestion| suggestion.key == key)
            .ok_or_else(|| CoreError::not_found("Suggestion introuvable."))?;
        self.save_budget(BudgetDraft {
            id: None,
            name: suggestion.name,
            amount: suggestion.amount,
            category_id: suggestion.category.id,
            account_id: suggestion.account_id,
        })
    }

    pub fn save_scheduled(&self, draft: ScheduledDraft) -> CoreResult<String> {
        self.write(ops::save_scheduled(&self.pool, draft))
    }

    pub fn delete_scheduled(&self, id: &str) -> CoreResult<()> {
        self.write(ops::delete_scheduled(&self.pool, id))
    }

    pub fn accept_scheduled_suggestion(&self, key: &str, accounts: &[String], today: NaiveDate) -> CoreResult<String> {
        let snapshot = self.snapshot()?;
        let suggestion = scheduled_view::scheduled_suggestions(&snapshot, accounts, today)
            .into_iter()
            .find(|suggestion| suggestion.key == key)
            .ok_or_else(|| CoreError::not_found("Suggestion introuvable."))?;
        self.write(ops::add_scheduled(&self.pool, suggestion.to_scheduled()))
    }

    // --- Paramètres ---

    pub fn apply_settings_change(&self, change: SettingsChange) -> CoreResult<SettingsPatchResult> {
        self.write(settings::apply_change(&self.pool, change))
    }

    pub fn apply_settings_patch(&self, patch: SettingsPatch) -> CoreResult<SettingsPatchResult> {
        self.write(settings::apply_settings_patch(&self.pool, patch))
    }

    pub fn save_fake_transaction(&self, draft: FakeTransactionDraft, today: NaiveDate) -> CoreResult<String> {
        let fake = predictions::validate_fake_transaction(&*self.snapshot()?, draft, today)?;
        let id = fake.id.clone();
        self.apply_settings_change(SettingsChange::UpsertFakeTransaction(fake))?;
        Ok(id)
    }

    pub fn toggle_fake_transaction(&self, id: &str) -> CoreResult<()> {
        let fake: PredictionFakeTransaction = self
            .snapshot()?
            .settings
            .prediction_fake_transactions
            .iter()
            .find(|fake| fake.id == id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("Transaction fictive introuvable."))?;
        self.apply_settings_change(SettingsChange::UpsertFakeTransaction(PredictionFakeTransaction {
            enabled: !fake.enabled,
            ..fake
        }))
        .map(|_| ())
    }

    pub fn delete_fake_transactions(&self, ids: Vec<String>) -> CoreResult<()> {
        self.apply_settings_change(SettingsChange::DeleteFakeTransactions(ids))
            .map(|_| ())
    }

    /// Retire les transactions fictives visibles avec le filtre de comptes courant.
    pub fn clear_applied_fake_transactions(&self, accounts: &[String]) -> CoreResult<()> {
        let ids: Vec<String> = self
            .snapshot()?
            .settings
            .prediction_fake_transactions
            .iter()
            .filter(|fake| {
                predictions::applies_to_filter(
                    accounts,
                    &fake.account_id,
                    fake.transaction_type,
                    fake.to_account_id.as_deref(),
                )
            })
            .map(|fake| fake.id.clone())
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        self.delete_fake_transactions(ids)
    }

    // --- Imports et sauvegardes ---

    /// Inventaire de la base ouverte, pour la comparer à une base 1.x trouvée à côté.
    pub fn current_inventory(&self) -> CoreResult<legacy::DatabaseInventory> {
        let snapshot = self.snapshot()?;
        Ok(legacy::DatabaseInventory {
            path: self.open_report.database_path.clone(),
            accounts: snapshot.accounts.len() as u32,
            transactions: snapshot.transactions.len() as u32,
            categories: snapshot.categories.len() as u32,
            budgets: snapshot.budgets.len() as u32,
            scheduled: snapshot.scheduled.len() as u32,
            last_transaction_date: snapshot
                .transactions
                .iter()
                .map(|transaction| transaction.date.clone())
                .max(),
        })
    }

    /// Ne plus proposer cette base 1.x (marqueur dans le dossier de données).
    pub fn ignore_legacy_candidate(&self) -> CoreResult<()> {
        let Some(candidate) = self.open_report.legacy_candidate.as_ref() else {
            return Ok(());
        };
        std::fs::write(self.data_dir.join(LEGACY_IGNORED_FILE), legacy_marker(candidate))?;
        Ok(())
    }

    /// Reprend une base DmxMoney 1.x dans la base courante.
    ///
    /// L'existante est d'abord exportée en `.dmx` dans le dossier de données (rien n'est perdu),
    /// puis remplacée par le contenu de la base 1.x, qui n'est jamais modifiée (copie
    /// `VACUUM INTO` en lecture seule).
    pub fn adopt_legacy_database(&self, path: &str, today: NaiveDate) -> CoreResult<LegacyAdoption> {
        let source = PathBuf::from(path);
        let inventory = self.runtime.block_on(legacy::inventory(&source))?;

        // 1. Sauvegarde de l'existante, au format d'export habituel.
        let backup = self.export_backup()?;
        let backup_name = format!("avant-reprise-{}.dmx", today.format("%Y-%m-%d"));
        let backup_path = self.data_dir.join(&backup_name);
        std::fs::write(&backup_path, backup)?;

        // 2. Copie de la base 1.x dans un fichier temporaire, puis reprise par le format `.dmx`.
        let staging = self.data_dir.join("reprise-1x.db");
        let _ = std::fs::remove_file(&staging);
        self.runtime.block_on(legacy::copy_legacy_database(&source, &staging))?;
        let content = self.runtime.block_on(async {
            let pool = db::open_pool(&staging).await?;
            let file = backup::build_backup(&pool).await?;
            pool.close().await;
            backup::encode_backup(&file)
        });
        let _ = std::fs::remove_file(&staging);
        let content = content?;

        let summary = self.write(backup::restore_backup(
            &self.pool,
            &content,
            backup::RestoreMode::Replace,
        ))?;
        if let Err(error) = legacy::copy_legacy_bridge_files(&source, &self.data_dir) {
            log::warn!("Certificats du pont 1.x non repris : {error}");
        }
        Ok(LegacyAdoption {
            inventory,
            summary,
            backup_file: backup_path.to_string_lossy().to_string(),
        })
    }

    pub fn export_backup(&self) -> CoreResult<String> {
        self.runtime.block_on(backup::export_backup(&self.pool))
    }

    pub fn inspect_backup(&self, content: &str) -> CoreResult<BackupSummary> {
        backup::decode_backup(content).map(|file| backup::summarize(&file))
    }

    pub fn restore_backup(&self, content: &str, mode: RestoreMode) -> CoreResult<BackupSummary> {
        let summary = self.write(backup::restore_backup(&self.pool, content, mode))?;
        self.run_startup()?;
        Ok(summary)
    }

    pub fn preview_csv(&self, content: &str, options: CsvOptions) -> CoreResult<CsvPreview> {
        import::preview_csv(content, options)
    }

    pub fn parse_statement(
        &self,
        format: StatementFormat,
        content: &str,
        csv: Option<(CsvOptions, CsvColumnMapping)>,
        today: NaiveDate,
    ) -> CoreResult<Vec<ParsedStatementTransaction>> {
        import::parse_statement(format, content, csv, today)
    }

    pub fn suggest_category_mapping(&self, sources: &[String]) -> CoreResult<Vec<import::CategoryMatch>> {
        Ok(import::suggest_category_mapping(&*self.snapshot()?, sources))
    }

    pub fn import_statement(&self, request: StatementImportRequest) -> CoreResult<StatementImportResult> {
        self.write(import::import_statement(&self.pool, request))
    }

    // --- Synchronisation ---

    pub fn pending_sync_changes(&self, limit: u32) -> CoreResult<Vec<SyncChange>> {
        self.runtime.block_on(sync::pending_changes(&self.pool, limit))
    }

    pub fn pending_sync_count(&self) -> CoreResult<i64> {
        self.runtime.block_on(sync::pending_count(&self.pool))
    }

    pub fn acknowledge_sync_changes(&self, changes: &[SyncChange]) -> CoreResult<()> {
        self.runtime.block_on(sync::acknowledge_changes(&self.pool, changes))
    }

    pub fn apply_remote_changes(&self, changes: Vec<RemoteChange>) -> CoreResult<ApplyReport> {
        self.write(sync::apply_remote_changes(&self.pool, changes))
    }

    pub fn enqueue_all_for_sync(&self) -> CoreResult<i64> {
        self.runtime.block_on(sync::enqueue_all(&self.pool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::parse_date;
    use crate::models::TransactionType;

    fn today() -> NaiveDate {
        parse_date("2026-09-15").unwrap()
    }

    #[test]
    fn end_to_end_account_transaction_and_views() {
        let engine = Engine::open_in_memory().unwrap();
        assert_eq!(
            engine.snapshot().unwrap().categories.len(),
            crate::seed::DEFAULT_CATEGORIES.len()
        );

        let account = engine
            .save_account(AccountDraft {
                name: "Courant".into(),
                initial_balance: 1000.0,
                ..ops::new_account_draft()
            })
            .unwrap();
        let mut draft = engine.transaction_draft(None, &[], today()).unwrap();
        assert_eq!(draft.account_id, account);
        draft.amount = 250.0;
        draft.category_id = "5".into();
        draft.description = "Courses".into();
        draft.kind = TransactionType::Expense;
        engine.save_transaction(draft).unwrap();

        let dashboard = engine.dashboard(&[], today()).unwrap();
        assert_eq!(dashboard.balances.current_balance, 750.0);
        assert_eq!(dashboard.month.expenses, 250.0);

        let version = engine.snapshot().unwrap().data_version;
        assert_eq!(engine.snapshot().unwrap().data_version, version);
        engine
            .set_transactions_checked(&[dashboard.recent_transactions[0].transaction.id.clone()], true)
            .unwrap();
        assert_eq!(engine.dashboard(&[], today()).unwrap().balances.checked_balance, 750.0);
    }

    #[test]
    fn opening_imports_the_legacy_database_once() {
        let root = std::env::temp_dir().join(format!("dmx-engine-{}", uuid::Uuid::new_v4()));
        let legacy_path = root.join("legacy").join(DATABASE_FILE_NAME);
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();

        {
            let legacy = Engine::open(EngineConfig {
                data_dir: root.join("legacy"),
                legacy_database_paths: vec![],
            })
            .unwrap();
            legacy
                .save_account(AccountDraft {
                    name: "Compte 1.x".into(),
                    ..ops::new_account_draft()
                })
                .unwrap();
        }

        let config = EngineConfig {
            data_dir: root.join("v2"),
            legacy_database_paths: vec![legacy_path.clone()],
        };
        let engine = Engine::open(config.clone()).unwrap();
        assert_eq!(
            engine.open_report().imported_legacy_database.as_deref(),
            Some(legacy_path.to_string_lossy().as_ref())
        );
        assert_eq!(engine.snapshot().unwrap().accounts[0].name, "Compte 1.x");
        drop(engine);

        let reopened = Engine::open(config).unwrap();
        assert!(reopened.open_report().imported_legacy_database.is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fake_transactions_round_trip_through_settings() {
        let engine = Engine::open_in_memory().unwrap();
        let account = engine
            .save_account(AccountDraft {
                name: "Courant".into(),
                ..ops::new_account_draft()
            })
            .unwrap();
        let mut draft = engine.fake_transaction_draft(None, &[], today()).unwrap();
        draft.amount = 90.0;
        draft.date = "2026-09-20".into();
        let id = engine.save_fake_transaction(draft, today()).unwrap();

        engine.toggle_fake_transaction(&id).unwrap();
        let settings = engine.settings().unwrap();
        assert_eq!(settings.prediction_fake_transactions.len(), 1);
        assert!(!settings.prediction_fake_transactions[0].enabled);
        assert_eq!(settings.prediction_fake_transactions[0].account_id, account);

        engine.clear_applied_fake_transactions(&[]).unwrap();
        assert!(engine.settings().unwrap().prediction_fake_transactions.is_empty());
    }
}
