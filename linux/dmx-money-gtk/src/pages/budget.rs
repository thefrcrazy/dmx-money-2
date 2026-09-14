//! Budget du mois : indicateurs, consommation, enveloppes par catégorie, suggestions.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use dmx_core::budget::{BudgetCategoryRow, BudgetEnvelope, BudgetQuery, BudgetState};

use crate::store::{FormRequest, Route, Store};
use crate::{format, icons, widgets};

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    subtitle: gtk::Label,
    suggestions: gtk::Button,
    kpis: gtk::Box,
    progress_host: gtk::Box,
    rows: gtk::Box,
    empty: gtk::Box,
    empty_title: gtk::Label,
    empty_message: gtk::Label,
    count_label: gtk::Label,
    category_button: gtk::MenuButton,
    search: Rc<RefCell<String>>,
    categories: Rc<RefCell<Vec<String>>>,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 2);
        titles.set_hexpand(true);
        titles.append(&widgets::title_label("Budget"));
        let subtitle = widgets::caption("");
        titles.append(&subtitle);
        header.append(&titles);
        let suggestions = widgets::action_button("Suggestions", Some("Sparkles"), false);
        suggestions.set_visible(false);
        let store_for_suggestions = store.clone();
        suggestions.connect_clicked(move |_| store_for_suggestions.present(FormRequest::BudgetSuggestions));
        header.append(&suggestions);
        let new_budget = widgets::action_button("Nouveau budget", Some("Plus"), true);
        let store_for_new = store.clone();
        new_budget.connect_clicked(move |_| store_for_new.present(FormRequest::Budget(None, None)));
        header.append(&new_budget);
        content.append(&header);

        let kpis = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        kpis.set_homogeneous(true);
        content.append(&kpis);

        let (progress_card, progress_host, _) = widgets::card(None, None);
        content.append(&progress_card);

        let (categories_card, categories_content, categories_actions) =
            widgets::card(Some("Budget par catégorie"), None);
        let count_label = widgets::caption("");
        categories_actions.append(&count_label);
        let filters = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Rechercher un budget..."));
        search_entry.set_width_request(300);
        filters.append(&search_entry);
        let category_button = gtk::MenuButton::new();
        category_button.set_label("Toutes les catégories");
        filters.append(&category_button);
        categories_content.append(&filters);
        categories_content.append(&widgets::separator());
        let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
        categories_content.append(&rows);
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_visible(false);
        let empty_title = gtk::Label::new(None);
        empty_title.add_css_class("heading");
        let empty_message = gtk::Label::new(None);
        empty_message.add_css_class("dim-label");
        empty.append(&empty_title);
        empty.append(&empty_message);
        categories_content.append(&empty);
        content.append(&categories_card);

        let root = widgets::page_scroll(&content).upcast();
        let page = Self {
            store: store.clone(),
            root,
            subtitle,
            suggestions,
            kpis,
            progress_host,
            rows,
            empty,
            empty_title,
            empty_message,
            count_label,
            category_button: category_button.clone(),
            search: Rc::new(RefCell::new(String::new())),
            categories: Rc::new(RefCell::new(Vec::new())),
        };

        {
            let search = page.search.clone();
            let store = store.clone();
            search_entry.connect_search_changed(move |entry| {
                *search.borrow_mut() = entry.text().to_string();
                store.navigate(Route::Budget);
            });
        }

        let popover = gtk::Popover::new();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        list.set_margin_start(8);
        list.set_margin_end(8);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_max_content_height(360);
        scroll.set_propagate_natural_height(true);
        scroll.set_child(Some(&list));
        popover.set_child(Some(&scroll));
        category_button.set_popover(Some(&popover));
        for category in store.selectable_categories() {
            let check = gtk::CheckButton::with_label(&category.name);
            let categories = page.categories.clone();
            let store_for_check = store.clone();
            let id = category.id.clone();
            check.connect_toggled(move |check| {
                let mut categories = categories.borrow_mut();
                if check.is_active() {
                    categories.push(id.clone());
                } else {
                    categories.retain(|candidate| candidate != &id);
                }
                drop(categories);
                store_for_check.navigate(Route::Budget);
            });
            list.append(&check);
        }

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let query = BudgetQuery {
            accounts: self.store.selected_accounts(),
            search: self.search.borrow().clone(),
            categories: self.categories.borrow().clone(),
        };
        let today = self.store.today();
        let Some(view) = self.store.read(|engine| engine.budget(&query, today)) else {
            return;
        };

        self.subtitle.set_text(&format!(
            "{} · budgets configurés et dépenses du Journal",
            view.month_label
        ));
        self.suggestions.set_visible(!view.suggestions.is_empty());
        if let Some(label) = self.suggestions.child().and_downcast::<gtk::Box>() {
            if let Some(text) = label.last_child().and_downcast::<gtk::Label>() {
                text.set_text(&format!("Suggestions ({})", view.suggestions.len()));
            }
        }

        // Indicateurs
        widgets::clear(&self.kpis);
        self.kpis.append(&kpi(
            "Prévu",
            "CalendarClock",
            "dmx-transfer",
            &format::rounded(view.total_budgeted),
            None,
            &format!(
                "{} {}",
                format::plural(view.budget_count as usize, "budget"),
                if view.budget_count > 1 {
                    "configurés"
                } else {
                    "configuré"
                }
            ),
        ));
        self.kpis.append(&kpi(
            "Dépensé",
            "TrendingDown",
            "dmx-expense",
            &format::rounded(view.total_spent),
            None,
            &format!("{} ce mois-ci", format::plural(view.expense_count as usize, "dépense")),
        ));
        let remaining_class = if view.remaining >= 0.0 {
            "dmx-income"
        } else {
            "dmx-expense"
        };
        self.kpis.append(&kpi(
            "Restant",
            "Wallet",
            remaining_class,
            &format::rounded(view.remaining),
            Some(remaining_class),
            &format!("{} / jour restant", format::rounded(view.remaining_per_day)),
        ));
        let (state_icon, state_class) = match view.state {
            BudgetState::ToConfigure => ("CalendarClock", "dim-label"),
            BudgetState::UnderControl => ("CheckCircle2", "dmx-income"),
            BudgetState::Overrun => ("AlertCircle", "dmx-expense"),
        };
        let pace = if view.pace_delta > 0.0 {
            format!("{} au-dessus du rythme", format::rounded(view.pace_delta))
        } else {
            format!("{} sous le rythme", format::rounded(view.pace_delta.abs()))
        };
        self.kpis.append(&kpi(
            "État",
            state_icon,
            state_class,
            view.state.label(),
            Some(state_class),
            &pace,
        ));

        // Consommation du mois
        widgets::clear(&self.progress_host);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 2);
        titles.set_hexpand(true);
        let title = gtk::Label::new(Some("Consommation du mois"));
        title.add_css_class("dmx-card-title");
        title.set_xalign(0.0);
        titles.append(&title);
        titles.append(&widgets::caption(
            "Le budget suit les enveloppes configurées et les dépenses du Journal.",
        ));
        head.append(&titles);
        head.append(&widgets::caption(&format!(
            "{} utilisé",
            format::percent_tight(view.progress)
        )));
        self.progress_host.append(&head);
        self.progress_host
            .append(&widgets::progress(view.progress / 100.0, view.progress > 100.0));
        let mut details: Vec<String> = vec![
            format!("{} dépensés", format::money(view.total_spent)),
            format!("{} prévus", format::money(view.total_budgeted)),
            format!(
                "{} théoriques au {}",
                format::money(view.expected_spend),
                view.today_label
            ),
        ];
        if view.over_budget_count > 0 {
            details.push(format::plural(view.over_budget_count as usize, "dépassement"));
        }
        if view.unbudgeted_count > 0 {
            details.push(format!(
                "{} non {}",
                format::plural(view.unbudgeted_count as usize, "catégorie"),
                if view.unbudgeted_count > 1 {
                    "budgétées"
                } else {
                    "budgétée"
                }
            ));
        }
        self.progress_host.append(&widgets::caption(&details.join("   ·   ")));

        // Budget par catégorie
        let budgeted = view.categories.iter().filter(|row| !row.is_unbudgeted).count();
        self.count_label
            .set_text(&if budgeted as u32 == view.budgeted_category_count {
                format!(
                    "{} {}",
                    view.budgeted_category_count,
                    if view.budgeted_category_count > 1 {
                        "catégories budgétées"
                    } else {
                        "catégorie budgétée"
                    }
                )
            } else {
                format!("{budgeted} / {} catégories budgétées", view.budgeted_category_count)
            });
        self.category_button.set_label(&if self.categories.borrow().is_empty() {
            "Toutes les catégories".to_string()
        } else {
            format!("Catégories ({})", self.categories.borrow().len())
        });

        widgets::clear(&self.rows);
        let is_empty = view.categories.is_empty();
        self.empty.set_visible(is_empty);
        self.rows.set_visible(!is_empty);
        self.empty_title.set_text(if view.budgeted_category_count == 0 {
            "Aucun budget configuré"
        } else {
            "Aucun budget ne correspond aux filtres."
        });
        self.empty_message.set_text(if view.budgeted_category_count == 0 {
            "Crée une enveloppe mensuelle, même sans échéance liée."
        } else {
            "Modifie la recherche ou les catégories sélectionnées."
        });

        for row in &view.categories {
            self.rows.append(&self.category_row(row));
        }
    }

    fn category_row(&self, row: &BudgetCategoryRow) -> gtk::Box {
        let host = gtk::Box::new(gtk::Orientation::Vertical, 12);
        host.set_margin_top(14);
        host.set_margin_bottom(14);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.append(&icons::badge(&row.category.icon, &row.category.color, 36, false));
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let name = gtk::Label::new(Some(&row.category.name));
        name.set_xalign(0.0);
        name.add_css_class("heading");
        labels.append(&name);
        labels.append(&widgets::caption(&if row.is_unbudgeted {
            format!("{} dépensés sans budget ce mois-ci", format::money(row.spent))
        } else {
            format!(
                "{} · {} {}",
                format::plural(row.envelopes.len(), "enveloppe"),
                format::plural(row.linked_scheduled_count as usize, "échéance"),
                if row.linked_scheduled_count > 1 {
                    "liées"
                } else {
                    "liée"
                }
            )
        }));
        header.append(&labels);

        if row.is_unbudgeted {
            let create = widgets::action_button("Créer un budget", Some("Plus"), false);
            let store = self.store.clone();
            let category_id = row.category.id.clone();
            create.connect_clicked(move |_| store.present(FormRequest::Budget(None, Some(category_id.clone()))));
            header.append(&create);
        } else if row.is_over_budget {
            header.append(&widgets::chip("Dépassé", "#ef4444", Some("AlertCircle")));
        } else {
            header.append(&widgets::chip("OK", "#10b981", Some("CheckCircle2")));
        }
        host.append(&header);

        for envelope in &row.envelopes {
            host.append(&self.envelope_row(envelope));
        }
        host.append(&widgets::separator());
        host
    }

    fn envelope_row(&self, envelope: &BudgetEnvelope) -> gtk::Box {
        let host = gtk::Box::new(gtk::Orientation::Vertical, 8);
        host.add_css_class("dmx-card");
        host.add_css_class("card");

        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let name = gtk::Label::new(Some(&envelope.budget.name));
        name.set_xalign(0.0);
        name.add_css_class("heading");
        labels.append(&name);
        labels.append(&widgets::caption(&envelope.account_name));
        line.append(&labels);
        line.append(&widgets::cell(
            &widgets::amount(&format::money(envelope.budget.amount), None),
            100,
            gtk::Align::End,
        ));
        line.append(&widgets::cell(
            &widgets::amount(&format::money(envelope.spent), None),
            100,
            gtk::Align::End,
        ));
        let is_over = envelope.remaining < 0.0;
        line.append(&widgets::cell(
            &widgets::amount(
                &format::money(envelope.remaining),
                Some(if is_over { "dmx-expense" } else { "dmx-income" }),
            ),
            100,
            gtk::Align::End,
        ));

        let edit = widgets::icon_button("Edit2", "Modifier");
        let store = self.store.clone();
        let id = envelope.budget.id.clone();
        edit.connect_clicked(move |_| store.present(FormRequest::Budget(Some(id.clone()), None)));
        line.append(&edit);

        let delete = widgets::icon_button("Trash2", "Supprimer");
        delete.add_css_class("dmx-expense");
        let store = self.store.clone();
        let id = envelope.budget.id.clone();
        delete.connect_clicked(move |_| {
            let store_for_action = store.clone();
            let id = id.clone();
            store.confirm(
                "Supprimer le budget",
                "Ce budget sera supprimé et les échéances liées seront simplement déliées.",
                "Supprimer",
                move || {
                    let id = id.clone();
                    store_for_action.run(Some("Budget supprimé"), move |engine| engine.delete_budget(&id));
                },
            );
        });
        line.append(&delete);
        host.append(&line);

        host.append(&widgets::progress(envelope.progress / 100.0, is_over));
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let used = widgets::caption(&format!("{} utilisé", format::percent_tight(envelope.progress)));
        used.set_hexpand(true);
        footer.append(&used);
        if is_over {
            let over = widgets::caption(&format!("{} au-dessus", format::money(envelope.remaining.abs())));
            over.add_css_class("dmx-expense");
            footer.append(&over);
        }
        host.append(&footer);

        if !envelope.linked_scheduled.is_empty() {
            host.append(&widgets::separator());
            let scheduled = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            head.append(&icons::colored_image("CalendarClock", "#6366f1", 12));
            head.append(&widgets::caption("Échéances"));
            scheduled.append(&head);
            for item in &envelope.linked_scheduled {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let description = widgets::caption(&item.description);
                description.set_hexpand(true);
                row.append(&description);
                row.append(&widgets::amount(
                    &format::signed(item.amount, item.transaction_type),
                    Some(match item.transaction_type {
                        dmx_core::models::TransactionType::Income => "dmx-income",
                        dmx_core::models::TransactionType::Transfer => "dmx-transfer",
                        dmx_core::models::TransactionType::Expense => "dmx-expense",
                    }),
                ));
                row.append(&widgets::caption(&format::day_short(&item.next_date)));
                scheduled.append(&row);
            }
            host.append(&scheduled);
        }

        host
    }
}

fn kpi(label: &str, icon: &str, icon_class: &str, value: &str, value_class: Option<&str>, caption: &str) -> gtk::Box {
    let (card, content, actions) = widgets::card(None, None);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = widgets::section_label(label);
    title.set_hexpand(true);
    header.append(&title);
    let image = icons::image(icon, 15);
    image.add_css_class(icon_class);
    header.append(&image);
    content.append(&header);
    let amount = gtk::Label::new(Some(value));
    amount.set_xalign(0.0);
    amount.add_css_class("title-2");
    if let Some(class) = value_class {
        amount.add_css_class(class);
    }
    content.append(&amount);
    content.append(&widgets::caption(caption));
    let _ = actions;
    card
}
