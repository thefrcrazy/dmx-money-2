//! Prédictions de trésorerie (port de `pages/Predictions.tsx` et `utils/predictions.ts`).
//!
//! Les banques débitent avant de créditer : le point bas d'une journée (retraits appliqués,
//! revenus pas encore arrivés) décide des frais de découvert, même si la journée finit positive.

use crate::dates::{
    add_days, add_months, days_between, end_of_month, format_date, next_occurrence, parse_date, start_of_month,
};
use crate::error::{CoreError, CoreResult};
use crate::format::{date_day_month, date_day_month_year, date_long};
use crate::metrics::{cents, euros, signed_cents};
use crate::models::{string_enum, PredictionFakeTransaction, TimeRange, TransactionType, TRANSFER_CATEGORY_ID};
use crate::settings::AppSettings;
use crate::snapshot::{is_selected, Snapshot};
use crate::text::compare_fr;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

string_enum!(Severity, default = Warning, {
    Warning => "warning",
    Danger => "danger",
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DayFlow {
    /// Somme des retraits du jour en centimes (négative ou nulle).
    pub debits: i64,
    /// Somme des revenus du jour en centimes (positive ou nulle).
    pub credits: i64,
}

impl DayFlow {
    pub fn add(&mut self, amount_cents: i64) {
        if amount_cents < 0 {
            self.debits += amount_cents;
        } else {
            self.credits += amount_cents;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayBalances {
    pub low: i64,
    pub close: i64,
}

pub fn apply_day_flow(opening: i64, flow: Option<&DayFlow>) -> DayBalances {
    let low = opening + flow.map(|flow| flow.debits).unwrap_or(0);
    DayBalances {
        low,
        close: low + flow.map(|flow| flow.credits).unwrap_or(0),
    }
}

/// La courbe de fin de journée passe sous zéro (danger) ou sous le seuil (avertissement).
pub fn detect_closing_crossing(value: i64, previous: i64, threshold: i64) -> Option<Severity> {
    if value < 0 && previous >= 0 {
        return Some(Severity::Danger);
    }
    if threshold > 0 && value < threshold && previous >= threshold {
        return Some(Severity::Warning);
    }
    None
}

/// La journée ne finit bien que grâce à un revenu arrivé après les retraits.
pub fn detect_intraday_risk(low: i64, value: i64, threshold: i64) -> Option<Severity> {
    if low < 0 && value >= 0 {
        return Some(Severity::Danger);
    }
    if threshold > 0 && low < threshold && value >= threshold {
        return Some(Severity::Warning);
    }
    None
}

pub fn highest_severity(severities: impl IntoIterator<Item = Severity>) -> Option<Severity> {
    severities.into_iter().max()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PredictionQuery {
    pub accounts: Vec<String>,
    pub range: TimeRange,
    pub custom_end_date: Option<String>,
    pub month_starts_on_first: bool,
    pub alert_threshold: f64,
}

impl PredictionQuery {
    pub fn from_settings(settings: &AppSettings, accounts: Vec<String>) -> Self {
        Self {
            accounts,
            range: settings.prediction_time_range,
            custom_end_date: settings.prediction_custom_end_date.clone(),
            month_starts_on_first: settings.prediction_month_starts_on_first,
            alert_threshold: settings.prediction_alert_threshold,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PredictionSeries {
    pub id: String,
    pub name: String,
    pub color: String,
    /// Soldes de fin de journée.
    pub closes: Vec<f64>,
    /// Points bas (retraits appliqués avant les revenus).
    pub lows: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BalanceAtDate {
    pub name: String,
    pub color: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntradayBalance {
    pub name: String,
    pub low: f64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FakeTransactionRow {
    pub transaction: PredictionFakeTransaction,
    pub source_account_name: String,
    pub destination_account_name: Option<String>,
    pub category_name: Option<String>,
    pub type_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PredictionView {
    pub start_date: String,
    pub end_date: String,
    /// Ex. « 14 sept. 2027 »
    pub end_label: String,
    /// Ex. « 1 an »
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

pub const DANGER_COLOR: &str = "#ef4444";
pub const WARNING_COLOR: &str = "#f97316";
pub const INTRADAY_DANGER_COLOR: &str = "#a855f7";
pub const INTRADAY_WARNING_COLOR: &str = "#eab308";

fn marker_color(severity: Option<Severity>, intraday: Option<Severity>) -> &'static str {
    match (severity, intraday) {
        (Some(Severity::Danger), _) => DANGER_COLOR,
        (Some(Severity::Warning), _) => WARNING_COLOR,
        (None, Some(Severity::Danger)) => INTRADAY_DANGER_COLOR,
        (None, _) => INTRADAY_WARNING_COLOR,
    }
}

/// Bornes de la projection, jour de début et jour de fin inclus.
pub fn projection_range(
    range: TimeRange,
    custom_end: Option<&str>,
    month_starts_on_first: bool,
    today: NaiveDate,
) -> (NaiveDate, NaiveDate) {
    let start = if range != TimeRange::Custom && month_starts_on_first {
        start_of_month(today)
    } else {
        today
    };
    let end = match range {
        TimeRange::Month if month_starts_on_first => end_of_month(today),
        TimeRange::Custom => custom_end
            .and_then(parse_date)
            .unwrap_or_else(|| add_months(today, 1))
            .max(today),
        TimeRange::Week => add_days(today, 7),
        other => add_months(today, other.months().unwrap_or(12) as i32),
    };
    (start, end)
}

/// Une opération s'applique au filtre si son compte, ou la destination d'un virement, est sélectionné.
pub fn applies_to_filter(
    accounts: &[String],
    account_id: &str,
    kind: TransactionType,
    to_account: Option<&str>,
) -> bool {
    is_selected(accounts, account_id)
        || (kind == TransactionType::Transfer && to_account.is_some_and(|to_account| is_selected(accounts, to_account)))
}

pub fn predictions(snapshot: &Snapshot, query: &PredictionQuery, today: NaiveDate) -> PredictionView {
    let (start, end) = projection_range(
        query.range,
        query.custom_end_date.as_deref(),
        query.month_starts_on_first,
        today,
    );
    let days = days_between(start, end).max(0) as usize;
    let threshold = cents(query.alert_threshold);

    let accounts: Vec<_> = snapshot
        .accounts
        .iter()
        .filter(|account| is_selected(&query.accounts, &account.id))
        .collect();
    let index_of: HashMap<&str, usize> = accounts
        .iter()
        .enumerate()
        .map(|(index, account)| (account.id.as_str(), index))
        .collect();

    let mut balances: Vec<i64> = accounts.iter().map(|account| cents(account.initial_balance)).collect();
    let mut flows: Vec<Vec<DayFlow>> = vec![vec![DayFlow::default(); days + 1]; accounts.len()];
    let mut add_impact = |account_id: &str, date: NaiveDate, amount: i64, balances: &mut Vec<i64>| {
        let Some(index) = index_of.get(account_id) else {
            return;
        };
        if date < start {
            balances[*index] += amount;
        } else if date <= end && amount != 0 {
            flows[*index][days_between(start, date) as usize].add(amount);
        }
    };

    let mut current_total = accounts
        .iter()
        .map(|account| cents(account.initial_balance))
        .sum::<i64>();
    for transaction in &snapshot.transactions {
        if !index_of.contains_key(transaction.account_id.as_str()) {
            continue;
        }
        current_total += signed_cents(transaction);
        if let Some(date) = parse_date(&transaction.date) {
            add_impact(&transaction.account_id, date, signed_cents(transaction), &mut balances);
        }
    }

    let applied_fakes: Vec<&PredictionFakeTransaction> = snapshot
        .settings
        .prediction_fake_transactions
        .iter()
        .filter(|fake| {
            applies_to_filter(
                &query.accounts,
                &fake.account_id,
                fake.transaction_type,
                fake.to_account_id.as_deref(),
            )
        })
        .collect();

    let mut fake_impact = 0_i64;
    for fake in applied_fakes.iter().filter(|fake| fake.enabled) {
        let Some(date) = parse_date(&fake.date).filter(|date| *date >= start && *date <= end) else {
            continue;
        };
        let amount = cents(fake.amount);
        let source_visible = index_of.contains_key(fake.account_id.as_str());
        match (fake.transaction_type, fake.to_account_id.as_deref()) {
            (TransactionType::Transfer, to_account) => {
                add_impact(&fake.account_id, date, -amount, &mut balances);
                if source_visible {
                    fake_impact -= amount;
                }
                if let Some(to_account) = to_account {
                    add_impact(to_account, date, amount, &mut balances);
                    if index_of.contains_key(to_account) {
                        fake_impact += amount;
                    }
                }
            }
            (kind, _) => {
                let signed = if kind == TransactionType::Income {
                    amount
                } else {
                    -amount
                };
                add_impact(&fake.account_id, date, signed, &mut balances);
                if source_visible {
                    fake_impact += signed;
                }
            }
        }
    }

    for item in snapshot.scheduled.iter().filter(|item| {
        applies_to_filter(
            &query.accounts,
            &item.account_id,
            item.transaction_type,
            item.to_account_id.as_deref(),
        )
    }) {
        let Some(mut next) = parse_date(&item.next_date) else {
            continue;
        };
        let item_end = item.end_date.as_deref().and_then(parse_date);
        let amount = cents(item.amount);

        while next <= end {
            if item_end.is_some_and(|item_end| next > item_end) {
                break;
            }
            if next >= today {
                match (item.transaction_type, item.to_account_id.as_deref()) {
                    (TransactionType::Transfer, Some(to_account)) => {
                        add_impact(&item.account_id, next, -amount, &mut balances);
                        add_impact(to_account, next, amount, &mut balances);
                    }
                    (TransactionType::Income, _) => add_impact(&item.account_id, next, amount, &mut balances),
                    _ => add_impact(&item.account_id, next, -amount, &mut balances),
                }
            }
            match next_occurrence(next, item.frequency) {
                Some(following) if following > next => next = following,
                _ => break,
            }
        }
    }

    let mut series: Vec<PredictionSeries> = accounts
        .iter()
        .map(|account| PredictionSeries {
            id: account.id.clone(),
            name: account.name.clone(),
            color: if account.color.is_empty() {
                "#3b82f6".to_string()
            } else {
                account.color.clone()
            },
            closes: Vec::with_capacity(days + 1),
            lows: Vec::with_capacity(days + 1),
        })
        .collect();
    let mut total = PredictionSeries {
        id: "total".to_string(),
        name: "Total".to_string(),
        color: DANGER_COLOR.to_string(),
        closes: Vec::with_capacity(days + 1),
        lows: Vec::with_capacity(days + 1),
    };

    let mut dates = Vec::with_capacity(days + 1);
    let mut labels = Vec::with_capacity(days + 1);
    let mut full_labels = Vec::with_capacity(days + 1);
    let mut closes_cents: Vec<Vec<i64>> = vec![Vec::with_capacity(days + 1); accounts.len() + 1];
    let mut lows_cents: Vec<Vec<i64>> = vec![Vec::with_capacity(days + 1); accounts.len() + 1];

    for day in 0..=days {
        let date = add_days(start, day as i64);
        dates.push(format_date(date));
        labels.push(date_day_month(date));
        full_labels.push(date_long(date));

        let (mut total_close, mut total_low) = (0_i64, 0_i64);
        for (index, balance) in balances.iter_mut().enumerate() {
            let DayBalances { low, close } = apply_day_flow(*balance, Some(&flows[index][day]));
            *balance = close;
            closes_cents[index + 1].push(close);
            lows_cents[index + 1].push(low);
            series[index].closes.push(euros(close));
            series[index].lows.push(euros(low));
            total_close += close;
            total_low += low;
        }
        closes_cents[0].push(total_close);
        lows_cents[0].push(total_low);
        total.closes.push(euros(total_close));
        total.lows.push(euros(total_low));
    }

    let names: Vec<&str> = std::iter::once("Total")
        .chain(accounts.iter().map(|account| account.name.as_str()))
        .collect();
    let mut markers = Vec::new();
    for day in 0..=days {
        let mut crossings: Vec<(String, Severity)> = Vec::new();
        let mut intraday: Vec<(IntradayBalance, Severity)> = Vec::new();
        for key in 0..names.len() {
            let value = closes_cents[key][day];
            let low = lows_cents[key][day];
            let previous = if day == 0 { 0 } else { closes_cents[key][day - 1] };
            if let Some(severity) = detect_closing_crossing(value, previous, threshold) {
                crossings.push((names[key].to_string(), severity));
            }
            if let Some(severity) = detect_intraday_risk(low, value, threshold) {
                intraday.push((
                    IntradayBalance {
                        name: names[key].to_string(),
                        low: euros(low),
                        value: euros(value),
                    },
                    severity,
                ));
            }
        }
        if crossings.is_empty() && intraday.is_empty() {
            continue;
        }

        let severity = highest_severity(crossings.iter().map(|(_, severity)| *severity));
        let intraday_severity = highest_severity(intraday.iter().map(|(_, severity)| *severity));
        markers.push(PredictionMarker {
            date: dates[day].clone(),
            index: day as u32,
            full_label: full_labels[day].clone(),
            crossing_names: crossings.into_iter().map(|(name, _)| name).collect(),
            severity,
            intraday_names: intraday.iter().map(|(balance, _)| balance.name.clone()).collect(),
            intraday_severity,
            intraday_balances: intraday.into_iter().map(|(balance, _)| balance).collect(),
            balances: if accounts.is_empty() {
                vec![BalanceAtDate {
                    name: "Total".to_string(),
                    color: DANGER_COLOR.to_string(),
                    value: total.closes[day],
                }]
            } else {
                series
                    .iter()
                    .map(|series| BalanceAtDate {
                        name: series.name.clone(),
                        color: series.color.clone(),
                        value: series.closes[day],
                    })
                    .collect()
            },
            stroke_color: marker_color(severity, intraday_severity).to_string(),
        });
    }

    let mut fake_rows: Vec<FakeTransactionRow> = applied_fakes
        .into_iter()
        .map(|fake| FakeTransactionRow {
            source_account_name: snapshot
                .account(&fake.account_id)
                .map(|account| account.name.clone())
                .unwrap_or_else(|| "Compte supprimé".to_string()),
            destination_account_name: (fake.transaction_type == TransactionType::Transfer).then(|| {
                fake.to_account_id
                    .as_deref()
                    .and_then(|id| snapshot.account(id))
                    .map(|account| account.name.clone())
                    .unwrap_or_else(|| "Compte supprimé".to_string())
            }),
            category_name: (fake.transaction_type != TransactionType::Transfer)
                .then(|| snapshot.category(&fake.category).map(|category| category.name.clone()))
                .flatten(),
            type_label: fake.transaction_type.label().to_string(),
            transaction: fake.clone(),
        })
        .collect();
    fake_rows.sort_by(|left, right| {
        left.transaction
            .date
            .cmp(&right.transaction.date)
            .then_with(|| compare_fr(&left.transaction.description, &right.transaction.description))
    });

    let midpoint_index = days / 2;
    PredictionView {
        start_date: format_date(start),
        end_date: format_date(end),
        end_label: date_day_month_year(end),
        title_label: query.range.title_label().to_string(),
        current_total_balance: euros(current_total),
        midpoint_balance: total
            .closes
            .get(midpoint_index)
            .copied()
            .unwrap_or(euros(current_total)),
        final_balance: total.closes.last().copied().unwrap_or(euros(current_total)),
        alert_threshold: query.alert_threshold,
        intraday_risk_count: markers
            .iter()
            .filter(|marker| marker.intraday_severity.is_some())
            .count() as u32,
        markers,
        enabled_fake_count: fake_rows.iter().filter(|row| row.transaction.enabled).count() as u32,
        fake_transactions: fake_rows,
        fake_impact: euros(fake_impact),
        dates,
        labels,
        full_labels,
        accounts: series,
        total,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

pub fn new_fake_transaction_draft(snapshot: &Snapshot, accounts: &[String], today: NaiveDate) -> FakeTransactionDraft {
    let account_id = snapshot
        .accounts
        .iter()
        .find(|account| is_selected(accounts, &account.id))
        .or_else(|| snapshot.accounts.first())
        .map(|account| account.id.clone())
        .unwrap_or_default();
    FakeTransactionDraft {
        id: None,
        date: format_date(today),
        description: String::new(),
        amount: 0.0,
        kind: TransactionType::Expense,
        to_account_id: snapshot
            .accounts
            .iter()
            .find(|account| account.id != account_id)
            .map(|account| account.id.clone()),
        account_id,
        category_id: snapshot
            .selectable_categories()
            .first()
            .map(|category| category.id.clone())
            .unwrap_or_default(),
    }
}

pub fn fake_transaction_draft(snapshot: &Snapshot, id: &str) -> Option<FakeTransactionDraft> {
    let fake = snapshot
        .settings
        .prediction_fake_transactions
        .iter()
        .find(|fake| fake.id == id)?;
    Some(FakeTransactionDraft {
        id: Some(fake.id.clone()),
        date: fake.date.clone(),
        description: fake.description.clone(),
        amount: fake.amount,
        kind: fake.transaction_type,
        account_id: fake.account_id.clone(),
        to_account_id: fake.to_account_id.clone(),
        category_id: if fake.transaction_type == TransactionType::Transfer {
            TRANSFER_CATEGORY_ID.to_string()
        } else {
            fake.category.clone()
        },
    })
}

/// Valide une transaction fictive avec les messages de la page Prédictions 1.x.
pub fn validate_fake_transaction(
    snapshot: &Snapshot,
    draft: FakeTransactionDraft,
    today: NaiveDate,
) -> CoreResult<PredictionFakeTransaction> {
    let date = parse_date(&draft.date)
        .filter(|_| draft.date.len() == 10)
        .ok_or_else(|| CoreError::validation("Sélectionnez une date valide"))?;
    if date < today {
        return Err(CoreError::validation("La date doit être aujourd'hui ou dans le futur"));
    }
    if draft.account_id.is_empty() {
        return Err(CoreError::validation("Sélectionnez un compte"));
    }
    if !draft.amount.is_finite() || draft.amount <= 0.0 {
        return Err(CoreError::validation("Saisissez un montant valide"));
    }
    let is_transfer = draft.kind == TransactionType::Transfer;
    if !is_transfer && draft.category_id.trim().is_empty() {
        return Err(CoreError::validation("Sélectionnez une catégorie"));
    }
    let to_account_id = draft.to_account_id.filter(|to_account| !to_account.is_empty());
    if is_transfer
        && to_account_id
            .as_deref()
            .is_none_or(|to_account| to_account == draft.account_id)
    {
        return Err(CoreError::validation("Sélectionnez un compte destination différent"));
    }

    let enabled = draft
        .id
        .as_deref()
        .and_then(|id| {
            snapshot
                .settings
                .prediction_fake_transactions
                .iter()
                .find(|fake| fake.id == id)
        })
        .map(|fake| fake.enabled)
        .unwrap_or(true);
    let description = draft.description.trim();

    Ok(PredictionFakeTransaction {
        id: draft.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        date: format_date(date),
        account_id: draft.account_id,
        transaction_type: draft.kind,
        amount: draft.amount,
        category: if is_transfer {
            TRANSFER_CATEGORY_ID.to_string()
        } else {
            draft.category_id.trim().to_string()
        },
        description: if description.is_empty() {
            "Transaction fictive".to_string()
        } else {
            description.to_string()
        },
        enabled,
        to_account_id: if is_transfer { to_account_id } else { None },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Account, Periodicity, ScheduledTransaction};

    fn date(value: &str) -> NaiveDate {
        parse_date(value).unwrap()
    }

    #[test]
    fn day_flow_keeps_withdrawals_and_income_apart() {
        let mut flow = DayFlow::default();
        flow.add(-50_000);
        flow.add(120_000);
        flow.add(-1_000);
        assert_eq!(
            flow,
            DayFlow {
                debits: -51_000,
                credits: 120_000
            }
        );

        let mut zero = DayFlow::default();
        zero.add(0);
        assert_eq!(zero, DayFlow::default());
    }

    #[test]
    fn settles_withdrawals_before_income() {
        let balances = apply_day_flow(
            20_000,
            Some(&DayFlow {
                debits: -50_000,
                credits: 100_000,
            }),
        );
        assert_eq!(
            balances,
            DayBalances {
                low: -30_000,
                close: 70_000
            }
        );
        assert_eq!(
            apply_day_flow(45_000, None),
            DayBalances {
                low: 45_000,
                close: 45_000
            }
        );
    }

    #[test]
    fn crossing_detection_matches_v1() {
        assert_eq!(detect_closing_crossing(-500, 1_000, 0), Some(Severity::Danger));
        assert_eq!(detect_closing_crossing(-800, -500, 0), None);
        assert_eq!(detect_closing_crossing(8_000, 12_000, 10_000), Some(Severity::Warning));
        assert_eq!(detect_closing_crossing(8_000, 12_000, 0), None);

        assert_eq!(detect_intraday_risk(-30_000, 70_000, 0), Some(Severity::Danger));
        assert_eq!(detect_intraday_risk(5_000, 70_000, 10_000), Some(Severity::Warning));
        assert_eq!(detect_intraday_risk(70_000, 70_000, 10_000), None);
        assert_eq!(detect_intraday_risk(-30_000, -10_000, 0), None);
        assert_eq!(detect_intraday_risk(-1, 70_000, 10_000), Some(Severity::Danger));

        let balances = apply_day_flow(
            12_000,
            Some(&DayFlow {
                debits: -20_000,
                credits: 15_000,
            }),
        );
        assert_eq!(balances.close, 7_000);
        assert_eq!(
            detect_closing_crossing(balances.close, 12_000, 10_000),
            Some(Severity::Warning)
        );
        assert_eq!(balances.low, -8_000);
        assert_eq!(
            detect_intraday_risk(balances.low, balances.close, 10_000),
            Some(Severity::Danger)
        );

        assert_eq!(
            highest_severity([Severity::Warning, Severity::Danger]),
            Some(Severity::Danger)
        );
        assert_eq!(highest_severity([]), None);
    }

    #[test]
    fn projection_flags_the_rent_day_rescued_by_salary() {
        let scheduled = |id: &str, kind, amount: f64, next: &str| ScheduledTransaction {
            id: id.into(),
            description: id.into(),
            amount,
            transaction_type: kind,
            frequency: Periodicity::Monthly,
            account_id: "a1".into(),
            next_date: next.into(),
            category: "1".into(),
            to_account_id: None,
            include_in_forecast: None,
            budget_id: None,
            end_date: None,
        };
        let snapshot = Snapshot {
            accounts: vec![Account {
                id: "a1".into(),
                name: "Courant".into(),
                account_type: "Courant".into(),
                initial_balance: 200.0,
                color: "#3b82f6".into(),
                icon: "Wallet".into(),
            }],
            scheduled: vec![
                scheduled("Loyer", TransactionType::Expense, 500.0, "2026-10-01"),
                scheduled("Salaire", TransactionType::Income, 1000.0, "2026-10-01"),
            ],
            ..Snapshot::default()
        };

        let view = predictions(
            &snapshot,
            &PredictionQuery {
                range: TimeRange::Month,
                month_starts_on_first: false,
                ..PredictionQuery::default()
            },
            date("2026-09-15"),
        );

        assert_eq!(view.start_date, "2026-09-15");
        assert_eq!(view.end_date, "2026-10-15");
        assert_eq!(view.dates.len(), 31);
        let rent_day = view.dates.iter().position(|day| day == "2026-10-01").unwrap();
        assert_eq!(view.accounts[0].lows[rent_day], -300.0);
        assert_eq!(view.accounts[0].closes[rent_day], 700.0);
        assert_eq!(view.final_balance, 700.0);
        assert_eq!(view.intraday_risk_count, 1);
        let marker = &view.markers[0];
        assert_eq!(marker.intraday_severity, Some(Severity::Danger));
        assert_eq!(marker.stroke_color, INTRADAY_DANGER_COLOR);
        assert_eq!(marker.intraday_names, vec!["Total", "Courant"]);
    }

    #[test]
    fn fake_transactions_validation_and_impact() {
        let mut snapshot = Snapshot {
            accounts: vec![
                Account {
                    id: "a1".into(),
                    name: "Courant".into(),
                    account_type: "Courant".into(),
                    initial_balance: 0.0,
                    color: "#000".into(),
                    icon: "Wallet".into(),
                },
                Account {
                    id: "a2".into(),
                    name: "Épargne".into(),
                    account_type: "Épargne".into(),
                    initial_balance: 0.0,
                    color: "#111".into(),
                    icon: "PiggyBank".into(),
                },
            ],
            ..Snapshot::default()
        };
        let today = date("2026-09-15");
        let draft = FakeTransactionDraft {
            id: None,
            date: "2026-09-20".into(),
            description: " ".into(),
            amount: 100.0,
            kind: TransactionType::Transfer,
            account_id: "a1".into(),
            to_account_id: Some("a2".into()),
            category_id: String::new(),
        };
        let fake = validate_fake_transaction(&snapshot, draft.clone(), today).unwrap();
        assert_eq!(fake.description, "Transaction fictive");
        assert_eq!(fake.category, TRANSFER_CATEGORY_ID);

        let error = validate_fake_transaction(
            &snapshot,
            FakeTransactionDraft {
                date: "2026-09-01".into(),
                ..draft.clone()
            },
            today,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "La date doit être aujourd'hui ou dans le futur");

        snapshot.settings.prediction_fake_transactions = vec![fake];
        let only_source = predictions(
            &snapshot,
            &PredictionQuery {
                accounts: vec!["a1".into()],
                range: TimeRange::Month,
                ..PredictionQuery::default()
            },
            today,
        );
        assert_eq!(only_source.fake_impact, -100.0);
        assert_eq!(
            only_source.fake_transactions[0].destination_account_name.as_deref(),
            Some("Épargne")
        );

        let both = predictions(
            &snapshot,
            &PredictionQuery {
                range: TimeRange::Month,
                ..PredictionQuery::default()
            },
            today,
        );
        assert_eq!(both.fake_impact, 0.0);
    }
}
