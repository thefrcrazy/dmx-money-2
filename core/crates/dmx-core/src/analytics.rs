//! Page Analyses (port de `pages/Analytics.tsx`).

use crate::dates::{
    add_days, add_months, days_between, each_day, each_month, format_date, month_key, parse_date, start_of_month,
};
use crate::format::{date_day_month, date_long, month_year};
use crate::metrics::{cents, euros, is_internal_transfer, signed_cents};
use crate::models::{TimeRange, TransactionType};
use crate::settings::AppSettings;
use crate::snapshot::{is_selected, CategoryDisplay, Snapshot};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

/// Couleurs de repli des séries quand un compte n'a pas de couleur.
pub const FALLBACK_SERIES_COLORS: [&str; 8] = [
    "#0088FE", "#00C49F", "#FFBB28", "#FF8042", "#8884d8", "#82ca9d", "#ffc658", "#ff7300",
];

pub const INCOME_COLOR: &str = "#10B981";
pub const EXPENSE_COLOR: &str = "#EF4444";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnalyticsQuery {
    pub accounts: Vec<String>,
    pub range: TimeRange,
    pub custom_start: Option<String>,
    pub custom_end: Option<String>,
    pub month_starts_on_first: bool,
    pub hidden_expense_categories: Vec<String>,
    pub hidden_income_categories: Vec<String>,
}

impl AnalyticsQuery {
    pub fn from_settings(settings: &AppSettings, accounts: Vec<String>) -> Self {
        Self {
            accounts,
            range: settings.analytics_time_range,
            custom_start: settings.analytics_custom_start_date.clone(),
            custom_end: settings.analytics_custom_end_date.clone(),
            month_starts_on_first: settings.analytics_month_starts_on_first,
            hidden_expense_categories: settings.analytics_hidden_expense_categories.clone(),
            hidden_income_categories: settings.analytics_hidden_income_categories.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChartSeries {
    pub id: String,
    pub name: String,
    pub color: String,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BalanceHistory {
    pub dates: Vec<String>,
    /// Ex. « 05 mai »
    pub labels: Vec<String>,
    /// Ex. « 5 mai 2026 »
    pub full_labels: Vec<String>,
    pub series: Vec<ChartSeries>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CategorySlice {
    pub category: CategoryDisplay,
    pub value: f64,
    pub hidden: bool,
    /// Part parmi les catégories visibles.
    pub percentage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeriodBar {
    pub label: String,
    pub income: f64,
    pub expenses: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalyticsView {
    pub start_date: String,
    pub end_date: String,
    pub balance_history: BalanceHistory,
    pub expenses_by_category: Vec<CategorySlice>,
    pub income_by_category: Vec<CategorySlice>,
    pub income_vs_expenses: Vec<PeriodBar>,
    /// Barres par jour (période de 31 jours au plus) plutôt que par mois.
    pub daily_buckets: bool,
}

/// Bornes incluses de la période analysée.
pub fn analytics_range(
    range: TimeRange,
    custom_start: Option<&str>,
    custom_end: Option<&str>,
    month_starts_on_first: bool,
    today: NaiveDate,
) -> (NaiveDate, NaiveDate) {
    let align = |date: NaiveDate| {
        if month_starts_on_first {
            start_of_month(date)
        } else {
            date
        }
    };
    match range {
        TimeRange::Week => (align(add_days(today, -7)), today),
        TimeRange::Month if month_starts_on_first => (start_of_month(today), today),
        TimeRange::Custom => {
            let start = custom_start
                .and_then(parse_date)
                .unwrap_or_else(|| add_months(today, -1));
            let end = custom_end.and_then(parse_date).unwrap_or(today);
            if start <= end {
                (start, end)
            } else {
                (end, start)
            }
        }
        other => (align(add_months(today, -(other.months().unwrap_or(12) as i32))), today),
    }
}

fn category_slices(snapshot: &Snapshot, totals: Vec<(String, i64)>, hidden: &[String]) -> Vec<CategorySlice> {
    let visible_total: i64 = totals
        .iter()
        .filter(|(category, _)| !hidden.contains(category))
        .map(|(_, value)| *value)
        .sum();
    let mut slices: Vec<CategorySlice> = totals
        .into_iter()
        .map(|(category, value)| {
            let is_hidden = hidden.contains(&category);
            CategorySlice {
                category: snapshot.category_display(&category),
                value: euros(value),
                hidden: is_hidden,
                percentage: if is_hidden || visible_total == 0 {
                    0.0
                } else {
                    value as f64 / visible_total as f64 * 100.0
                },
            }
        })
        .collect();
    slices.sort_by(|left, right| right.value.total_cmp(&left.value));
    slices
}

pub fn analytics(snapshot: &Snapshot, query: &AnalyticsQuery, today: NaiveDate) -> AnalyticsView {
    let (start, end) = analytics_range(
        query.range,
        query.custom_start.as_deref(),
        query.custom_end.as_deref(),
        query.month_starts_on_first,
        today,
    );

    let accounts: Vec<_> = snapshot
        .accounts
        .iter()
        .filter(|account| is_selected(&query.accounts, &account.id))
        .collect();
    let transactions: Vec<(_, NaiveDate)> = snapshot
        .transactions
        .iter()
        .filter(|transaction| is_selected(&query.accounts, &transaction.account_id))
        .filter_map(|transaction| parse_date(&transaction.date).map(|date| (transaction, date)))
        .collect();
    let in_range: Vec<_> = transactions
        .iter()
        .filter(|(_, date)| *date >= start && *date <= end)
        .collect();

    let mut expenses: Vec<(String, i64)> = Vec::new();
    let mut income: Vec<(String, i64)> = Vec::new();
    for (transaction, _) in &in_range {
        if is_internal_transfer(transaction) {
            continue;
        }
        let target = match transaction.transaction_type {
            TransactionType::Income => &mut income,
            TransactionType::Expense => &mut expenses,
            TransactionType::Transfer => continue,
        };
        match target
            .iter_mut()
            .find(|(category, _)| *category == transaction.category)
        {
            Some((_, total)) => *total += cents(transaction.amount),
            None => target.push((transaction.category.clone(), cents(transaction.amount))),
        }
    }

    let daily_buckets = days_between(start, end) <= 31;
    let mut buckets: HashMap<String, (i64, i64)> = HashMap::new();
    for (transaction, date) in &in_range {
        if is_internal_transfer(transaction) {
            continue;
        }
        let key = if daily_buckets {
            format_date(*date)
        } else {
            month_key(*date)
        };
        let bucket = buckets.entry(key).or_default();
        if transaction.transaction_type == TransactionType::Income {
            bucket.0 += cents(transaction.amount);
        } else {
            bucket.1 += cents(transaction.amount);
        }
    }
    let bar = |key: String, label: String| {
        let (income, expenses) = buckets.get(&key).copied().unwrap_or_default();
        PeriodBar {
            label,
            income: euros(income),
            expenses: euros(expenses),
        }
    };
    let income_vs_expenses = if daily_buckets {
        each_day(start, end)
            .map(|day| bar(format_date(day), date_day_month(day)))
            .collect()
    } else {
        each_month(start, end)
            .map(|month| bar(month_key(month), month_year(month)))
            .collect()
    };

    let mut balances: Vec<i64> = accounts.iter().map(|account| cents(account.initial_balance)).collect();
    let account_index: HashMap<&str, usize> = accounts
        .iter()
        .enumerate()
        .map(|(index, account)| (account.id.as_str(), index))
        .collect();
    let mut by_day: HashMap<NaiveDate, Vec<&crate::models::Transaction>> = HashMap::new();
    for (transaction, date) in &transactions {
        let Some(index) = account_index.get(transaction.account_id.as_str()) else {
            continue;
        };
        if *date < start {
            balances[*index] += signed_cents(transaction);
        } else if *date <= end {
            by_day.entry(*date).or_default().push(transaction);
        }
    }

    let mut history = BalanceHistory {
        dates: Vec::new(),
        labels: Vec::new(),
        full_labels: Vec::new(),
        series: accounts
            .iter()
            .enumerate()
            .map(|(index, account)| ChartSeries {
                id: account.id.clone(),
                name: account.name.clone(),
                color: if account.color.is_empty() {
                    FALLBACK_SERIES_COLORS[index % FALLBACK_SERIES_COLORS.len()].to_string()
                } else {
                    account.color.clone()
                },
                values: Vec::new(),
            })
            .collect(),
    };
    for day in each_day(start, end) {
        for transaction in by_day.get(&day).into_iter().flatten() {
            if let Some(index) = account_index.get(transaction.account_id.as_str()) {
                balances[*index] += signed_cents(transaction);
            }
        }
        history.dates.push(format_date(day));
        history.labels.push(date_day_month(day));
        history.full_labels.push(date_long(day));
        for (series, balance) in history.series.iter_mut().zip(&balances) {
            series.values.push(euros(*balance));
        }
    }

    AnalyticsView {
        start_date: format_date(start),
        end_date: format_date(end),
        balance_history: history,
        expenses_by_category: category_slices(snapshot, expenses, &query.hidden_expense_categories),
        income_by_category: category_slices(snapshot, income, &query.hidden_income_categories),
        income_vs_expenses,
        daily_buckets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Account, Transaction};

    fn date(value: &str) -> NaiveDate {
        parse_date(value).unwrap()
    }

    #[test]
    fn ranges_align_on_month_start_when_requested() {
        let today = date("2026-09-15");
        assert_eq!(
            analytics_range(TimeRange::Month, None, None, true, today),
            (date("2026-09-01"), today)
        );
        assert_eq!(
            analytics_range(TimeRange::ThreeMonths, None, None, false, today),
            (date("2026-06-15"), today)
        );
        assert_eq!(
            analytics_range(TimeRange::Week, None, None, true, today),
            (date("2026-09-01"), today)
        );
        assert_eq!(
            analytics_range(TimeRange::Custom, Some("2026-09-10"), Some("2026-08-01"), true, today),
            (date("2026-08-01"), date("2026-09-10"))
        );
    }

    #[test]
    fn balance_history_and_buckets() {
        let tx = |id: &str, day: &str, kind, amount: f64, category: &str| Transaction {
            id: id.into(),
            date: day.into(),
            account_id: "a1".into(),
            transaction_type: kind,
            amount,
            category: category.into(),
            description: String::new(),
            checked: false,
            is_transfer: false,
            linked_transaction_id: None,
        };
        let snapshot = Snapshot {
            accounts: vec![Account {
                id: "a1".into(),
                name: "Courant".into(),
                account_type: "Courant".into(),
                initial_balance: 100.0,
                color: String::new(),
                icon: "Wallet".into(),
            }],
            transactions: vec![
                tx("t3", "2026-09-03", TransactionType::Expense, 30.0, "5"),
                tx("t2", "2026-09-02", TransactionType::Income, 500.0, "21"),
                tx("t1", "2026-08-20", TransactionType::Expense, 20.0, "5"),
            ],
            ..Snapshot::default()
        };

        let view = analytics(
            &snapshot,
            &AnalyticsQuery {
                range: TimeRange::Month,
                month_starts_on_first: true,
                ..AnalyticsQuery::default()
            },
            date("2026-09-03"),
        );
        let series = &view.balance_history.series[0];
        assert_eq!(series.color, FALLBACK_SERIES_COLORS[0]);
        assert_eq!(series.values, vec![80.0, 580.0, 550.0]);
        assert!(view.daily_buckets);
        assert_eq!(view.income_vs_expenses.len(), 3);
        assert_eq!(view.income_vs_expenses[2].expenses, 30.0);
        assert_eq!(view.expenses_by_category[0].value, 30.0);
        assert_eq!(view.income_by_category[0].percentage, 100.0);
    }
}
