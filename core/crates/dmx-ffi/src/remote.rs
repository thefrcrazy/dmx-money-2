//! Types de `dmx-core` exposés tels quels (définitions miroirs vérifiées à la compilation).

use dmx_core::accounts_view::{
    AccountCard, AccountGroupSection, AccountsQuery, AccountsView, TrayAccount, TraySummary,
};
use dmx_core::analytics::{AnalyticsQuery, AnalyticsView, BalanceHistory, CategorySlice, ChartSeries, PeriodBar};
use dmx_core::backup::{BackupSummary, RestoreMode};
use dmx_core::budget::{
    BudgetCategoryRow, BudgetEnvelope, BudgetOverview, BudgetQuery, BudgetState, BudgetSuggestion, LinkedScheduled,
};
use dmx_core::dashboard::{
    BudgetGauge, CategoryShare, DashboardAccount, DashboardView, RecentTransaction, UpcomingScheduled,
};
use dmx_core::engine::LegacyAdoption;
use dmx_core::import::{
    CategoryMatch, CsvColumnMapping, CsvPreview, ParsedStatementTransaction, StatementFormat, StatementImportResult,
};
use dmx_core::journal::{
    BudgetRemaining, BudgetStatus, CheckStatus, JournalDayGroup, JournalQuery, JournalRow, JournalView,
};
use dmx_core::legacy::DatabaseInventory;
use dmx_core::metrics::{BalanceSummary, MonthlySummary};
use dmx_core::models::{
    Account, Budget, Category, Periodicity, PredictionFakeTransaction, ScheduledDueRange, ScheduledTransaction, Theme,
    TimeRange, Transaction, TransactionType, WindowPosition, WindowSize,
};
use dmx_core::ops::{AccountDraft, BudgetDraft, CategoryDraft, ScheduledDraft, TransactionDraft};
use dmx_core::predictions::{
    BalanceAtDate, FakeTransactionDraft, FakeTransactionRow, IntradayBalance, PredictionMarker, PredictionQuery,
    PredictionSeries, PredictionView, Severity,
};
use dmx_core::scheduled_view::{ScheduledQuery, ScheduledRow, ScheduledSuggestion, ScheduledView};
use dmx_core::scheduling::ProcessDueResult;
use dmx_core::settings::SettingsPatchResult;
use dmx_core::snapshot::CategoryDisplay;
use dmx_core::sync::{ApplyReport, RemoteChange, SyncChange};
use dmx_core::OpenReport;

// --- Énumérations ---

#[uniffi::remote(Enum)]
pub enum TransactionType {
    Income,
    Expense,
    Transfer,
}

#[uniffi::remote(Enum)]
pub enum Periodicity {
    Once,
    Daily,
    Weekly,
    Biweekly,
    Bimonthly,
    Fourweekly,
    Monthly,
    Bimestrial,
    Quarterly,
    Fourmonthly,
    Semiannual,
    Annual,
    Biennial,
}

#[uniffi::remote(Enum)]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[uniffi::remote(Enum)]
pub enum TimeRange {
    Week,
    Month,
    TwoMonths,
    ThreeMonths,
    SixMonths,
    NineMonths,
    Year,
    Custom,
}

#[uniffi::remote(Enum)]
pub enum ScheduledDueRange {
    All,
    Month,
    TwoMonths,
    ThreeMonths,
    SixMonths,
    Year,
}

#[uniffi::remote(Enum)]
pub enum CheckStatus {
    Checked,
    Unchecked,
}

#[uniffi::remote(Enum)]
pub enum BudgetStatus {
    Budgeted,
    Unbudgeted,
}

#[uniffi::remote(Enum)]
pub enum BudgetState {
    ToConfigure,
    UnderControl,
    Overrun,
}

#[uniffi::remote(Enum)]
pub enum Severity {
    Warning,
    Danger,
}

#[uniffi::remote(Enum)]
pub enum RestoreMode {
    Replace,
    Merge,
}

#[uniffi::remote(Enum)]
pub enum StatementFormat {
    Csv,
    Qif,
    Ofx,
}

// --- Modèles ---

#[uniffi::remote(Record)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub initial_balance: f64,
    pub color: String,
    pub icon: String,
}

#[uniffi::remote(Record)]
pub struct Transaction {
    pub id: String,
    pub date: String,
    pub account_id: String,
    pub transaction_type: TransactionType,
    pub amount: f64,
    pub category: String,
    pub description: String,
    pub checked: bool,
    pub is_transfer: bool,
    pub linked_transaction_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
}

#[uniffi::remote(Record)]
pub struct ScheduledTransaction {
    pub id: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub frequency: Periodicity,
    pub account_id: String,
    pub next_date: String,
    pub category: String,
    pub to_account_id: Option<String>,
    pub include_in_forecast: Option<bool>,
    pub budget_id: Option<String>,
    pub end_date: Option<String>,
}

#[uniffi::remote(Record)]
pub struct Budget {
    pub id: String,
    pub name: String,
    pub amount: f64,
    pub category: String,
    pub account_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[uniffi::remote(Record)]
pub struct WindowSize {
    pub width: i32,
    pub height: i32,
}

#[uniffi::remote(Record)]
pub struct PredictionFakeTransaction {
    pub id: String,
    pub date: String,
    pub account_id: String,
    pub transaction_type: TransactionType,
    pub amount: f64,
    pub category: String,
    pub description: String,
    pub enabled: bool,
    pub to_account_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct CategoryDisplay {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
}

// --- Tableau de bord ---

#[uniffi::remote(Record)]
pub struct BalanceSummary {
    pub current_balance: f64,
    pub checked_balance: f64,
}

#[uniffi::remote(Record)]
pub struct MonthlySummary {
    pub income: f64,
    pub expenses: f64,
    pub saved: f64,
}

#[uniffi::remote(Record)]
pub struct UpcomingScheduled {
    pub scheduled_id: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub next_date: String,
    pub days_until: i64,
}

#[uniffi::remote(Record)]
pub struct CategoryShare {
    pub category: CategoryDisplay,
    pub amount: f64,
    pub percentage: f64,
}

#[uniffi::remote(Record)]
pub struct DashboardAccount {
    pub account: Account,
    pub balance: f64,
}

#[uniffi::remote(Record)]
pub struct RecentTransaction {
    pub transaction: Transaction,
    pub category: CategoryDisplay,
}

#[uniffi::remote(Record)]
pub struct BudgetGauge {
    pub total_budgeted: f64,
    pub spent: f64,
    pub remaining: f64,
    pub progress: f64,
}

#[uniffi::remote(Record)]
pub struct DashboardView {
    pub balances: BalanceSummary,
    pub month: MonthlySummary,
    pub upcoming: Vec<UpcomingScheduled>,
    pub top_categories: Vec<CategoryShare>,
    pub accounts: Vec<DashboardAccount>,
    pub accounts_total: f64,
    pub recent_transactions: Vec<RecentTransaction>,
    pub budget: BudgetGauge,
}

// --- Journal ---

#[uniffi::remote(Record)]
pub struct JournalQuery {
    pub accounts: Vec<String>,
    pub search: String,
    pub categories: Vec<String>,
    pub types: Vec<TransactionType>,
    pub statuses: Vec<CheckStatus>,
    pub budget_statuses: Vec<BudgetStatus>,
}

#[uniffi::remote(Record)]
pub struct BudgetRemaining {
    pub budget_id: String,
    pub budget_name: String,
    pub remaining: f64,
}

#[uniffi::remote(Record)]
pub struct JournalRow {
    pub transaction: Transaction,
    pub balance: f64,
    pub account_name: String,
    pub account_color: String,
    pub category: CategoryDisplay,
    pub display_type: TransactionType,
    pub budget: Option<BudgetRemaining>,
}

#[uniffi::remote(Record)]
pub struct JournalDayGroup {
    pub date: String,
    pub label: String,
    pub start_index: u32,
    pub count: u32,
    pub net: f64,
}

#[uniffi::remote(Record)]
pub struct JournalView {
    pub rows: Vec<JournalRow>,
    pub day_groups: Vec<JournalDayGroup>,
    pub active_filter_count: u32,
    pub has_filters: bool,
    pub visible_net: f64,
    pub total_transaction_count: u32,
}

// --- Budget ---

#[uniffi::remote(Record)]
pub struct BudgetQuery {
    pub accounts: Vec<String>,
    pub search: String,
    pub categories: Vec<String>,
}

#[uniffi::remote(Record)]
pub struct LinkedScheduled {
    pub scheduled_id: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub next_date: String,
}

#[uniffi::remote(Record)]
pub struct BudgetEnvelope {
    pub budget: Budget,
    pub account_name: String,
    pub spent: f64,
    pub remaining: f64,
    pub progress: f64,
    pub linked_scheduled: Vec<LinkedScheduled>,
}

#[uniffi::remote(Record)]
pub struct BudgetCategoryRow {
    pub category: CategoryDisplay,
    pub budgeted: f64,
    pub spent: f64,
    pub remaining: f64,
    pub progress: f64,
    pub is_over_budget: bool,
    pub is_unbudgeted: bool,
    pub envelopes: Vec<BudgetEnvelope>,
    pub linked_scheduled_count: u32,
}

#[uniffi::remote(Record)]
pub struct BudgetSuggestion {
    pub key: String,
    pub name: String,
    pub amount: f64,
    pub category: CategoryDisplay,
    pub account_id: Option<String>,
    pub account_name: String,
    pub month_count: u32,
    pub current_month_spent: f64,
}

#[uniffi::remote(Record)]
pub struct BudgetOverview {
    pub month_label: String,
    pub today_label: String,
    pub total_budgeted: f64,
    pub total_spent: f64,
    pub remaining: f64,
    pub progress: f64,
    pub expected_spend: f64,
    pub pace_delta: f64,
    pub remaining_per_day: f64,
    pub budget_count: u32,
    pub expense_count: u32,
    pub state: BudgetState,
    pub over_budget_count: u32,
    pub unbudgeted_count: u32,
    pub categories: Vec<BudgetCategoryRow>,
    pub budgeted_category_count: u32,
    pub suggestions: Vec<BudgetSuggestion>,
}

// --- Échéancier ---

#[uniffi::remote(Record)]
pub struct ScheduledQuery {
    pub accounts: Vec<String>,
    pub due_range: ScheduledDueRange,
    pub search: String,
    pub categories: Vec<String>,
    pub frequencies: Vec<Periodicity>,
}

#[uniffi::remote(Record)]
pub struct ScheduledRow {
    pub scheduled: ScheduledTransaction,
    pub account_name: String,
    pub account_color: String,
    pub to_account_name: Option<String>,
    pub category: CategoryDisplay,
    pub budget_name: Option<String>,
    pub is_ended: bool,
    pub frequency_label: String,
}

#[uniffi::remote(Record)]
pub struct ScheduledSuggestion {
    pub key: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub category: CategoryDisplay,
    pub account_id: String,
    pub account_name: String,
    pub account_color: String,
    pub to_account_id: Option<String>,
    pub frequency: Periodicity,
    pub next_date: String,
    pub occurrence_count: u32,
}

#[uniffi::remote(Record)]
pub struct ScheduledView {
    pub rows: Vec<ScheduledRow>,
    pub suggestions: Vec<ScheduledSuggestion>,
    pub has_filters: bool,
    pub total_count: u32,
}

// --- Analyses ---

#[uniffi::remote(Record)]
pub struct AnalyticsQuery {
    pub accounts: Vec<String>,
    pub range: TimeRange,
    pub custom_start: Option<String>,
    pub custom_end: Option<String>,
    pub month_starts_on_first: bool,
    pub hidden_expense_categories: Vec<String>,
    pub hidden_income_categories: Vec<String>,
}

#[uniffi::remote(Record)]
pub struct ChartSeries {
    pub id: String,
    pub name: String,
    pub color: String,
    pub values: Vec<f64>,
}

#[uniffi::remote(Record)]
pub struct BalanceHistory {
    pub dates: Vec<String>,
    pub labels: Vec<String>,
    pub full_labels: Vec<String>,
    pub series: Vec<ChartSeries>,
}

#[uniffi::remote(Record)]
pub struct CategorySlice {
    pub category: CategoryDisplay,
    pub value: f64,
    pub hidden: bool,
    pub percentage: f64,
}

#[uniffi::remote(Record)]
pub struct PeriodBar {
    pub label: String,
    pub income: f64,
    pub expenses: f64,
}

#[uniffi::remote(Record)]
pub struct AnalyticsView {
    pub start_date: String,
    pub end_date: String,
    pub balance_history: BalanceHistory,
    pub expenses_by_category: Vec<CategorySlice>,
    pub income_by_category: Vec<CategorySlice>,
    pub income_vs_expenses: Vec<PeriodBar>,
    pub daily_buckets: bool,
}

// --- Prédictions ---

#[uniffi::remote(Record)]
pub struct PredictionQuery {
    pub accounts: Vec<String>,
    pub range: TimeRange,
    pub custom_end_date: Option<String>,
    pub month_starts_on_first: bool,
    pub alert_threshold: f64,
}

#[uniffi::remote(Record)]
pub struct PredictionSeries {
    pub id: String,
    pub name: String,
    pub color: String,
    pub closes: Vec<f64>,
    pub lows: Vec<f64>,
}

#[uniffi::remote(Record)]
pub struct BalanceAtDate {
    pub name: String,
    pub color: String,
    pub value: f64,
}

#[uniffi::remote(Record)]
pub struct IntradayBalance {
    pub name: String,
    pub low: f64,
    pub value: f64,
}

#[uniffi::remote(Record)]
pub struct PredictionMarker {
    pub date: String,
    pub index: u32,
    pub full_label: String,
    pub crossing_names: Vec<String>,
    pub severity: Option<Severity>,
    pub intraday_names: Vec<String>,
    pub intraday_severity: Option<Severity>,
    pub intraday_balances: Vec<IntradayBalance>,
    pub balances: Vec<BalanceAtDate>,
    pub stroke_color: String,
}

#[uniffi::remote(Record)]
pub struct FakeTransactionRow {
    pub transaction: PredictionFakeTransaction,
    pub source_account_name: String,
    pub destination_account_name: Option<String>,
    pub category_name: Option<String>,
    pub type_label: String,
}

#[uniffi::remote(Record)]
pub struct PredictionView {
    pub start_date: String,
    pub end_date: String,
    pub end_label: String,
    pub title_label: String,
    pub dates: Vec<String>,
    pub labels: Vec<String>,
    pub full_labels: Vec<String>,
    pub accounts: Vec<PredictionSeries>,
    pub total: PredictionSeries,
    pub current_total_balance: f64,
    pub midpoint_balance: f64,
    pub final_balance: f64,
    pub alert_threshold: f64,
    pub markers: Vec<PredictionMarker>,
    pub intraday_risk_count: u32,
    pub fake_transactions: Vec<FakeTransactionRow>,
    pub enabled_fake_count: u32,
    pub fake_impact: f64,
}

#[uniffi::remote(Record)]
pub struct FakeTransactionDraft {
    pub id: Option<String>,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub kind: TransactionType,
    pub account_id: String,
    pub to_account_id: Option<String>,
    pub category_id: String,
}

// --- Comptes ---

#[uniffi::remote(Record)]
pub struct AccountsQuery {
    pub search: String,
    pub types: Vec<String>,
}

#[uniffi::remote(Record)]
pub struct AccountCard {
    pub account: Account,
    pub current_balance: f64,
    pub checked_balance: f64,
    pub group: Option<String>,
}

#[uniffi::remote(Record)]
pub struct AccountGroupSection {
    pub name: String,
    pub is_ungrouped: bool,
    pub accounts: Vec<AccountCard>,
}

#[uniffi::remote(Record)]
pub struct AccountsView {
    pub groups: Vec<AccountGroupSection>,
    pub visible_count: u32,
    pub total_count: u32,
}

#[uniffi::remote(Record)]
pub struct TrayAccount {
    pub account_id: String,
    pub name: String,
    pub balance: f64,
}

#[uniffi::remote(Record)]
pub struct TraySummary {
    pub accounts: Vec<TrayAccount>,
    pub total: f64,
}

// --- Formulaires ---

#[uniffi::remote(Record)]
pub struct AccountDraft {
    pub id: Option<String>,
    pub name: String,
    pub account_type: String,
    pub initial_balance: f64,
    pub color: String,
    pub icon: String,
    pub group: Option<String>,
}

#[uniffi::remote(Record)]
pub struct CategoryDraft {
    pub id: Option<String>,
    pub name: String,
    pub icon: String,
    pub color: String,
}

#[uniffi::remote(Record)]
pub struct TransactionDraft {
    pub id: Option<String>,
    pub kind: TransactionType,
    pub date: String,
    pub amount: f64,
    pub description: String,
    pub category_id: String,
    pub account_id: String,
    pub to_account_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct BudgetDraft {
    pub id: Option<String>,
    pub name: String,
    pub amount: f64,
    pub category_id: String,
    pub account_id: Option<String>,
}

#[uniffi::remote(Record)]
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

// --- Résultats d'opérations ---

#[uniffi::remote(Record)]
pub struct SettingsPatchResult {
    pub ok: bool,
    pub revision: i64,
    pub conflicts: Vec<String>,
}

#[uniffi::remote(Record)]
pub struct ProcessDueResult {
    pub created_transactions: u32,
    pub updated_scheduled: u32,
    pub deleted_scheduled: u32,
}

#[uniffi::remote(Record)]
pub struct BackupSummary {
    pub version: u32,
    pub timestamp: String,
    pub accounts: u32,
    pub transactions: u32,
    pub categories: u32,
    pub scheduled: u32,
    pub budgets: u32,
}

#[uniffi::remote(Record)]
pub struct OpenReport {
    pub database_path: String,
    pub created: bool,
    pub imported_legacy_database: Option<String>,
    pub legacy_import_error: Option<String>,
    pub legacy_candidate: Option<DatabaseInventory>,
}

#[uniffi::remote(Record)]
pub struct DatabaseInventory {
    pub path: String,
    pub accounts: u32,
    pub transactions: u32,
    pub categories: u32,
    pub budgets: u32,
    pub scheduled: u32,
    pub last_transaction_date: Option<String>,
}

#[uniffi::remote(Record)]
pub struct LegacyAdoption {
    pub inventory: DatabaseInventory,
    pub summary: BackupSummary,
    pub backup_file: String,
}

// --- Imports ---

#[uniffi::remote(Record)]
pub struct ParsedStatementTransaction {
    pub date: String,
    pub amount: f64,
    pub description: String,
    pub category: Option<String>,
}

#[uniffi::remote(Record)]
pub struct CsvColumnMapping {
    pub date: Option<u32>,
    pub amount: Option<u32>,
    pub description: Option<u32>,
    pub category: Option<u32>,
}

#[uniffi::remote(Record)]
pub struct CsvPreview {
    pub rows: Vec<Vec<String>>,
    pub column_count: u32,
    pub row_count: u32,
}

#[uniffi::remote(Record)]
pub struct CategoryMatch {
    pub source: String,
    pub category_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct StatementImportResult {
    pub imported: u32,
    pub duplicates: u32,
    pub account_id: String,
    pub created_categories: u32,
}

// --- Synchronisation ---

#[uniffi::remote(Record)]
pub struct SyncChange {
    pub seq: i64,
    pub entity: String,
    pub record_id: String,
    pub deleted: bool,
    pub updated_at: String,
    pub payload: Option<String>,
}

#[uniffi::remote(Record)]
pub struct RemoteChange {
    pub entity: String,
    pub record_id: String,
    pub deleted: bool,
    pub updated_at: String,
    pub payload: Option<String>,
}

#[uniffi::remote(Record)]
pub struct ApplyReport {
    pub applied: u32,
    pub skipped: u32,
}
