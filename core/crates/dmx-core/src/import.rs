//! Imports bancaires CSV, QIF et OFX (port de `utils/importParsers.ts` et des assistants
//! `CsvImportModal`, `QifImportModal`, `OfxImportModal`).

use crate::dates::format_date;
use crate::db::DbPool;
use crate::error::{CoreError, CoreResult, DbContext};
use crate::metrics::cents;
use crate::models::{
    string_enum, Account, Category, Transaction, TransactionType, ACCOUNT_TYPES, DEFAULT_ACCOUNT_COLOR,
    DEFAULT_ACCOUNT_ICON, TRANSFER_CATEGORY_ID,
};
use crate::ops::IMPORTED_CATEGORY_COLOR;
use crate::repo;
use crate::seed::FALLBACK_CATEGORY_ID;
use crate::settings;
use crate::snapshot::Snapshot;
use crate::text::{collapse_whitespace, normalize_search};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedStatementTransaction {
    pub date: String,
    /// Montant signé : négatif pour une dépense.
    pub amount: f64,
    pub description: String,
    pub category: Option<String>,
    #[serde(rename = "bankSource", default)]
    pub bank_source: Option<String>,
    #[serde(rename = "bankTransactionId", default)]
    pub bank_transaction_id: Option<String>,
}

// --- CSV ---

/// Découpe un CSV avec guillemets (séparateurs et retours à la ligne autorisés entre guillemets).
pub fn parse_delimited_rows(content: &str, separator: char, has_header: bool) -> Vec<Vec<String>> {
    scan_delimited_rows(content, separator, has_header, usize::MAX).0
}

/// Scan all rows for an exact count while retaining only the requested preview rows.
fn scan_delimited_rows(content: &str, separator: char, has_header: bool, keep: usize) -> (Vec<Vec<String>>, usize) {
    let mut chars = content.strip_prefix('\u{feff}').unwrap_or(content).chars().peekable();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut count = 0;
    let mut header = has_header;
    let mut in_quotes = false;
    let mut push_row = |row: &mut Vec<String>, cell: &mut String, rows: &mut Vec<Vec<String>>| {
        row.push(cell.trim().to_string());
        cell.clear();
        if row.iter().any(|value| !value.is_empty()) {
            if header {
                header = false;
            } else {
                count += 1;
                if rows.len() < keep {
                    rows.push(std::mem::take(row));
                }
            }
        }
        row.clear();
    };
    while let Some(character) = chars.next() {
        if character == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                cell.push('"');
                chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if character == separator && !in_quotes {
            row.push(cell.trim().to_string());
            cell.clear();
        } else if (character == '\n' || character == '\r') && !in_quotes {
            if character == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            push_row(&mut row, &mut cell, &mut rows);
        } else {
            cell.push(character);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        push_row(&mut row, &mut cell, &mut rows);
    }
    (rows, count)
}

/// Montant bancaire : `1 234,56 €`, `1,234.56`, `-12,5`… Une valeur illisible vaut 0.
pub fn parse_bank_amount(value: &str) -> f64 {
    parse_bank_amount_checked(value).unwrap_or(0.0)
}

fn parse_bank_amount_checked(value: &str) -> Option<f64> {
    let cleaned: String = value
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '€' | '$' | '£'))
        .collect();
    if cleaned.is_empty() {
        return None;
    }

    let normalized = match (cleaned.rfind(','), cleaned.rfind('.')) {
        (Some(comma), Some(dot)) if comma > dot => cleaned.replace('.', "").replacen(',', ".", 1),
        (Some(_), Some(_)) => cleaned.replace(',', ""),
        _ => cleaned.replacen(',', ".", 1),
    };

    normalized
        .parse::<f64>()
        .ok()
        .filter(|amount| crate::metrics::is_valid_money(*amount))
}

fn take_digits(chars: &[char], start: usize, max: usize) -> (String, usize) {
    let digits: String = chars
        .iter()
        .skip(start)
        .take(max)
        .take_while(|character| character.is_ascii_digit())
        .collect();
    let len = digits.len();
    (digits, start + len)
}

/// Date bancaire : `2026-05-18`, `2026/5/18`, `18/05/2026`, `18-05-26`. Aujourd'hui sinon.
pub fn parse_bank_date(value: &str, today: NaiveDate) -> String {
    parse_bank_date_checked(value).unwrap_or_else(|| format_date(today))
}

fn parse_bank_date_checked(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }
    let chars: Vec<char> = raw.chars().collect();
    let is_separator =
        |index: usize, allowed: &[char]| chars.get(index).is_some_and(|character| allowed.contains(character));

    // ^(\d{4})[-/](\d{1,2})[-/](\d{1,2})
    let (year, position) = take_digits(&chars, 0, 4);
    if year.len() == 4 && is_separator(position, &['-', '/']) {
        let (month, position) = take_digits(&chars, position + 1, 2);
        if !month.is_empty() && is_separator(position, &['-', '/']) {
            let (day, _) = take_digits(&chars, position + 1, 2);
            if !day.is_empty() {
                return NaiveDate::from_ymd_opt(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
                    .map(format_date);
            }
        }
    }

    // ^(\d{1,2})[/-](\d{1,2})[/-](\d{2,4})$
    let (day, position) = take_digits(&chars, 0, 2);
    if !day.is_empty() && is_separator(position, &['/', '-']) {
        let (month, position) = take_digits(&chars, position + 1, 2);
        if !month.is_empty() && is_separator(position, &['/', '-']) {
            let (year, end) = take_digits(&chars, position + 1, 4);
            if (2..=4).contains(&year.len()) && end == chars.len() {
                let year = if year.len() == 2 { format!("20{year}") } else { year };
                return NaiveDate::from_ymd_opt(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
                    .map(format_date);
            }
        }
    }

    None
}

// --- QIF ---

pub fn parse_qif_date(value: &str, today: NaiveDate) -> String {
    parse_bank_date(value.replacen('\'', "/", 1).trim(), today)
}

pub fn parse_qif_transactions(content: &str, _today: NaiveDate) -> CoreResult<Vec<ParsedStatementTransaction>> {
    crate::limits::import_size(content)?;
    #[derive(Default)]
    struct Current {
        touched: bool,
        date: Option<String>,
        amount: Option<f64>,
        description: Option<String>,
        category: Option<String>,
    }

    let mut transactions = Vec::new();
    let mut current = Current::default();
    let commit = |current: &mut Current, transactions: &mut Vec<ParsedStatementTransaction>| -> CoreResult<()> {
        if !current.touched {
            return Ok(());
        }
        let taken = std::mem::take(current);
        transactions.push(ParsedStatementTransaction {
            date: taken
                .date
                .ok_or_else(|| CoreError::import("Date QIF manquante ou invalide."))?,
            amount: taken
                .amount
                .ok_or_else(|| CoreError::import("Montant QIF manquant ou invalide."))?,
            description: taken
                .description
                .filter(|description| !description.is_empty())
                .unwrap_or_else(|| "Transaction QIF".to_string()),
            category: taken.category.filter(|category| !category.is_empty()),
            bank_source: None,
            bank_transaction_id: None,
        });
        Ok(())
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('!') {
            continue;
        }
        let mut characters = trimmed.chars();
        let field = characters.next();
        let value = characters.as_str().trim();

        match field {
            Some('^') => commit(&mut current, &mut transactions)?,
            Some('D') => {
                current.touched = true;
                current.date = parse_bank_date_checked(value.replacen('\'', "/", 1).trim());
            }
            Some('T') => {
                current.touched = true;
                current.amount = parse_bank_amount_checked(value);
            }
            Some('P') | Some('M') => {
                current.touched = true;
                current.description = Some(collapse_whitespace(value));
            }
            Some('L') => {
                current.touched = true;
                let value = value.strip_prefix('[').unwrap_or(value);
                let value = value.strip_suffix(']').unwrap_or(value);
                current.category = Some(collapse_whitespace(value));
            }
            _ => {}
        }
    }
    commit(&mut current, &mut transactions)?;

    if transactions.is_empty() {
        return Err(CoreError::import(
            "Aucune transaction valide trouvée dans le fichier QIF.",
        ));
    }
    Ok(transactions)
}

// --- OFX ---

fn decode_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn ofx_tag_value(block: &str, tag: &str) -> String {
    let lower = block.to_ascii_lowercase();
    let needle = format!("<{}>", tag.to_ascii_lowercase());
    let Some(position) = lower.find(&needle) else {
        return String::new();
    };
    let rest = &block[position + needle.len()..];
    let end = rest.find(['<', '\r', '\n']).unwrap_or(rest.len());
    decode_entities(rest[..end].trim())
}

fn parse_ofx_date(value: &str) -> Option<String> {
    NaiveDate::parse_from_str(value.get(..8)?, "%Y%m%d")
        .ok()
        .map(format_date)
}

pub fn parse_ofx_transactions(content: &str, _today: NaiveDate) -> CoreResult<Vec<ParsedStatementTransaction>> {
    crate::limits::import_size(content)?;
    let lower = content.to_ascii_lowercase();
    let marker = "<stmttrn>";
    let starts: Vec<usize> = lower
        .match_indices(marker)
        .map(|(index, _)| index + marker.len())
        .collect();
    if starts.is_empty() {
        return Err(CoreError::import(
            "Aucune transaction trouvée dans le fichier OFX. Le format est peut-être incorrect.",
        ));
    }

    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts
                .get(index + 1)
                .map(|next| next - marker.len())
                .unwrap_or(content.len());
            let block = &content[*start..end];
            let prefix = &lower[..*start];
            let scope_start = prefix
                .rfind("<stmtrs>")
                .into_iter()
                .chain(prefix.rfind("<ccstmtrs>"))
                .max()
                .unwrap_or(0);
            let scope = &content[scope_start..end];
            let account = ofx_tag_value(scope, "ACCTID");
            let fitid = ofx_tag_value(block, "FITID");
            let bank_source = (!account.is_empty() && !fitid.is_empty()).then(|| {
                format!(
                    "ofx:{}",
                    serde_json::to_string(&[
                        ofx_tag_value(content, "FID"),
                        ofx_tag_value(content, "ORG"),
                        ofx_tag_value(scope, "BANKID"),
                        ofx_tag_value(scope, "BRANCHID"),
                        account,
                        ofx_tag_value(scope, "ACCTTYPE"),
                    ])
                    .expect("string array is serializable")
                )
            });
            let bank_transaction_id = bank_source.as_ref().map(|_| fitid);
            let name = collapse_whitespace(&ofx_tag_value(block, "NAME"));
            let memo = collapse_whitespace(&ofx_tag_value(block, "MEMO"));
            let description = if !memo.is_empty() && memo != name {
                collapse_whitespace(
                    &[name.as_str(), memo.as_str()]
                        .iter()
                        .filter(|part| !part.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join(" - "),
                )
            } else if !name.is_empty() {
                name
            } else if !memo.is_empty() {
                memo
            } else {
                "Transaction OFX".to_string()
            };

            Ok(ParsedStatementTransaction {
                date: parse_ofx_date(&ofx_tag_value(block, "DTPOSTED"))
                    .ok_or_else(|| CoreError::import(format!("Date OFX invalide pour l'opération {}.", index + 1)))?,
                amount: parse_bank_amount_checked(&ofx_tag_value(block, "TRNAMT")).ok_or_else(|| {
                    CoreError::import(format!("Montant OFX invalide pour l'opération {}.", index + 1))
                })?,
                description,
                category: None,
                bank_source,
                bank_transaction_id,
            })
        })
        .collect()
}

// --- Doublons ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportTransactionInput {
    pub date: String,
    pub amount: f64,
    pub kind: TransactionType,
    pub description: String,
    pub category: String,
    pub account_id: Option<String>,
    pub checked: bool,
    pub bank_source: Option<String>,
    pub bank_transaction_id: Option<String>,
}

pub fn transaction_fingerprint(
    date: &str,
    account_id: &str,
    kind: TransactionType,
    amount: f64,
    description: &str,
) -> String {
    format!(
        "{date}|{account_id}|{}|{}|{}",
        kind.as_str(),
        cents(amount),
        normalize_search(&collapse_whitespace(description))
    )
}

/// Écarte les transactions déjà présentes (même date, compte, type, montant et libellé normalisé).
pub fn filter_duplicate_transactions(
    incoming: Vec<ImportTransactionInput>,
    existing: &[Transaction],
    fallback_account_id: &str,
) -> (Vec<ImportTransactionInput>, u32) {
    let mut remaining: HashMap<String, usize> = HashMap::new();
    let mut identities: HashSet<(String, String, String)> = HashSet::new();
    let existing_ids: HashSet<&str> = existing.iter().map(|transaction| transaction.id.as_str()).collect();
    for transaction in existing {
        if let (Some(source), Some(id)) = (&transaction.bank_source, &transaction.bank_transaction_id) {
            identities.insert((transaction.account_id.clone(), source.clone(), id.clone()));
        }
        *remaining
            .entry(transaction_fingerprint(
                &transaction.date,
                &transaction.account_id,
                transaction.transaction_type,
                transaction.amount,
                &transaction.description,
            ))
            .or_default() += 1;
    }
    let mut unique = Vec::new();
    let mut duplicates = 0;
    for transaction in incoming {
        let account_id = transaction
            .account_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .unwrap_or(fallback_account_id);
        let duplicate = if let (Some(source), Some(id)) = (&transaction.bank_source, &transaction.bank_transaction_id) {
            // Bank identity wins: two FITIDs with identical values are distinct purchases.
            // A later manual move keeps its stable ID: reimport must preserve that edit too.
            existing_ids.contains(bank_import_id(account_id, Some(source), Some(id)).as_str())
                || !identities.insert((account_id.to_string(), source.clone(), id.clone()))
        } else {
            let key = transaction_fingerprint(
                &transaction.date,
                account_id,
                transaction.kind,
                transaction.amount,
                &transaction.description,
            );
            let count = remaining.entry(key).or_default();
            if *count > 0 {
                *count -= 1;
                true
            } else {
                false
            }
        };
        if duplicate {
            duplicates += 1;
        } else {
            unique.push(transaction);
        }
    }
    (unique, duplicates)
}

// --- Assistant d'import ---

string_enum!(StatementFormat, default = Csv, {
    Csv => "csv",
    Qif => "qif",
    Ofx => "ofx",
});

impl StatementFormat {
    pub fn from_file_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower.ends_with(".csv") {
            Some(StatementFormat::Csv)
        } else if lower.ends_with(".qif") {
            Some(StatementFormat::Qif)
        } else if lower.ends_with(".ofx") {
            Some(StatementFormat::Ofx)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CsvOptions {
    pub separator: char,
    pub has_header: bool,
}

/// Colonnes assignées (index à partir de 0) ; `None` pour ignorer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CsvColumnMapping {
    pub date: Option<u32>,
    pub amount: Option<u32>,
    pub description: Option<u32>,
    pub category: Option<u32>,
}

impl Default for CsvColumnMapping {
    /// Assignation initiale de l'assistant 1.x.
    fn default() -> Self {
        Self {
            date: Some(0),
            amount: Some(1),
            description: Some(3),
            category: None,
        }
    }
}

/// `;` si la première ligne en contient, sinon `,` si présent, sinon `;`.
pub fn detect_csv_separator(content: &str) -> char {
    let first_line = content.split('\n').next().unwrap_or_default();
    if first_line.contains(';') {
        ';'
    } else if first_line.contains(',') {
        ','
    } else {
        ';'
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CsvPreview {
    pub rows: Vec<Vec<String>>,
    pub column_count: u32,
    pub row_count: u32,
}

pub fn preview_csv(content: &str, options: CsvOptions) -> CoreResult<CsvPreview> {
    crate::limits::import_size(content)?;
    let (preview, row_count) = scan_delimited_rows(content, options.separator, options.has_header, 5);
    if preview.is_empty() {
        return Err(CoreError::import(if options.has_header {
            "Le fichier ne contient que l'en-tête."
        } else {
            "Le fichier est vide."
        }));
    }
    Ok(CsvPreview {
        column_count: preview.iter().map(Vec::len).max().unwrap_or(0) as u32,
        row_count: row_count as u32,
        rows: preview,
    })
}

pub fn parse_statement(
    format: StatementFormat,
    content: &str,
    csv: Option<(CsvOptions, CsvColumnMapping)>,
    today: NaiveDate,
) -> CoreResult<Vec<ParsedStatementTransaction>> {
    crate::limits::import_size(content)?;
    match format {
        StatementFormat::Qif => parse_qif_transactions(content, today),
        StatementFormat::Ofx => parse_ofx_transactions(content, today),
        StatementFormat::Csv => {
            let (options, mapping) = csv.unwrap_or((
                CsvOptions {
                    separator: detect_csv_separator(content),
                    has_header: false,
                },
                CsvColumnMapping::default(),
            ));
            let (Some(date_column), Some(amount_column)) = (mapping.date, mapping.amount) else {
                return Err(CoreError::validation(
                    "Vous devez assigner au moins la Date et le Montant",
                ));
            };
            let rows = parse_delimited_rows(content, options.separator, options.has_header);
            if rows.is_empty() {
                return Err(CoreError::import("Aucune donnée à importer"));
            }
            let cell = |row: &Vec<String>, column: Option<u32>| {
                column
                    .and_then(|column| row.get(column as usize))
                    .map(String::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            rows.iter()
                .enumerate()
                .map(|(index, row)| {
                    let description = cell(row, mapping.description);
                    let category = cell(row, mapping.category);
                    Ok(ParsedStatementTransaction {
                        date: parse_bank_date_checked(&cell(row, Some(date_column)))
                            .ok_or_else(|| CoreError::import(format!("Date CSV invalide à la ligne {}.", index + 1)))?,
                        amount: parse_bank_amount_checked(&cell(row, Some(amount_column))).ok_or_else(|| {
                            CoreError::import(format!("Montant CSV invalide à la ligne {}.", index + 1))
                        })?,
                        description: if description.is_empty() {
                            "Import CSV".to_string()
                        } else {
                            description
                        },
                        category: (!category.is_empty()).then_some(category),
                        bank_source: None,
                        bank_transaction_id: None,
                    })
                })
                .collect()
        }
    }
}

/// Catégories distinctes du fichier, triées.
pub fn source_categories(transactions: &[ParsedStatementTransaction]) -> Vec<String> {
    transactions
        .iter()
        .filter_map(|transaction| transaction.category.clone())
        .filter(|category| !category.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryMatch {
    pub source: String,
    /// Catégorie existante ; `None` pour créer une catégorie portant le nom source.
    pub category_id: Option<String>,
}

/// Associe chaque catégorie du fichier à une catégorie de même nom (casse ignorée).
pub fn suggest_category_mapping(snapshot: &Snapshot, sources: &[String]) -> Vec<CategoryMatch> {
    sources
        .iter()
        .map(|source| CategoryMatch {
            source: source.clone(),
            category_id: snapshot
                .categories
                .iter()
                .find(|category| category.name.to_lowercase() == source.to_lowercase())
                .map(|category| category.id.clone()),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportTarget {
    Existing(String),
    New {
        name: String,
        account_type: String,
        /// Solde final du relevé, pour déduire le solde initial.
        final_balance: Option<f64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatementImportRequest {
    pub transactions: Vec<ParsedStatementTransaction>,
    pub target: ImportTarget,
    pub category_mapping: Vec<CategoryMatch>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatementImportResult {
    pub imported: u32,
    pub duplicates: u32,
    pub account_id: String,
    pub created_categories: u32,
}

/// Solde initial d'un nouveau compte pour que le solde final corresponde au relevé.
pub fn initial_balance_from_final(transactions: &[ParsedStatementTransaction], final_balance: f64) -> f64 {
    let mut identities = HashSet::new();
    let net: i128 = transactions
        .iter()
        .filter(
            |transaction| match (&transaction.bank_source, &transaction.bank_transaction_id) {
                (Some(source), Some(id)) => identities.insert((source, id)),
                _ => true,
            },
        )
        .map(|transaction| i128::from(cents(transaction.amount)))
        .sum();
    (i128::from(cents(final_balance)) - net) as f64 / 100.0
}

/// Stable across desktop/mobile and independent imports of the same bank record.
pub fn bank_import_id(account: &str, source: Option<&str>, id: Option<&str>) -> String {
    match (source, id) {
        (Some(source), Some(id)) => uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            serde_json::to_string(&[account, source, id])
                .expect("string array serializable")
                .as_bytes(),
        )
        .to_string(),
        _ => uuid::Uuid::new_v4().to_string(),
    }
}

pub async fn import_statement(pool: &DbPool, request: StatementImportRequest) -> CoreResult<StatementImportResult> {
    if request.transactions.is_empty() {
        return Err(CoreError::import("Aucune transaction à importer"));
    }

    for (index, transaction) in request.transactions.iter().enumerate() {
        if !crate::metrics::is_valid_money(transaction.amount) {
            return Err(CoreError::validation(format!(
                "Montant invalide à la ligne {}.",
                index + 1
            )));
        }
        if transaction.date.len() != 10 || crate::dates::parse_date(&transaction.date).is_none() {
            return Err(CoreError::validation(format!(
                "Date invalide à la ligne {}.",
                index + 1
            )));
        }
    }
    if let ImportTarget::New {
        final_balance: Some(balance),
        ..
    } = &request.target
    {
        if !crate::metrics::is_valid_money(*balance) {
            return Err(CoreError::validation("Solde final invalide."));
        }
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.ctx("import bancaire")?;
    let accent = settings::load_app_settings(&mut *tx)
        .await?
        .accent_color()
        .map(str::to_string);
    let existing = repo::list_transactions(&mut *tx).await?;
    let categories = repo::list_categories(&mut *tx).await?;

    let account_id = match &request.target {
        ImportTarget::Existing(account_id) => {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE id = $1")
                .bind(account_id)
                .fetch_one(&mut *tx)
                .await
                .ctx("import bancaire")?;
            if count == 0 {
                return Err(CoreError::not_found("Compte introuvable."));
            }
            account_id.clone()
        }
        ImportTarget::New {
            name,
            account_type,
            final_balance,
        } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(CoreError::validation("Nom du compte requis"));
            }
            let account = Account {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.to_string(),
                account_type: if account_type.trim().is_empty() {
                    ACCOUNT_TYPES[0].to_string()
                } else {
                    account_type.trim().to_string()
                },
                initial_balance: final_balance
                    .filter(|balance| balance.is_finite())
                    .map(|balance| initial_balance_from_final(&request.transactions, balance))
                    .unwrap_or(0.0),
                color: accent.unwrap_or_else(|| DEFAULT_ACCOUNT_COLOR.to_string()),
                icon: DEFAULT_ACCOUNT_ICON.to_string(),
            };
            repo::insert_account(&mut tx, &account).await?;
            account.id
        }
    };

    let mut created_categories = 0;
    let mut resolved: Vec<(String, String)> = Vec::new();
    for entry in &request.category_mapping {
        let category_id = match &entry.category_id {
            Some(category_id) => {
                if category_id == TRANSFER_CATEGORY_ID || !categories.iter().any(|category| category.id == *category_id)
                {
                    return Err(CoreError::validation(
                        "Catégorie d'import introuvable ou réservée aux virements.",
                    ));
                }
                category_id.clone()
            }
            None => {
                let category = Category {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: entry.source.clone(),
                    icon: "Tag".to_string(),
                    color: IMPORTED_CATEGORY_COLOR.to_string(),
                };
                repo::insert_category(&mut tx, &category).await?;
                created_categories += 1;
                category.id
            }
        };
        resolved.push((entry.source.clone(), category_id));
    }

    let default_category = categories
        .iter()
        .find(|category| category.id == FALLBACK_CATEGORY_ID)
        .or_else(|| categories.iter().find(|category| category.id != TRANSFER_CATEGORY_ID))
        .map(|category| category.id.clone())
        .unwrap_or_else(|| "uncategorized".to_string());

    let prepared: Vec<ImportTransactionInput> = request
        .transactions
        .iter()
        .map(|transaction| ImportTransactionInput {
            date: transaction.date.clone(),
            amount: transaction.amount.abs(),
            kind: if transaction.amount >= 0.0 {
                TransactionType::Income
            } else {
                TransactionType::Expense
            },
            description: transaction.description.clone(),
            category: transaction
                .category
                .as_ref()
                .and_then(|source| resolved.iter().find(|(name, _)| name == source))
                .map(|(_, id)| id.clone())
                .unwrap_or_else(|| default_category.clone()),
            account_id: Some(account_id.clone()),
            checked: true,
            bank_source: transaction.bank_source.clone(),
            bank_transaction_id: transaction.bank_transaction_id.clone(),
        })
        .collect();

    let (unique, duplicates) = filter_duplicate_transactions(prepared, &existing, &account_id);
    for input in &unique {
        repo::insert_transaction(
            &mut tx,
            &Transaction {
                id: bank_import_id(
                    &account_id,
                    input.bank_source.as_deref(),
                    input.bank_transaction_id.as_deref(),
                ),
                date: input.date.clone(),
                account_id: account_id.clone(),
                transaction_type: input.kind,
                amount: input.amount,
                category: input.category.clone(),
                description: input.description.clone(),
                checked: input.checked,
                is_transfer: false,
                linked_transaction_id: None,
                bank_source: input.bank_source.clone(),
                bank_transaction_id: input.bank_transaction_id.clone(),
            },
        )
        .await?;
    }

    tx.commit().await.ctx("import bancaire")?;
    Ok(StatementImportResult {
        imported: unique.len() as u32,
        duplicates,
        account_id,
        created_categories,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_pool;
    use crate::seed::ensure_initial_data;

    fn today() -> NaiveDate {
        crate::dates::parse_date("2026-09-15").unwrap()
    }

    #[test]
    fn parses_quoted_csv_rows_with_separators_inside_cells() {
        let rows = parse_delimited_rows(
            "Date;Montant;Libelle\n\"18/05/2026\";\"-1 234,56\";\"Achat; carte\"\n",
            ';',
            true,
        );
        assert_eq!(rows, vec![vec!["18/05/2026", "-1 234,56", "Achat; carte"]]);

        let rows = parse_delimited_rows("\u{feff}a,\"b \"\"c\"\"\"\r\n,\r\nd,\"e\nf\"", ',', false);
        assert_eq!(rows, vec![vec!["a", "b \"c\""], vec!["d", "e\nf"]]);
    }

    #[test]
    fn normalizes_common_bank_amount_and_date_formats() {
        assert_eq!(parse_bank_amount("1 234,56 €"), 1234.56);
        assert_eq!(parse_bank_amount("1,234.56"), 1234.56);
        assert_eq!(parse_bank_amount("-12,5"), -12.5);
        assert_eq!(parse_bank_amount("abc"), 0.0);
        assert_eq!(parse_bank_date("18/05/26", today()), "2026-05-18");
        assert_eq!(parse_bank_date("2026-05-18", today()), "2026-05-18");
        assert_eq!(parse_bank_date("2026/5/8 10:00", today()), "2026-05-08");
        assert_eq!(parse_bank_date("5-8-2026", today()), "2026-08-05");
        assert_eq!(parse_bank_date("n/a", today()), "2026-09-15");
    }

    #[test]
    fn statement_parsers_reject_invalid_amounts_and_dates() {
        for content in ["31/02/2026;-12", "18/05/2026;not-money", "18/05/2026;NaN"] {
            assert!(parse_statement(StatementFormat::Csv, content, None, today()).is_err());
        }
        assert!(parse_qif_transactions("!Type:Bank\nD31/02'26\nT-12\n^", today()).is_err());
        assert!(parse_qif_transactions("!Type:Bank\nD18/05'26\nTbad\n^", today()).is_err());
        assert!(parse_ofx_transactions("<STMTTRN><DTPOSTED>20260231<TRNAMT>-12", today()).is_err());
        assert!(parse_ofx_transactions("<STMTTRN><DTPOSTED>20260518<TRNAMT>bad", today()).is_err());
    }

    #[tokio::test]
    async fn identical_purchases_keep_multiplicity_and_final_balance() {
        let pool = open_memory_pool().await.unwrap();
        ensure_initial_data(&pool).await.unwrap();
        let transaction = ParsedStatementTransaction {
            date: "2026-09-15".into(),
            amount: -20.0,
            description: "Café".into(),
            category: None,
            bank_source: None,
            bank_transaction_id: None,
        };
        let result = import_statement(
            &pool,
            StatementImportRequest {
                transactions: vec![transaction.clone(), transaction],
                target: ImportTarget::New {
                    name: "Compte".into(),
                    account_type: "Courant".into(),
                    final_balance: Some(100.0),
                },
                category_mapping: vec![],
            },
        )
        .await
        .unwrap();
        assert_eq!((result.imported, result.duplicates), (2, 0));
        let snapshot = crate::snapshot::load(&pool).await.unwrap();
        assert_eq!(snapshot.accounts[0].initial_balance, 140.0);
        assert_eq!(crate::metrics::balance_summary(&snapshot, &[]).current_balance, 100.0);
    }

    #[tokio::test]
    async fn invalid_import_request_leaves_the_database_unchanged() {
        let pool = open_memory_pool().await.unwrap();
        ensure_initial_data(&pool).await.unwrap();
        for (date, amount) in [
            ("2026-02-31", -12.0),
            ("2026-09-15", f64::NAN),
            ("2026-09-15", f64::INFINITY),
        ] {
            let result = import_statement(
                &pool,
                StatementImportRequest {
                    transactions: vec![ParsedStatementTransaction {
                        date: date.into(),
                        amount,
                        description: "Test".into(),
                        category: None,
                        bank_source: None,
                        bank_transaction_id: None,
                    }],
                    target: ImportTarget::New {
                        name: "Compte".into(),
                        account_type: "Courant".into(),
                        final_balance: None,
                    },
                    category_mapping: vec![],
                },
            )
            .await;
            assert!(result.is_err());
            assert!(repo::list_accounts(&pool).await.unwrap().is_empty());
            assert!(repo::list_transactions(&pool).await.unwrap().is_empty());
        }
    }

    #[test]
    fn parses_qif_transactions() {
        let transactions =
            parse_qif_transactions("!Type:Bank\nD18/05'26\nT-12,34\nPCafe\nL[Restaurants]\n^\n", today()).unwrap();
        assert_eq!(
            transactions,
            vec![ParsedStatementTransaction {
                date: "2026-05-18".into(),
                amount: -12.34,
                description: "Cafe".into(),
                category: Some("Restaurants".into()),
                bank_source: None,
                bank_transaction_id: None,
            }]
        );
        assert!(parse_qif_transactions("!Type:Bank\n", today()).is_err());
    }

    #[test]
    fn parses_ofx_sgml_transactions() {
        let transactions = parse_ofx_transactions(
            "<OFX><STMTTRN><DTPOSTED>20260518120000<TRNAMT>-42.50<NAME>SHOP &amp; CO<MEMO>Card</STMTTRN></OFX>",
            today(),
        )
        .unwrap();
        assert_eq!(
            transactions,
            vec![ParsedStatementTransaction {
                date: "2026-05-18".into(),
                amount: -42.5,
                description: "SHOP & CO - Card".into(),
                category: None,
                bank_source: None,
                bank_transaction_id: None,
            }]
        );
    }

    #[test]
    fn filters_duplicate_imports_with_accent_insensitive_descriptions() {
        let existing = vec![Transaction {
            id: "existing".into(),
            date: "2026-05-18".into(),
            account_id: "account-1".into(),
            transaction_type: TransactionType::Expense,
            amount: 12.34,
            category: "food".into(),
            description: "Café du centre".into(),
            checked: true,
            is_transfer: false,
            linked_transaction_id: None,
            bank_source: None,
            bank_transaction_id: None,
        }];
        let incoming = ImportTransactionInput {
            date: "2026-05-18".into(),
            amount: 12.34,
            kind: TransactionType::Expense,
            description: "Cafe  du centre".into(),
            category: "food".into(),
            account_id: None,
            checked: true,
            bank_source: None,
            bank_transaction_id: None,
        };
        let (unique, duplicates) =
            filter_duplicate_transactions(vec![incoming.clone(), incoming], &existing, "account-1");
        assert_eq!(unique.len(), 1);
        assert_eq!(duplicates, 1);
    }

    #[tokio::test]
    async fn imports_into_a_new_account_with_final_balance_and_categories() {
        let pool = open_memory_pool().await.unwrap();
        ensure_initial_data(&pool).await.unwrap();

        let content = "Date;Montant;Catégorie;Libellé\n01/09/2026;-50,00;Courses;Supermarché\n02/09/2026;2000;Salaire;Paie\n03/09/2026;-20;;Divers\n";
        let options = CsvOptions {
            separator: detect_csv_separator(content),
            has_header: true,
        };
        let mapping = CsvColumnMapping {
            date: Some(0),
            amount: Some(1),
            description: Some(3),
            category: Some(2),
        };
        let transactions = parse_statement(StatementFormat::Csv, content, Some((options, mapping)), today()).unwrap();
        let sources = source_categories(&transactions);
        assert_eq!(sources, vec!["Courses", "Salaire"]);

        let snapshot = crate::snapshot::load(&pool).await.unwrap();
        let mapping = suggest_category_mapping(&snapshot, &sources);
        assert_eq!(mapping[0].category_id, None);
        assert_eq!(mapping[1].category_id.as_deref(), Some("21"));

        let request = StatementImportRequest {
            transactions: transactions.clone(),
            target: ImportTarget::New {
                name: "Banque".into(),
                account_type: "Courant".into(),
                final_balance: Some(3000.0),
            },
            category_mapping: mapping,
        };
        let result = import_statement(&pool, request.clone()).await.unwrap();
        assert_eq!(
            (result.imported, result.duplicates, result.created_categories),
            (3, 0, 1)
        );

        let snapshot = crate::snapshot::load(&pool).await.unwrap();
        let account = snapshot.account(&result.account_id).unwrap();
        assert_eq!(account.initial_balance, 1070.0);
        let divers = snapshot
            .transactions
            .iter()
            .find(|transaction| transaction.description == "Divers")
            .unwrap();
        assert_eq!(divers.category, FALLBACK_CATEGORY_ID);

        let again = import_statement(
            &pool,
            StatementImportRequest {
                target: ImportTarget::Existing(result.account_id.clone()),
                category_mapping: vec![],
                ..request
            },
        )
        .await
        .unwrap();
        assert_eq!((again.imported, again.duplicates), (0, 3));
    }
}
