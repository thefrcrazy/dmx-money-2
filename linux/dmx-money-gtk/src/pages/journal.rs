//! Journal : tableau des opérations, édition en ligne, sélection multiple, filtres.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
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
    category_choices: RefCell<Vec<(String, String)>>,
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
            category_choices: RefCell::new(Vec::new()),
            syncing: syncing.clone(),
            selected_ids: Rc::new(RefCell::new(Vec::new())),
        };

        // Sélection → barre d'actions
        {
            let selection_bar = page.selection_bar.clone();
            let selection_label = page.selection_label.clone();
            let selected_ids = page.selected_ids.clone();
            let model = page.model.clone();
            let syncing = syncing.clone();
            selection.connect_selection_changed(move |selection, _, _| {
                if syncing.get() {
                    return;
                }
                let selected = selection.selection();
                let ids = gtk::BitsetIter::init_first(&selected)
                    .into_iter()
                    .flat_map(|(rest, first)| std::iter::once(first).chain(rest))
                    .filter_map(|index| transaction_id_at(&model, index))
                    .collect::<Vec<_>>();
                update_selection_bar(&selection_bar, &selection_label, ids.len());
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

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let choices = self.store.categories();
        let valid = choices
            .iter()
            .map(|category| category.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.filters
            .borrow_mut()
            .categories
            .retain(|id| valid.contains(id.as_str()));
        let signature = choices
            .iter()
            .map(|category| (category.id.clone(), category.name.clone()))
            .collect::<Vec<_>>();
        if *self.category_choices.borrow() != signature {
            *self.category_choices.borrow_mut() = signature;
            let selected = self.filters.borrow().categories.clone();
            let filters = self.filters.clone();
            let store = self.store.clone();
            widgets::category_filter(&self.category_button, &choices, &selected, move |id, active| {
                {
                    let mut filters = filters.borrow_mut();
                    let selected = &mut filters.categories;
                    if active {
                        if !selected.contains(&id) {
                            selected.push(id);
                        }
                    } else {
                        selected.retain(|candidate| candidate != &id);
                    }
                }
                store.navigate(Route::Transactions);
            });
        }
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

        let Some(mut view) = self.store.read(|engine| engine.journal(&query)) else {
            return;
        };

        self.syncing.set(true);
        let selected: HashSet<_> = self.selected_ids.borrow().iter().cloned().collect();
        let row_count = view.rows.len();
        let mut kept = Vec::with_capacity(selected.len());
        let mut selected_indexes = Vec::with_capacity(selected.len());
        for (index, row) in view.rows.iter().enumerate() {
            if selected.contains(&row.transaction.id) {
                selected_indexes.push(index as u32);
                kept.push(row.transaction.id.clone());
            }
        }
        // Une notification du modèle, sans copier les données de chaque ligne.
        let objects: Vec<_> = std::mem::take(&mut view.rows)
            .into_iter()
            .map(glib::BoxedAnyObject::new)
            .collect();
        self.model.splice(0, self.model.n_items(), &objects);
        for index in selected_indexes {
            self.selection.select_item(index, false);
        }
        self.syncing.set(false);
        update_selection_bar(&self.selection_bar, &self.selection_label, kept.len());
        *self.selected_ids.borrow_mut() = kept;

        self.summary.set_text(&format!(
            "{} / {} lignes · Net {}{}",
            row_count,
            view.total_transaction_count,
            if view.visible_net >= 0.0 { "+" } else { "" },
            format::money(view.visible_net)
        ));

        let is_empty = row_count == 0;
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

fn update_selection_bar(bar: &gtk::Box, label: &gtk::Label, count: usize) {
    label.set_text(&format!(
        "{count} {}",
        if count > 1 { "sélectionnées" } else { "sélectionnée" }
    ));
    bar.set_visible(count > 0);
}

fn transaction_id_at(model: &gio::ListStore, index: u32) -> Option<String> {
    model
        .item(index)
        .and_downcast::<glib::BoxedAnyObject>()
        .map(|object| object.borrow::<JournalRow>().transaction.id.clone())
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
    view.append_column(&recycled_column("Compte", 150, |_| {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.set_size_request(4, 16);
        bar.add_css_class("dmx-badge");
        bar.set_valign(gtk::Align::Center);
        host.append(&bar);
        let label = gtk::Label::new(None);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_xalign(0.0);
        host.append(&label);
        RecycledCell::new(host, move |row| {
            bar.set_css_classes(&["dmx-badge", &icons::fill_class(&row.account_color)]);
            label.set_text(&row.account_name);
        })
    }));

    view.append_column(&recycled_column("Date", 90, |_| {
        let label = gtk::Label::new(None);
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        RecycledCell::new(label.clone(), move |row| {
            label.set_text(&format::day_short(&row.transaction.date))
        })
    }));

    view.append_column(&recycled_column("Catégorie", 160, |_| {
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        chip.set_halign(gtk::Align::Start);
        let image = icons::image("Tag", 11);
        let label = gtk::Label::new(None);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        chip.append(&image);
        chip.append(&label);
        RecycledCell::new(chip.clone(), move |row| {
            chip.set_css_classes(&["dmx-chip", &icons::tint_class(&row.category.color)]);
            image.set_icon_name(Some(&icons::icon_name(&row.category.icon)));
            label.set_text(&row.category.name);
        })
    }));

    // Description : modifiable au clic.
    let description_store = store.clone();
    view.append_column(&editable_column(
        "Description",
        240,
        move |id, original, text| {
            let text = text.trim().to_string();
            if text.is_empty() || text == original {
                return;
            }
            let id = id.to_string();
            let original = original.to_string();
            description_store.run(Some("Transaction mise à jour"), move |engine| {
                engine.update_transaction_inline_with_base(
                    &id,
                    dmx_core::ops::InlineEdit::Description(text),
                    dmx_core::ops::InlineEdit::Description(original.to_string()),
                )
            });
        },
        |row| row.transaction.description.clone(),
        |row| row.transaction.description.clone(),
    ));

    // Montant : modifiable au clic (le signe vient du type d'opération).
    let amount_store = store.clone();
    view.append_column(&editable_column(
        "Montant",
        130,
        move |id, original, text| match format::parse_amount(&text).map(f64::abs) {
            Some(amount) if amount > 0.0 => {
                let id = id.to_string();
                let Some(base) = format::parse_amount(original) else {
                    return;
                };
                amount_store.run(Some("Transaction mise à jour"), move |engine| {
                    engine.update_transaction_inline_with_base(
                        &id,
                        dmx_core::ops::InlineEdit::Amount(amount),
                        dmx_core::ops::InlineEdit::Amount(base),
                    )
                });
            }
            _ => amount_store.show_error("Saisissez un montant valide"),
        },
        |row| format::signed(row.transaction.amount, row.transaction.transaction_type),
        |row| row.transaction.amount.to_string(),
    ));

    view.append_column(&recycled_column("Budget restant", 130, |_| {
        let chip = widgets::chip("", "#6366f1", None);
        let label = chip.first_child().and_downcast::<gtk::Label>().expect("budget label");
        RecycledCell::new(chip.clone(), move |row| {
            chip.set_visible(row.budget.is_some());
            if let Some(budget) = &row.budget {
                label.set_text(&format::money(budget.remaining));
                chip.set_tooltip_text(Some(&budget.budget_name));
            } else {
                chip.set_tooltip_text(None);
            }
        })
    }));

    let status_store = store.clone();
    view.append_column(&recycled_column("État", 70, move |cell| {
        let button = gtk::Button::new();
        button.add_css_class("flat");
        let icon = icons::image("Circle", 18);
        button.set_child(Some(&icon));
        let store = status_store.clone();
        let cell = cell.downgrade();
        button.connect_clicked(move |_| {
            if let Some(id) = cell.upgrade().and_then(|cell| transaction_id_for_cell(&cell)) {
                store.run(None, move |engine| engine.toggle_transactions_checked(&[id]));
            }
        });
        RecycledCell::new(button.clone(), move |row| {
            let checked = row.transaction.checked;
            icon.set_icon_name(Some(&icons::icon_name(if checked { "CheckCircle2" } else { "Circle" })));
            icon.set_css_classes(&[if checked { "dmx-income" } else { "dim-label" }]);
            button.set_tooltip_text(Some(if checked { "Dépointer" } else { "Pointer" }));
        })
    }));

    view.append_column(&recycled_column("Solde", 120, |_| {
        let label = widgets::amount("", Some("dim-label"));
        RecycledCell::new(label.clone(), move |row| label.set_text(&format::money(row.balance)))
    }));

    let actions_store = store.clone();
    view.append_column(&recycled_column("", 90, move |cell| {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let edit = widgets::icon_button("Edit2", "Modifier");
        let store = actions_store.clone();
        let edit_cell = cell.downgrade();
        edit.connect_clicked(move |_| {
            if let Some(id) = edit_cell.upgrade().and_then(|cell| transaction_id_for_cell(&cell)) {
                store.present(FormRequest::Transaction(Some(id)));
            }
        });
        host.append(&edit);

        let delete = widgets::icon_button("Trash2", "Supprimer");
        delete.add_css_class("dmx-expense");
        let store = actions_store.clone();
        let delete_cell = cell.downgrade();
        delete.connect_clicked(move |_| {
            let Some(id) = delete_cell.upgrade().and_then(|cell| transaction_id_for_cell(&cell)) else {
                return;
            };
            let store_for_action = store.clone();
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
        RecycledCell::new(host, |_| {})
    }));
}

fn transaction_id_for_cell(cell: &gtk::ColumnViewCell) -> Option<String> {
    cell.item()
        .and_downcast::<glib::BoxedAnyObject>()
        .map(|object| object.borrow::<JournalRow>().transaction.id.clone())
}

struct RecycledCell {
    widget: gtk::Widget,
    update: Box<dyn Fn(&JournalRow)>,
    unbind: Box<dyn Fn()>,
}

impl RecycledCell {
    fn new(widget: impl IsA<gtk::Widget>, update: impl Fn(&JournalRow) + 'static) -> Self {
        Self {
            widget: widget.upcast(),
            update: Box::new(update),
            unbind: Box::new(|| {}),
        }
    }
}

/// GTK recycles the cell, its widgets and handlers; binding changes only presentation.
fn recycled_column(
    title: &str,
    width: i32,
    setup: impl Fn(&gtk::ColumnViewCell) -> RecycledCell + 'static,
) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    let cells: Rc<RefCell<HashMap<usize, RecycledCell>>> = Rc::new(RefCell::new(HashMap::new()));
    let setup_cells = cells.clone();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ColumnViewCell>() else {
            return;
        };
        let cell = setup(item);
        cell.widget.set_valign(gtk::Align::Center);
        item.set_child(Some(&cell.widget));
        setup_cells.borrow_mut().insert(item.as_ptr() as usize, cell);
    });
    let bind_cells = cells.clone();
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ColumnViewCell>() else {
            return;
        };
        let Some(row) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
            return;
        };
        if let Some(cell) = bind_cells.borrow().get(&(item.as_ptr() as usize)) {
            (cell.update)(&row.borrow::<JournalRow>());
        }
    });
    let unbind_cells = cells.clone();
    factory.connect_unbind(move |_, item| {
        if let Some(cell) = unbind_cells.borrow().get(&(item.as_ptr() as usize)) {
            (cell.unbind)();
        }
    });
    factory.connect_teardown(move |_, item| {
        let cell = cells.borrow_mut().remove(&(item.as_ptr() as usize));
        if let Some(cell) = cell {
            (cell.unbind)();
        }
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
    commit: impl Fn(&str, &str, String) + Clone + 'static,
    text: impl Fn(&JournalRow) -> String + Clone + 'static,
    base: impl Fn(&JournalRow) -> String + Clone + 'static,
) -> gtk::ColumnViewColumn {
    recycled_column(title, width, move |_| {
        let label = gtk::EditableLabel::new("");
        let current: Rc<RefCell<Option<(String, String, String)>>> = Rc::new(RefCell::new(None));
        let commit = commit.clone();
        let edit_current = current.clone();
        label.connect_editing_notify(move |label| {
            if !label.is_editing() {
                let binding = edit_current.borrow().clone();
                if let Some((id, original, base)) = binding {
                    if label.text() != original {
                        commit(&id, &base, label.text().to_string());
                    }
                }
            }
        });
        let bind_current = current.clone();
        let bind_label = label.clone();
        let text = text.clone();
        let base = base.clone();
        let mut cell = RecycledCell::new(label.clone(), move |row| {
            bind_current.borrow_mut().take();
            bind_label.stop_editing(false);
            let original = text(row);
            bind_label.set_text(&original);
            *bind_current.borrow_mut() = Some((row.transaction.id.clone(), original, base(row)));
        });
        cell.unbind = Box::new(move || {
            // Recycling an editor must not write its draft to the next operation.
            current.borrow_mut().take();
            label.stop_editing(false);
        });
        cell
    })
}
