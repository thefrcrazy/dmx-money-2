//! Formats français (dates date-fns `fr`, montants `Intl.NumberFormat('fr-FR')`).
//!
//! Les interfaces formatent avec leurs API natives ; ces fonctions servent aux libellés
//! calculés par le noyau (axes de graphiques, groupes du journal) et à la recherche plein texte.

use chrono::{Datelike, NaiveDate};

const MONTHS_SHORT: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc.",
];

const MONTHS_LONG: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

const WEEKDAYS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

/// Espace fine insécable utilisée par ICU comme séparateur de milliers en `fr-FR`.
pub const GROUP_SEPARATOR: char = '\u{202f}';
/// Espace insécable placée avant le symbole monétaire.
pub const CURRENCY_SPACE: char = '\u{a0}';

fn month_short(date: NaiveDate) -> &'static str {
    MONTHS_SHORT[date.month0() as usize]
}

fn month_long(date: NaiveDate) -> &'static str {
    MONTHS_LONG[date.month0() as usize]
}

/// `dd/MM/yyyy`
pub fn date_numeric(date: NaiveDate) -> String {
    format!("{:02}/{:02}/{}", date.day(), date.month(), date.year())
}

/// `dd MMM`, ex. « 05 mai »
pub fn date_day_month(date: NaiveDate) -> String {
    format!("{:02} {}", date.day(), month_short(date))
}

/// `d MMM`, ex. « 5 mai »
pub fn date_day_month_compact(date: NaiveDate) -> String {
    format!("{} {}", date.day(), month_short(date))
}

/// `dd MMM yyyy`
pub fn date_day_month_year(date: NaiveDate) -> String {
    format!("{:02} {} {}", date.day(), month_short(date), date.year())
}

/// `d MMMM yyyy`, ex. « 5 mai 2026 »
pub fn date_long(date: NaiveDate) -> String {
    format!("{} {} {}", date.day(), month_long(date), date.year())
}

/// `MMM yyyy`
pub fn month_year(date: NaiveDate) -> String {
    format!("{} {}", month_short(date), date.year())
}

/// `MMMM yyyy`
pub fn month_year_long(date: NaiveDate) -> String {
    format!("{} {}", month_long(date), date.year())
}

/// `EEEE d MMM`, ex. « lundi 5 mai »
pub fn weekday_day_month(date: NaiveDate) -> String {
    format!(
        "{} {} {}",
        WEEKDAYS[date.weekday().num_days_from_monday() as usize],
        date.day(),
        month_short(date)
    )
}

/// Représentation d'un nombre comme `String(number)` en JavaScript.
pub fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    format!("{value}")
}

/// Arrondit la représentation décimale la plus courte (comme ICU, arrondi au plus proche).
fn rounded_digits(value: f64, max_fraction: usize) -> (String, String) {
    let repr = format!("{}", value.abs());
    let (integer, fraction) = match repr.split_once('.') {
        Some((integer, fraction)) => (integer.to_string(), fraction.to_string()),
        None => (repr, String::new()),
    };

    if fraction.len() <= max_fraction {
        return (integer, fraction);
    }

    let round_up = fraction.as_bytes()[max_fraction] >= b'5';
    let mut digits: Vec<u8> = integer
        .bytes()
        .chain(fraction.bytes().take(max_fraction))
        .map(|digit| digit - b'0')
        .collect();

    if round_up {
        let mut index = digits.len();
        loop {
            if index == 0 {
                digits.insert(0, 1);
                break;
            }
            index -= 1;
            if digits[index] == 9 {
                digits[index] = 0;
            } else {
                digits[index] += 1;
                break;
            }
        }
    }

    let split = digits.len() - max_fraction;
    let to_string = |slice: &[u8]| slice.iter().map(|digit| (digit + b'0') as char).collect();
    (to_string(&digits[..split]), to_string(&digits[split..]))
}

/// Nombre au format `fr-FR` avec un nombre de décimales borné.
pub fn number_fr(value: f64, min_fraction: usize, max_fraction: usize) -> String {
    if !value.is_finite() {
        return js_number(value);
    }

    let (integer, mut fraction) = rounded_digits(value, max_fraction);
    while fraction.len() > min_fraction && fraction.ends_with('0') {
        fraction.pop();
    }
    while fraction.len() < min_fraction {
        fraction.push('0');
    }

    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let mut grouped = String::new();
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(GROUP_SEPARATOR);
        }
        grouped.push(digit);
    }

    let is_zero = grouped == "0" && fraction.chars().all(|digit| digit == '0');
    let sign = if value < 0.0 && !is_zero { "-" } else { "" };

    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped},{fraction}")
    }
}

/// Montant en euros, ex. « 1 234,56 € ».
pub fn currency_fr(value: f64) -> String {
    currency_fr_with(value, 2)
}

/// Montant en euros avec un minimum de décimales (maximum 2).
/// Montant arrondi à l'euro, demi vers le haut, comme `Math.round` en 1.x
/// (cartes de la vue d'ensemble et résumé du tray).
pub fn currency_fr_rounded(value: f64) -> String {
    currency_fr_with((value + 0.5).floor(), 0)
}

pub fn currency_fr_with(value: f64, min_fraction: usize) -> String {
    format!("{}{CURRENCY_SPACE}€", number_fr(value, min_fraction, 2))
}

/// Interprète une saisie utilisateur (`12,5`, `12.50`) comme `parseFloat` en 1.x.
pub fn parse_decimal_input(value: &str) -> Option<f64> {
    // Les espaces, y compris les séparateurs de milliers collés ou insécables, sont ignorés.
    let cleaned: String = value.chars().filter(|character| !character.is_whitespace()).collect();
    let normalized = cleaned.replacen(',', ".", 1);
    let mut end = 0;
    let mut seen_digit = false;
    let mut seen_dot = false;
    for (index, character) in normalized.char_indices() {
        match character {
            '+' | '-' if index == 0 => end = index + 1,
            '0'..='9' => {
                seen_digit = true;
                end = index + 1;
            }
            '.' if !seen_dot => {
                seen_dot = true;
                end = index + 1;
            }
            _ => break,
        }
    }
    if !seen_digit {
        return None;
    }
    normalized[..end]
        .trim_end_matches('.')
        .parse::<f64>()
        .ok()
        .filter(|amount| amount.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn formats_currency_like_intl() {
        assert_eq!(currency_fr(1234.56), "1\u{202f}234,56\u{a0}€");
        assert_eq!(currency_fr(-5.0), "-5,00\u{a0}€");
        assert_eq!(currency_fr(1.005), "1,01\u{a0}€");
        assert_eq!(currency_fr_rounded(1234.49), "1\u{202f}234\u{a0}€");
        // Demi vers le haut, comme `Math.round` en 1.x (y compris sur les négatifs).
        assert_eq!(currency_fr_rounded(1234.5), "1\u{202f}235\u{a0}€");
        assert_eq!(currency_fr_rounded(-1234.5), "-1\u{202f}234\u{a0}€");
        assert_eq!(currency_fr_rounded(0.4), "0\u{a0}€");
        assert_eq!(currency_fr_with(1200.0, 0), "1\u{202f}200\u{a0}€");
        assert_eq!(currency_fr_with(1200.5, 0), "1\u{202f}200,5\u{a0}€");
        assert_eq!(currency_fr(999.999), "1\u{202f}000,00\u{a0}€");
    }

    #[test]
    fn formats_dates_like_date_fns_fr() {
        assert_eq!(date_day_month(date("2026-05-05")), "05 mai");
        assert_eq!(date_day_month_year(date("2026-02-18")), "18 févr. 2026");
        assert_eq!(date_long(date("2026-08-01")), "1 août 2026");
        assert_eq!(weekday_day_month(date("2026-09-14")), "lundi 14 sept.");
        assert_eq!(date_numeric(date("2026-01-09")), "09/01/2026");
    }

    #[test]
    fn parses_decimal_input_like_parse_float() {
        assert_eq!(parse_decimal_input("12,5"), Some(12.5));
        assert_eq!(parse_decimal_input(" 42.10 €"), Some(42.1));
        assert_eq!(parse_decimal_input("abc"), None);
        assert_eq!(parse_decimal_input("1 200,50"), Some(1200.5));
        assert_eq!(parse_decimal_input("1\u{202f}200,50"), Some(1200.5));
    }

    #[test]
    fn js_numbers_drop_trailing_zero() {
        assert_eq!(js_number(1234.0), "1234");
        assert_eq!(js_number(12.5), "12.5");
    }
}
