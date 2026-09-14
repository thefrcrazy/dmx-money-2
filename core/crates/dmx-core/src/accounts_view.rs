//! Page Comptes (groupes et ordre personnalisés) et résumé du menu de barre d'état.

use crate::format::js_number;
use crate::metrics::{account_balances, cents, euros};
use crate::models::{Account, UNGROUPED_ACCOUNTS_LABEL};
use crate::snapshot::Snapshot;
use crate::text::{search_tokens, SearchText};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccountsQuery {
    pub search: String,
    pub types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountCard {
    pub account: Account,
    pub current_balance: f64,
    pub checked_balance: f64,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountGroupSection {
    pub name: String,
    pub is_ungrouped: bool,
    pub accounts: Vec<AccountCard>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountsView {
    pub groups: Vec<AccountGroupSection>,
    pub visible_count: u32,
    pub total_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrayAccount {
    pub account_id: String,
    pub name: String,
    pub balance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraySummary {
    pub accounts: Vec<TrayAccount>,
    pub total: f64,
}

pub fn accounts_view(snapshot: &Snapshot, query: &AccountsQuery) -> AccountsView {
    let settings = &snapshot.settings;
    let balances: HashMap<String, (f64, f64)> = account_balances(snapshot)
        .into_iter()
        .map(|balance| (balance.account_id, (balance.current_balance, balance.checked_balance)))
        .collect();

    let tokens = search_tokens(&query.search);
    let visible: Vec<&Account> = snapshot
        .accounts
        .iter()
        .filter(|account| query.types.is_empty() || query.types.contains(&account.account_type))
        .filter(|account| {
            if tokens.is_empty() {
                return true;
            }
            let balance = balances
                .get(&account.id)
                .map(|balance| balance.0)
                .unwrap_or(account.initial_balance);
            let mut text = SearchText::new();
            text.push(&account.name)
                .push(&account.account_type)
                .push(js_number(account.initial_balance))
                .push(js_number(balance))
                .push(&account.color)
                .push(&account.icon)
                .push(
                    settings
                        .account_groups
                        .get(&account.id)
                        .map(String::as_str)
                        .unwrap_or(UNGROUPED_ACCOUNTS_LABEL),
                );
            text.matches(&tokens)
        })
        .collect();

    let mut group_names: Vec<String> = Vec::new();
    for group in settings.effective_group_order() {
        if group != UNGROUPED_ACCOUNTS_LABEL && !group_names.contains(&group) {
            group_names.push(group);
        }
    }
    for group in &settings.custom_groups {
        if group != UNGROUPED_ACCOUNTS_LABEL && !group_names.contains(group) {
            group_names.push(group.clone());
        }
    }

    let order = settings.accounts_order.clone().unwrap_or_default();
    let position = |id: &str| order.iter().position(|item| item == id).unwrap_or(usize::MAX);
    let mut sorted = visible;
    sorted.sort_by_key(|account| position(&account.id));

    let mut sections: Vec<AccountGroupSection> = group_names
        .iter()
        .map(|name| AccountGroupSection {
            name: name.clone(),
            is_ungrouped: false,
            accounts: Vec::new(),
        })
        .collect();
    let mut ungrouped = AccountGroupSection {
        name: UNGROUPED_ACCOUNTS_LABEL.to_string(),
        is_ungrouped: true,
        accounts: Vec::new(),
    };

    for account in &sorted {
        let (current, checked) = balances
            .get(&account.id)
            .copied()
            .unwrap_or((account.initial_balance, account.initial_balance));
        let group = settings.account_groups.get(&account.id).cloned();
        let card = AccountCard {
            account: (*account).clone(),
            current_balance: current,
            checked_balance: checked,
            group: group.clone(),
        };
        match group.and_then(|group| sections.iter_mut().find(|section| section.name == group)) {
            Some(section) => section.accounts.push(card),
            None => ungrouped.accounts.push(AccountCard { group: None, ..card }),
        }
    }

    if !ungrouped.accounts.is_empty() {
        sections.push(ungrouped);
    }

    AccountsView {
        groups: sections,
        visible_count: sorted.len() as u32,
        total_count: snapshot.accounts.len() as u32,
    }
}

/// Soldes du menu de barre d'état, dans l'ordre de création des comptes.
pub fn tray_summary(snapshot: &Snapshot) -> TraySummary {
    let balances = account_balances(snapshot);
    let total = euros(balances.iter().map(|balance| cents(balance.current_balance)).sum());
    TraySummary {
        accounts: snapshot
            .accounts
            .iter()
            .zip(balances)
            .map(|(account, balance)| TrayAccount {
                account_id: account.id.clone(),
                name: account.name.clone(),
                balance: balance.current_balance,
            })
            .collect(),
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str, account_type: &str) -> Account {
        Account {
            id: id.into(),
            name: format!("Compte {id}"),
            account_type: account_type.into(),
            initial_balance: 10.0,
            color: "#000".into(),
            icon: "Wallet".into(),
        }
    }

    #[test]
    fn groups_follow_custom_order_and_ungrouped_comes_last() {
        let mut snapshot = Snapshot {
            accounts: vec![
                account("a1", "Courant"),
                account("a2", "Épargne"),
                account("a3", "Courant"),
            ],
            ..Snapshot::default()
        };
        snapshot.settings.custom_groups = vec!["Perso".into(), "Pro".into()];
        snapshot.settings.custom_groups_order = Some(vec!["Pro".into(), "Perso".into()]);
        snapshot.settings.accounts_order = Some(vec!["a3".into(), "a1".into()]);
        snapshot.settings.account_groups.insert("a2".into(), "Perso".into());

        let view = accounts_view(&snapshot, &AccountsQuery::default());
        let names: Vec<_> = view.groups.iter().map(|group| group.name.as_str()).collect();
        assert_eq!(names, vec!["Pro", "Perso", UNGROUPED_ACCOUNTS_LABEL]);
        let ungrouped: Vec<_> = view.groups[2]
            .accounts
            .iter()
            .map(|card| card.account.id.as_str())
            .collect();
        assert_eq!(ungrouped, vec!["a3", "a1"]);

        let filtered = accounts_view(
            &snapshot,
            &AccountsQuery {
                types: vec!["Épargne".into()],
                ..AccountsQuery::default()
            },
        );
        assert_eq!(filtered.visible_count, 1);
        assert_eq!(filtered.groups.len(), 2);
    }
}
