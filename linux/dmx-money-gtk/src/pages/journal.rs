//! Journal : tableau des opérations, édition en ligne, sélection multiple, filtres.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use dmx_core::journal::{BudgetStatus, CheckStatus, JournalQuery, JournalRow};
use dmx_core::models::TransactionType;
use gtk::{gio, glib};

use crate::store::{FormRequest, Route, Store};
use crate::{format, icons, widgets};

/// Options des filtres, identiques aux autres plateformes.
const TYPE_OPTIONS: [(&str, TransactionType); 3] = [
    ("Dépenses", TransactionType::Expense),
    ("Revenus", TransactionType::Income),
    ("Virements", TransactionType::Transfer),
];

const STATUS_OPTIONS: [(&str, CheckStatus); 2] = [
    ("Pointées", CheckStatus::Checked),
    ("Non pointées", CheckStatus::Unchecked),
];

const BUDGET_OPTIONS: [(&str, BudgetStatus); 2] = [
    ("Avec budget", BudgetStatus::Budgeted),
    ("Hors budget", BudgetStatus::Unbudgeted),
];

#[derive(Default)]
struct Filters {
    search: String,
    categories: Vec<String>,
    types: Vec<TransactionType>,
    statuses: Vec<CheckStatus>,
    budgets: Vec<BudgetStatus>,
}

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    model: gio::ListStore,
    selection: gtk::MultiSelection,
    summary: gtk::Label,
    selection_bar: gtk::Box,
    selection_label: gtk::Label,
    empty: gtk::Box,
    empty_message: gtk::Label,
    empty_action: gtk::Button,
    table: gtk::Widget,
    filters: Rc<RefCell<Filters>>,
    category_button: gtk::MenuButton,
    syncing: Rc<Cell<bool>>,
    selected_ids: Rc<RefCell<Vec<String>>>,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.set_margin_top(20);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_margin_bottom(12);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = widgets::title_label("Journal");
        title.set_hexpand(true);
        header.append(&title);
        let summary = widgets::caption("");
        header.append(&summary);
        let new_transaction = widgets::action_button("Nouvelle transaction", Some("Plus"), true);
        let store_for_new = store.clone();
        new_transaction.connect_clicked(move |_| store_for_new.present(FormRequest::Transaction(None)));
        header.append(&new_transaction);
        content.append(&header);

        let filters = Rc::new(RefCell::new(Filters::default()));
        let syncing = Rc::new(Cell::new(false));

        let filter_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Rechercher dans toutes les colonnes..."));
        search.set_width_request(300);
        filter_row.append(&search);

        let category_button = gtk::MenuButton::new();
        category_button.set_label("Toutes les catégories");
        filter_row.append(&category_button);

        let type_button = multi_select_button(
            "Tous les types",
            TYPE_OPTIONS.iter().map(|(label, _)| *label).collect(),
            {
                let filters = filters.clone();
                let store = store.clone();
                let syncing = syncing.clone();
                move |index, active| {
                    if syncing.get() {
                        return;
                    }
                    let value = TYPE_OPTIONS[index].1;
                    let mut filters = filters.borrow_mut();
                    if active {
                        filters.types.push(value);
                    } else {
                        filters.types.retain(|candidate| *candidate != value);
                    }
                    drop(filters);
                    store.navigate(Route::Transactions);
                }
            },
        );
        filter_row.append(&type_button);

        let status_button = multi_select_button(
            "Tous les états",
            STATUS_OPTIONS.iter().map(|(label, _)| *label).collect(),
            {
                let filters = filters.clone();
                let store = store.clone();
                let syncing = syncing.clone();
                move |index, active| {
                    if syncing.get() {
                        return;
                    }
                    let value = STATUS_OPTIONS[index].1;
                    let mut filters = filters.borrow_mut();
                    if active {
                        filters.statuses.push(value);
                    } else {
                        filters.statuses.retain(|candidate| *candidate != value);
                    }
                    drop(filters);
                    store.navigate(Route::Transactions);
                }
            },
        );
        filter_row.append(&status_button);

        let budget_button = multi_select_button(
            "Tous les budgets",
            BUDGET_OPTIONS.iter().map(|(label, _)| *label).collect(),
            {
                let filters = filters.clone();
                let store = store.clone();
                let syncing = syncing.clone();
                move |index, active| {
                    if syncing.get() {
                        return;
                    }
                    let value = BUDGET_OPTIONS[index].1;
                    let mut filters = filters.borrow_mut();
                    if active {
                        filters.budgets.push(value);
                    } else {
                        filters.budgets.retain(|candidate| *candidate != value);
                    }
                    drop(filters);
                    store.navigate(Route::Transactions);
                }
            },
        );
        filter_row.append(&budget_button);
        content.append(&filter_row);

        {
            let filters = filters.clone();
            let store = store.clone();
            search.connect_search_changed(move |entry| {
                filters.borrow_mut().search = entry.text().to_string();
                store.navigate(Route::Transactions);
            });
        }

        // Barre de sélection multiple
        let selection_bar = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        selection_bar.add_css_class("dmx-card");
        selection_bar.add_css_class("card");
        selection_bar.set_visible(false);
        let selection_label = gtk::Label::new(None);
        selection_label.add_css_class("heading");
        selection_bar.append(&selection_label);
        let toggle_selection = widgets::action_button("Pointer/Dépointer", Some("CheckCircle2"), false);
        toggle_selection.add_css_class("flat");
        selection_bar.append(&toggle_selection);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        selection_bar.append(&spacer);
        let cancel_selection = gtk::Button::with_label("Annuler");
        cancel_selection.add_css_class("flat");
        selection_bar.append(&cancel_selection);
        let delete_selection = widgets::action_button("Supprimer", Some("Trash2"), false);
        delete_selection.add_css_class("destructive-action");
        selection_bar.append(&delete_selection);
        content.append(&selection_bar);

        // Tableau
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::MultiSelection::new(Some(model.clone()));
        let column_view = gtk::ColumnView::new(Some(selection.clone()));
        column_view.set_show_row_separators(true);
        column_view.set_reorderable(false);
        column_view.set_vexpand(true);
        add_columns(&column_view, store);

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_child(Some(&column_view));
        let table_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        table_card.add_css_class("card");
        table_card.append(&scroll);
        table_card.set_vexpand(true);
        content.append(&table_card);

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        empty.set_visible(false);
        let empty_title = gtk::Label::new(Some("Aucune transaction"));
        empty_title.add_css_class("title-3");
        empty.append(&empty_title);
        let empty_message = gtk::Label::new(None);
        empty_message.add_css_class("dim-label");
        empty.append(&empty_message);
        let empty_action = widgets::action_button("Ajouter une transaction", Some("Plus"), true);
        empty_action.set_halign(gtk::Align::Center);
        let store_for_empty = store.clone();
        empty_action.connect_clicked(move |_| store_for_empty.present(FormRequest::Transaction(None)));
        empty.append(&empty_action);
        content.append(&empty);

        let page = Self {
            store: store.clone(),
            root: content.clone().upcast(),
            model,
            selection: selection.clone(),
            summary,
            selection_bar,
            selection_label,
            empty,
            empty_message,
            empty_action,
            table: table_card.clone().upcast(),
            filters: filters.clone(),
            category_button: category_button.clone(),
            syncing: syncing.clone(),
            selected_ids: Rc::new(RefCell::new(Vec::new())),
        };

        // Sélection → barre d'actions
        {
            let selection_bar = page.selection_bar.clone();
            let selection_label = page.selection_label.clone();
            let selected_ids = page.selected_ids.clone();
            let model = page.model.clone();
            selection.connect_selection_changed(move |selection, _, _| {
                let mut ids = Vec::new();
                for index in 0..model.n_items() {
                    if selection.is_selected(index) {
                        if let Some(row) = row_at(&model, index) {
                            ids.push(row.transaction.id.clone());
                        }
                    }
                }
                selection_label.set_text(&format!(
                    "{} {}",
                    ids.len(),
                    if ids.len() > 1 {
                        "sélectionnées"
                    } else {
                        "sélectionnée"
                    }
                ));
                selection_bar.set_visible(!ids.is_empty());
                *selected_ids.borrow_mut() = ids;
            });
        }

        {
            let store = store.clone();
            let selected_ids = page.selected_ids.clone();
            toggle_selection.connect_clicked(move |_| {
                let ids = selected_ids.borrow().clone();
                if ids.is_empty() {
                    return;
                }
                store.run(None, move |engine| engine.toggle_transactions_checked(&ids));
            });
        }

        {
            let selection = selection.clone();
            cancel_selection.connect_clicked(move |_| {
                selection.unselect_all();
            });
        }

        {
            let store = store.clone();
            let selected_ids = page.selected_ids.clone();
            delete_selection.connect_clicked(move |_| {
                let ids = selected_ids.borrow().clone();
                if ids.is_empty() {
                    return;
                }
                let store_for_action = store.clone();
                store.confirm(
                    "Supprimer la sélection",
                    &format!(
                        "Voulez-vous vraiment supprimer {} ?",
                        format::plural(ids.len(), "transaction")
                    ),
                    "Tout supprimer",
                    move || {
                        let ids = ids.clone();
                        store_for_action.run(Some("Transactions supprimées"), move |engine| {
                            engine.delete_transactions(&ids)
                        });
                    },
                );
            });
        }

        // Filtre de catégories
        let popover = gtk::Popover::new();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        list.set_margin_start(8);
        list.set_margin_end(8);
        let scroll_categories = gtk::ScrolledWindow::new();
        scroll_categories.set_max_content_height(360);
        scroll_categories.set_propagate_natural_height(true);
        scroll_categories.set_child(Some(&list));
        popover.set_child(Some(&scroll_categories));
        category_button.set_popover(Some(&popover));
        for category in store.categories() {
            let check = gtk::CheckButton::with_label(&category.name);
            let filters = filters.clone();
            let store_for_check = store.clone();
            let syncing_for_check = syncing.clone();
            let id = category.id.clone();
            check.connect_toggled(move |check| {
                if syncing_for_check.get() {
                    return;
                }
                let mut filters = filters.borrow_mut();
                if check.is_active() {
                    filters.categories.push(id.clone());
                } else {
                    filters.categories.retain(|candidate| candidate != &id);
                }
                drop(filters);
                store_for_check.navigate(Route::Transactions);
            });
            list.append(&check);
        }

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let filters = self.filters.borrow();
        let query = JournalQuery {
            accounts: self.store.selected_accounts(),
            search: filters.search.clone(),
            categories: filters.categories.clone(),
            types: filters.types.clone(),
            statuses: filters.statuses.clone(),
            budget_statuses: filters.budgets.clone(),
        };
        let category_count = filters.categories.len();
        drop(filters);

        let Some(view) = self.store.read(|engine| engine.journal(&query)) else {
            return;
        };

        self.syncing.set(true);
        let selected = self.selected_ids.borrow().clone();
        self.model.remove_all();
        for row in &view.rows {
            self.model.append(&glib::BoxedAnyObject::new(row.clone()));
        }
        for (index, row) in view.rows.iter().enumerate() {
            if selected.contains(&row.transaction.id) {
                self.selection.select_item(index as u32, false);
            }
        }
        self.syncing.set(false);

        self.summary.set_text(&format!(
            "{} / {} lignes · Net {}{}",
            view.rows.len(),
            view.total_transaction_count,
            if view.visible_net >= 0.0 { "+" } else { "" },
            format::money(view.visible_net)
        ));

        let is_empty = view.rows.is_empty();
        self.empty.set_visible(is_empty);
        self.table.set_visible(!is_empty);
        self.empty_message.set_text(if view.has_filters {
            "Aucun résultat pour vos filtres actuels."
        } else {
            "Commencez par ajouter une transaction ou importez un relevé bancaire."
        });
        self.empty_action.set_visible(!view.has_filters);

        self.category_button.set_label(&if category_count == 0 {
            "Toutes les catégories".to_string()
        } else {
            format!("Catégories ({category_count})")
        });
    }
}

fn row_at(model: &gio::ListStore, index: u32) -> Option<JournalRow> {
    model
        .item(index)
        .and_downcast::<glib::BoxedAnyObject>()
        .map(|object| object.borrow::<JournalRow>().clone())
}

fn multi_select_button(
    label: &'static str,
    options: Vec<&'static str>,
    on_toggle: impl Fn(usize, bool) + Clone + 'static,
) -> gtk::MenuButton {
    let button = gtk::MenuButton::new();
    button.set_label(label);
    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.set_margin_top(8);
    list.set_margin_bottom(8);
    list.set_margin_start(8);
    list.set_margin_end(8);
    for (index, option) in options.iter().enumerate() {
        let check = gtk::CheckButton::with_label(option);
        let on_toggle = on_toggle.clone();
        check.connect_toggled(move |check| on_toggle(index, check.is_active()));
        list.append(&check);
    }
    popover.set_child(Some(&list));
    button.set_popover(Some(&popover));
    button
}

/// Colonnes du journal : compte, date, catégorie, description, montant, budget, état, solde, actions.
fn add_columns(view: &gtk::ColumnView, store: &Rc<Store>) {
    view.append_column(&text_column("Compte", 150, |row| {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.set_size_request(4, 16);
        bar.add_css_class("dmx-badge");
        bar.add_css_class(&icons::fill_class(&row.account_color));
        bar.set_valign(gtk::Align::Center);
        host.append(&bar);
        let label = gtk::Label::new(Some(&row.account_name));
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_xalign(0.0);
        host.append(&label);
        host.upcast()
    }));

    view.append_column(&text_column("Date", 90, |row| {
        let label = gtk::Label::new(Some(&format::day_short(&row.transaction.date)));
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.upcast()
    }));

    view.append_column(&text_column("Catégorie", 160, |row| {
        widgets::chip(&row.category.name, &row.category.color, Some(&row.category.icon)).upcast()
    }));

    // Description : modifiable au clic.
    let description_store = store.clone();
    view.append_column(&editable_column(
        "Description",
        240,
        move |row, text| {
            let text = text.trim().to_string();
            if text.is_empty() || text == row.transaction.description {
                return;
            }
            let id = row.transaction.id.clone();
            description_store.run(Some("Transaction mise à jour"), move |engine| {
                engine.update_transaction_inline(&id, dmx_core::ops::InlineEdit::Description(text))
            });
        },
        |row| row.transaction.description.clone(),
    ));

    // Montant : modifiable au clic (le signe vient du type d'opération).
    let amount_store = store.clone();
    view.append_column(&editable_column(
        "Montant",
        130,
        move |row, text| match format::parse_amount(&text).map(f64::abs) {
            Some(amount) if amount > 0.0 => {
                let id = row.transaction.id.clone();
                amount_store.run(Some("Transaction mise à jour"), move |engine| {
                    engine.update_transaction_inline(&id, dmx_core::ops::InlineEdit::Amount(amount))
                });
            }
            _ => amount_store.show_error("Saisissez un montant valide"),
        },
        |row| format::signed(row.transaction.amount, row.transaction.transaction_type),
    ));

    view.append_column(&text_column("Budget restant", 130, |row| match &row.budget {
        Some(budget) => {
            let chip = widgets::chip(&format::money(budget.remaining), "#6366f1", None);
            chip.set_tooltip_text(Some(&budget.budget_name));
            chip.upcast()
        }
        None => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
    }));

    let status_store = store.clone();
    view.append_column(&text_column("État", 70, move |row| {
        let button = gtk::Button::new();
        button.add_css_class("flat");
        let checked = row.transaction.checked;
        let icon = icons::image(if checked { "CheckCircle2" } else { "Circle" }, 18);
        if checked {
            icon.add_css_class("dmx-income");
        } else {
            icon.add_css_class("dim-label");
        }
        button.set_child(Some(&icon));
        button.set_tooltip_text(Some(if checked { "Dépointer" } else { "Pointer" }));
        let store = status_store.clone();
        let id = row.transaction.id.clone();
        button.connect_clicked(move |_| {
            let ids = vec![id.clone()];
            store.run(None, move |engine| engine.toggle_transactions_checked(&ids));
        });
        button.upcast()
    }));

    view.append_column(&text_column("Solde", 120, |row| {
        let label = widgets::amount(&format::money(row.balance), Some("dim-label"));
        label.upcast()
    }));

    let actions_store = store.clone();
    view.append_column(&text_column("", 90, move |row| {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let edit = widgets::icon_button("Edit2", "Modifier");
        let store = actions_store.clone();
        let id = row.transaction.id.clone();
        edit.connect_clicked(move |_| store.present(FormRequest::Transaction(Some(id.clone()))));
        host.append(&edit);

        let delete = widgets::icon_button("Trash2", "Supprimer");
        delete.add_css_class("dmx-expense");
        let store = actions_store.clone();
        let id = row.transaction.id.clone();
        delete.connect_clicked(move |_| {
            let store_for_action = store.clone();
            let id = id.clone();
            store.confirm(
                "Supprimer",
                "Voulez-vous vraiment supprimer cette transaction ?",
                "Supprimer",
                move || {
                    let ids = vec![id.clone()];
                    store_for_action.run(Some("Transaction supprimée"), move |engine| {
                        engine.delete_transactions(&ids)
                    });
                },
            );
        });
        host.append(&delete);
        host.upcast()
    }));
}

/// Colonne dont chaque cellule est reconstruite à partir de la ligne.
fn text_column(title: &str, width: i32, build: impl Fn(&JournalRow) -> gtk::Widget + 'static) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ColumnViewCell>() else {
            return;
        };
        let Some(row) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
            return;
        };
        let row = row.borrow::<JournalRow>().clone();
        let child = build(&row);
        child.set_valign(gtk::Align::Center);
        item.set_child(Some(&child));
    });
    let column = gtk::ColumnViewColumn::new(Some(title), Some(factory));
    if width > 0 {
        column.set_fixed_width(width);
    } else {
        column.set_expand(true);
    }
    column
}

/// Colonne éditable (description, montant) : `GtkEditableLabel` valide à la fin de l'édition.
fn editable_column(
    title: &str,
    width: i32,
    commit: impl Fn(&JournalRow, String) + Clone + 'static,
    text: impl Fn(&JournalRow) -> String + Clone + 'static,
) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ColumnViewCell>() else {
            return;
        };
        let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
            return;
        };
        let row = object.borrow::<JournalRow>().clone();
        let original = text(&row);
        let label = gtk::EditableLabel::new(&original);
        label.set_valign(gtk::Align::Center);
        let commit = commit.clone();
        let row_for_commit = row.clone();
        label.connect_editing_notify(move |label| {
            // Quitter une cellule sans rien changer ne doit rien écrire ni rien signaler.
            if !label.is_editing() && label.text() != original {
                commit(&row_for_commit, label.text().to_string());
            }
        });
        item.set_child(Some(&label));
    });
    let column = gtk::ColumnViewColumn::new(Some(title), Some(factory));
    if width > 0 {
        column.set_fixed_width(width);
    } else {
        column.set_expand(true);
    }
    column
}
