//! Shared resource limits, enforced before writes or chart allocations.
use crate::{
    dates::{add_days, days_between, parse_date},
    error::{CoreError, CoreResult},
    models::Transaction,
};
use chrono::NaiveDate;

pub const MAX_IMPORT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DESCRIPTION_BYTES: usize = 4096;
pub const MAX_ID_BYTES: usize = 256;
pub const MAX_BANK_SOURCE_BYTES: usize = 2048;
/// Inclusive daily points, approximately five years. Never interpreted as five calendar years.
pub const MAX_GRAPH_DAYS: i64 = 1826;
pub const MAX_GRAPH_POINTS: usize = 500_000;

pub fn text(value: &str, max: usize, label: &str) -> CoreResult<()> {
    if value.len() > max {
        Err(CoreError::validation(format!(
            "{label} dépasse la limite de {max} octets."
        )))
    } else {
        Ok(())
    }
}
pub fn import_size(content: &str) -> CoreResult<()> {
    if content.len() > MAX_IMPORT_BYTES {
        Err(CoreError::import("Le fichier dépasse la limite de 16 Mio."))
    } else {
        Ok(())
    }
}
pub fn transaction(item: &Transaction) -> CoreResult<()> {
    text(&item.id, MAX_ID_BYTES, "L’identifiant")?;
    text(&item.account_id, MAX_ID_BYTES, "Le compte")?;
    text(&item.category, MAX_ID_BYTES, "La catégorie")?;
    text(&item.description, MAX_DESCRIPTION_BYTES, "Le libellé")?;
    if let Some(id) = &item.linked_transaction_id {
        text(id, MAX_ID_BYTES, "La contrepartie")?;
    }
    match (&item.bank_source, &item.bank_transaction_id) {
        (None, None) => {}
        (Some(source), Some(id)) if !source.trim().is_empty() && !id.trim().is_empty() => {
            text(source, MAX_BANK_SOURCE_BYTES, "La source bancaire")?;
            text(id, MAX_ID_BYTES, "L’identifiant bancaire")?;
        }
        _ => {
            return Err(CoreError::validation(
                "L’identité bancaire doit contenir une source et un identifiant.",
            ))
        }
    }
    Ok(())
}
pub fn graph_range(start: NaiveDate, end: NaiveDate, series: usize) -> CoreResult<()> {
    let days = days_between(start, end).abs().saturating_add(1);
    if days > MAX_GRAPH_DAYS {
        return Err(CoreError::validation(
            "La période ne peut pas dépasser 1826 jours (environ cinq ans).",
        ));
    }
    if (days as usize)
        .checked_mul(series.max(1))
        .is_none_or(|points| points > MAX_GRAPH_POINTS)
    {
        return Err(CoreError::validation("La période et les comptes sélectionnés dépassent la limite de 500 000 points. Réduisez la période ou le nombre de comptes."));
    }
    Ok(())
}
/// Defense for old persisted settings and callers of pure calculation helpers.
pub fn bounded_end(start: NaiveDate, end: NaiveDate) -> NaiveDate {
    end.min(add_days(start, MAX_GRAPH_DAYS - 1))
}
pub fn custom_date(value: &str) -> CoreResult<Option<NaiveDate>> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    if value.len() != 10 {
        return Err(CoreError::validation("Date personnalisée invalide."));
    }
    parse_date(value)
        .map(Some)
        .ok_or_else(|| CoreError::validation("Date personnalisée invalide."))
}

pub fn account(item: &crate::models::Account) -> CoreResult<()> {
    text(&item.id, MAX_ID_BYTES, "L’identifiant")?;
    text(&item.name, MAX_DESCRIPTION_BYTES, "Le nom")?;
    text(&item.account_type, MAX_ID_BYTES, "Le type")?;
    text(&item.color, MAX_ID_BYTES, "La couleur")?;
    text(&item.icon, MAX_ID_BYTES, "L’icône")
}
pub fn category(item: &crate::models::Category) -> CoreResult<()> {
    text(&item.id, MAX_ID_BYTES, "L’identifiant")?;
    text(&item.name, MAX_DESCRIPTION_BYTES, "Le nom")?;
    text(&item.color, MAX_ID_BYTES, "La couleur")?;
    text(&item.icon, MAX_ID_BYTES, "L’icône")
}
pub fn budget(item: &crate::models::Budget) -> CoreResult<()> {
    text(&item.id, MAX_ID_BYTES, "L’identifiant")?;
    text(&item.name, MAX_DESCRIPTION_BYTES, "Le nom")?;
    text(&item.category, MAX_ID_BYTES, "La catégorie")?;
    if let Some(id) = &item.account_id {
        text(id, MAX_ID_BYTES, "Le compte")?;
    }
    Ok(())
}
pub fn scheduled(item: &crate::models::ScheduledTransaction) -> CoreResult<()> {
    text(&item.id, MAX_ID_BYTES, "L’identifiant")?;
    text(&item.description, MAX_DESCRIPTION_BYTES, "Le libellé")?;
    text(&item.account_id, MAX_ID_BYTES, "Le compte")?;
    text(&item.category, MAX_ID_BYTES, "La catégorie")?;
    for id in [&item.to_account_id, &item.budget_id].into_iter().flatten() {
        text(id, MAX_ID_BYTES, "La référence")?;
    }
    Ok(())
}
