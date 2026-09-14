//! Échéancier : transactions récurrentes, plage d'affichage persistée, filtres et suggestions.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use dmx_core::models::{Periodicity, ScheduledDueRange, TransactionType};
use dmx_core::scheduled_view::{ScheduledQuery, ScheduledRow};

use crate::store::{FormRequest, Route, Store};
use crate::{format, icons, widgets};

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    suggestions: gtk::Button,
    range_box: gtk::Box,
    rows: gtk::Box,
    table: gtk::Box,
    empty: gtk::Box,
    empty_title: gtk::Label,
    empty_message: gtk::Label,
    category_button: gtk::MenuButton,
    frequency_button: gtk::MenuButton,
    search: Rc<RefCell<String>>,
    categories: Rc<RefCell<Vec<String>>>,
    frequencies: Rc<RefCell<Vec<Periodicity>>>,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = widgets::title_label("Transactions Récurrentes");
        title.set_hexpand(true);
        header.append(&title);
        let suggestions = widgets::action_button("Suggestions", Some("Sparkles"), false);
        suggestions.set_visible(false);
        let store_for_suggestions = store.clone();
        suggestions.connect_clicked(move |_| store_for_suggestions.present(FormRequest::ScheduledSuggestions));
        header.append(&suggestions);
        let new_scheduled = widgets::action_button("Nouvelle transaction", Some("Plus"), true);
        let store_for_new = store.clone();
        new_scheduled.connect_clicked(move |_| store_for_new.present(FormRequest::Scheduled(None)));
        header.append(&new_scheduled);
        content.append(&header);

        let range_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        range_box.add_css_class("linked");
        range_box.set_halign(gtk::Align::Start);
        content.append(&range_box);

        let filters = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Rechercher dans l'échéancier..."));
        search_entry.set_width_request(280);
        filters.append(&search_entry);
        let category_button = gtk::MenuButton::new();
        category_button.set_label("Toutes les catégories");
        filters.append(&category_button);
        let frequency_button = gtk::MenuButton::new();
        frequency_button.set_label("Toutes les fréquences");
        filters.append(&frequency_button);
        content.append(&filters);

        let table = gtk::Box::new(gtk::Orientation::Vertical, 0);
        table.add_css_class("card");
        let table_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        table_header.set_margin_top(10);
        table_header.set_margin_bottom(10);
        table_header.set_margin_start(16);
        table_header.set_margin_end(16);
        table_header.append(&widgets::column_header("Compte", 150, gtk::Align::Start));
        table_header.append(&widgets::column_header("Échéance", 170, gtk::Align::Start));
        table_header.append(&widgets::column_header("Fréquence", 140, gtk::Align::Start));
        table_header.append(&widgets::column_header("Catégorie", 160, gtk::Align::Start));
        table_header.append(&widgets::column_header("Description", 0, gtk::Align::Start));
        table_header.append(&widgets::column_header("Montant", 120, gtk::Align::End));
        table_header.append(&widgets::column_header("Actions", 80, gtk::Align::End));
        table.append(&table_header);
        table.append(&widgets::separator());
        let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
        table.append(&rows);
        content.append(&table);

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_visible(false);
        let empty_title = gtk::Label::new(None);
        empty_title.add_css_class("title-3");
        let empty_message = gtk::Label::new(None);
        empty_message.add_css_class("dim-label");
        empty.append(&empty_title);
        empty.append(&empty_message);
        empty.set_halign(gtk::Align::Center);
        empty.set_margin_top(40);
        content.append(&empty);

        let root = widgets::page_scroll(&content).upcast();
        let page = Self {
            store: store.clone(),
            root,
            suggestions,
            range_box: range_box.clone(),
            rows,
            table,
            empty,
            empty_title,
            empty_message,
            category_button: category_button.clone(),
            frequency_button: frequency_button.clone(),
            search: Rc::new(RefCell::new(String::new())),
            categories: Rc::new(RefCell::new(Vec::new())),
            frequencies: Rc::new(RefCell::new(Vec::new())),
        };

        // Plages d'échéance (réglage synchronisé).
        for range in ScheduledDueRange::ALL {
            let range = *range;
            let button = gtk::ToggleButton::with_label(range.label());
            let store_for_range = store.clone();
            button.connect_clicked(move |button| {
                if button.is_active() {
                    store_for_range.apply(dmx_core::settings::SettingsChange::ScheduledDueRange(range));
                }
            });
            range_box.append(&button);
        }

        {
            let search = page.search.clone();
            let store = store.clone();
            search_entry.connect_search_changed(move |entry| {
                *search.borrow_mut() = entry.text().to_string();
                store.navigate(Route::Scheduled);
            });
        }

        fill_check_popover(
            &category_button,
            store
                .categories()
                .iter()
                .map(|category| (category.id.clone(), category.name.clone()))
                .collect(),
            {
                let categories = page.categories.clone();
                let store = store.clone();
                move |id, active| {
                    let mut categories = categories.borrow_mut();
                    if active {
                        categories.push(id);
                    } else {
                        categories.retain(|candidate| candidate != &id);
                    }
                    drop(categories);
                    store.navigate(Route::Scheduled);
                }
            },
        );

        fill_check_popover(
            &frequency_button,
            Periodicity::ALL
                .iter()
                .map(|frequency| (frequency.as_str().to_string(), frequency.label().to_string()))
                .collect(),
            {
                let frequencies = page.frequencies.clone();
                let store = store.clone();
                move |id, active| {
                    let Some(frequency) = Periodicity::try_parse(&id) else {
                        return;
                    };
                    let mut frequencies = frequencies.borrow_mut();
                    if active {
                        frequencies.push(frequency);
                    } else {
                        frequencies.retain(|candidate| *candidate != frequency);
                    }
                    drop(frequencies);
                    store.navigate(Route::Scheduled);
                }
            },
        );

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let settings = self.store.settings();
        let query = ScheduledQuery {
            accounts: self.store.selected_accounts(),
            due_range: settings.scheduled_due_range,
            search: self.search.borrow().clone(),
            categories: self.categories.borrow().clone(),
            frequencies: self.frequencies.borrow().clone(),
        };
        let today = self.store.today();
        let Some(view) = self.store.read(|engine| engine.scheduled(&query, today)) else {
            return;
        };

        // Plage sélectionnée
        let mut child = self.range_box.first_child();
        let mut index = 0usize;
        while let Some(widget) = child {
            if let Some(button) = widget.downcast_ref::<gtk::ToggleButton>() {
                let is_current = ScheduledDueRange::ALL.get(index) == Some(&settings.scheduled_due_range);
                if button.is_active() != is_current {
                    button.set_active(is_current);
                }
            }
            child = widget.next_sibling();
            index += 1;
        }

        self.suggestions.set_visible(!view.suggestions.is_empty());
        if let Some(content) = self.suggestions.child().and_downcast::<gtk::Box>() {
            if let Some(label) = content.last_child().and_downcast::<gtk::Label>() {
                label.set_text(&format!("Suggestions ({})", view.suggestions.len()));
            }
        }

        self.category_button.set_label(&if self.categories.borrow().is_empty() {
            "Toutes les catégories".to_string()
        } else {
            format!("Catégories ({})", self.categories.borrow().len())
        });
        self.frequency_button
            .set_label(&if self.frequencies.borrow().is_empty() {
                "Toutes les fréquences".to_string()
            } else {
                format!("Fréquences ({})", self.frequencies.borrow().len())
            });

        widgets::clear(&self.rows);
        let is_empty = view.rows.is_empty();
        self.table.set_visible(!is_empty);
        self.empty.set_visible(is_empty);
        self.empty_title.set_text(if view.has_filters {
            "Aucune transaction récurrente ne correspond aux filtres."
        } else {
            "Aucune transaction récurrente configurée"
        });
        self.empty_message.set_text(if view.has_filters {
            ""
        } else {
            "Ajoute une transaction récurrente pour commencer."
        });

        for row in &view.rows {
            self.rows.append(&self.row(row));
            self.rows.append(&widgets::separator());
        }
    }

    fn row(&self, row: &ScheduledRow) -> gtk::Box {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        host.set_margin_top(10);
        host.set_margin_bottom(10);
        host.set_margin_start(16);
        host.set_margin_end(16);
        if row.is_ended {
            host.add_css_class("dmx-row-ended");
        }

        // Compte
        let account = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.set_size_request(4, 18);
        bar.add_css_class("dmx-badge");
        bar.add_css_class(&icons::fill_class(if row.is_ended {
            "#ef4444"
        } else {
            &row.account_color
        }));
        bar.set_valign(gtk::Align::Center);
        account.append(&bar);
        let names = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let name = gtk::Label::new(Some(&row.account_name));
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        names.append(&name);
        if let Some(destination) = &row.to_account_name {
            names.append(&widgets::caption(&format!("→ {destination}")));
        }
        account.append(&names);
        host.append(&widgets::cell(&account, 150, gtk::Align::Start));

        // Échéance
        let due = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let date_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        date_row.append(&icons::image("Calendar", 13));
        date_row.append(&gtk::Label::new(Some(&format::day_medium(&row.scheduled.next_date))));
        due.append(&date_row);
        if let Some(end) = &row.scheduled.end_date {
            due.append(&widgets::caption(&format!("→ {}", format::day_medium(end))));
        }
        if row.is_ended {
            due.append(&widgets::chip("Terminé", "#ef4444", None));
        } else if row.scheduled.budget_id.is_some() {
            due.append(&widgets::chip("Budget", "#6366f1", None));
        }
        host.append(&widgets::cell(&due, 170, gtk::Align::Start));

        // Fréquence
        let frequency = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        frequency.append(&icons::image("Clock", 13));
        frequency.append(&widgets::caption(&row.frequency_label));
        host.append(&widgets::cell(&frequency, 140, gtk::Align::Start));

        // Catégorie
        let is_transfer = matches!(row.scheduled.transaction_type, TransactionType::Transfer);
        let chip = if is_transfer {
            widgets::chip("Virement", "#6366f1", Some("ArrowRightLeft"))
        } else {
            widgets::chip(&row.category.name, &row.category.color, Some(&row.category.icon))
        };
        host.append(&widgets::cell(&chip, 160, gtk::Align::Start));

        // Description
        let description = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let label = gtk::Label::new(Some(&row.scheduled.description));
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        description.append(&label);
        if let Some(budget) = &row.budget_name {
            let linked = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            linked.append(&icons::colored_image("Tag", "#10b981", 11));
            let budget_label = widgets::caption(budget);
            budget_label.add_css_class("dmx-income");
            linked.append(&budget_label);
            description.append(&linked);
        }
        host.append(&widgets::cell(&description, 0, gtk::Align::Start));

        // Montant
        let amount_class = match row.scheduled.transaction_type {
            TransactionType::Income => "dmx-income",
            TransactionType::Transfer => "dmx-transfer",
            TransactionType::Expense => "dmx-expense",
        };
        let amount = widgets::amount(
            &format::signed(row.scheduled.amount, row.scheduled.transaction_type),
            Some(amount_class),
        );
        host.append(&widgets::cell(&amount, 120, gtk::Align::End));

        // Actions
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let edit = widgets::icon_button("Edit2", "Modifier");
        let store = self.store.clone();
        let id = row.scheduled.id.clone();
        edit.connect_clicked(move |_| store.present(FormRequest::Scheduled(Some(id.clone()))));
        actions.append(&edit);
        let delete = widgets::icon_button("Trash2", "Supprimer");
        delete.add_css_class("dmx-expense");
        let store = self.store.clone();
        let id = row.scheduled.id.clone();
        delete.connect_clicked(move |_| {
            let store_for_action = store.clone();
            let id = id.clone();
            store.confirm(
                "Supprimer la transaction récurrente",
                "Êtes-vous sûr de vouloir supprimer cette transaction récurrente ?",
                "Supprimer",
                move || {
                    let id = id.clone();
                    store_for_action.run(Some("Transaction récurrente supprimée"), move |engine| {
                        engine.delete_scheduled(&id)
                    });
                },
            );
        });
        actions.append(&delete);
        host.append(&widgets::cell(&actions, 80, gtk::Align::End));

        host
    }
}

/// Remplit un popover de cases à cocher (identifiant, libellé).
fn fill_check_popover(
    button: &gtk::MenuButton,
    options: Vec<(String, String)>,
    on_toggle: impl Fn(String, bool) + Clone + 'static,
) {
    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.set_margin_top(8);
    list.set_margin_bottom(8);
    list.set_margin_start(8);
    list.set_margin_end(8);
    for (id, label) in options {
        let check = gtk::CheckButton::with_label(&label);
        let on_toggle = on_toggle.clone();
        check.connect_toggled(move |check| on_toggle(id.clone(), check.is_active()));
        list.append(&check);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_max_content_height(360);
    scroll.set_propagate_natural_height(true);
    scroll.set_child(Some(&list));
    popover.set_child(Some(&scroll));
    button.set_popover(Some(&popover));
}
