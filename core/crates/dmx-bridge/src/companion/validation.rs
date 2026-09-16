//! Validate mobile payloads before repository writes; clients are not a trust boundary.
use dmx_core::{
    dates::parse_date,
    models::{Account, Budget, Category, ScheduledTransaction, Transaction, TransactionType, TRANSFER_CATEGORY_ID},
};
use serde::Deserialize;

fn required(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err("Un champ obligatoire est vide.".into())
    } else {
        Ok(())
    }
}
fn amount(value: f64) -> Result<(), String> {
    if !dmx_core::metrics::is_valid_money(value) || (value * 100.0).round() < 1.0 {
        Err("Montant invalide.".into())
    } else {
        Ok(())
    }
}
fn date(value: &str) -> Result<(), String> {
    if parse_date(value).is_none() {
        Err("Date invalide.".into())
    } else {
        Ok(())
    }
}
fn transaction(item: &Transaction) -> Result<(), String> {
    required(&item.id)?;
    required(&item.account_id)?;
    amount(item.amount)?;
    date(&item.date)?;
    if item.transaction_type == TransactionType::Transfer {
        return Err("Un virement doit contenir un débit et un crédit.".into());
    }
    if item.is_transfer
        && (item.category != TRANSFER_CATEGORY_ID
            || item
                .linked_transaction_id
                .as_deref()
                .is_none_or(|id| id.is_empty() || id == item.id))
    {
        return Err("Contrepartie de virement invalide.".into());
    }
    if !item.is_transfer && item.linked_transaction_id.is_some() {
        return Err("Une opération simple ne peut pas désigner une contrepartie.".into());
    }
    Ok(())
}
fn decode<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|_| "Données JSON invalides.".into())
}

pub(super) fn validate(resource: &str, body: &[u8]) -> Result<(), String> {
    match resource {
        "accounts" => {
            let item: Account = decode(body)?;
            required(&item.id)?;
            required(&item.name)?;
            if !dmx_core::metrics::is_valid_money(item.initial_balance) {
                return Err("Solde invalide.".into());
            }
        }
        "categories" => {
            let item: Category = decode(body)?;
            required(&item.id)?;
            required(&item.name)?;
        }
        "transactions" => transaction(&decode(body)?)?,
        "budgets" => {
            let item: Budget = decode(body)?;
            required(&item.id)?;
            required(&item.name)?;
            amount(item.amount)?;
        }
        "scheduled" => {
            let item: ScheduledTransaction = decode(body)?;
            required(&item.id)?;
            required(&item.account_id)?;
            amount(item.amount)?;
            date(&item.next_date)?;
            if let Some(end) = &item.end_date {
                date(end)?;
            }
            if item.transaction_type == TransactionType::Transfer
                && item
                    .to_account_id
                    .as_deref()
                    .is_none_or(|id| id.is_empty() || id == item.account_id)
            {
                return Err("Compte destinataire invalide.".into());
            }
        }
        "transfers" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Pair {
                from_transaction: Transaction,
                to_transaction: Transaction,
            }
            let pair: Pair = decode(body)?;
            let (from, to) = (&pair.from_transaction, &pair.to_transaction);
            transaction(from)?;
            transaction(to)?;
            if !from.is_transfer
                || !to.is_transfer
                || from.id == to.id
                || from.account_id == to.account_id
                || from.transaction_type != TransactionType::Expense
                || to.transaction_type != TransactionType::Income
                || from.amount != to.amount
                || from.date != to.date
                || from.linked_transaction_id.as_deref() != Some(to.id.as_str())
                || to.linked_transaction_id.as_deref() != Some(from.id.as_str())
            {
                return Err("Les deux côtés du virement doivent correspondre.".into());
            }
        }
        _ => {}
    }
    Ok(())
}
