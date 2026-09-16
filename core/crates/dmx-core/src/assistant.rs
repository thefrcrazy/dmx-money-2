//! Assistant : comprend une demande courte en français et y répond avec les chiffres du noyau.
//!
//! Deux usages, une seule logique :
//! * les intentions Siri (macOS) appellent les fonctions structurées ;
//! * la PWA envoie du texte libre à `/api/assistant`, qui passe par `interpret`.
//!
//! Rien n'est calculé ailleurs : les montants viennent des mêmes vues que l'interface, et
//! l'analyse du texte est déterministe (aucun modèle). Un hôte capable de faire mieux (le modèle
//! sur l'appareil de macOS, par exemple) peut d'abord traduire la phrase en intention, puis
//! demander son exécution — le résultat reste produit ici.

use crate::dates::format_date;
use crate::format::{currency_fr, month_year_long};
use crate::models::{ScheduledDueRange, TransactionType};
use crate::snapshot::Snapshot;
use crate::text::normalize_search;
use crate::{accounts_view, budget, dashboard, metrics, ops, scheduled_view};
use chrono::NaiveDate;
use serde::Serialize;

/// Ce qu'on a compris de la demande.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AssistantIntent {
    /// Opération prête à enregistrer (le brouillon est déjà rempli et validé par le noyau).
    AddTransaction(ops::TransactionDraft),
    /// Solde d'un compte, ou de tous.
    Balance { account_id: Option<String> },
    /// Reste à dépenser, pour une catégorie ou pour l'ensemble du mois.
    BudgetRemaining { category_id: Option<String> },
    /// Prochaines échéances.
    Upcoming,
    /// Revenus, dépenses et épargne du mois.
    MonthSummary,
    /// Traite les échéances dues.
    ProcessDue,
}

/// Réponse à lire ou à afficher.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssistantReply {
    /// Phrase principale, prête pour Siri.
    pub summary: String,
    /// Lignes de détail (comptes, catégories…).
    pub details: Vec<String>,
    /// Vrai si les données ont changé.
    pub changed: bool,
    /// Intention retenue, pour qu'une interface puisse la montrer ou la confirmer.
    pub intent: Option<AssistantIntent>,
}

impl AssistantReply {
    fn read(summary: impl Into<String>, details: Vec<String>, intent: AssistantIntent) -> Self {
        Self {
            summary: summary.into(),
            details,
            changed: false,
            intent: Some(intent),
        }
    }

    /// Demande non comprise : on le dit, avec des exemples.
    pub fn not_understood() -> Self {
        Self {
            summary: "Je n'ai pas compris la demande.".to_string(),
            details: vec![
                "Essayez « ajoute 12,50 € en courses »,".to_string(),
                "« virement de 50 € de Courant vers Livret A »,".to_string(),
                "« quel est mon solde ? », « combien me reste-t-il en alimentation ? »,".to_string(),
                "« prochaines échéances » ou « résumé du mois ».".to_string(),
            ],
            changed: false,
            intent: None,
        }
    }
}

/// Traduit une phrase en intention. `None` si la demande n'est pas reconnue.
pub fn interpret(snapshot: &Snapshot, text: &str, today: NaiveDate) -> Option<AssistantIntent> {
    let normalized = crate::text::collapse_whitespace(&normalize_search(text));
    if normalized.is_empty() {
        return None;
    }

    if contains_any(
        &normalized,
        &[
            "echeance",
            "echeancier",
            "prelevement a venir",
            "a venir",
            "upcoming",
            "scheduled",
            "due payments",
        ],
    ) {
        if contains_any(
            &normalized,
            &[
                "traite",
                "traiter",
                "applique",
                "appliquer",
                "enregistre les",
                "process",
                "record due",
            ],
        ) {
            return Some(AssistantIntent::ProcessDue);
        }
        return Some(AssistantIntent::Upcoming);
    }

    if contains_any(
        &normalized,
        &[
            "resume",
            "bilan",
            "ce mois",
            "du mois",
            "mois ci",
            "monthly summary",
            "month summary",
        ],
    ) && !contains_any(&normalized, &["budget", "reste"])
    {
        return Some(AssistantIntent::MonthSummary);
    }

    if contains_any(
        &normalized,
        &["budget", "reste", "restant", "reste t il", "enveloppe", "remaining"],
    ) {
        let category_id = find_category(snapshot, &normalized);
        return Some(AssistantIntent::BudgetRemaining { category_id });
    }

    if contains_any(
        &normalized,
        &["solde", "combien j ai", "combien ai je", "combien sur", "balance"],
    ) {
        return Some(AssistantIntent::Balance {
            account_id: find_account(snapshot, &normalized),
        });
    }

    // An incomplete transfer must never fall through to an expense or guess an account.
    if contains_any(
        &normalized,
        &["virement", "transfert", "vire", "virer", "transfer", "transfere"],
    ) {
        let (from_id, to_id) = find_transfer_accounts(snapshot, &normalized)?;
        // Account names may contain digits (Livret A 2026, Account 2).
        let mut amount_text = normalized.clone();
        let mut names: Vec<_> = snapshot.accounts.iter().map(|a| normalize_search(&a.name)).collect();
        names.sort_by_key(|name| std::cmp::Reverse(name.len()));
        for name in names {
            if !name.is_empty() {
                amount_text = amount_text.replace(&name, " ");
            }
        }
        let amount = find_amount(&amount_text)?;
        let mut draft = ops::new_transaction_draft(snapshot, &[], today);
        draft.kind = TransactionType::Transfer;
        draft.amount = amount;
        draft.account_id = from_id;
        draft.to_account_id = Some(to_id);
        draft.category_id = crate::models::TRANSFER_CATEGORY_ID.to_string();
        draft.description = "Virement".to_string();
        return Some(AssistantIntent::AddTransaction(draft));
    }

    // Reste le cas d'une saisie : il faut un montant.
    if !contains_any(
        &normalized,
        &[
            "ajout",
            "note",
            "depens",
            "paye",
            "achat",
            "recu",
            "recois",
            "revenu",
            "salaire",
            "remboursement",
            "encaisse",
            "credit",
            "add",
            "spent",
            "paid",
            "expense",
            "income",
            "received",
            "salary",
        ],
    ) {
        return None;
    }
    let amount = find_amount(&normalized)?;
    let kind = if contains_any(
        &normalized,
        &[
            "recu",
            "recois",
            "revenu",
            "salaire",
            "remboursement",
            "encaisse",
            "credit",
            "income",
            "received",
            "salary",
        ],
    ) {
        TransactionType::Income
    } else {
        TransactionType::Expense
    };
    Some(AssistantIntent::AddTransaction(draft(
        snapshot,
        &normalized,
        amount,
        kind,
        today,
    )))
}

/// Brouillon d'opération déduit de la phrase (compte, catégorie et libellé au mieux).
fn draft(
    snapshot: &Snapshot,
    normalized: &str,
    amount: f64,
    kind: TransactionType,
    today: NaiveDate,
) -> ops::TransactionDraft {
    let mut draft = ops::new_transaction_draft(snapshot, &[], today);
    draft.kind = kind;
    draft.amount = amount;
    draft.date = format_date(today);
    if let Some(account_id) = find_account(snapshot, normalized) {
        draft.account_id = account_id;
    }
    let category = find_category(snapshot, normalized);
    if let Some(category_id) = category.clone() {
        draft.category_id = category_id;
    } else if let Some(fallback) = snapshot
        .categories
        .iter()
        .find(|category| category.id == "other" || normalize_search(&category.name) == "divers")
    {
        draft.category_id = fallback.id.clone();
    }
    draft.description = category
        .and_then(|id| snapshot.categories.iter().find(|category| category.id == id))
        .map(|category| category.name.clone())
        .unwrap_or_else(|| "Ajout vocal".to_string());
    draft
}

/// Brouillon construit à partir de paramètres déjà structurés (intention Siri).
///
/// Les noms de compte et de catégorie sont résolus par le noyau : l'hôte n'a pas à connaître les
/// identifiants.
pub fn draft_from_parts(
    snapshot: &Snapshot,
    amount: f64,
    kind: TransactionType,
    category: Option<&str>,
    account: Option<&str>,
    description: Option<&str>,
    today: NaiveDate,
) -> AssistantIntent {
    let mut phrase = String::new();
    if let Some(category) = category {
        phrase.push_str(&normalize_search(category));
        phrase.push(' ');
    }
    if let Some(account) = account {
        phrase.push_str(&normalize_search(account));
    }
    let mut built = draft(snapshot, &phrase, amount, kind, today);
    // Structured entities must resolve independently: category words cannot select an account.
    if let Some(account) = account {
        let matches: Vec<_> = snapshot
            .accounts
            .iter()
            .filter(|a| a.id == account || normalize_search(&a.name) == normalize_search(account))
            .collect();
        built.account_id = if matches.len() == 1 {
            matches[0].id.clone()
        } else {
            account.to_string()
        };
    }
    if let Some(category) = category {
        let matches: Vec<_> = snapshot
            .categories
            .iter()
            .filter(|c| c.id == category || normalize_search(&c.name) == normalize_search(category))
            .collect();
        built.category_id = if matches.len() == 1 {
            matches[0].id.clone()
        } else {
            category.to_string()
        };
    }
    if let Some(description) = description {
        let trimmed = description.trim();
        if !trimmed.is_empty() {
            built.description = trimmed.to_string();
        }
    }
    AssistantIntent::AddTransaction(built)
}

/// Intention « solde », par nom de compte (résolu par le noyau) ou pour l'ensemble.
pub fn balance_intent(snapshot: &Snapshot, account: Option<&str>) -> AssistantIntent {
    AssistantIntent::Balance {
        account_id: account.and_then(|name| find_account(snapshot, &normalize_search(name))),
    }
}

/// Intention « reste à dépenser », par nom de catégorie ou pour le mois entier.
pub fn budget_intent(snapshot: &Snapshot, category: Option<&str>) -> AssistantIntent {
    AssistantIntent::BudgetRemaining {
        category_id: category.and_then(|name| find_category(snapshot, &normalize_search(name))),
    }
}

/// Exécute une intention de lecture. Les écritures passent par `run`, qui a le pool.
pub fn answer(snapshot: &Snapshot, intent: &AssistantIntent, today: NaiveDate) -> AssistantReply {
    match intent {
        AssistantIntent::Balance { account_id } => balance_reply(snapshot, account_id.as_deref(), intent),
        AssistantIntent::BudgetRemaining { category_id } => {
            budget_reply(snapshot, category_id.as_deref(), today, intent)
        }
        AssistantIntent::Upcoming => upcoming_reply(snapshot, today, intent),
        AssistantIntent::MonthSummary => month_reply(snapshot, today, intent),
        AssistantIntent::AddTransaction(draft) => {
            if draft.kind == TransactionType::Transfer {
                let from_account = snapshot
                    .accounts
                    .iter()
                    .find(|account| account.id == draft.account_id)
                    .map(|account| account.name.clone())
                    .unwrap_or_else(|| "Compte source".to_string());
                let to_account = draft
                    .to_account_id
                    .as_ref()
                    .and_then(|to_id| snapshot.accounts.iter().find(|account| account.id == *to_id))
                    .map(|account| account.name.clone())
                    .unwrap_or_else(|| "Compte destination".to_string());
                AssistantReply::read(
                    format!(
                        "Virement de {} de {} vers {} à enregistrer.",
                        currency_fr(draft.amount),
                        from_account,
                        to_account
                    ),
                    Vec::new(),
                    intent.clone(),
                )
            } else {
                AssistantReply::read(
                    format!(
                        "{} de {} à enregistrer.",
                        if draft.kind == TransactionType::Income {
                            "Revenu"
                        } else {
                            "Dépense"
                        },
                        currency_fr(draft.amount)
                    ),
                    Vec::new(),
                    intent.clone(),
                )
            }
        }
        AssistantIntent::ProcessDue => {
            AssistantReply::read("Échéances dues à traiter.".to_string(), Vec::new(), intent.clone())
        }
    }
}

fn balance_reply(snapshot: &Snapshot, account_id: Option<&str>, intent: &AssistantIntent) -> AssistantReply {
    let summary = accounts_view::tray_summary(snapshot);
    if let Some(account_id) = account_id {
        if let Some(account) = summary.accounts.iter().find(|account| account.account_id == account_id) {
            return AssistantReply::read(
                format!("{} : {}.", account.name, currency_fr(account.balance)),
                Vec::new(),
                intent.clone(),
            );
        }
    }
    let details = summary
        .accounts
        .iter()
        .map(|account| format!("{} : {}", account.name, currency_fr(account.balance)))
        .collect();
    AssistantReply::read(
        format!("Total de vos comptes : {}.", currency_fr(summary.total)),
        details,
        intent.clone(),
    )
}

fn budget_reply(
    snapshot: &Snapshot,
    category_id: Option<&str>,
    today: NaiveDate,
    intent: &AssistantIntent,
) -> AssistantReply {
    let query = budget::BudgetQuery {
        accounts: Vec::new(),
        search: String::new(),
        categories: Vec::new(),
    };
    let overview = budget::budget_overview(snapshot, &query, today);
    if let Some(category_id) = category_id {
        if let Some(row) = overview.categories.iter().find(|row| row.category.id == category_id) {
            let verb = if row.remaining < 0.0 { "dépassé de" } else { "reste" };
            return AssistantReply::read(
                format!(
                    "{} : {} {} sur {}.",
                    row.category.name,
                    verb,
                    currency_fr(row.remaining.abs()),
                    currency_fr(row.budgeted)
                ),
                vec![format!("Dépensé : {}", currency_fr(row.spent))],
                intent.clone(),
            );
        }
    }
    AssistantReply::read(
        format!(
            "Budget de {} : {} restants sur {}.",
            overview.month_label,
            currency_fr(overview.remaining),
            currency_fr(overview.total_budgeted)
        ),
        vec![
            format!("Dépensé : {}", currency_fr(overview.total_spent)),
            format!("Par jour restant : {}", currency_fr(overview.remaining_per_day)),
        ],
        intent.clone(),
    )
}

fn upcoming_reply(snapshot: &Snapshot, today: NaiveDate, intent: &AssistantIntent) -> AssistantReply {
    let view = dashboard::dashboard(snapshot, &[], today);
    if view.upcoming.is_empty() {
        return AssistantReply::read("Aucune échéance à venir.".to_string(), Vec::new(), intent.clone());
    }
    let details = view
        .upcoming
        .iter()
        .map(|item| {
            format!(
                "{} : {} dans {} jour{}",
                item.description,
                currency_fr(item.amount),
                item.days_until,
                if item.days_until.abs() > 1 { "s" } else { "" }
            )
        })
        .collect();
    let next = &view.upcoming[0];
    AssistantReply::read(
        format!(
            "Prochaine échéance : {} de {} dans {} jour{}.",
            next.description,
            currency_fr(next.amount),
            next.days_until,
            if next.days_until.abs() > 1 { "s" } else { "" }
        ),
        details,
        intent.clone(),
    )
}

fn month_reply(snapshot: &Snapshot, today: NaiveDate, intent: &AssistantIntent) -> AssistantReply {
    let month: metrics::MonthlySummary = dashboard::dashboard(snapshot, &[], today).month;
    AssistantReply::read(
        format!(
            "En {}, {} de revenus et {} de dépenses, soit {} d'écart.",
            month_year_long(today),
            currency_fr(month.income),
            currency_fr(month.expenses),
            currency_fr(month.saved)
        ),
        Vec::new(),
        intent.clone(),
    )
}

/// Nombre d'échéances traitées, formulé pour Siri.
pub fn due_reply(processed: usize) -> AssistantReply {
    AssistantReply {
        summary: match processed {
            0 => "Aucune échéance à traiter.".to_string(),
            1 => "1 échéance a été enregistrée.".to_string(),
            count => format!("{count} échéances ont été enregistrées."),
        },
        details: Vec::new(),
        changed: processed > 0,
        intent: Some(AssistantIntent::ProcessDue),
    }
}

/// Confirmation d'une opération enregistrée.
pub fn saved_reply(snapshot: &Snapshot, draft: &ops::TransactionDraft) -> AssistantReply {
    let account = snapshot
        .accounts
        .iter()
        .find(|account| account.id == draft.account_id)
        .map(|account| account.name.clone())
        .unwrap_or_default();

    if draft.kind == TransactionType::Transfer {
        let to_account = draft
            .to_account_id
            .as_ref()
            .and_then(|to_id| snapshot.accounts.iter().find(|account| account.id == *to_id))
            .map(|account| account.name.clone())
            .unwrap_or_else(|| "Compte destination".to_string());
        return AssistantReply {
            summary: format!(
                "Virement de {} de {} vers {} enregistré.",
                currency_fr(draft.amount),
                account,
                to_account
            ),
            details: Vec::new(),
            changed: true,
            intent: Some(AssistantIntent::AddTransaction(draft.clone())),
        };
    }

    let category = snapshot
        .categories
        .iter()
        .find(|category| category.id == draft.category_id)
        .map(|category| category.name.clone())
        .unwrap_or_default();
    let verb = if draft.kind == TransactionType::Income {
        "Revenu"
    } else {
        "Dépense"
    };
    AssistantReply {
        summary: format!(
            "{} de {} enregistrée en {} sur {}.",
            verb,
            currency_fr(draft.amount),
            category,
            account
        ),
        details: Vec::new(),
        changed: true,
        intent: Some(AssistantIntent::AddTransaction(draft.clone())),
    }
}

/// Vue de l'échéancier utilisée par l'intention « prochaines échéances » côté hôte.
pub fn scheduled_count(snapshot: &Snapshot, today: NaiveDate) -> usize {
    let query = scheduled_view::ScheduledQuery {
        accounts: Vec::new(),
        due_range: ScheduledDueRange::All,
        search: String::new(),
        categories: Vec::new(),
        frequencies: Vec::new(),
    };
    scheduled_view::scheduled_view(snapshot, &query, today).rows.len()
}

/// Analyse et exécute une demande à partir du pool, pour le pont (la PWA n'a pas de moteur).
pub async fn run(
    pool: &crate::db::DbPool,
    text: &str,
    today: NaiveDate,
    apply: bool,
) -> crate::error::CoreResult<AssistantReply> {
    let snapshot = crate::snapshot::load(pool).await?;
    let Some(intent) = interpret(&snapshot, text, today) else {
        return Ok(AssistantReply::not_understood());
    };
    if !apply {
        return Ok(answer(&snapshot, &intent, today));
    }
    match intent {
        AssistantIntent::AddTransaction(draft) => {
            ops::save_transaction(pool, draft.clone()).await?;
            let refreshed = crate::snapshot::load(pool).await?;
            Ok(saved_reply(&refreshed, &draft))
        }
        AssistantIntent::ProcessDue => {
            let result = crate::scheduling::process_due(pool, today).await?;
            Ok(due_reply(result.created_transactions as usize))
        }
        other => Ok(answer(&snapshot, &other, today)),
    }
}

// --- Analyse de la phrase ---

fn contains_any(normalized: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| normalized.contains(needle))
}

/// Premier montant de la phrase (« 12,50 », « 12.50 », « 12 euros », « 1 200 »).
fn find_amount(normalized: &str) -> Option<f64> {
    let bytes: Vec<char> = normalized.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        if start > 0 && matches!(bytes[start - 1], '-' | '−') {
            return None;
        }
        let mut digits = String::new();
        let mut separator_seen = false;
        while index < bytes.len() {
            let current = bytes[index];
            if current.is_ascii_digit() {
                digits.push(current);
                index += 1;
            } else if (current == ',' || current == '.')
                && !separator_seen
                && index + 1 < bytes.len()
                && bytes[index + 1].is_ascii_digit()
            {
                separator_seen = true;
                digits.push('.');
                index += 1;
            } else if current == ' '
                && !separator_seen
                && index + 1 < bytes.len()
                && bytes[index + 1].is_ascii_digit()
                && index - start <= 3
            {
                // Séparateur de milliers : « 1 200 ».
                index += 1;
            } else {
                break;
            }
        }
        if let Ok(value) = digits.parse::<f64>() {
            if value.is_finite() && value > 0.0 {
                return Some(value);
            }
        }
    }
    None
}

/// Compte dont le nom apparaît dans la phrase (le nom le plus long gagne).
fn find_account(snapshot: &Snapshot, normalized: &str) -> Option<String> {
    if let Some(account) = snapshot.accounts.iter().find(|account| account.id == normalized) {
        return Some(account.id.clone());
    }
    snapshot
        .accounts
        .iter()
        .filter(|account| {
            let name = normalize_search(&account.name);
            !name.is_empty() && normalized.contains(&name)
        })
        .max_by_key(|account| normalize_search(&account.name).len())
        .map(|account| account.id.clone())
}

/// Catégorie dont le nom apparaît dans la phrase ; « Virement » est réservée aux virements.
fn find_category(snapshot: &Snapshot, normalized: &str) -> Option<String> {
    if let Some(category) = snapshot
        .categories
        .iter()
        .find(|category| category.id == normalized && category.id != "transfer")
    {
        return Some(category.id.clone());
    }
    snapshot
        .categories
        .iter()
        .filter(|category| category.id != "transfer")
        .filter_map(|category| {
            // Les noms composés (« Shopping / Vêtements ») sont testés morceau par morceau.
            normalize_search(&category.name)
                .split(" ")
                .filter(|part| part.len() >= 4)
                .find(|part| normalized.contains(*part))
                .map(|part| (category.id.clone(), part.len()))
        })
        .max_by_key(|(_, length)| *length)
        .map(|(id, _)| id)
}

/// Resolve exactly two unambiguous account mentions, with explicit direction markers.
fn find_transfer_accounts(snapshot: &Snapshot, text: &str) -> Option<(String, String)> {
    let mut matches = Vec::new();
    for account in &snapshot.accounts {
        let name = normalize_search(&account.name);
        if name.is_empty() {
            continue;
        }
        for (start, _) in text.match_indices(&name) {
            let end = start + name.len();
            if text[..start].chars().next_back().is_some_and(char::is_alphanumeric)
                || text[end..].chars().next().is_some_and(char::is_alphanumeric)
            {
                continue;
            }
            matches.push((account, start, end));
        }
    }
    // A longer account name wins, but identical names remain ambiguous.
    let all = matches.clone();
    matches.retain(|(_, start, end)| {
        !all.iter().any(|(_, other_start, other_end)| {
            other_start <= start && other_end >= end && other_end - other_start > end - start
        })
    });
    matches.sort_by_key(|(_, start, _)| *start);
    if matches.len() != 2 || matches[0].0.id == matches[1].0.id {
        return None;
    }
    let (first, start, _) = matches[0];
    let (second, second_start, _) = matches[1];
    let destination = |before: &str| {
        [
            "vers",
            "sur",
            "a",
            "au",
            "to",
            "into",
            "vers le compte",
            "sur le compte",
            "to account",
            "to the account",
        ]
        .iter()
        .any(|marker| before.trim_end().ends_with(&format!(" {marker}")))
    };
    let source = |before: &str| {
        [
            "de",
            "du",
            "depuis",
            "from",
            "depuis le compte",
            "du compte",
            "from account",
            "from the account",
        ]
        .iter()
        .any(|marker| before.trim_end().ends_with(&format!(" {marker}")))
    };
    if source(&text[..start]) && destination(&text[..second_start]) {
        Some((first.id.clone(), second.id.clone()))
    } else if destination(&text[..start]) && source(&text[..second_start]) {
        Some((second.id.clone(), first.id.clone()))
    } else {
        None
    }
}
