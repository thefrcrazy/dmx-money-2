//! Formats fr-FR : tous délégués au noyau, pour rester identiques sur les trois plateformes.

use dmx_core::models::TransactionType;
use dmx_core::{dates, format};

pub fn money(amount: f64) -> String {
    format::currency_fr(amount)
}

/// Montant arrondi à l'euro, comme les cartes de la vue d'ensemble.
pub fn rounded(amount: f64) -> String {
    format::currency_fr_rounded(amount)
}

pub fn signed(amount: f64, kind: TransactionType) -> String {
    match kind {
        TransactionType::Income => format!("+{}", money(amount)),
        TransactionType::Expense => format!("-{}", money(amount)),
        TransactionType::Transfer => money(amount),
    }
}

pub fn signed_income_only(amount: f64, kind: TransactionType) -> String {
    match kind {
        TransactionType::Income => format!("+{}", money(amount)),
        _ => money(amount),
    }
}

pub fn percent(value: f64) -> String {
    format!("{} %", value.round() as i64)
}

pub fn percent_tight(value: f64) -> String {
    format!("{}%", value.round() as i64)
}

pub fn day_short(date: &str) -> String {
    parse(date).map(format::date_day_month).unwrap_or_default()
}

pub fn day_medium(date: &str) -> String {
    parse(date).map(format::date_day_month_year).unwrap_or_default()
}

pub fn day_numeric(date: &str) -> String {
    parse(date).map(format::date_numeric).unwrap_or_default()
}

pub fn day_long(date: &str) -> String {
    parse(date).map(format::date_long).unwrap_or_default()
}

fn parse(date: &str) -> Option<chrono::NaiveDate> {
    dates::parse_date(date)
}

/// « Aujourd'hui », « Demain », « Dans 3 jours », « En retard de 2 jours ».
pub fn relative(days: i64) -> String {
    match days {
        0 => "Aujourd'hui".to_string(),
        1 => "Demain".to_string(),
        -1 => "En retard d'un jour".to_string(),
        days if days < 0 => format!("En retard de {} jours", -days),
        days => format!("Dans {days} jours"),
    }
}

pub fn plural(count: usize, singular: &str) -> String {
    if count > 1 {
        format!("{count} {singular}s")
    } else {
        format!("{count} {singular}")
    }
}

/// Saisie d'un montant : virgule ou point, espaces ignorés (analyse faite par le noyau).
pub fn parse_amount(text: &str) -> Option<f64> {
    format::parse_decimal_input(text)
}

/// Montant pour un champ de saisie (sans séparateur de milliers).
pub fn amount_input(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        format::js_number(value).replace('.', ",")
    }
}
