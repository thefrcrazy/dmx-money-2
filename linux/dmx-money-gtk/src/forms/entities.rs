//! Formulaires des entités : comptes, groupes, catégories, opérations, budgets, échéances.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use dmx_core::budget::BudgetQuery;
use dmx_core::models::{account_type_defaults, Periodicity, ScheduledDueRange, TransactionType, ACCOUNT_TYPES};
use dmx_core::ops::{AccountDraft, BudgetDraft, CategoryDraft, ScheduledDraft, TransactionDraft};
use dmx_core::palette::{CATEGORY_COLORS, ICON_PICKER};
use dmx_core::predictions::FakeTransactionDraft;
use dmx_core::scheduled_view::ScheduledQuery;
use dmx_core::settings::SettingsChange;

use super::controls::{
    amount_entry, entry, field, optional_amount, pair, required_amount, BadgePreview, ColorGrid, DateButton, IconGrid,
    KindPicker, Select, SelectOption,
};
use super::FormShell;
use crate::store::Store;
use crate::{format, icons, widgets};

/// Options de comptes (icône et couleur comprises).
fn account_options(store: &Rc<Store>) -> Vec<SelectOption> {
    store
        .accounts()
        .iter()
        .map(|account| SelectOption::with_icon(&account.id, &account.name, &account.icon, &account.color))
        .collect()
}

/// Options de catégories, sans « Virement » (réservée aux virements).
fn category_options(store: &Rc<Store>) -> Vec<SelectOption> {
    store
        .selectable_categories()
        .iter()
        .map(|category| SelectOption::with_icon(&category.id, &category.name, &category.icon, &category.color))
        .collect()
}

// --- Compte ---

pub fn account(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, id: Option<String>) {
    let Some(draft) = store.read(|engine| engine.account_draft(id.as_deref())) else {
        return;
    };
    let editing = draft.id.is_some();
    let shell = FormShell::new(
        if editing {
            "Modifier le compte"
        } else {
            "Nouveau compte"
        },
        Some(if editing { "Mettre à jour" } else { "Créer" }),
        540,
    );

    let name = entry("Ex: Compte Courant", &draft.name);
    shell.body().append(&field("Nom du compte", &name));

    let type_select = Select::new(
        "Type",
        ACCOUNT_TYPES
            .iter()
            .map(|value| SelectOption::simple(value, value))
            .collect(),
        Some(draft.account_type.clone()),
        None,
    );
    let balance = amount_entry(draft.initial_balance);
    shell.body().append(&pair(
        &field("Type", &type_select.widget()),
        &field("Solde initial", &balance),
    ));

    let group_select = Select::new(
        "Aucun groupe",
        store
            .settings()
            .custom_groups
            .iter()
            .map(|group| SelectOption::simple(group, group))
            .collect(),
        draft.group.clone(),
        Some("Aucun groupe"),
    );
    shell.body().append(&field("Groupe", &group_select.widget()));

    let icon = Rc::new(RefCell::new(draft.icon.clone()));
    let preview = BadgePreview::new(&draft.icon, &draft.color, 40);
    let colors = ColorGrid::new(&CATEGORY_COLORS, &draft.color, 14, 160);
    let color_row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    color_row.append(&preview.widget());
    color_row.append(&colors.widget());
    shell.body().append(&field("Couleur", &color_row));

    {
        let preview = preview.clone();
        let icon = icon.clone();
        colors.connect_changed(move |color| preview.set(&icon.borrow(), &color));
    }
    {
        // Changer de type reprend l'icône et la couleur par défaut, comme en 1.x.
        let preview = preview.clone();
        let icon = icon.clone();
        let colors = colors.clone();
        type_select.connect_changed(move |value| {
            let (default_icon, default_color) = account_type_defaults(value.as_deref().unwrap_or(ACCOUNT_TYPES[0]));
            *icon.borrow_mut() = default_icon.to_string();
            colors.set_value(default_color);
            preview.set(default_icon, default_color);
        });
    }

    {
        let store = store.clone();
        let shell = shell.clone();
        let draft = draft.clone();
        shell.clone().on_submit(move || {
            let initial_balance = match optional_amount(&balance) {
                Ok(value) => value,
                Err(message) => return shell.set_error(Some(&message)),
            };
            let draft = AccountDraft {
                id: draft.id.clone(),
                name: name.text().to_string(),
                account_type: type_select.required_value(),
                initial_balance,
                color: colors.value(),
                icon: icon.borrow().clone(),
                group: group_select.value(),
            };
            let error = store.attempt(|engine| engine.save_account(draft));
            shell.finish(&store, error, if editing { "Compte mis à jour" } else { "Compte créé" });
        });
    }
    shell.present(parent);
}

// --- Groupes de comptes ---

pub fn account_groups(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>) {
    let shell = FormShell::new("Gérer les groupes", None, 480);

    let new_group = entry("Nouveau groupe...", "");
    let add = widgets::action_button("Ajouter", Some("Plus"), true);
    let add_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    add_row.append(&new_group);
    add_row.append(&add);
    shell.body().append(&add_row);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    shell.body().append(&list);

    let rebuild: Rc<dyn Fn()> = {
        let store = store.clone();
        let list = list.clone();
        Rc::new(move || {
            widgets::clear(&list);
            let groups: Vec<String> = store
                .settings()
                .effective_group_order()
                .into_iter()
                .filter(|group| group != "Non groupés")
                .collect();
            if groups.is_empty() {
                list.append(&widgets::empty_state(
                    "Users",
                    "Aucun groupe",
                    Some("Les comptes sans groupe apparaissent dans « Non groupés »."),
                ));
                return;
            }
            for (index, group) in groups.iter().enumerate() {
                list.append(&group_row(&store, &groups, index, group));
            }
        })
    };

    {
        let store = store.clone();
        let rebuild = rebuild.clone();
        let new_group_for_add = new_group.clone();
        let commit = move || {
            let name = new_group_for_add.text().trim().to_string();
            if name.is_empty() {
                return;
            }
            store.run(None, |engine| {
                engine.apply_settings_change(SettingsChange::AddCustomGroup(name))
            });
            new_group_for_add.set_text("");
            rebuild();
        };
        let commit_for_entry = commit.clone();
        new_group.connect_activate(move |_| commit_for_entry());
        add.connect_clicked(move |_| commit());
    }

    // La liste suit les modifications appliquées par le noyau.
    shell.follow(store, rebuild.clone());
    rebuild();
    shell.present(parent);
}

fn group_row(store: &Rc<Store>, groups: &[String], index: usize, group: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.add_css_class("card");
    row.set_margin_top(2);

    let label = gtk::EditableLabel::new(group);
    label.set_hexpand(true);
    label.set_tooltip_text(Some("Double-cliquez pour renommer"));
    row.append(&label);
    {
        let store = store.clone();
        let original = group.to_string();
        label.connect_editing_notify(move |label| {
            if label.is_editing() {
                return;
            }
            let name = label.text().trim().to_string();
            if name.is_empty() || name == original {
                label.set_text(&original);
                return;
            }
            store.run(Some("Groupe renommé"), |engine| {
                engine.apply_settings_change(SettingsChange::RenameCustomGroup {
                    old_name: original.clone(),
                    new_name: name,
                })
            });
        });
    }

    let up = widgets::icon_button("ChevronUp", "Monter");
    up.set_sensitive(index > 0);
    let down = widgets::icon_button("ChevronDown", "Descendre");
    down.set_sensitive(index + 1 < groups.len());
    for (button, offset) in [(&up, -1i32), (&down, 1)] {
        let store = store.clone();
        let groups = groups.to_vec();
        button.connect_clicked(move |_| {
            let mut order = groups.clone();
            let target = index as i32 + offset;
            if target < 0 || target as usize >= order.len() {
                return;
            }
            order.swap(index, target as usize);
            store.run(None, |engine| {
                engine.apply_settings_change(SettingsChange::CustomGroupsOrder(order))
            });
        });
    }
    row.append(&up);
    row.append(&down);

    let delete = widgets::icon_button("Trash2", "Supprimer");
    delete.add_css_class("dmx-expense");
    {
        let store = store.clone();
        let group = group.to_string();
        delete.connect_clicked(move |_| {
            let store_for_action = store.clone();
            let group_for_action = group.clone();
            store.confirm(
                "Supprimer le groupe",
                &format!("Les comptes du groupe « {group} » seront déplacés dans « Non groupés »."),
                "Supprimer",
                move || {
                    let group = group_for_action.clone();
                    store_for_action.run(Some("Groupe supprimé"), |engine| {
                        engine.apply_settings_change(SettingsChange::DeleteCustomGroup(group))
                    });
                },
            );
        });
    }
    row.append(&delete);
    row
}

// --- Catégorie ---

pub fn category(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, id: Option<String>) {
    let Some(draft) = store.read(|engine| engine.category_draft(id.as_deref())) else {
        return;
    };
    let editing = draft.id.is_some();
    let shell = FormShell::new(
        if editing {
            "Modifier la catégorie"
        } else {
            "Nouvelle catégorie"
        },
        Some(if editing { "Modifier" } else { "Ajouter" }),
        560,
    );

    let preview = BadgePreview::new(&draft.icon, &draft.color, 40);
    let name = entry("Ex: Loisirs", &draft.name);
    let name_row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    name_row.append(&preview.widget());
    name_row.append(&field("Nom", &name));
    shell.body().append(&name_row);

    let icons_grid = IconGrid::new(&ICON_PICKER, &draft.icon, 12, 180);
    shell.body().append(&field("Icône", &icons_grid.widget()));
    let colors = ColorGrid::new(&CATEGORY_COLORS, &draft.color, 17, 150);
    shell.body().append(&field("Couleur", &colors.widget()));

    {
        let preview = preview.clone();
        let colors = colors.clone();
        icons_grid.connect_changed(move |icon| preview.set(&icon, &colors.value()));
    }
    {
        let preview = preview.clone();
        let icons_grid = icons_grid.clone();
        colors.connect_changed(move |color| preview.set(&icons_grid.value(), &color));
    }

    {
        let store = store.clone();
        let shell = shell.clone();
        shell.clone().on_submit(move || {
            let draft = CategoryDraft {
                id: draft.id.clone(),
                name: name.text().to_string(),
                icon: icons_grid.value(),
                color: colors.value(),
            };
            let error = store.attempt(|engine| engine.save_category(draft));
            shell.finish(
                &store,
                error,
                if editing {
                    "Catégorie modifiée"
                } else {
                    "Catégorie ajoutée"
                },
            );
        });
    }
    shell.present(parent);
}

// --- Transaction ---

pub fn transaction(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, id: Option<String>) {
    let accounts = store.selected_accounts();
    let today = store.today();
    let Some(draft) = store.read(|engine| engine.transaction_draft(id.as_deref(), &accounts, today)) else {
        return;
    };
    let editing = draft.id.is_some();
    let shell = FormShell::new(
        if editing {
            "Modifier la transaction"
        } else {
            "Nouvelle transaction"
        },
        Some("Enregistrer"),
        560,
    );

    let kind = KindPicker::new(draft.kind);
    shell.body().append(&field("Type", &kind.widget()));
    let description = entry("Ex: Loyer", &draft.description);
    shell.body().append(&field("Description", &description));
    let amount = amount_entry(draft.amount);
    let date = DateButton::new(&draft.date);
    shell
        .body()
        .append(&pair(&field("Montant", &amount), &field("Date", &date.widget())));

    let account_select = Select::new(
        "Sélectionner un compte",
        account_options(store),
        Some(draft.account_id.clone()),
        None,
    );
    let account_label = widgets::section_label("Compte");
    let account_field = gtk::Box::new(gtk::Orientation::Vertical, 4);
    account_field.append(&account_label);
    account_field.append(&account_select.widget());
    shell.body().append(&account_field);

    let to_select = Select::new(
        "Sélectionner un compte",
        account_options(store),
        draft.to_account_id.clone(),
        None,
    );
    let to_field = field("Compte destination", &to_select.widget());
    shell.body().append(&to_field);
    let category_select = Select::new(
        "Sélectionner une catégorie",
        category_options(store),
        Some(draft.category_id.clone()),
        None,
    );
    let category_field = field("Catégorie", &category_select.widget());
    shell.body().append(&category_field);

    let sync_kind = {
        let to_field = to_field.clone();
        let category_field = category_field.clone();
        let account_label = account_label.clone();
        move |kind: TransactionType| {
            let transfer = kind == TransactionType::Transfer;
            to_field.set_visible(transfer);
            category_field.set_visible(!transfer);
            account_label.set_text(if transfer { "Compte source" } else { "Compte" });
        }
    };
    sync_kind(draft.kind);
    kind.connect_changed(sync_kind);

    {
        let store = store.clone();
        let shell = shell.clone();
        shell.clone().on_submit(move || {
            let value = match required_amount(&amount) {
                Ok(value) => value,
                Err(message) => return shell.set_error(Some(&message)),
            };
            let kind_value = kind.value();
            let draft = TransactionDraft {
                id: draft.id.clone(),
                kind: kind_value,
                date: date.value(),
                amount: value,
                description: description.text().to_string(),
                category_id: category_select.required_value(),
                account_id: account_select.required_value(),
                to_account_id: to_select.value(),
            };
            let error = store.attempt(|engine| engine.save_transaction(draft));
            let success = if editing {
                "Transaction mise à jour"
            } else if kind_value == TransactionType::Transfer {
                "Virement ajouté"
            } else {
                "Transaction ajoutée"
            };
            shell.finish(&store, error, success);
        });
    }
    shell.present(parent);
}

// --- Budget ---

pub fn budget(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, id: Option<String>, category_id: Option<String>) {
    let accounts = store.selected_accounts();
    let Some(mut draft) = store.read(|engine| engine.budget_draft(id.as_deref(), &accounts)) else {
        return;
    };
    // Créer un budget depuis une catégorie de la page Budget : catégorie et nom préremplis.
    if let Some(category_id) = category_id {
        if let Some(name) = store.category_name(&category_id) {
            if draft.name.is_empty() {
                draft.name = name;
            }
        }
        draft.category_id = category_id;
    }
    let editing = draft.id.is_some();
    let shell = FormShell::new(
        if editing {
            "Modifier le budget"
        } else {
            "Nouveau budget"
        },
        Some(if editing { "Modifier" } else { "Créer" }),
        520,
    );

    let name = entry("Ex: Courses, Essence, Loisirs", &draft.name);
    shell.body().append(&field("Nom", &name));
    let amount = amount_entry(draft.amount);
    shell.body().append(&field("Montant mensuel", &amount));
    let category_select = Select::new(
        "Sélectionner",
        category_options(store),
        Some(draft.category_id.clone()),
        None,
    );
    shell.body().append(&field("Catégorie", &category_select.widget()));
    let account_select = Select::new(
        "Tous les comptes",
        account_options(store),
        draft.account_id.clone(),
        Some("Tous les comptes"),
    );
    shell.body().append(&field("Compte", &account_select.widget()));

    {
        let store = store.clone();
        let shell = shell.clone();
        shell.clone().on_submit(move || {
            let value = match required_amount(&amount) {
                Ok(value) => value,
                Err(message) => return shell.set_error(Some(&message)),
            };
            let draft = BudgetDraft {
                id: draft.id.clone(),
                name: name.text().to_string(),
                amount: value,
                category_id: category_select.required_value(),
                account_id: account_select.value(),
            };
            let error = store.attempt(|engine| engine.save_budget(draft));
            shell.finish(&store, error, if editing { "Budget modifié" } else { "Budget ajouté" });
        });
    }
    shell.present(parent);
}

// --- Échéance ---

pub fn scheduled(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, id: Option<String>) {
    let today = store.today();
    let Some(draft) = store.read(|engine| engine.scheduled_draft(id.as_deref(), today)) else {
        return;
    };
    let budgets = store
        .read(|engine| engine.snapshot())
        .map(|snapshot| snapshot.budgets.clone())
        .unwrap_or_default();
    let editing = draft.id.is_some();
    let shell = FormShell::new(
        if editing {
            "Modifier la transaction"
        } else {
            "Nouvelle transaction récurrente"
        },
        Some(if editing { "Modifier" } else { "Ajouter" }),
        580,
    );

    let kind = KindPicker::new(draft.kind);
    shell.body().append(&field("Type", &kind.widget()));
    let description = entry("Ex: Loyer", &draft.description);
    shell.body().append(&field("Description", &description));

    let amount = amount_entry(draft.amount);
    let frequency_select = Select::new(
        "Fréquence",
        Periodicity::ALL
            .iter()
            .map(|value| SelectOption::simple(value.as_str(), value.form_label()))
            .collect(),
        Some(draft.frequency.as_str().to_string()),
        None,
    );
    shell.body().append(&pair(
        &field("Montant", &amount),
        &field("Fréquence", &frequency_select.widget()),
    ));

    let start = DateButton::new(&draft.next_date);
    let end_switch = gtk::Switch::new();
    end_switch.set_valign(gtk::Align::Center);
    end_switch.set_active(draft.end_date.is_some());
    let end = DateButton::new(draft.end_date.as_deref().unwrap_or(&draft.next_date));
    let end_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    end_row.append(&end_switch);
    end_row.append(&end.widget());
    end.widget().set_visible(draft.end_date.is_some());
    {
        let end = end.clone();
        end_switch.connect_state_set(move |_, state| {
            end.widget().set_visible(state);
            gtk::glib::Propagation::Proceed
        });
    }
    shell.body().append(&pair(
        &field("Date de début", &start.widget()),
        &field("Date de fin (optionnel)", &end_row),
    ));

    let budget_select = Select::new(
        "Hors budget",
        budgets
            .iter()
            .map(|budget| SelectOption::simple(&budget.id, &budget.name))
            .collect(),
        draft.budget_id.clone(),
        Some("Hors budget"),
    );
    let budget_field = field("Budget lié", &budget_select.widget());
    shell.body().append(&budget_field);

    let account_select = Select::new(
        "Sélectionner un compte",
        account_options(store),
        Some(draft.account_id.clone()),
        None,
    );
    let account_label = widgets::section_label("Compte");
    let account_field = gtk::Box::new(gtk::Orientation::Vertical, 4);
    account_field.append(&account_label);
    account_field.append(&account_select.widget());
    shell.body().append(&account_field);

    let to_select = Select::new(
        "Sélectionner un compte",
        account_options(store),
        draft.to_account_id.clone(),
        None,
    );
    let to_field = field("Compte destination", &to_select.widget());
    shell.body().append(&to_field);
    let category_select = Select::new(
        "Sélectionner une catégorie",
        category_options(store),
        Some(draft.category_id.clone()),
        None,
    );
    let category_field = field("Catégorie", &category_select.widget());
    shell.body().append(&category_field);

    // Un budget lié impose sa catégorie et, s'il en a un, son compte (règle du noyau).
    {
        let budgets = budgets.clone();
        let category_select = category_select.clone();
        let account_select = account_select.clone();
        budget_select.connect_changed(move |value| {
            let linked = value.and_then(|id| budgets.iter().find(|budget| budget.id == id).cloned());
            match linked {
                Some(budget) => {
                    category_select.set_value(Some(budget.category.clone()));
                    category_select.set_sensitive(false);
                    if let Some(account_id) = budget.account_id.clone() {
                        account_select.set_value(Some(account_id));
                        account_select.set_sensitive(false);
                    } else {
                        account_select.set_sensitive(true);
                    }
                }
                None => {
                    category_select.set_sensitive(true);
                    account_select.set_sensitive(true);
                }
            }
        });
    }
    if let Some(budget) = draft
        .budget_id
        .as_ref()
        .and_then(|id| budgets.iter().find(|budget| &budget.id == id))
    {
        category_select.set_sensitive(false);
        account_select.set_sensitive(budget.account_id.is_none());
    }

    let sync_kind = {
        let to_field = to_field.clone();
        let category_field = category_field.clone();
        let budget_field = budget_field.clone();
        let account_label = account_label.clone();
        move |kind: TransactionType| {
            let transfer = kind == TransactionType::Transfer;
            to_field.set_visible(transfer);
            category_field.set_visible(!transfer);
            budget_field.set_visible(kind == TransactionType::Expense);
            account_label.set_text(if transfer { "Compte source" } else { "Compte" });
        }
    };
    sync_kind(draft.kind);
    kind.connect_changed(sync_kind);

    {
        let store = store.clone();
        let shell = shell.clone();
        shell.clone().on_submit(move || {
            let value = match required_amount(&amount) {
                Ok(value) => value,
                Err(message) => return shell.set_error(Some(&message)),
            };
            let kind_value = kind.value();
            let draft = ScheduledDraft {
                id: draft.id.clone(),
                description: description.text().to_string(),
                amount: value,
                kind: kind_value,
                category_id: category_select.required_value(),
                account_id: account_select.required_value(),
                to_account_id: to_select.value(),
                frequency: Periodicity::parse(&frequency_select.required_value()),
                next_date: start.value(),
                end_date: end_switch.is_active().then(|| end.value()),
                budget_id: (kind_value == TransactionType::Expense)
                    .then(|| budget_select.value())
                    .flatten(),
            };
            let error = store.attempt(|engine| engine.save_scheduled(draft));
            shell.finish(
                &store,
                error,
                if editing {
                    "Échéance modifiée"
                } else {
                    "Échéance ajoutée"
                },
            );
        });
    }
    shell.present(parent);
}

// --- Transaction fictive (prédictions) ---

pub fn fake_transaction(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, id: Option<String>) {
    let accounts = store.selected_accounts();
    let today = store.today();
    let Some(draft) = store.read(|engine| engine.fake_transaction_draft(id.as_deref(), &accounts, today)) else {
        return;
    };
    let editing = draft.id.is_some();
    let shell = FormShell::new(
        if editing {
            "Modifier la transaction fictive"
        } else {
            "Nouvelle transaction fictive"
        },
        Some(if editing { "Enregistrer" } else { "Ajouter" }),
        560,
    );

    let kind = KindPicker::new(draft.kind);
    shell.body().append(&field("Type", &kind.widget()));
    let description = entry("Ex: Réparation voiture", &draft.description);
    shell.body().append(&field("Description", &description));
    let amount = amount_entry(draft.amount);
    let date = DateButton::new(&draft.date);
    shell
        .body()
        .append(&pair(&field("Montant", &amount), &field("Date", &date.widget())));

    let account_select = Select::new(
        "Sélectionner un compte",
        account_options(store),
        Some(draft.account_id.clone()),
        None,
    );
    let account_label = widgets::section_label("Compte");
    let account_field = gtk::Box::new(gtk::Orientation::Vertical, 4);
    account_field.append(&account_label);
    account_field.append(&account_select.widget());
    shell.body().append(&account_field);

    let to_select = Select::new(
        "Sélectionner un compte",
        account_options(store),
        draft.to_account_id.clone(),
        None,
    );
    let to_field = field("Compte destination", &to_select.widget());
    shell.body().append(&to_field);
    let category_select = Select::new(
        "Sélectionner une catégorie",
        category_options(store),
        Some(draft.category_id.clone()),
        None,
    );
    let category_field = field("Catégorie", &category_select.widget());
    shell.body().append(&category_field);

    let sync_kind = {
        let to_field = to_field.clone();
        let category_field = category_field.clone();
        let account_label = account_label.clone();
        move |kind: TransactionType| {
            let transfer = kind == TransactionType::Transfer;
            to_field.set_visible(transfer);
            category_field.set_visible(!transfer);
            account_label.set_text(if transfer { "Compte source" } else { "Compte" });
        }
    };
    sync_kind(draft.kind);
    kind.connect_changed(sync_kind);

    {
        let store = store.clone();
        let shell = shell.clone();
        shell.clone().on_submit(move || {
            let value = match required_amount(&amount) {
                Ok(value) => value,
                Err(message) => return shell.set_error(Some(&message)),
            };
            let draft = FakeTransactionDraft {
                id: draft.id.clone(),
                date: date.value(),
                description: description.text().to_string(),
                amount: value,
                kind: kind.value(),
                account_id: account_select.required_value(),
                to_account_id: to_select.value(),
                category_id: category_select.required_value(),
            };
            let error = store.attempt(|engine| engine.save_fake_transaction(draft, today));
            shell.finish(
                &store,
                error,
                if editing {
                    "Transaction fictive mise à jour"
                } else {
                    "Transaction fictive ajoutée"
                },
            );
        });
    }
    shell.present(parent);
}

// --- Suggestions ---

pub fn budget_suggestions(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>) {
    let shell = FormShell::new("Suggestions du journal", None, 620);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    shell.body().append(&list);

    let rebuild: Rc<dyn Fn()> = {
        let store = store.clone();
        let list = list.clone();
        Rc::new(move || {
            widgets::clear(&list);
            let query = BudgetQuery {
                accounts: store.selected_accounts(),
                search: String::new(),
                categories: Vec::new(),
            };
            let today = store.today();
            let suggestions = store
                .read(|engine| engine.budget(&query, today))
                .map(|view| view.suggestions)
                .unwrap_or_default();
            if suggestions.is_empty() {
                list.append(&widgets::empty_state(
                    "Sparkles",
                    "Aucune suggestion pour le moment",
                    None,
                ));
                return;
            }
            for suggestion in suggestions {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.add_css_class("card");
                row.append(&icons::badge(
                    &suggestion.category.icon,
                    &suggestion.category.color,
                    40,
                    false,
                ));
                let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
                labels.set_hexpand(true);
                let name = gtk::Label::new(Some(&suggestion.name));
                name.set_xalign(0.0);
                name.add_css_class("heading");
                labels.append(&name);
                let mut meta = vec![
                    suggestion.account_name.clone(),
                    format!(
                        "{} mois {}",
                        suggestion.month_count,
                        if suggestion.month_count > 1 {
                            "observés"
                        } else {
                            "observé"
                        }
                    ),
                ];
                if suggestion.current_month_spent > 0.0 {
                    meta.push(format!("{} ce mois-ci", format::money(suggestion.current_month_spent)));
                }
                labels.append(&widgets::caption(&meta.join(" · ")));
                row.append(&labels);
                row.append(&widgets::amount(&format::money(suggestion.amount), None));

                let accept = widgets::action_button("Ajouter", Some("Plus"), false);
                let store_for_accept = store.clone();
                let key = suggestion.key.clone();
                accept.connect_clicked(move |_| {
                    let key = key.clone();
                    let accounts = store_for_accept.selected_accounts();
                    let today = store_for_accept.today();
                    store_for_accept.run(Some("Budget ajouté"), |engine| {
                        engine.accept_budget_suggestion(&key, &accounts, today)
                    });
                });
                row.append(&accept);

                let dismiss = widgets::icon_button("Trash2", "Supprimer la suggestion");
                dismiss.add_css_class("dmx-expense");
                let store_for_dismiss = store.clone();
                let key = suggestion.key.clone();
                dismiss.connect_clicked(move |_| {
                    let key = key.clone();
                    store_for_dismiss.run(Some("Suggestion supprimée"), |engine| {
                        engine.apply_settings_change(SettingsChange::DismissBudgetSuggestion(key))
                    });
                });
                row.append(&dismiss);
                list.append(&row);
            }
        })
    };

    shell.follow(store, rebuild.clone());
    rebuild();
    shell.present(parent);
}

pub fn scheduled_suggestions(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>) {
    let shell = FormShell::new("Suggestions du journal", None, 660);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    shell.body().append(&list);

    let rebuild: Rc<dyn Fn()> = {
        let store = store.clone();
        let list = list.clone();
        Rc::new(move || {
            widgets::clear(&list);
            let query = ScheduledQuery {
                accounts: store.selected_accounts(),
                due_range: ScheduledDueRange::default(),
                search: String::new(),
                categories: Vec::new(),
                frequencies: Vec::new(),
            };
            let today = store.today();
            let suggestions = store
                .read(|engine| engine.scheduled(&query, today))
                .map(|view| view.suggestions)
                .unwrap_or_default();
            if suggestions.is_empty() {
                list.append(&widgets::empty_state(
                    "Sparkles",
                    "Aucune suggestion pour le moment",
                    None,
                ));
                return;
            }
            for suggestion in suggestions {
                let transfer = suggestion.transaction_type == TransactionType::Transfer;
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.add_css_class("card");
                row.append(&icons::badge(
                    if transfer {
                        "ArrowRightLeft"
                    } else {
                        &suggestion.category.icon
                    },
                    if transfer {
                        "#6366f1"
                    } else {
                        &suggestion.category.color
                    },
                    40,
                    false,
                ));
                let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
                labels.set_hexpand(true);
                let name = gtk::Label::new(Some(&suggestion.description));
                name.set_xalign(0.0);
                name.add_css_class("heading");
                labels.append(&name);
                labels.append(&widgets::caption(&format!(
                    "{} · {} · {} · prochaine le {}",
                    suggestion.account_name,
                    suggestion.frequency.label(),
                    format::plural(suggestion.occurrence_count as usize, "occurrence"),
                    format::day_short(&suggestion.next_date)
                )));
                row.append(&labels);
                row.append(&widgets::amount(
                    &format::signed(suggestion.amount, suggestion.transaction_type),
                    Some(match suggestion.transaction_type {
                        TransactionType::Income => "dmx-income",
                        TransactionType::Transfer => "dmx-transfer",
                        TransactionType::Expense => "dmx-expense",
                    }),
                ));

                let accept = widgets::action_button("Ajouter", Some("Plus"), false);
                let store_for_accept = store.clone();
                let key = suggestion.key.clone();
                accept.connect_clicked(move |_| {
                    let key = key.clone();
                    let accounts = store_for_accept.selected_accounts();
                    let today = store_for_accept.today();
                    store_for_accept.run(Some("Suggestion ajoutée à l'échéancier"), |engine| {
                        engine.accept_scheduled_suggestion(&key, &accounts, today)
                    });
                });
                row.append(&accept);

                let dismiss = widgets::icon_button("Trash2", "Supprimer la suggestion");
                dismiss.add_css_class("dmx-expense");
                let store_for_dismiss = store.clone();
                let key = suggestion.key.clone();
                dismiss.connect_clicked(move |_| {
                    let key = key.clone();
                    store_for_dismiss.run(Some("Suggestion supprimée"), |engine| {
                        engine.apply_settings_change(SettingsChange::DismissScheduledSuggestion(key))
                    });
                });
                row.append(&dismiss);
                list.append(&row);
            }
        })
    };

    shell.follow(store, rebuild.clone());
    rebuild();
    shell.present(parent);
}
