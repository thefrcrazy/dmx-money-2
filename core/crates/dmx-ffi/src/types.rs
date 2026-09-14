//! Types propres à la FFI : formes non exprimables telles quelles (`BTreeMap`, `char`, variantes
//! tuple) et état du pont, disponible même quand le pont n'est pas compilé (iOS).

use dmx_core::import::{self as core_import, CategoryMatch, ParsedStatementTransaction};
use dmx_core::models::{
    PredictionFakeTransaction, ScheduledDueRange, Theme, TimeRange, TransactionType, WindowPosition, WindowSize,
};
use dmx_core::ops::AccountDropTarget;
use dmx_core::settings::{AppSettings as CoreAppSettings, SettingsChange as CoreSettingsChange};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AppSettings {
    pub settings_revision: i64,
    pub theme: Theme,
    /// `default` ou une couleur hexadécimale.
    pub primary_color: String,
    pub window_position: Option<WindowPosition>,
    pub window_size: Option<WindowSize>,
    pub account_groups: HashMap<String, String>,
    pub custom_groups: Vec<String>,
    pub custom_groups_order: Option<Vec<String>>,
    pub accounts_order: Option<Vec<String>>,
    pub last_seen_version: Option<String>,
    pub dismissed_budget_suggestions: Vec<String>,
    pub dismissed_scheduled_suggestions: Vec<String>,
    pub prediction_time_range: TimeRange,
    pub prediction_custom_end_date: Option<String>,
    pub prediction_alert_threshold: f64,
    pub prediction_month_starts_on_first: bool,
    pub prediction_fake_transactions: Vec<PredictionFakeTransaction>,
    pub analytics_time_range: TimeRange,
    pub analytics_custom_start_date: Option<String>,
    pub analytics_custom_end_date: Option<String>,
    pub analytics_month_starts_on_first: bool,
    pub analytics_hidden_expense_categories: Vec<String>,
    pub analytics_hidden_income_categories: Vec<String>,
    pub scheduled_due_range: ScheduledDueRange,
    /// Couleur d'accent effective (`None` pour la couleur par défaut).
    pub accent_color: Option<String>,
    /// Ordre effectif des groupes de comptes.
    pub effective_group_order: Vec<String>,
}

impl From<CoreAppSettings> for AppSettings {
    fn from(settings: CoreAppSettings) -> Self {
        Self {
            accent_color: settings.accent_color().map(str::to_string),
            effective_group_order: settings.effective_group_order(),
            settings_revision: settings.settings_revision,
            theme: settings.theme,
            primary_color: settings.primary_color,
            window_position: settings.window_position,
            window_size: settings.window_size,
            account_groups: settings.account_groups.into_iter().collect(),
            custom_groups: settings.custom_groups,
            custom_groups_order: settings.custom_groups_order,
            accounts_order: settings.accounts_order,
            last_seen_version: settings.last_seen_version,
            dismissed_budget_suggestions: settings.dismissed_budget_suggestions,
            dismissed_scheduled_suggestions: settings.dismissed_scheduled_suggestions,
            prediction_time_range: settings.prediction_time_range,
            prediction_custom_end_date: settings.prediction_custom_end_date,
            prediction_alert_threshold: settings.prediction_alert_threshold,
            prediction_month_starts_on_first: settings.prediction_month_starts_on_first,
            prediction_fake_transactions: settings.prediction_fake_transactions,
            analytics_time_range: settings.analytics_time_range,
            analytics_custom_start_date: settings.analytics_custom_start_date,
            analytics_custom_end_date: settings.analytics_custom_end_date,
            analytics_month_starts_on_first: settings.analytics_month_starts_on_first,
            analytics_hidden_expense_categories: settings.analytics_hidden_expense_categories,
            analytics_hidden_income_categories: settings.analytics_hidden_income_categories,
            scheduled_due_range: settings.scheduled_due_range,
        }
    }
}

/// Modification d'un paramètre (les suggestions et transactions fictives ont leurs méthodes).
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum SettingsChange {
    SetTheme {
        theme: Theme,
    },
    SetPrimaryColor {
        color: String,
    },
    SetWindowFrame {
        position: Option<WindowPosition>,
        size: Option<WindowSize>,
    },
    SetAccountGroup {
        account_id: String,
        group: Option<String>,
    },
    AddCustomGroup {
        name: String,
    },
    RenameCustomGroup {
        old_name: String,
        new_name: String,
    },
    DeleteCustomGroup {
        name: String,
    },
    SetCustomGroupsOrder {
        order: Vec<String>,
    },
    SetAccountsOrder {
        order: Vec<String>,
    },
    SetLastSeenVersion {
        version: String,
    },
    SetPredictionTimeRange {
        range: TimeRange,
    },
    SetPredictionCustomEndDate {
        date: String,
    },
    SetPredictionAlertThreshold {
        threshold: f64,
    },
    SetPredictionMonthStartsOnFirst {
        enabled: bool,
    },
    SetAnalyticsTimeRange {
        range: TimeRange,
    },
    SetAnalyticsCustomStartDate {
        date: String,
    },
    SetAnalyticsCustomEndDate {
        date: String,
    },
    SetAnalyticsMonthStartsOnFirst {
        enabled: bool,
    },
    SetAnalyticsCategoryHidden {
        kind: TransactionType,
        category_id: String,
        hidden: bool,
    },
    SetScheduledDueRange {
        range: ScheduledDueRange,
    },
}

impl From<SettingsChange> for CoreSettingsChange {
    fn from(change: SettingsChange) -> Self {
        match change {
            SettingsChange::SetTheme { theme } => CoreSettingsChange::Theme(theme),
            SettingsChange::SetPrimaryColor { color } => CoreSettingsChange::PrimaryColor(color),
            SettingsChange::SetWindowFrame { position, size } => CoreSettingsChange::WindowFrame { position, size },
            SettingsChange::SetAccountGroup { account_id, group } => {
                CoreSettingsChange::AccountGroup { account_id, group }
            }
            SettingsChange::AddCustomGroup { name } => CoreSettingsChange::AddCustomGroup(name),
            SettingsChange::RenameCustomGroup { old_name, new_name } => {
                CoreSettingsChange::RenameCustomGroup { old_name, new_name }
            }
            SettingsChange::DeleteCustomGroup { name } => CoreSettingsChange::DeleteCustomGroup(name),
            SettingsChange::SetCustomGroupsOrder { order } => CoreSettingsChange::CustomGroupsOrder(order),
            SettingsChange::SetAccountsOrder { order } => CoreSettingsChange::AccountsOrder(order),
            SettingsChange::SetLastSeenVersion { version } => CoreSettingsChange::LastSeenVersion(version),
            SettingsChange::SetPredictionTimeRange { range } => CoreSettingsChange::PredictionTimeRange(range),
            SettingsChange::SetPredictionCustomEndDate { date } => CoreSettingsChange::PredictionCustomEndDate(date),
            SettingsChange::SetPredictionAlertThreshold { threshold } => {
                CoreSettingsChange::PredictionAlertThreshold(threshold)
            }
            SettingsChange::SetPredictionMonthStartsOnFirst { enabled } => {
                CoreSettingsChange::PredictionMonthStartsOnFirst(enabled)
            }
            SettingsChange::SetAnalyticsTimeRange { range } => CoreSettingsChange::AnalyticsTimeRange(range),
            SettingsChange::SetAnalyticsCustomStartDate { date } => CoreSettingsChange::AnalyticsCustomStartDate(date),
            SettingsChange::SetAnalyticsCustomEndDate { date } => CoreSettingsChange::AnalyticsCustomEndDate(date),
            SettingsChange::SetAnalyticsMonthStartsOnFirst { enabled } => {
                CoreSettingsChange::AnalyticsMonthStartsOnFirst(enabled)
            }
            SettingsChange::SetAnalyticsCategoryHidden {
                kind,
                category_id,
                hidden,
            } => CoreSettingsChange::AnalyticsCategoryHidden {
                kind,
                category_id,
                hidden,
            },
            SettingsChange::SetScheduledDueRange { range } => CoreSettingsChange::ScheduledDueRange(range),
        }
    }
}

/// Cible d'un glisser-déposer dans la page Comptes.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum DropTarget {
    OnAccount {
        account_id: String,
    },
    /// `None` désigne « Non groupés ».
    OnGroup {
        group: Option<String>,
    },
}

impl From<DropTarget> for AccountDropTarget {
    fn from(target: DropTarget) -> Self {
        match target {
            DropTarget::OnAccount { account_id } => AccountDropTarget::Account(account_id),
            DropTarget::OnGroup { group } => AccountDropTarget::Group(group),
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CsvOptions {
    /// Un seul caractère, `;` ou `,` en pratique.
    pub separator: String,
    pub has_header: bool,
}

impl CsvOptions {
    pub fn to_core(&self) -> core_import::CsvOptions {
        core_import::CsvOptions {
            separator: self.separator.chars().next().unwrap_or(';'),
            has_header: self.has_header,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum ImportTarget {
    ExistingAccount {
        account_id: String,
    },
    NewAccount {
        name: String,
        account_type: String,
        final_balance: Option<f64>,
    },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StatementImportRequest {
    pub transactions: Vec<ParsedStatementTransaction>,
    pub target: ImportTarget,
    pub category_mapping: Vec<CategoryMatch>,
}

impl From<StatementImportRequest> for core_import::StatementImportRequest {
    fn from(request: StatementImportRequest) -> Self {
        Self {
            transactions: request.transactions,
            target: match request.target {
                ImportTarget::ExistingAccount { account_id } => core_import::ImportTarget::Existing(account_id),
                ImportTarget::NewAccount {
                    name,
                    account_type,
                    final_balance,
                } => core_import::ImportTarget::New {
                    name,
                    account_type,
                    final_balance,
                },
            },
            category_mapping: request.category_mapping,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct IconColor {
    pub icon: String,
    pub color: String,
}

// --- Pont compagnon ---

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PasskeyInfo {
    pub id: String,
    pub credential_id: String,
    pub device_label: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SecureBridgeInfo {
    pub enabled: bool,
    pub configured: bool,
    pub active: bool,
    pub domain: Option<String>,
    pub app_url: Option<String>,
    pub local_host: Option<String>,
    pub device_id: Option<String>,
    pub api_url: Option<String>,
    pub port: Option<u16>,
    pub pairing_url: Option<String>,
    pub pairing_token_expires_at: Option<String>,
    pub certificate_expires_at: Option<String>,
    pub certificate_ready: bool,
    pub dns_record_id: Option<String>,
    pub dns_last_updated_at: Option<String>,
    pub managed_service_url: String,
    pub managed_credential_ready: bool,
    pub passkeys: Vec<PasskeyInfo>,
    pub last_error: Option<String>,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CompanionStatus {
    pub enabled: bool,
    pub active: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub url: Option<String>,
    pub data_version: i64,
    pub secure_bridge: Option<SecureBridgeInfo>,
}

#[cfg(feature = "bridge")]
impl From<dmx_bridge::MobileCompanionStatus> for CompanionStatus {
    fn from(status: dmx_bridge::MobileCompanionStatus) -> Self {
        Self {
            enabled: status.enabled,
            active: status.active,
            host: status.host,
            port: status.port,
            url: status.url,
            data_version: status.data_version,
            secure_bridge: status.secure_bridge.map(|bridge| SecureBridgeInfo {
                enabled: bridge.enabled,
                configured: bridge.configured,
                active: bridge.active,
                domain: bridge.domain,
                app_url: bridge.app_url,
                local_host: bridge.local_host,
                device_id: bridge.device_id,
                api_url: bridge.api_url,
                port: bridge.port,
                pairing_url: bridge.pairing_url,
                pairing_token_expires_at: bridge.pairing_token_expires_at,
                certificate_expires_at: bridge.certificate_expires_at,
                certificate_ready: bridge.certificate_ready,
                dns_record_id: bridge.dns_record_id,
                dns_last_updated_at: bridge.dns_last_updated_at,
                managed_service_url: bridge.managed_service_url,
                managed_credential_ready: bridge.managed_credential_ready,
                passkeys: bridge
                    .passkeys
                    .into_iter()
                    .map(|passkey| PasskeyInfo {
                        id: passkey.id,
                        credential_id: passkey.credential_id,
                        device_label: passkey.device_label,
                        created_at: passkey.created_at,
                        last_used_at: passkey.last_used_at,
                        revoked_at: passkey.revoked_at,
                    })
                    .collect(),
                last_error: bridge.last_error,
                degraded: bridge.degraded,
            }),
        }
    }
}
