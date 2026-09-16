//! Façade UniFFI de `dmx-core` pour les applications Swift (macOS, iOS) et C# (Windows).
//!
//! Toutes les méthodes sont synchrones et bloquent le thread appelant : les hôtes les appellent
//! depuis un thread de fond. Les dates sont échangées au format `YYYY-MM-DD`.

uniffi::setup_scaffolding!();

mod remote;
mod types;

pub use types::*;

use dmx_core::accounts_view::{AccountsQuery, AccountsView, TraySummary};
use dmx_core::analytics::{AnalyticsQuery, AnalyticsView};
use dmx_core::backup::{BackupSummary, RestoreMode};
use dmx_core::budget::{BudgetOverview, BudgetQuery, BudgetState};
use dmx_core::dashboard::DashboardView;
use dmx_core::engine::LegacyAdoption;
use dmx_core::import::{
    CategoryMatch, CsvColumnMapping, CsvPreview, ParsedStatementTransaction, StatementFormat, StatementImportResult,
};
use dmx_core::journal::{JournalQuery, JournalView};
use dmx_core::legacy::DatabaseInventory;
use dmx_core::models::{Account, Budget, Category, Periodicity, ScheduledDueRange, TimeRange, TransactionType};
use dmx_core::ops::{AccountDraft, BudgetDraft, CategoryDraft, InlineEdit, ScheduledDraft, TransactionDraft};
use dmx_core::predictions::{FakeTransactionDraft, PredictionQuery, PredictionView};
use dmx_core::scheduled_view::{ScheduledQuery, ScheduledView};
use dmx_core::scheduling::ProcessDueResult;
use dmx_core::settings::{SettingsChange as CoreSettingsChange, SettingsPatchResult};
use dmx_core::sync::{ApplyReport, RemoteChange, SyncChange};
use dmx_core::{dates, format, import, palette, CoreError, Engine, EngineConfig, OpenReport};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum DmxError {
    #[error("{message}")]
    Database { message: String },
    #[error("{message}")]
    Validation { message: String },
    #[error("{message}")]
    NotFound { message: String },
    #[error("{message}")]
    Import { message: String },
    #[error("{message}")]
    Io { message: String },
    #[error("{message}")]
    Bridge { message: String },
    #[error("{message}")]
    Unsupported { message: String },
}

impl From<CoreError> for DmxError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::Database(message) => DmxError::Database { message },
            CoreError::Validation(message) => DmxError::Validation { message },
            CoreError::NotFound(message) => DmxError::NotFound { message },
            CoreError::Import(message) => DmxError::Import { message },
            CoreError::Io(message) => DmxError::Io { message },
        }
    }
}

type FfiResult<T> = Result<T, DmxError>;

fn day(value: &str) -> FfiResult<chrono::NaiveDate> {
    dates::parse_date(value).ok_or_else(|| DmxError::Validation {
        message: format!("Date invalide : {value}"),
    })
}

/// Réponse de l'assistant, à plat pour les hôtes.
#[derive(uniffi::Record)]
pub struct AssistantResult {
    /// Phrase à lire ou à afficher.
    pub summary: String,
    /// Lignes de détail (comptes, catégories…).
    pub details: Vec<String>,
    /// Vrai si des données ont été écrites.
    pub changed: bool,
    /// Faux quand la demande n'a pas été comprise.
    pub understood: bool,
    /// Opération proposée, renseignée pour un ajout (utile pour confirmer avant d'écrire).
    pub draft: Option<TransactionDraft>,
}

impl From<dmx_core::assistant::AssistantReply> for AssistantResult {
    fn from(reply: dmx_core::assistant::AssistantReply) -> Self {
        let draft = match &reply.intent {
            Some(dmx_core::assistant::AssistantIntent::AddTransaction(draft)) => Some(draft.clone()),
            _ => None,
        };
        Self {
            summary: reply.summary,
            details: reply.details,
            changed: reply.changed,
            understood: reply.intent.is_some(),
            draft,
        }
    }
}

/// Notifications vers l'application hôte, appelées depuis un thread quelconque.
#[uniffi::export(callback_interface)]
pub trait EngineListener: Send + Sync {
    /// Les données ont changé hors de l'interface (PWA, synchronisation).
    fn data_changed(&self, data_version: i64);
    /// L'état du pont compagnon a changé.
    fn bridge_status_changed(&self);
    /// Reformulation d'une demande de l'assistant par le modèle sur l'appareil de l'hôte.
    ///
    /// Renvoyer `nil` laisse le noyau analyser la phrase telle quelle.
    fn rephrase_assistant_request(&self, text: String) -> Option<String>;
}

type ListenerSlot = RwLock<Option<Box<dyn EngineListener>>>;

#[cfg(feature = "bridge")]
struct ListenerEvents(Arc<ListenerSlot>);

#[cfg(feature = "bridge")]
impl dmx_bridge::BridgeEvents for ListenerEvents {
    fn data_changed(&self, data_version: i64) {
        if let Ok(slot) = self.0.read() {
            if let Some(listener) = slot.as_ref() {
                listener.data_changed(data_version);
            }
        }
    }

    fn status_changed(&self) {
        if let Ok(slot) = self.0.read() {
            if let Some(listener) = slot.as_ref() {
                listener.bridge_status_changed();
            }
        }
    }

    fn rephrase_assistant_request(&self, text: &str) -> Option<String> {
        let slot = self.0.read().ok()?;
        let listener = slot.as_ref()?;
        listener
            .rephrase_assistant_request(text.to_string())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }
}

#[derive(uniffi::Object)]
pub struct DmxEngine {
    engine: Engine,
    listener: Arc<ListenerSlot>,
    #[cfg(feature = "bridge")]
    companion: Mutex<Option<Arc<dmx_bridge::MobileCompanion>>>,
    #[cfg(not(feature = "bridge"))]
    #[allow(dead_code)]
    companion: Mutex<()>,
}

impl DmxEngine {
    fn wrap(engine: Engine) -> Arc<Self> {
        Arc::new(Self {
            engine,
            listener: Arc::new(RwLock::new(None)),
            companion: Mutex::new(Default::default()),
        })
    }

    #[cfg(feature = "bridge")]
    fn companion(&self) -> FfiResult<Arc<dmx_bridge::MobileCompanion>> {
        self.start_bridge(None)?;
        self.companion
            .lock()
            .map_err(|_| DmxError::Bridge {
                message: "Pont indisponible.".to_string(),
            })?
            .clone()
            .ok_or_else(|| DmxError::Bridge {
                message: "Pont non démarré.".to_string(),
            })
    }
}

#[cfg(feature = "bridge")]
fn bridge_error(message: String) -> DmxError {
    DmxError::Bridge { message }
}

#[uniffi::export]
impl DmxEngine {
    /// Ouvre (ou crée) la base dans `data_dir`, en reprenant au besoin une base DmxMoney 1.x.
    #[uniffi::constructor]
    pub fn open(data_dir: String, legacy_database_paths: Vec<String>) -> FfiResult<Arc<Self>> {
        let engine = Engine::open(EngineConfig {
            data_dir: PathBuf::from(data_dir),
            legacy_database_paths: legacy_database_paths.into_iter().map(PathBuf::from).collect(),
        })?;
        Ok(Self::wrap(engine))
    }

    /// Base en mémoire, pour les aperçus d'interface et les tests.
    #[uniffi::constructor]
    pub fn open_in_memory() -> FfiResult<Arc<Self>> {
        Ok(Self::wrap(Engine::open_in_memory()?))
    }

    pub fn open_report(&self) -> OpenReport {
        self.engine.open_report().clone()
    }

    pub fn set_listener(&self, listener: Box<dyn EngineListener>) {
        if let Ok(mut slot) = self.listener.write() {
            *slot = Some(listener);
        }
    }

    pub fn data_version(&self) -> FfiResult<i64> {
        Ok(self.engine.data_version()?)
    }

    // --- Listes et vues ---

    pub fn accounts_list(&self) -> FfiResult<Vec<Account>> {
        Ok(self.engine.snapshot()?.accounts.clone())
    }

    pub fn categories(&self) -> FfiResult<Vec<Category>> {
        Ok(self.engine.snapshot()?.categories.clone())
    }

    pub fn budgets_list(&self) -> FfiResult<Vec<Budget>> {
        Ok(self.engine.snapshot()?.budgets.clone())
    }

    pub fn settings(&self) -> FfiResult<AppSettings> {
        Ok(self.engine.settings()?.into())
    }

    pub fn dashboard(&self, accounts: Vec<String>, today: String) -> FfiResult<DashboardView> {
        Ok(self.engine.dashboard(&accounts, day(&today)?)?)
    }

    pub fn journal(&self, query: JournalQuery) -> FfiResult<JournalView> {
        Ok(self.engine.journal(&query)?)
    }

    pub fn budget(&self, query: BudgetQuery, today: String) -> FfiResult<BudgetOverview> {
        Ok(self.engine.budget(&query, day(&today)?)?)
    }

    pub fn scheduled(&self, query: ScheduledQuery, today: String) -> FfiResult<ScheduledView> {
        Ok(self.engine.scheduled(&query, day(&today)?)?)
    }

    pub fn analytics(&self, query: AnalyticsQuery, today: String) -> FfiResult<AnalyticsView> {
        Ok(self.engine.analytics(&query, day(&today)?)?)
    }

    pub fn predictions(&self, query: PredictionQuery, today: String) -> FfiResult<PredictionView> {
        Ok(self.engine.predictions(&query, day(&today)?)?)
    }

    pub fn accounts(&self, query: AccountsQuery) -> FfiResult<AccountsView> {
        Ok(self.engine.accounts(&query)?)
    }

    pub fn tray_summary(&self) -> FfiResult<TraySummary> {
        Ok(self.engine.tray_summary()?)
    }

    /// Requête Analyses préremplie avec les préférences enregistrées.
    pub fn analytics_query(&self, accounts: Vec<String>) -> FfiResult<AnalyticsQuery> {
        Ok(AnalyticsQuery::from_settings(&self.engine.settings()?, accounts))
    }

    /// Requête Prédictions préremplie avec les préférences enregistrées.
    pub fn prediction_query(&self, accounts: Vec<String>) -> FfiResult<PredictionQuery> {
        Ok(PredictionQuery::from_settings(&self.engine.settings()?, accounts))
    }

    // --- Brouillons de formulaires ---

    pub fn account_draft(&self, id: Option<String>) -> FfiResult<AccountDraft> {
        Ok(self.engine.account_draft(id.as_deref())?)
    }

    /// Brouillon de catégorie : la catégorie existante, ou les valeurs par défaut du noyau.
    pub fn category_draft(&self, id: Option<String>) -> FfiResult<CategoryDraft> {
        Ok(self.engine.category_draft(id.as_deref())?)
    }

    pub fn transaction_draft(
        &self,
        id: Option<String>,
        accounts: Vec<String>,
        today: String,
    ) -> FfiResult<TransactionDraft> {
        Ok(self.engine.transaction_draft(id.as_deref(), &accounts, day(&today)?)?)
    }

    pub fn budget_draft(&self, id: Option<String>, accounts: Vec<String>) -> FfiResult<BudgetDraft> {
        Ok(self.engine.budget_draft(id.as_deref(), &accounts)?)
    }

    pub fn scheduled_draft(&self, id: Option<String>, today: String) -> FfiResult<ScheduledDraft> {
        Ok(self.engine.scheduled_draft(id.as_deref(), day(&today)?)?)
    }

    pub fn fake_transaction_draft(
        &self,
        id: Option<String>,
        accounts: Vec<String>,
        today: String,
    ) -> FfiResult<FakeTransactionDraft> {
        Ok(self
            .engine
            .fake_transaction_draft(id.as_deref(), &accounts, day(&today)?)?)
    }

    // --- Écritures ---

    pub fn process_due_scheduled(&self, today: String) -> FfiResult<ProcessDueResult> {
        Ok(self.engine.process_due_scheduled(day(&today)?)?)
    }

    // --- Assistant (Siri sur macOS, texte libre depuis la PWA) ---

    /// Répond à une phrase et applique ce qu'elle demande quand `apply` est vrai.
    pub fn assistant(&self, text: String, today: String, apply: bool) -> FfiResult<AssistantResult> {
        Ok(self.engine.assistant(&text, day(&today)?, apply)?.into())
    }

    /// Enregistre une opération dictée, en laissant le noyau résoudre compte et catégorie.
    pub fn assistant_add_transaction(
        &self,
        amount: f64,
        kind: TransactionType,
        category: Option<String>,
        account: Option<String>,
        description: Option<String>,
        today: String,
    ) -> FfiResult<AssistantResult> {
        let day = day(&today)?;
        let intent = self.engine.assistant_draft(
            amount,
            kind,
            category.as_deref(),
            account.as_deref(),
            description.as_deref(),
            day,
        )?;
        Ok(self.engine.assistant_execute(intent, day)?.into())
    }

    /// Structured transfer: account IDs come directly from Siri entities, never from prose.
    pub fn assistant_transfer(
        &self,
        amount: f64,
        from_account_id: String,
        to_account_id: String,
        today: String,
    ) -> FfiResult<AssistantResult> {
        let today = day(&today)?;
        let mut draft = self.engine.transaction_draft(None, &[], today)?;
        draft.kind = TransactionType::Transfer;
        draft.amount = amount;
        draft.account_id = from_account_id;
        draft.to_account_id = Some(to_account_id);
        draft.category_id = dmx_core::models::TRANSFER_CATEGORY_ID.to_string();
        draft.description = "Virement".to_string();
        Ok(self
            .engine
            .assistant_execute(dmx_core::assistant::AssistantIntent::AddTransaction(draft), today)?
            .into())
    }

    pub fn assistant_balance(&self, account: Option<String>, today: String) -> FfiResult<AssistantResult> {
        Ok(self.engine.assistant_balance(account.as_deref(), day(&today)?)?.into())
    }

    pub fn assistant_budget(&self, category: Option<String>, today: String) -> FfiResult<AssistantResult> {
        Ok(self.engine.assistant_budget(category.as_deref(), day(&today)?)?.into())
    }

    pub fn assistant_upcoming(&self, today: String) -> FfiResult<AssistantResult> {
        Ok(self.engine.assistant_upcoming(day(&today)?)?.into())
    }

    pub fn assistant_month(&self, today: String) -> FfiResult<AssistantResult> {
        Ok(self.engine.assistant_month(day(&today)?)?.into())
    }

    pub fn save_account(&self, draft: AccountDraft) -> FfiResult<String> {
        Ok(self.engine.save_account(draft)?)
    }

    pub fn delete_account(&self, id: String) -> FfiResult<()> {
        Ok(self.engine.delete_account(&id)?)
    }

    pub fn move_account(&self, account_id: String, target: DropTarget) -> FfiResult<()> {
        Ok(self.engine.move_account(&account_id, target.into())?)
    }

    pub fn move_group(&self, group: String, over_group: String) -> FfiResult<()> {
        Ok(self.engine.move_group(&group, &over_group)?)
    }

    pub fn save_category(&self, draft: CategoryDraft) -> FfiResult<String> {
        Ok(self.engine.save_category(draft)?)
    }

    pub fn delete_category(&self, id: String) -> FfiResult<()> {
        Ok(self.engine.delete_category(&id)?)
    }

    pub fn save_transaction(&self, draft: TransactionDraft) -> FfiResult<Vec<String>> {
        Ok(self.engine.save_transaction(draft)?)
    }

    pub fn delete_transactions(&self, ids: Vec<String>) -> FfiResult<()> {
        Ok(self.engine.delete_transactions(&ids)?)
    }

    pub fn set_transactions_checked(&self, ids: Vec<String>, checked: bool) -> FfiResult<()> {
        Ok(self.engine.set_transactions_checked(&ids, checked)?)
    }

    /// Pointage groupé ; renvoie le nouvel état.
    pub fn toggle_transactions_checked(&self, ids: Vec<String>) -> FfiResult<bool> {
        Ok(self.engine.toggle_transactions_checked(&ids)?)
    }

    pub fn update_transaction_description(&self, id: String, description: String) -> FfiResult<()> {
        Ok(self
            .engine
            .update_transaction_inline(&id, InlineEdit::Description(description))?)
    }

    pub fn update_transaction_amount(&self, id: String, amount: f64) -> FfiResult<()> {
        Ok(self.engine.update_transaction_inline(&id, InlineEdit::Amount(amount))?)
    }

    pub fn save_budget(&self, draft: BudgetDraft) -> FfiResult<String> {
        Ok(self.engine.save_budget(draft)?)
    }

    pub fn delete_budget(&self, id: String) -> FfiResult<()> {
        Ok(self.engine.delete_budget(&id)?)
    }

    pub fn accept_budget_suggestion(&self, key: String, accounts: Vec<String>, today: String) -> FfiResult<String> {
        Ok(self.engine.accept_budget_suggestion(&key, &accounts, day(&today)?)?)
    }

    pub fn dismiss_budget_suggestion(&self, key: String) -> FfiResult<()> {
        self.engine
            .apply_settings_change(CoreSettingsChange::DismissBudgetSuggestion(key))?;
        Ok(())
    }

    pub fn save_scheduled(&self, draft: ScheduledDraft) -> FfiResult<String> {
        Ok(self.engine.save_scheduled(draft)?)
    }

    pub fn delete_scheduled(&self, id: String) -> FfiResult<()> {
        Ok(self.engine.delete_scheduled(&id)?)
    }

    pub fn accept_scheduled_suggestion(&self, key: String, accounts: Vec<String>, today: String) -> FfiResult<String> {
        Ok(self.engine.accept_scheduled_suggestion(&key, &accounts, day(&today)?)?)
    }

    pub fn dismiss_scheduled_suggestion(&self, key: String) -> FfiResult<()> {
        self.engine
            .apply_settings_change(CoreSettingsChange::DismissScheduledSuggestion(key))?;
        Ok(())
    }

    // --- Paramètres ---

    pub fn apply_settings_change(&self, change: SettingsChange) -> FfiResult<SettingsPatchResult> {
        Ok(self.engine.apply_settings_change(change.into())?)
    }

    pub fn save_fake_transaction(&self, draft: FakeTransactionDraft, today: String) -> FfiResult<String> {
        Ok(self.engine.save_fake_transaction(draft, day(&today)?)?)
    }

    pub fn toggle_fake_transaction(&self, id: String) -> FfiResult<()> {
        Ok(self.engine.toggle_fake_transaction(&id)?)
    }

    pub fn delete_fake_transactions(&self, ids: Vec<String>) -> FfiResult<()> {
        Ok(self.engine.delete_fake_transactions(ids)?)
    }

    pub fn clear_applied_fake_transactions(&self, accounts: Vec<String>) -> FfiResult<()> {
        Ok(self.engine.clear_applied_fake_transactions(&accounts)?)
    }

    // --- Imports et sauvegardes ---

    /// Contenu d'un fichier `.dmx` à écrire.
    /// Inventaire de la base ouverte (comparaison avec une base 1.x trouvée à côté).
    pub fn current_inventory(&self) -> FfiResult<DatabaseInventory> {
        Ok(self.engine.current_inventory()?)
    }

    /// Ne plus proposer la base 1.x détectée.
    pub fn ignore_legacy_candidate(&self) -> FfiResult<()> {
        Ok(self.engine.ignore_legacy_candidate()?)
    }

    /// Reprend une base DmxMoney 1.x détectée par `open_report().legacy_candidate`.
    /// La base actuelle est d'abord exportée en `.dmx` dans le dossier de données.
    pub fn adopt_legacy_database(&self, path: String, today: String) -> FfiResult<LegacyAdoption> {
        Ok(self.engine.adopt_legacy_database(&path, day(&today)?)?)
    }

    pub fn export_backup(&self) -> FfiResult<String> {
        Ok(self.engine.export_backup()?)
    }

    pub fn inspect_backup(&self, content: String) -> FfiResult<BackupSummary> {
        Ok(self.engine.inspect_backup(&content)?)
    }

    pub fn restore_backup(&self, content: String, mode: RestoreMode) -> FfiResult<BackupSummary> {
        Ok(self.engine.restore_backup(&content, mode)?)
    }

    pub fn preview_csv(&self, content: String, options: CsvOptions) -> FfiResult<CsvPreview> {
        Ok(self.engine.preview_csv(&content, options.to_core())?)
    }

    pub fn parse_statement(
        &self,
        format: StatementFormat,
        content: String,
        csv_options: Option<CsvOptions>,
        csv_mapping: Option<CsvColumnMapping>,
        today: String,
    ) -> FfiResult<Vec<ParsedStatementTransaction>> {
        let csv = csv_options.map(|options| (options.to_core(), csv_mapping.unwrap_or_default()));
        Ok(self.engine.parse_statement(format, &content, csv, day(&today)?)?)
    }

    pub fn suggest_category_mapping(&self, sources: Vec<String>) -> FfiResult<Vec<CategoryMatch>> {
        Ok(self.engine.suggest_category_mapping(&sources)?)
    }

    pub fn import_statement(&self, request: StatementImportRequest) -> FfiResult<StatementImportResult> {
        Ok(self.engine.import_statement(request.into())?)
    }

    // --- Synchronisation ---

    pub fn pending_sync_changes(&self, limit: u32) -> FfiResult<Vec<SyncChange>> {
        Ok(self.engine.pending_sync_changes(limit)?)
    }

    pub fn pending_sync_count(&self) -> FfiResult<i64> {
        Ok(self.engine.pending_sync_count()?)
    }

    pub fn acknowledge_sync_changes(&self, changes: Vec<SyncChange>) -> FfiResult<()> {
        Ok(self.engine.acknowledge_sync_changes(&changes)?)
    }

    pub fn apply_remote_changes(&self, changes: Vec<RemoteChange>) -> FfiResult<ApplyReport> {
        let report = self.engine.apply_remote_changes(changes)?;
        if report.applied > 0 {
            if let (Ok(version), Ok(slot)) = (self.engine.data_version(), self.listener.read()) {
                if let Some(listener) = slot.as_ref() {
                    listener.data_changed(version);
                }
            }
        }
        Ok(report)
    }

    pub fn enqueue_all_for_sync(&self) -> FfiResult<i64> {
        Ok(self.engine.enqueue_all_for_sync()?)
    }

    // --- Pont compagnon (desktop) ---

    pub fn bridge_supported(&self) -> bool {
        cfg!(feature = "bridge")
    }

    /// Démarre le pont si le mode compagnon est actif ; `assets_dir` contient le build de la PWA.
    pub fn start_bridge(&self, assets_dir: Option<String>) -> FfiResult<()> {
        #[cfg(feature = "bridge")]
        {
            let mut slot = self
                .companion
                .lock()
                .map_err(|_| bridge_error("Pont indisponible.".to_string()))?;
            if slot.is_none() {
                dmx_bridge::install_crypto_provider();
                let companion = dmx_bridge::MobileCompanion::new(dmx_bridge::BridgeHost {
                    pool: self.engine.pool().clone(),
                    runtime: self.engine.handle(),
                    data_dir: self.engine.data_dir().to_path_buf(),
                    assets_dir: assets_dir.map(PathBuf::from),
                    events: Arc::new(ListenerEvents(Arc::clone(&self.listener))),
                });
                companion.bootstrap().map_err(bridge_error)?;
                *slot = Some(companion);
            }
            Ok(())
        }
        #[cfg(not(feature = "bridge"))]
        {
            let _ = assets_dir;
            Err(DmxError::Unsupported {
                message: "Le compagnon mobile PWA n'est pas disponible sur cette plateforme.".to_string(),
            })
        }
    }

    pub fn stop_bridge(&self) {
        #[cfg(feature = "bridge")]
        if let Ok(mut slot) = self.companion.lock() {
            if let Some(companion) = slot.take() {
                companion.shutdown();
            }
        }
    }

    pub fn bridge_status(&self) -> FfiResult<CompanionStatus> {
        #[cfg(feature = "bridge")]
        {
            Ok(self.companion()?.status().map_err(bridge_error)?.into())
        }
        #[cfg(not(feature = "bridge"))]
        {
            Err(DmxError::Unsupported {
                message: "Le compagnon mobile PWA n'est pas disponible sur cette plateforme.".to_string(),
            })
        }
    }

    pub fn set_secure_bridge_enabled(&self, enabled: bool) -> FfiResult<CompanionStatus> {
        #[cfg(feature = "bridge")]
        {
            Ok(self
                .companion()?
                .set_secure_bridge_enabled(enabled)
                .map_err(bridge_error)?
                .into())
        }
        #[cfg(not(feature = "bridge"))]
        {
            let _ = enabled;
            Err(DmxError::Unsupported {
                message: "Le compagnon mobile PWA n'est pas disponible sur cette plateforme.".to_string(),
            })
        }
    }

    pub fn regenerate_pairing_token(&self) -> FfiResult<CompanionStatus> {
        #[cfg(feature = "bridge")]
        {
            Ok(self
                .companion()?
                .regenerate_secure_pairing_token()
                .map_err(bridge_error)?
                .into())
        }
        #[cfg(not(feature = "bridge"))]
        {
            Err(DmxError::Unsupported {
                message: "Le compagnon mobile PWA n'est pas disponible sur cette plateforme.".to_string(),
            })
        }
    }

    pub fn revoke_mobile_passkey(&self, passkey_id: String) -> FfiResult<CompanionStatus> {
        #[cfg(feature = "bridge")]
        {
            Ok(self
                .companion()?
                .revoke_mobile_passkey(passkey_id)
                .map_err(bridge_error)?
                .into())
        }
        #[cfg(not(feature = "bridge"))]
        {
            let _ = passkey_id;
            Err(DmxError::Unsupported {
                message: "Le compagnon mobile PWA n'est pas disponible sur cette plateforme.".to_string(),
            })
        }
    }
}

// --- Fonctions utilitaires ---

#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[uniffi::export]
pub fn today() -> String {
    dates::format_date(dates::today_local())
}

#[uniffi::export]
pub fn default_legacy_database_paths() -> Vec<String> {
    dmx_core::legacy::default_legacy_database_paths()
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

#[uniffi::export]
pub fn account_types() -> Vec<String> {
    dmx_core::models::ACCOUNT_TYPES
        .iter()
        .map(|value| value.to_string())
        .collect()
}

#[uniffi::export]
pub fn account_type_defaults(account_type: String) -> IconColor {
    let (icon, color) = dmx_core::models::account_type_defaults(&account_type);
    IconColor {
        icon: icon.to_string(),
        color: color.to_string(),
    }
}

#[uniffi::export]
pub fn category_colors() -> Vec<String> {
    palette::CATEGORY_COLORS.iter().map(|value| value.to_string()).collect()
}

#[uniffi::export]
pub fn accent_colors() -> Vec<String> {
    palette::ACCENT_COLORS.iter().map(|value| value.to_string()).collect()
}

#[uniffi::export]
pub fn icon_picker_names() -> Vec<String> {
    palette::ICON_PICKER.iter().map(|value| value.to_string()).collect()
}

#[uniffi::export]
pub fn all_periodicities() -> Vec<Periodicity> {
    Periodicity::ALL.to_vec()
}

#[uniffi::export]
pub fn periodicity_label(frequency: Periodicity) -> String {
    frequency.label().to_string()
}

#[uniffi::export]
pub fn periodicity_form_label(frequency: Periodicity) -> String {
    frequency.form_label().to_string()
}

#[uniffi::export]
pub fn all_time_ranges() -> Vec<TimeRange> {
    TimeRange::ALL.to_vec()
}

#[uniffi::export]
pub fn time_range_label(range: TimeRange) -> String {
    range.label().to_string()
}

#[uniffi::export]
pub fn time_range_title(range: TimeRange) -> String {
    range.title_label().to_string()
}

#[uniffi::export]
pub fn all_due_ranges() -> Vec<ScheduledDueRange> {
    ScheduledDueRange::ALL.to_vec()
}

#[uniffi::export]
pub fn due_range_label(range: ScheduledDueRange) -> String {
    range.label().to_string()
}

#[uniffi::export]
pub fn transaction_type_label(kind: TransactionType) -> String {
    kind.label().to_string()
}

#[uniffi::export]
pub fn budget_state_label(state: BudgetState) -> String {
    state.label().to_string()
}

/// Montant en euros au format français, ex. « 1 234,56 € ».
#[uniffi::export]
pub fn format_currency(amount: f64) -> String {
    format::currency_fr(amount)
}

#[uniffi::export]
pub fn format_currency_rounded(amount: f64) -> String {
    format::currency_fr_rounded(amount)
}

#[uniffi::export]
pub fn parse_amount_input(value: String) -> Option<f64> {
    format::parse_decimal_input(&value)
}

/// `dd/MM/yyyy`
#[uniffi::export]
pub fn format_date_numeric(date: String) -> String {
    dates::parse_date(&date).map(format::date_numeric).unwrap_or(date)
}

/// `dd MMM`
#[uniffi::export]
pub fn format_date_short(date: String) -> String {
    dates::parse_date(&date).map(format::date_day_month).unwrap_or(date)
}

/// `dd MMM yyyy`
#[uniffi::export]
pub fn format_date_medium(date: String) -> String {
    dates::parse_date(&date)
        .map(format::date_day_month_year)
        .unwrap_or(date)
}

/// `d MMMM yyyy`
#[uniffi::export]
pub fn format_date_long(date: String) -> String {
    dates::parse_date(&date).map(format::date_long).unwrap_or(date)
}

#[uniffi::export]
pub fn backup_file_name(today: String) -> String {
    dates::parse_date(&today)
        .map(dmx_core::backup::default_backup_file_name)
        .unwrap_or_else(|| "dmxmoney_backup.dmx".to_string())
}

#[uniffi::export]
pub fn detect_csv_separator(content: String) -> String {
    import::detect_csv_separator(&content).to_string()
}

#[uniffi::export]
pub fn statement_format_for_file(file_name: String) -> Option<StatementFormat> {
    StatementFormat::from_file_name(&file_name)
}

#[uniffi::export]
pub fn source_categories(transactions: Vec<ParsedStatementTransaction>) -> Vec<String> {
    import::source_categories(&transactions)
}

#[uniffi::export]
pub fn initial_balance_from_final(transactions: Vec<ParsedStatementTransaction>, final_balance: f64) -> f64 {
    import::initial_balance_from_final(&transactions, final_balance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_round_trip_through_the_facade() {
        let engine = DmxEngine::open_in_memory().unwrap();
        let account = engine
            .save_account(AccountDraft {
                name: "Courant".into(),
                ..engine.account_draft(None).unwrap()
            })
            .unwrap();
        let mut draft = engine.transaction_draft(None, vec![], today()).unwrap();
        draft.amount = 10.0;
        draft.category_id = "5".into();
        engine.save_transaction(draft).unwrap();

        let dashboard = engine.dashboard(vec![account.clone()], today()).unwrap();
        assert_eq!(dashboard.balances.current_balance, -10.0);

        engine
            .apply_settings_change(SettingsChange::SetAccountGroup {
                account_id: account.clone(),
                group: Some("Perso".into()),
            })
            .unwrap();
        assert_eq!(
            engine
                .settings()
                .unwrap()
                .account_groups
                .get(&account)
                .map(String::as_str),
            Some("Perso")
        );

        assert!(matches!(
            engine.dashboard(vec![], "15/09/2026".into()),
            Err(DmxError::Validation { .. })
        ));
        assert_eq!(format_currency(1234.5), "1\u{202f}234,50\u{a0}€");
    }
}
