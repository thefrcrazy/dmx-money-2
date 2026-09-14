//! Paramètres synchronisés.
//!
//! - [`SettingsRecord`] : forme stockée et échangée par l'API compagnon 1.x (tableaux en JSON).
//! - [`apply_settings_patch`] : port de `src-tauri/src/settings_sync.rs` (versions par champ,
//!   conflits, fusions additives) utilisé par le pont PWA.
//! - [`AppSettings`] et [`SettingsChange`] : API typée des interfaces natives.

use crate::db::DbPool;
use crate::error::{CoreError, CoreResult, DbContext};
use crate::models::{
    PredictionFakeTransaction, ScheduledDueRange, Theme, TimeRange, TransactionType, WindowPosition, WindowSize,
};
use crate::repo::{row_bool, row_f64, row_i64, row_opt_string, row_string};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row, SqliteConnection};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// Paramètres dans leur forme 1.x.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsRecord {
    #[serde(rename = "settingsRevision")]
    pub settings_revision: Option<i64>,
    pub theme: String,
    #[serde(rename = "primaryColor")]
    pub primary_color: String,
    #[serde(rename = "displayStyle")]
    pub display_style: Option<String>,
    #[serde(rename = "windowPosition")]
    pub window_position: Option<WindowPosition>,
    #[serde(rename = "windowSize")]
    pub window_size: Option<WindowSize>,
    #[serde(rename = "accountGroups")]
    pub account_groups: Option<String>,
    #[serde(rename = "customGroups")]
    pub custom_groups: Option<String>,
    #[serde(rename = "customGroupsOrder")]
    pub custom_groups_order: Option<String>,
    #[serde(rename = "accountsOrder")]
    pub accounts_order: Option<String>,
    #[serde(rename = "lastSeenVersion")]
    pub last_seen_version: Option<String>,
    #[serde(rename = "componentSpacing")]
    pub component_spacing: i32,
    #[serde(rename = "componentPadding")]
    pub component_padding: i32,
    #[serde(rename = "dismissedBudgetSuggestions")]
    pub dismissed_budget_suggestions: Option<String>,
    #[serde(rename = "dismissedScheduledSuggestions")]
    pub dismissed_scheduled_suggestions: Option<String>,
    #[serde(rename = "predictionTimeRange")]
    pub prediction_time_range: Option<String>,
    #[serde(rename = "predictionCustomEndDate")]
    pub prediction_custom_end_date: Option<String>,
    #[serde(rename = "predictionAlertThreshold")]
    pub prediction_alert_threshold: Option<f64>,
    #[serde(rename = "predictionMonthStartsOnFirst")]
    pub prediction_month_starts_on_first: Option<bool>,
    #[serde(rename = "predictionFakeTransactions")]
    pub prediction_fake_transactions: Option<String>,
    #[serde(rename = "analyticsTimeRange")]
    pub analytics_time_range: Option<String>,
    #[serde(rename = "analyticsCustomStartDate")]
    pub analytics_custom_start_date: Option<String>,
    #[serde(rename = "analyticsCustomEndDate")]
    pub analytics_custom_end_date: Option<String>,
    #[serde(rename = "analyticsMonthStartsOnFirst")]
    pub analytics_month_starts_on_first: Option<bool>,
    #[serde(rename = "analyticsHiddenExpenseCategories")]
    pub analytics_hidden_expense_categories: Option<String>,
    #[serde(rename = "analyticsHiddenIncomeCategories")]
    pub analytics_hidden_income_categories: Option<String>,
    #[serde(rename = "scheduledDueRange")]
    pub scheduled_due_range: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub base_revision: i64,
    #[serde(default)]
    pub values: SettingsValuesPatch,
    #[serde(default)]
    pub expected_values: BTreeMap<String, Value>,
    #[serde(default)]
    pub dismissed_budget_suggestions_add: Vec<String>,
    #[serde(default)]
    pub dismissed_scheduled_suggestions_add: Vec<String>,
    #[serde(default)]
    pub prediction_fake_transactions_upsert: Vec<Value>,
    #[serde(default)]
    pub prediction_fake_transaction_delete_ids: Vec<String>,
    #[serde(default)]
    pub prediction_fake_transactions_expected: BTreeMap<String, Value>,
    #[serde(default)]
    pub analytics_hidden_expense_categories_add: Vec<String>,
    #[serde(default)]
    pub analytics_hidden_expense_categories_remove: Vec<String>,
    #[serde(default)]
    pub analytics_hidden_expense_categories_expected: BTreeMap<String, bool>,
    #[serde(default)]
    pub analytics_hidden_income_categories_add: Vec<String>,
    #[serde(default)]
    pub analytics_hidden_income_categories_remove: Vec<String>,
    #[serde(default)]
    pub analytics_hidden_income_categories_expected: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatchResult {
    pub ok: bool,
    pub revision: i64,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsValuesPatch {
    pub theme: Option<String>,
    pub primary_color: Option<String>,
    pub display_style: Option<String>,
    pub window_position: Option<WindowPosition>,
    pub window_size: Option<WindowSize>,
    pub account_groups: Option<String>,
    pub custom_groups: Option<String>,
    pub custom_groups_order: Option<String>,
    pub accounts_order: Option<String>,
    pub last_seen_version: Option<String>,
    pub component_spacing: Option<i32>,
    pub component_padding: Option<i32>,
    pub prediction_time_range: Option<String>,
    pub prediction_custom_end_date: Option<String>,
    pub prediction_alert_threshold: Option<f64>,
    pub prediction_month_starts_on_first: Option<bool>,
    pub analytics_time_range: Option<String>,
    pub analytics_custom_start_date: Option<String>,
    pub analytics_custom_end_date: Option<String>,
    pub analytics_month_starts_on_first: Option<bool>,
    pub scheduled_due_range: Option<String>,
}

// --- Lecture ---

fn record_from_row(row: &SqliteRow) -> SettingsRecord {
    let window_position = match (row_i64(row, "windowPositionX"), row_i64(row, "windowPositionY")) {
        (Some(x), Some(y)) => Some(WindowPosition {
            x: x as i32,
            y: y as i32,
        }),
        _ => None,
    };
    let window_size = match (row_i64(row, "windowSizeWidth"), row_i64(row, "windowSizeHeight")) {
        (Some(width), Some(height)) => Some(WindowSize {
            width: width as i32,
            height: height as i32,
        }),
        _ => None,
    };

    SettingsRecord {
        settings_revision: Some(row_i64(row, "settingsRevision").unwrap_or_default()),
        theme: row_string(row, "theme"),
        primary_color: row_string(row, "primaryColor"),
        display_style: Some(row_string(row, "displayStyle")),
        window_position,
        window_size,
        account_groups: row_opt_string(row, "accountGroups"),
        custom_groups: row_opt_string(row, "customGroups"),
        custom_groups_order: row_opt_string(row, "customGroupsOrder"),
        accounts_order: row_opt_string(row, "accountsOrder"),
        last_seen_version: row_opt_string(row, "lastSeenVersion"),
        component_spacing: row_i64(row, "componentSpacing").unwrap_or(6) as i32,
        component_padding: row_i64(row, "componentPadding").unwrap_or(6) as i32,
        dismissed_budget_suggestions: row_opt_string(row, "dismissedBudgetSuggestions"),
        dismissed_scheduled_suggestions: row_opt_string(row, "dismissedScheduledSuggestions"),
        prediction_time_range: row_opt_string(row, "predictionTimeRange"),
        prediction_custom_end_date: row_opt_string(row, "predictionCustomEndDate"),
        prediction_alert_threshold: Some(row_f64(row, "predictionAlertThreshold")),
        prediction_month_starts_on_first: Some(row_bool(row, "predictionMonthStartsOnFirst")),
        prediction_fake_transactions: row_opt_string(row, "predictionFakeTransactions"),
        analytics_time_range: row_opt_string(row, "analyticsTimeRange"),
        analytics_custom_start_date: row_opt_string(row, "analyticsCustomStartDate"),
        analytics_custom_end_date: row_opt_string(row, "analyticsCustomEndDate"),
        analytics_month_starts_on_first: Some(row_bool(row, "analyticsMonthStartsOnFirst")),
        analytics_hidden_expense_categories: row_opt_string(row, "analyticsHiddenExpenseCategories"),
        analytics_hidden_income_categories: row_opt_string(row, "analyticsHiddenIncomeCategories"),
        scheduled_due_range: row_opt_string(row, "scheduledDueRange"),
    }
}

pub async fn get_settings_record(pool: &DbPool) -> CoreResult<Option<SettingsRecord>> {
    let row = sqlx::query("SELECT * FROM settings WHERE id = 1")
        .fetch_optional(pool)
        .await
        .ctx("récupération des paramètres")?;
    Ok(row.as_ref().map(record_from_row))
}

// --- Versions vues (modale Nouveautés) ---

fn parse_version(value: &str) -> Option<Vec<u64>> {
    let normalized = value.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    if normalized.is_empty() {
        return None;
    }
    normalized
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

pub fn compare_versions(left: &str, right: &str) -> Ordering {
    match (parse_version(left), parse_version(right)) {
        (Some(left), Some(right)) => {
            let length = left.len().max(right.len());
            for index in 0..length {
                match left
                    .get(index)
                    .copied()
                    .unwrap_or_default()
                    .cmp(&right.get(index).copied().unwrap_or_default())
                {
                    Ordering::Equal => continue,
                    ordering => return ordering,
                }
            }
            Ordering::Equal
        }
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

/// La version déjà consultée ne régresse jamais (un appareil en retard ne la réinitialise pas).
pub fn newest_seen_version(current: Option<String>, incoming: Option<String>) -> Option<String> {
    match (current, incoming) {
        (Some(current), Some(incoming)) => {
            if compare_versions(&incoming, &current).is_gt() {
                Some(incoming)
            } else {
                Some(current)
            }
        }
        (current @ Some(_), None) => current,
        (None, incoming) => incoming,
    }
}

// --- Patchs compatibles 1.x ---

pub async fn apply_settings_patch(pool: &DbPool, patch: SettingsPatch) -> CoreResult<SettingsPatchResult> {
    if patch.schema_version > 2 {
        return Err(CoreError::validation(
            "Version de synchronisation des paramètres non supportée.",
        ));
    }

    let mut connection = pool.acquire().await.ctx("mise à jour des paramètres")?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await
        .ctx("mise à jour des paramètres")?;

    match apply_settings_patch_locked(&mut connection, patch).await {
        Ok(result) => {
            sqlx::query("COMMIT")
                .execute(&mut *connection)
                .await
                .ctx("mise à jour des paramètres")?;
            Ok(result)
        }
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
            Err(error)
        }
    }
}

/// Patch envoyé par les anciennes PWA (schéma 1) : seules les fusions additives sont conservées.
pub fn legacy_mobile_settings_patch(settings: SettingsRecord) -> SettingsPatch {
    SettingsPatch {
        schema_version: 1,
        base_revision: 0,
        values: SettingsValuesPatch {
            last_seen_version: settings.last_seen_version,
            ..SettingsValuesPatch::default()
        },
        dismissed_budget_suggestions_add: parse_string_array(settings.dismissed_budget_suggestions.as_deref()),
        dismissed_scheduled_suggestions_add: parse_string_array(settings.dismissed_scheduled_suggestions.as_deref()),
        analytics_hidden_expense_categories_add: parse_string_array(
            settings.analytics_hidden_expense_categories.as_deref(),
        ),
        analytics_hidden_income_categories_add: parse_string_array(
            settings.analytics_hidden_income_categories.as_deref(),
        ),
        ..SettingsPatch::default()
    }
}

/// Sauvegarde complète (ancien `save_settings` du desktop) exprimée en patch.
pub fn legacy_desktop_settings_patch(settings: SettingsRecord) -> SettingsPatch {
    SettingsPatch {
        schema_version: 2,
        base_revision: settings.settings_revision.unwrap_or_default(),
        values: SettingsValuesPatch {
            theme: Some(settings.theme),
            primary_color: Some(settings.primary_color),
            display_style: settings.display_style,
            window_position: settings.window_position,
            window_size: settings.window_size,
            account_groups: settings.account_groups,
            custom_groups: settings.custom_groups,
            custom_groups_order: settings.custom_groups_order,
            accounts_order: settings.accounts_order,
            last_seen_version: settings.last_seen_version,
            component_spacing: Some(settings.component_spacing),
            component_padding: Some(settings.component_padding),
            prediction_time_range: settings.prediction_time_range,
            prediction_custom_end_date: settings.prediction_custom_end_date,
            prediction_alert_threshold: settings.prediction_alert_threshold,
            prediction_month_starts_on_first: settings.prediction_month_starts_on_first,
            analytics_time_range: settings.analytics_time_range,
            analytics_custom_start_date: settings.analytics_custom_start_date,
            analytics_custom_end_date: settings.analytics_custom_end_date,
            analytics_month_starts_on_first: settings.analytics_month_starts_on_first,
            scheduled_due_range: settings.scheduled_due_range,
        },
        dismissed_budget_suggestions_add: parse_string_array(settings.dismissed_budget_suggestions.as_deref()),
        dismissed_scheduled_suggestions_add: parse_string_array(settings.dismissed_scheduled_suggestions.as_deref()),
        prediction_fake_transactions_upsert: parse_value_array(settings.prediction_fake_transactions.as_deref()),
        analytics_hidden_expense_categories_add: parse_string_array(
            settings.analytics_hidden_expense_categories.as_deref(),
        ),
        analytics_hidden_income_categories_add: parse_string_array(
            settings.analytics_hidden_income_categories.as_deref(),
        ),
        ..SettingsPatch::default()
    }
}

async fn apply_settings_patch_locked(
    connection: &mut SqliteConnection,
    mut patch: SettingsPatch,
) -> CoreResult<SettingsPatchResult> {
    sqlx::query(
        "INSERT OR IGNORE INTO settings (
            id, theme, \"primaryColor\", \"displayStyle\", \"componentSpacing\", \"componentPadding\"
        ) VALUES (1, 'system', '#6366f1', 'modern', 6, 6)",
    )
    .execute(&mut *connection)
    .await
    .ctx("mise à jour des paramètres")?;

    let row = sqlx::query("SELECT * FROM settings WHERE id = 1")
        .fetch_one(&mut *connection)
        .await
        .ctx("mise à jour des paramètres")?;

    let current_revision = row_i64(&row, "settingsRevision").unwrap_or_default();
    let mut field_versions = row_opt_string(&row, "settingsFieldVersions")
        .and_then(|value| serde_json::from_str::<BTreeMap<String, i64>>(&value).ok())
        .unwrap_or_default();
    let mut conflicts = Vec::new();
    let mut applied_fields = Vec::new();

    macro_rules! protect_scalar {
        ($field:ident, $name:literal, $current:expr) => {
            if patch.values.$field.is_some() {
                let changed_since_base = field_versions.get($name).copied().unwrap_or_default() > patch.base_revision;
                let expected_matches = patch
                    .expected_values
                    .get($name)
                    .map(|expected| json_values_equal(expected, &$current))
                    .unwrap_or(false);
                if changed_since_base && !expected_matches {
                    patch.values.$field = None;
                    conflicts.push($name.to_string());
                } else {
                    applied_fields.push($name.to_string());
                }
            }
        };
    }

    protect_scalar!(theme, "theme", row_string_value(&row, "theme"));
    protect_scalar!(primary_color, "primaryColor", row_string_value(&row, "primaryColor"));
    protect_scalar!(display_style, "displayStyle", row_string_value(&row, "displayStyle"));
    protect_scalar!(window_position, "windowPosition", row_window_position_value(&row));
    protect_scalar!(window_size, "windowSize", row_window_size_value(&row));
    protect_scalar!(account_groups, "accountGroups", row_string_value(&row, "accountGroups"));
    protect_scalar!(custom_groups, "customGroups", row_string_value(&row, "customGroups"));
    protect_scalar!(
        custom_groups_order,
        "customGroupsOrder",
        row_string_value(&row, "customGroupsOrder")
    );
    protect_scalar!(accounts_order, "accountsOrder", row_string_value(&row, "accountsOrder"));
    protect_scalar!(
        component_spacing,
        "componentSpacing",
        row_integer_value(&row, "componentSpacing")
    );
    protect_scalar!(
        component_padding,
        "componentPadding",
        row_integer_value(&row, "componentPadding")
    );
    protect_scalar!(
        prediction_time_range,
        "predictionTimeRange",
        row_string_value(&row, "predictionTimeRange")
    );
    protect_scalar!(
        prediction_custom_end_date,
        "predictionCustomEndDate",
        row_string_value(&row, "predictionCustomEndDate")
    );
    protect_scalar!(
        prediction_alert_threshold,
        "predictionAlertThreshold",
        row_float_value(&row, "predictionAlertThreshold")
    );
    protect_scalar!(
        prediction_month_starts_on_first,
        "predictionMonthStartsOnFirst",
        row_boolean_value(&row, "predictionMonthStartsOnFirst")
    );
    protect_scalar!(
        analytics_time_range,
        "analyticsTimeRange",
        row_string_value(&row, "analyticsTimeRange")
    );
    protect_scalar!(
        analytics_custom_start_date,
        "analyticsCustomStartDate",
        row_string_value(&row, "analyticsCustomStartDate")
    );
    protect_scalar!(
        analytics_custom_end_date,
        "analyticsCustomEndDate",
        row_string_value(&row, "analyticsCustomEndDate")
    );
    protect_scalar!(
        analytics_month_starts_on_first,
        "analyticsMonthStartsOnFirst",
        row_boolean_value(&row, "analyticsMonthStartsOnFirst")
    );
    protect_scalar!(
        scheduled_due_range,
        "scheduledDueRange",
        row_string_value(&row, "scheduledDueRange")
    );

    let current_fake_transactions = parse_value_map(row_opt_string(&row, "predictionFakeTransactions").as_deref());
    let expected_fake_transactions = patch.prediction_fake_transactions_expected.clone();
    let base_revision = patch.base_revision;
    let fake_transaction_is_stale = |id: &str, field_versions: &BTreeMap<String, i64>| {
        let key = format!("predictionFakeTransactions:{id}");
        let changed_since_base = field_versions.get(&key).copied().unwrap_or_default() > base_revision;
        let expected_matches = expected_fake_transactions
            .get(id)
            .map(|expected| {
                let current = current_fake_transactions.get(id).cloned().unwrap_or(Value::Null);
                json_values_equal(expected, &current)
            })
            .unwrap_or(false);
        changed_since_base && !expected_matches
    };

    patch.prediction_fake_transactions_upsert = std::mem::take(&mut patch.prediction_fake_transactions_upsert)
        .into_iter()
        .filter(|value| {
            let Some(id) = value_id(value) else {
                return false;
            };
            if fake_transaction_is_stale(&id, &field_versions) {
                conflicts.push(format!("predictionFakeTransactions:{id}"));
                false
            } else {
                true
            }
        })
        .collect();
    patch.prediction_fake_transaction_delete_ids = std::mem::take(&mut patch.prediction_fake_transaction_delete_ids)
        .into_iter()
        .filter(|id| {
            if fake_transaction_is_stale(id, &field_versions) {
                conflicts.push(format!("predictionFakeTransactions:{id}"));
                false
            } else {
                true
            }
        })
        .collect();

    macro_rules! protect_set_operations {
        ($add:ident, $remove:ident, $expected:ident, $prefix:literal, $column:literal) => {{
            let current_values = parse_string_array(row_opt_string(&row, $column).as_deref())
                .into_iter()
                .collect::<BTreeSet<_>>();
            let expected_values = patch.$expected.clone();
            let is_stale = |value: &String, field_versions: &BTreeMap<String, i64>| {
                let key = format!("{}:{value}", $prefix);
                let changed_since_base = field_versions.get(&key).copied().unwrap_or_default() > base_revision;
                let expected_matches = expected_values
                    .get(value)
                    .map(|expected| *expected == current_values.contains(value))
                    .unwrap_or(false);
                changed_since_base && !expected_matches
            };
            patch.$add = std::mem::take(&mut patch.$add)
                .into_iter()
                .filter(|value| {
                    if is_stale(value, &field_versions) {
                        conflicts.push(format!("{}:{value}", $prefix));
                        false
                    } else {
                        true
                    }
                })
                .collect();
            patch.$remove = std::mem::take(&mut patch.$remove)
                .into_iter()
                .filter(|value| {
                    if is_stale(value, &field_versions) {
                        conflicts.push(format!("{}:{value}", $prefix));
                        false
                    } else {
                        true
                    }
                })
                .collect();
        }};
    }
    protect_set_operations!(
        analytics_hidden_expense_categories_add,
        analytics_hidden_expense_categories_remove,
        analytics_hidden_expense_categories_expected,
        "analyticsHiddenExpenseCategories",
        "analyticsHiddenExpenseCategories"
    );
    protect_set_operations!(
        analytics_hidden_income_categories_add,
        analytics_hidden_income_categories_remove,
        analytics_hidden_income_categories_expected,
        "analyticsHiddenIncomeCategories",
        "analyticsHiddenIncomeCategories"
    );

    if patch.values.last_seen_version.is_some() {
        applied_fields.push("lastSeenVersion".to_string());
    }
    if !patch.dismissed_budget_suggestions_add.is_empty() {
        applied_fields.push("dismissedBudgetSuggestions".to_string());
    }
    if !patch.dismissed_scheduled_suggestions_add.is_empty() {
        applied_fields.push("dismissedScheduledSuggestions".to_string());
    }
    if !patch.prediction_fake_transactions_upsert.is_empty() || !patch.prediction_fake_transaction_delete_ids.is_empty()
    {
        applied_fields.push("predictionFakeTransactions".to_string());
        patch
            .prediction_fake_transactions_upsert
            .iter()
            .filter_map(value_id)
            .for_each(|id| applied_fields.push(format!("predictionFakeTransactions:{id}")));
        patch
            .prediction_fake_transaction_delete_ids
            .iter()
            .for_each(|id| applied_fields.push(format!("predictionFakeTransactions:{id}")));
    }
    for (add, remove, prefix) in [
        (
            &patch.analytics_hidden_expense_categories_add,
            &patch.analytics_hidden_expense_categories_remove,
            "analyticsHiddenExpenseCategories",
        ),
        (
            &patch.analytics_hidden_income_categories_add,
            &patch.analytics_hidden_income_categories_remove,
            "analyticsHiddenIncomeCategories",
        ),
    ] {
        if !add.is_empty() || !remove.is_empty() {
            applied_fields.push(prefix.to_string());
            add.iter()
                .chain(remove.iter())
                .for_each(|value| applied_fields.push(format!("{prefix}:{value}")));
        }
    }

    if applied_fields.is_empty() {
        return Ok(SettingsPatchResult {
            ok: true,
            revision: current_revision,
            conflicts,
        });
    }

    let next_revision = current_revision + 1;
    for field in &applied_fields {
        field_versions.insert(field.clone(), next_revision);
    }
    let serialized_field_versions =
        serde_json::to_string(&field_versions).map_err(|error| CoreError::Database(error.to_string()))?;

    let last_seen_version = if patch.values.last_seen_version.is_some() {
        newest_seen_version(
            row_opt_string(&row, "lastSeenVersion"),
            patch.values.last_seen_version.clone(),
        )
    } else {
        None
    };

    let dismissed_budget_update = (!patch.dismissed_budget_suggestions_add.is_empty()).then(|| {
        merge_string_array(
            row_opt_string(&row, "dismissedBudgetSuggestions").as_deref(),
            &patch.dismissed_budget_suggestions_add,
            &[],
        )
    });
    let dismissed_scheduled_update = (!patch.dismissed_scheduled_suggestions_add.is_empty()).then(|| {
        merge_string_array(
            row_opt_string(&row, "dismissedScheduledSuggestions").as_deref(),
            &patch.dismissed_scheduled_suggestions_add,
            &[],
        )
    });
    let fake_transactions_update = (!patch.prediction_fake_transactions_upsert.is_empty()
        || !patch.prediction_fake_transaction_delete_ids.is_empty())
    .then(|| {
        merge_values_by_id(
            row_opt_string(&row, "predictionFakeTransactions").as_deref(),
            &patch.prediction_fake_transactions_upsert,
            &patch.prediction_fake_transaction_delete_ids,
        )
    });
    let hidden_expense_update = (!patch.analytics_hidden_expense_categories_add.is_empty()
        || !patch.analytics_hidden_expense_categories_remove.is_empty())
    .then(|| {
        merge_string_array(
            row_opt_string(&row, "analyticsHiddenExpenseCategories").as_deref(),
            &patch.analytics_hidden_expense_categories_add,
            &patch.analytics_hidden_expense_categories_remove,
        )
    });
    let hidden_income_update = (!patch.analytics_hidden_income_categories_add.is_empty()
        || !patch.analytics_hidden_income_categories_remove.is_empty())
    .then(|| {
        merge_string_array(
            row_opt_string(&row, "analyticsHiddenIncomeCategories").as_deref(),
            &patch.analytics_hidden_income_categories_add,
            &patch.analytics_hidden_income_categories_remove,
        )
    });

    let values = patch.values;
    sqlx::query(
        "UPDATE settings SET
            theme = COALESCE($1, theme),
            \"primaryColor\" = COALESCE($2, \"primaryColor\"),
            \"displayStyle\" = COALESCE($3, \"displayStyle\"),
            \"windowPositionX\" = COALESCE($4, \"windowPositionX\"),
            \"windowPositionY\" = COALESCE($5, \"windowPositionY\"),
            \"windowSizeWidth\" = COALESCE($6, \"windowSizeWidth\"),
            \"windowSizeHeight\" = COALESCE($7, \"windowSizeHeight\"),
            \"accountGroups\" = COALESCE($8, \"accountGroups\"),
            \"customGroups\" = COALESCE($9, \"customGroups\"),
            \"customGroupsOrder\" = COALESCE($10, \"customGroupsOrder\"),
            \"accountsOrder\" = COALESCE($11, \"accountsOrder\"),
            \"lastSeenVersion\" = COALESCE($12, \"lastSeenVersion\"),
            \"componentSpacing\" = COALESCE($13, \"componentSpacing\"),
            \"componentPadding\" = COALESCE($14, \"componentPadding\"),
            \"dismissedBudgetSuggestions\" = COALESCE($15, \"dismissedBudgetSuggestions\"),
            \"dismissedScheduledSuggestions\" = COALESCE($16, \"dismissedScheduledSuggestions\"),
            \"predictionTimeRange\" = COALESCE($17, \"predictionTimeRange\"),
            \"predictionCustomEndDate\" = COALESCE($18, \"predictionCustomEndDate\"),
            \"predictionAlertThreshold\" = COALESCE($19, \"predictionAlertThreshold\"),
            \"predictionMonthStartsOnFirst\" = COALESCE($20, \"predictionMonthStartsOnFirst\"),
            \"predictionFakeTransactions\" = COALESCE($21, \"predictionFakeTransactions\"),
            \"analyticsTimeRange\" = COALESCE($22, \"analyticsTimeRange\"),
            \"analyticsCustomStartDate\" = COALESCE($23, \"analyticsCustomStartDate\"),
            \"analyticsCustomEndDate\" = COALESCE($24, \"analyticsCustomEndDate\"),
            \"analyticsMonthStartsOnFirst\" = COALESCE($25, \"analyticsMonthStartsOnFirst\"),
            \"analyticsHiddenExpenseCategories\" = COALESCE($26, \"analyticsHiddenExpenseCategories\"),
            \"analyticsHiddenIncomeCategories\" = COALESCE($27, \"analyticsHiddenIncomeCategories\"),
            \"scheduledDueRange\" = COALESCE($28, \"scheduledDueRange\"),
            \"settingsRevision\" = $29,
            \"settingsFieldVersions\" = $30
         WHERE id = 1",
    )
    .bind(values.theme)
    .bind(values.primary_color)
    .bind(values.display_style)
    .bind(values.window_position.map(|position| position.x))
    .bind(values.window_position.map(|position| position.y))
    .bind(values.window_size.map(|size| size.width))
    .bind(values.window_size.map(|size| size.height))
    .bind(values.account_groups)
    .bind(values.custom_groups)
    .bind(values.custom_groups_order)
    .bind(values.accounts_order)
    .bind(last_seen_version)
    .bind(values.component_spacing)
    .bind(values.component_padding)
    .bind(dismissed_budget_update)
    .bind(dismissed_scheduled_update)
    .bind(values.prediction_time_range)
    .bind(values.prediction_custom_end_date)
    .bind(values.prediction_alert_threshold)
    .bind(values.prediction_month_starts_on_first)
    .bind(fake_transactions_update)
    .bind(values.analytics_time_range)
    .bind(values.analytics_custom_start_date)
    .bind(values.analytics_custom_end_date)
    .bind(values.analytics_month_starts_on_first)
    .bind(hidden_expense_update)
    .bind(hidden_income_update)
    .bind(values.scheduled_due_range)
    .bind(next_revision)
    .bind(serialized_field_versions)
    .execute(&mut *connection)
    .await
    .ctx("mise à jour des paramètres")?;

    Ok(SettingsPatchResult {
        ok: true,
        revision: next_revision,
        conflicts,
    })
}

fn row_string_value(row: &SqliteRow, column: &str) -> Value {
    row_opt_string(row, column).map(Value::String).unwrap_or(Value::Null)
}

fn row_integer_value(row: &SqliteRow, column: &str) -> Value {
    row_i64(row, column).map(Value::from).unwrap_or(Value::Null)
}

fn row_float_value(row: &SqliteRow, column: &str) -> Value {
    row.try_get::<f64, _>(column)
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn row_boolean_value(row: &SqliteRow, column: &str) -> Value {
    row.try_get::<bool, _>(column).map(Value::Bool).unwrap_or(Value::Null)
}

fn row_window_position_value(row: &SqliteRow) -> Value {
    match (row_i64(row, "windowPositionX"), row_i64(row, "windowPositionY")) {
        (Some(x), Some(y)) => serde_json::json!({ "x": x, "y": y }),
        _ => Value::Null,
    }
}

fn row_window_size_value(row: &SqliteRow) -> Value {
    match (row_i64(row, "windowSizeWidth"), row_i64(row, "windowSizeHeight")) {
        (Some(width), Some(height)) => serde_json::json!({ "width": width, "height": height }),
        _ => Value::Null,
    }
}

fn json_values_equal(left: &Value, right: &Value) -> bool {
    match (left.as_f64(), right.as_f64()) {
        (Some(left), Some(right)) => (left - right).abs() < f64::EPSILON,
        _ => match (left.as_str(), right.as_str()) {
            (Some(left), Some(right)) => match (
                serde_json::from_str::<Value>(left),
                serde_json::from_str::<Value>(right),
            ) {
                (Ok(left), Ok(right)) => json_values_equal(&left, &right),
                _ => left == right,
            },
            _ => left == right,
        },
    }
}

pub(crate) fn parse_string_array(value: Option<&str>) -> Vec<String> {
    value
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default()
}

fn parse_value_array(value: Option<&str>) -> Vec<Value> {
    value
        .and_then(|value| serde_json::from_str::<Vec<Value>>(value).ok())
        .unwrap_or_default()
}

fn parse_value_map(value: Option<&str>) -> BTreeMap<String, Value> {
    parse_value_array(value)
        .into_iter()
        .filter_map(|value| value_id(&value).map(|id| (id, value)))
        .collect()
}

fn merge_string_array(current: Option<&str>, add: &[String], remove: &[String]) -> String {
    let mut values = parse_string_array(current).into_iter().collect::<BTreeSet<_>>();
    for value in remove {
        values.remove(value);
    }
    values.extend(add.iter().cloned());
    serde_json::to_string(&values.into_iter().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_string())
}

fn merge_values_by_id(current: Option<&str>, upserts: &[Value], delete_ids: &[String]) -> String {
    let mut values = parse_value_map(current);
    for id in delete_ids {
        values.remove(id);
    }
    for value in upserts {
        if let Some(id) = value_id(value) {
            values.insert(id, value.clone());
        }
    }
    serde_json::to_string(&values.into_values().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_string())
}

fn value_id(value: &Value) -> Option<String> {
    value.get("id")?.as_str().map(str::to_string)
}

// --- API typée ---

/// Paramètres interprétés, avec les valeurs par défaut de `SettingsContext` 1.x.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub settings_revision: i64,
    pub theme: Theme,
    /// `default` ou une couleur hexadécimale.
    pub primary_color: String,
    pub window_position: Option<WindowPosition>,
    pub window_size: Option<WindowSize>,
    pub account_groups: BTreeMap<String, String>,
    pub custom_groups: Vec<String>,
    pub custom_groups_order: Option<Vec<String>>,
    pub accounts_order: Option<Vec<String>>,
    pub component_spacing: i32,
    pub component_padding: i32,
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
}

pub const DEFAULT_PRIMARY_COLOR: &str = "default";

/// Palette de la page Paramètres (couleurs système Apple).
pub const ACCENT_COLORS: [&str; 10] = [
    "#007AFF", "#AF52DE", "#FF2D55", "#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#5AC8FA", "#5856D6", "#8E8E93",
];

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            settings_revision: 0,
            theme: Theme::System,
            primary_color: DEFAULT_PRIMARY_COLOR.to_string(),
            window_position: None,
            window_size: None,
            account_groups: BTreeMap::new(),
            custom_groups: Vec::new(),
            custom_groups_order: None,
            accounts_order: None,
            component_spacing: 6,
            component_padding: 6,
            last_seen_version: None,
            dismissed_budget_suggestions: Vec::new(),
            dismissed_scheduled_suggestions: Vec::new(),
            prediction_time_range: TimeRange::Year,
            prediction_custom_end_date: None,
            prediction_alert_threshold: 0.0,
            prediction_month_starts_on_first: true,
            prediction_fake_transactions: Vec::new(),
            analytics_time_range: TimeRange::Year,
            analytics_custom_start_date: None,
            analytics_custom_end_date: None,
            analytics_month_starts_on_first: true,
            analytics_hidden_expense_categories: Vec::new(),
            analytics_hidden_income_categories: Vec::new(),
            scheduled_due_range: ScheduledDueRange::All,
        }
    }
}

fn parse_json_option<T: for<'de> Deserialize<'de>>(value: Option<&str>) -> Option<T> {
    value.and_then(|value| serde_json::from_str::<T>(value).ok())
}

fn parse_fake_transactions(value: Option<&str>) -> Vec<PredictionFakeTransaction> {
    parse_value_array(value)
        .into_iter()
        .filter_map(|value| serde_json::from_value::<PredictionFakeTransaction>(value).ok())
        .filter(|transaction| {
            !transaction.id.is_empty()
                && !transaction.account_id.is_empty()
                && transaction.amount.is_finite()
                && transaction.amount > 0.0
        })
        .collect()
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value.filter(|value| !value.is_empty()).cloned()
}

impl AppSettings {
    pub fn from_record(record: Option<&SettingsRecord>) -> Self {
        let Some(record) = record else {
            return Self::default();
        };
        let defaults = Self::default();

        Self {
            settings_revision: record.settings_revision.unwrap_or_default(),
            theme: Theme::parse(&record.theme),
            primary_color: if record.primary_color.is_empty() {
                defaults.primary_color
            } else {
                record.primary_color.clone()
            },
            window_position: record.window_position,
            window_size: record.window_size,
            account_groups: parse_json_option::<BTreeMap<String, Value>>(record.account_groups.as_deref())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(account, group)| {
                    group
                        .as_str()
                        .filter(|group| !group.is_empty())
                        .map(|group| (account, group.to_string()))
                })
                .collect(),
            custom_groups: parse_json_option(record.custom_groups.as_deref()).unwrap_or_default(),
            custom_groups_order: parse_json_option(record.custom_groups_order.as_deref()),
            accounts_order: parse_json_option(record.accounts_order.as_deref()),
            component_spacing: record.component_spacing,
            component_padding: record.component_padding,
            last_seen_version: non_empty(record.last_seen_version.as_ref()),
            dismissed_budget_suggestions: parse_string_array(record.dismissed_budget_suggestions.as_deref()),
            dismissed_scheduled_suggestions: parse_string_array(record.dismissed_scheduled_suggestions.as_deref()),
            prediction_time_range: record
                .prediction_time_range
                .as_deref()
                .and_then(TimeRange::try_parse)
                .unwrap_or(TimeRange::Year),
            prediction_custom_end_date: non_empty(record.prediction_custom_end_date.as_ref()),
            prediction_alert_threshold: record
                .prediction_alert_threshold
                .filter(|value| value.is_finite())
                .unwrap_or(0.0),
            prediction_month_starts_on_first: record.prediction_month_starts_on_first.unwrap_or(true),
            prediction_fake_transactions: parse_fake_transactions(record.prediction_fake_transactions.as_deref()),
            analytics_time_range: record
                .analytics_time_range
                .as_deref()
                .and_then(TimeRange::try_parse)
                .unwrap_or(TimeRange::Year),
            analytics_custom_start_date: non_empty(record.analytics_custom_start_date.as_ref()),
            analytics_custom_end_date: non_empty(record.analytics_custom_end_date.as_ref()),
            analytics_month_starts_on_first: record.analytics_month_starts_on_first.unwrap_or(true),
            analytics_hidden_expense_categories: parse_string_array(
                record.analytics_hidden_expense_categories.as_deref(),
            ),
            analytics_hidden_income_categories: parse_string_array(
                record.analytics_hidden_income_categories.as_deref(),
            ),
            scheduled_due_range: record
                .scheduled_due_range
                .as_deref()
                .and_then(ScheduledDueRange::try_parse)
                .unwrap_or(ScheduledDueRange::All),
        }
    }

    /// Ordre effectif des groupes : `customGroupsOrder || customGroups`.
    pub fn effective_group_order(&self) -> Vec<String> {
        self.custom_groups_order
            .clone()
            .unwrap_or_else(|| self.custom_groups.clone())
    }

    /// Couleur d'accent personnalisée, `None` pour la couleur par défaut.
    pub fn accent_color(&self) -> Option<&str> {
        (self.primary_color != DEFAULT_PRIMARY_COLOR && !self.primary_color.is_empty())
            .then_some(self.primary_color.as_str())
    }
}

/// Modification unitaire demandée par une interface native.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsChange {
    Theme(Theme),
    PrimaryColor(String),
    WindowFrame {
        position: Option<WindowPosition>,
        size: Option<WindowSize>,
    },
    AccountGroup {
        account_id: String,
        group: Option<String>,
    },
    AddCustomGroup(String),
    RenameCustomGroup {
        old_name: String,
        new_name: String,
    },
    DeleteCustomGroup(String),
    CustomGroupsOrder(Vec<String>),
    AccountsOrder(Vec<String>),
    LastSeenVersion(String),
    DismissBudgetSuggestion(String),
    DismissScheduledSuggestion(String),
    PredictionTimeRange(TimeRange),
    PredictionCustomEndDate(String),
    PredictionAlertThreshold(f64),
    PredictionMonthStartsOnFirst(bool),
    UpsertFakeTransaction(PredictionFakeTransaction),
    DeleteFakeTransactions(Vec<String>),
    AnalyticsTimeRange(TimeRange),
    AnalyticsCustomStartDate(String),
    AnalyticsCustomEndDate(String),
    AnalyticsMonthStartsOnFirst(bool),
    AnalyticsCategoryHidden {
        kind: TransactionType,
        category_id: String,
        hidden: bool,
    },
    ScheduledDueRange(ScheduledDueRange),
}

fn to_json<T: Serialize>(value: &T) -> CoreResult<String> {
    serde_json::to_string(value).map_err(|error| CoreError::Database(error.to_string()))
}

/// Traduit une modification typée en patch. La révision de base maximale donne toujours
/// raison à l'action locale, comme une saisie directe dans l'interface.
pub fn change_to_patch(current: &AppSettings, change: SettingsChange) -> CoreResult<SettingsPatch> {
    let mut patch = SettingsPatch {
        schema_version: 2,
        base_revision: i64::MAX,
        ..SettingsPatch::default()
    };
    let values = &mut patch.values;

    match change {
        SettingsChange::Theme(theme) => values.theme = Some(theme.as_str().to_string()),
        SettingsChange::PrimaryColor(color) => {
            let color = color.trim().to_string();
            values.primary_color = Some(if color.is_empty() {
                DEFAULT_PRIMARY_COLOR.to_string()
            } else {
                color
            });
        }
        SettingsChange::WindowFrame { position, size } => {
            values.window_position = position;
            values.window_size = size;
        }
        SettingsChange::AccountGroup { account_id, group } => {
            let mut groups = current.account_groups.clone();
            match group
                .map(|group| group.trim().to_string())
                .filter(|group| !group.is_empty())
            {
                Some(group) => {
                    groups.insert(account_id, group);
                }
                None => {
                    groups.remove(&account_id);
                }
            }
            values.account_groups = Some(to_json(&groups)?);
        }
        SettingsChange::AddCustomGroup(name) => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(CoreError::validation("Le nom du groupe est requis."));
            }
            if current.custom_groups.contains(&name) {
                return Err(CoreError::validation("Ce groupe existe déjà."));
            }
            let mut groups = current.custom_groups.clone();
            groups.push(name.clone());
            values.custom_groups = Some(to_json(&groups)?);
            if let Some(order) = &current.custom_groups_order {
                let mut order = order.clone();
                order.push(name);
                values.custom_groups_order = Some(to_json(&order)?);
            }
        }
        SettingsChange::RenameCustomGroup { old_name, new_name } => {
            let new_name = new_name.trim().to_string();
            if new_name.is_empty() || new_name == old_name {
                return Ok(patch);
            }
            if current.custom_groups.contains(&new_name) {
                return Err(CoreError::validation("Ce groupe existe déjà."));
            }
            let rename = |items: &[String]| {
                items
                    .iter()
                    .map(|item| {
                        if *item == old_name {
                            new_name.clone()
                        } else {
                            item.clone()
                        }
                    })
                    .collect::<Vec<_>>()
            };
            values.custom_groups = Some(to_json(&rename(&current.custom_groups))?);
            if let Some(order) = &current.custom_groups_order {
                values.custom_groups_order = Some(to_json(&rename(order))?);
            }
            let groups = current
                .account_groups
                .iter()
                .map(|(account, group)| {
                    let group = if *group == old_name {
                        new_name.clone()
                    } else {
                        group.clone()
                    };
                    (account.clone(), group)
                })
                .collect::<BTreeMap<_, _>>();
            values.account_groups = Some(to_json(&groups)?);
        }
        SettingsChange::DeleteCustomGroup(name) => {
            let remaining = current
                .custom_groups
                .iter()
                .filter(|group| **group != name)
                .cloned()
                .collect::<Vec<_>>();
            values.custom_groups = Some(to_json(&remaining)?);
            if let Some(order) = &current.custom_groups_order {
                let order = order
                    .iter()
                    .filter(|group| **group != name)
                    .cloned()
                    .collect::<Vec<_>>();
                values.custom_groups_order = Some(to_json(&order)?);
            }
            let groups = current
                .account_groups
                .iter()
                .filter(|(_, group)| **group != name)
                .map(|(account, group)| (account.clone(), group.clone()))
                .collect::<BTreeMap<_, _>>();
            values.account_groups = Some(to_json(&groups)?);
        }
        SettingsChange::CustomGroupsOrder(order) => values.custom_groups_order = Some(to_json(&order)?),
        SettingsChange::AccountsOrder(order) => values.accounts_order = Some(to_json(&order)?),
        SettingsChange::LastSeenVersion(version) => values.last_seen_version = Some(version),
        SettingsChange::DismissBudgetSuggestion(key) => patch.dismissed_budget_suggestions_add = vec![key],
        SettingsChange::DismissScheduledSuggestion(key) => patch.dismissed_scheduled_suggestions_add = vec![key],
        SettingsChange::PredictionTimeRange(range) => values.prediction_time_range = Some(range.as_str().to_string()),
        SettingsChange::PredictionCustomEndDate(date) => values.prediction_custom_end_date = Some(date),
        SettingsChange::PredictionAlertThreshold(threshold) => {
            values.prediction_alert_threshold = Some(if threshold.is_finite() { threshold } else { 0.0 });
        }
        SettingsChange::PredictionMonthStartsOnFirst(value) => values.prediction_month_starts_on_first = Some(value),
        SettingsChange::UpsertFakeTransaction(transaction) => {
            patch.prediction_fake_transactions_upsert =
                vec![serde_json::to_value(&transaction).map_err(|error| CoreError::Database(error.to_string()))?];
        }
        SettingsChange::DeleteFakeTransactions(ids) => patch.prediction_fake_transaction_delete_ids = ids,
        SettingsChange::AnalyticsTimeRange(range) => values.analytics_time_range = Some(range.as_str().to_string()),
        SettingsChange::AnalyticsCustomStartDate(date) => values.analytics_custom_start_date = Some(date),
        SettingsChange::AnalyticsCustomEndDate(date) => values.analytics_custom_end_date = Some(date),
        SettingsChange::AnalyticsMonthStartsOnFirst(value) => values.analytics_month_starts_on_first = Some(value),
        SettingsChange::AnalyticsCategoryHidden {
            kind,
            category_id,
            hidden,
        } => {
            let (add, remove) = if kind == TransactionType::Income {
                (
                    &mut patch.analytics_hidden_income_categories_add,
                    &mut patch.analytics_hidden_income_categories_remove,
                )
            } else {
                (
                    &mut patch.analytics_hidden_expense_categories_add,
                    &mut patch.analytics_hidden_expense_categories_remove,
                )
            };
            if hidden {
                add.push(category_id);
            } else {
                remove.push(category_id);
            }
        }
        SettingsChange::ScheduledDueRange(range) => values.scheduled_due_range = Some(range.as_str().to_string()),
    }

    Ok(patch)
}

pub async fn load_app_settings(pool: &DbPool) -> CoreResult<AppSettings> {
    Ok(AppSettings::from_record(get_settings_record(pool).await?.as_ref()))
}

pub async fn apply_change(pool: &DbPool, change: SettingsChange) -> CoreResult<SettingsPatchResult> {
    let current = load_app_settings(pool).await?;
    let patch = change_to_patch(&current, change)?;
    apply_settings_patch(pool, patch).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_pool;
    use serde_json::json;

    #[test]
    fn keeps_the_newest_seen_version() {
        assert_eq!(
            newest_seen_version(Some("1.0.13".into()), Some("1.0.12".into())),
            Some("1.0.13".into())
        );
        assert_eq!(
            newest_seen_version(Some("1.9.9".into()), Some("1.10.0".into())),
            Some("1.10.0".into())
        );
        assert_eq!(
            newest_seen_version(Some("1.0.13".into()), Some("unknown".into())),
            Some("1.0.13".into())
        );
    }

    #[test]
    fn additive_suggestions_keep_existing_values() {
        assert_eq!(
            merge_string_array(Some(r#"["fuel","tax"]"#), &["car".into()], &[]),
            r#"["car","fuel","tax"]"#
        );
    }

    #[test]
    fn fake_transaction_operations_preserve_remote_items() {
        let merged = merge_values_by_id(
            Some(r#"[{"id":"local"},{"id":"remote"}]"#),
            &[json!({"id": "local", "amount": 20})],
            &[],
        );
        let value: Vec<Value> = serde_json::from_str(&merged).unwrap();
        assert_eq!(value.len(), 2);
        assert!(value.iter().any(|item| item["id"] == "remote"));
        assert!(value.iter().any(|item| item["amount"] == 20));
    }

    #[tokio::test]
    async fn stale_scalar_patch_is_rejected_without_blocking_unrelated_fields() {
        let pool = open_memory_pool().await.unwrap();

        let first = apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                values: SettingsValuesPatch {
                    prediction_alert_threshold: Some(100.0),
                    ..SettingsValuesPatch::default()
                },
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(first.revision, 1);

        let unrelated = apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                values: SettingsValuesPatch {
                    theme: Some("dark".to_string()),
                    ..SettingsValuesPatch::default()
                },
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(unrelated.revision, 2);
        assert!(unrelated.conflicts.is_empty());

        let stale = apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                expected_values: BTreeMap::from([("predictionAlertThreshold".to_string(), json!(0))]),
                values: SettingsValuesPatch {
                    prediction_alert_threshold: Some(25.0),
                    ..SettingsValuesPatch::default()
                },
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(stale.revision, 2);
        assert_eq!(stale.conflicts, vec!["predictionAlertThreshold"]);

        let settings = load_app_settings(&pool).await.unwrap();
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.prediction_alert_threshold, 100.0);
    }

    #[tokio::test]
    async fn sequential_local_scalar_patch_is_allowed_only_when_expected_value_matches() {
        let pool = open_memory_pool().await.unwrap();
        apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                values: SettingsValuesPatch {
                    prediction_alert_threshold: Some(100.0),
                    ..SettingsValuesPatch::default()
                },
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();

        let result = apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                expected_values: BTreeMap::from([("predictionAlertThreshold".to_string(), json!(100))]),
                values: SettingsValuesPatch {
                    prediction_alert_threshold: Some(200.0),
                    ..SettingsValuesPatch::default()
                },
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();

        assert!(result.conflicts.is_empty());
        assert_eq!(result.revision, 2);
        assert_eq!(
            load_app_settings(&pool).await.unwrap().prediction_alert_threshold,
            200.0
        );
    }

    #[tokio::test]
    async fn fake_transactions_merge_by_id_and_stale_delete_is_rejected() {
        let pool = open_memory_pool().await.unwrap();
        let fake = |id: &str, amount: f64| json!({"id": id, "date": "2026-10-01", "accountId": "a", "type": "expense", "amount": amount, "category": "5", "description": "Test", "enabled": true});

        apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                prediction_fake_transactions_upsert: vec![fake("desktop", 10.0)],
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();
        let second = apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                prediction_fake_transactions_upsert: vec![fake("mobile", 20.0)],
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(second.revision, 2);

        let stale_delete = apply_settings_patch(
            &pool,
            SettingsPatch {
                schema_version: 2,
                prediction_fake_transaction_delete_ids: vec!["desktop".to_string()],
                ..SettingsPatch::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(stale_delete.conflicts, vec!["predictionFakeTransactions:desktop"]);

        let settings = load_app_settings(&pool).await.unwrap();
        assert_eq!(settings.prediction_fake_transactions.len(), 2);
    }

    #[tokio::test]
    async fn typed_changes_manage_groups() {
        let pool = open_memory_pool().await.unwrap();
        apply_change(&pool, SettingsChange::AddCustomGroup("Perso".into()))
            .await
            .unwrap();
        apply_change(
            &pool,
            SettingsChange::AccountGroup {
                account_id: "a1".into(),
                group: Some("Perso".into()),
            },
        )
        .await
        .unwrap();
        apply_change(
            &pool,
            SettingsChange::RenameCustomGroup {
                old_name: "Perso".into(),
                new_name: "Famille".into(),
            },
        )
        .await
        .unwrap();

        let settings = load_app_settings(&pool).await.unwrap();
        assert_eq!(settings.custom_groups, vec!["Famille"]);
        assert_eq!(settings.account_groups.get("a1").map(String::as_str), Some("Famille"));

        apply_change(&pool, SettingsChange::DeleteCustomGroup("Famille".into()))
            .await
            .unwrap();
        let settings = load_app_settings(&pool).await.unwrap();
        assert!(settings.custom_groups.is_empty());
        assert!(settings.account_groups.is_empty());

        assert!(apply_change(&pool, SettingsChange::AddCustomGroup("  ".into()))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn typed_changes_toggle_hidden_categories_and_dismissals() {
        let pool = open_memory_pool().await.unwrap();
        apply_change(
            &pool,
            SettingsChange::AnalyticsCategoryHidden {
                kind: TransactionType::Expense,
                category_id: "5".into(),
                hidden: true,
            },
        )
        .await
        .unwrap();
        apply_change(&pool, SettingsChange::DismissBudgetSuggestion("5|all".into()))
            .await
            .unwrap();
        apply_change(&pool, SettingsChange::DismissBudgetSuggestion("9|all".into()))
            .await
            .unwrap();

        let settings = load_app_settings(&pool).await.unwrap();
        assert_eq!(settings.analytics_hidden_expense_categories, vec!["5"]);
        assert_eq!(settings.dismissed_budget_suggestions, vec!["5|all", "9|all"]);

        apply_change(
            &pool,
            SettingsChange::AnalyticsCategoryHidden {
                kind: TransactionType::Expense,
                category_id: "5".into(),
                hidden: false,
            },
        )
        .await
        .unwrap();
        assert!(load_app_settings(&pool)
            .await
            .unwrap()
            .analytics_hidden_expense_categories
            .is_empty());
    }
}
