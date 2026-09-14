//! Calculs de dates. Les dates métier sont des chaînes `YYYY-MM-DD` sans fuseau.
//!
//! Correction par rapport à 1.x : l'ajout de mois est toujours plafonné à la fin du mois
//! (31 janvier + 1 mois = 28/29 février), là où `Date.setMonth` débordait sur mars.

use crate::models::Periodicity;
use chrono::{Datelike, Duration, Local, Months, NaiveDate};

pub const DATE_FORMAT: &str = "%Y-%m-%d";

/// Accepte `YYYY-MM-DD` éventuellement suivi d'une heure ISO.
pub fn parse_date(value: &str) -> Option<NaiveDate> {
    let value = value.trim();
    let prefix = value.get(0..10)?;
    NaiveDate::parse_from_str(prefix, DATE_FORMAT).ok()
}

pub fn format_date(date: NaiveDate) -> String {
    date.format(DATE_FORMAT).to_string()
}

pub fn today_local() -> NaiveDate {
    Local::now().date_naive()
}

pub fn add_days(date: NaiveDate, days: i64) -> NaiveDate {
    date.checked_add_signed(Duration::days(days)).unwrap_or(date)
}

pub fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    if months >= 0 {
        date.checked_add_months(Months::new(months as u32))
    } else {
        date.checked_sub_months(Months::new(months.unsigned_abs()))
    }
    .unwrap_or(date)
}

pub fn start_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

pub fn end_of_month(date: NaiveDate) -> NaiveDate {
    add_days(add_months(start_of_month(date), 1), -1)
}

pub fn days_in_month(date: NaiveDate) -> u32 {
    end_of_month(date).day()
}

pub fn same_month(left: NaiveDate, right: NaiveDate) -> bool {
    left.year() == right.year() && left.month() == right.month()
}

/// Nombre de jours calendaires de `from` à `to` (négatif si `to` est avant).
pub fn days_between(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days()
}

/// Clé `YYYY-MM`.
pub fn month_key(date: NaiveDate) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

/// Index absolu du mois (année × 12 + mois), pour détecter des mois consécutifs.
pub fn month_index(date: NaiveDate) -> i32 {
    date.year() * 12 + date.month0() as i32
}

/// Prochaine occurrence d'une échéance, `None` pour une occurrence unique.
pub fn next_occurrence(date: NaiveDate, frequency: Periodicity) -> Option<NaiveDate> {
    Some(match frequency {
        Periodicity::Once => return None,
        Periodicity::Daily => add_days(date, 1),
        Periodicity::Weekly => add_days(date, 7),
        Periodicity::Biweekly => add_days(date, 14),
        Periodicity::Bimonthly => add_days(date, 15),
        Periodicity::Fourweekly => add_days(date, 28),
        Periodicity::Monthly => add_months(date, 1),
        Periodicity::Bimestrial => add_months(date, 2),
        Periodicity::Quarterly => add_months(date, 3),
        Periodicity::Fourmonthly => add_months(date, 4),
        Periodicity::Semiannual => add_months(date, 6),
        Periodicity::Annual => add_months(date, 12),
        Periodicity::Biennial => add_months(date, 24),
    })
}

/// Itère les jours de `start` à `end` inclus.
pub fn each_day(start: NaiveDate, end: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    let mut current = Some(start);
    std::iter::from_fn(move || {
        let day = current.filter(|day| *day <= end)?;
        current = day.succ_opt();
        Some(day)
    })
}

/// Itère le premier jour de chaque mois de `start` à `end` inclus.
pub fn each_month(start: NaiveDate, end: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    let mut current = Some(start_of_month(start));
    let last = start_of_month(end);
    std::iter::from_fn(move || {
        let month = current.filter(|month| *month <= last)?;
        current = Some(add_months(month, 1));
        Some(month)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        parse_date(value).unwrap()
    }

    #[test]
    fn month_addition_clamps_to_month_end() {
        assert_eq!(add_months(date("2026-01-31"), 1), date("2026-02-28"));
        assert_eq!(add_months(date("2028-02-29"), 12), date("2029-02-28"));
        assert_eq!(add_months(date("2026-03-31"), -1), date("2026-02-28"));
    }

    #[test]
    fn next_occurrence_follows_each_frequency() {
        let start = date("2026-01-31");
        assert_eq!(next_occurrence(start, Periodicity::Once), None);
        assert_eq!(next_occurrence(start, Periodicity::Bimonthly), Some(date("2026-02-15")));
        assert_eq!(next_occurrence(start, Periodicity::Monthly), Some(date("2026-02-28")));
        assert_eq!(next_occurrence(start, Periodicity::Biennial), Some(date("2028-01-31")));
    }

    #[test]
    fn parses_iso_datetime_prefix() {
        assert_eq!(parse_date("2026-05-18T12:00:00.000Z"), Some(date("2026-05-18")));
        assert_eq!(parse_date("18/05/2026"), None);
    }

    #[test]
    fn iterates_days_and_months() {
        assert_eq!(each_day(date("2026-02-27"), date("2026-03-01")).count(), 3);
        let months: Vec<_> = each_month(date("2026-11-15"), date("2027-01-02"))
            .map(month_key)
            .collect();
        assert_eq!(months, vec!["2026-11", "2026-12", "2027-01"]);
    }
}
