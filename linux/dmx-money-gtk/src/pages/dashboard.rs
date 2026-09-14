//! Vue d'ensemble : six cartes, comme sur les autres plateformes.

use std::rc::Rc;

use adw::prelude::*;

use crate::charts::{DonutChart, DonutData, DonutSlice};
use crate::store::{FormRequest, Route, Store};
use crate::{format, icons, widgets};

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    upcoming: gtk::Box,
    month: gtk::Box,
    categories: gtk::Box,
    accounts: gtk::Box,
    accounts_total: gtk::Label,
    recent: gtk::Box,
    budget_donut: DonutChart,
    budget_figures: gtk::Box,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
        content.append(&widgets::title_label("Tableau de Bord"));

        // Échéances
        let (upcoming_card, upcoming, upcoming_actions) = widgets::card(Some("Échéances"), Some("CalendarClock"));
        let see_all = gtk::Button::with_label("Tout voir");
        see_all.add_css_class("flat");
        let store_for_scheduled = store.clone();
        see_all.connect_clicked(move |_| store_for_scheduled.navigate(Route::Scheduled));
        upcoming_actions.append(&see_all);

        // Opérations du mois
        let (month_card, month, month_actions) = widgets::card(Some("Opérations"), Some("ArrowRightLeft"));
        month_actions.append(&widgets::chip("Ce mois-ci", "#9ca3af", None));

        // Catégories
        let (categories_card, categories, categories_actions) = widgets::card(Some("Catégories"), Some("Tag"));
        categories_actions.append(&widgets::chip("Ce mois-ci", "#9ca3af", None));

        let first_row = crate::window::card_row(vec![
            upcoming_card.clone().upcast(),
            month_card.clone().upcast(),
            categories_card.clone().upcast(),
        ]);

        // Mes comptes
        let (accounts_card, accounts_content, accounts_actions) = widgets::card(Some("Mes Comptes"), Some("Wallet"));
        let add_account = widgets::icon_button("Plus", "Ajouter un compte");
        let store_for_account = store.clone();
        add_account.connect_clicked(move |_| store_for_account.present(FormRequest::Account(None)));
        accounts_actions.append(&add_account);
        let accounts = gtk::Box::new(gtk::Orientation::Vertical, 0);
        accounts_content.append(&accounts);
        accounts_content.append(&widgets::separator());
        let total_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        total_row.append(&widgets::section_label("Total"));
        let accounts_total = widgets::amount("", None);
        accounts_total.add_css_class("title-4");
        accounts_total.set_hexpand(true);
        total_row.append(&accounts_total);
        accounts_content.append(&total_row);

        // Dernières opérations
        let (recent_card, recent_content, recent_actions) = widgets::card(Some("Dernières"), Some("Receipt"));
        let see_journal = gtk::Button::with_label("Tout voir");
        see_journal.add_css_class("flat");
        let store_for_journal = store.clone();
        see_journal.connect_clicked(move |_| store_for_journal.navigate(Route::Transactions));
        recent_actions.append(&see_journal);
        let recent = gtk::Box::new(gtk::Orientation::Vertical, 0);
        recent_content.append(&recent);

        // Budget
        let (budget_card, budget_content, budget_actions) = widgets::card(Some("Budget"), Some("DollarSign"));
        budget_actions.append(&widgets::chip("Ce mois-ci", "#9ca3af", None));
        let budget_donut = DonutChart::new(150, 14.0);
        budget_content.append(budget_donut.widget());
        let budget_figures = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        budget_figures.set_homogeneous(true);
        budget_content.append(&budget_figures);

        let second_row = crate::window::card_row(vec![
            accounts_card.clone().upcast(),
            recent_card.clone().upcast(),
            budget_card.clone().upcast(),
        ]);

        content.append(&first_row);
        content.append(&second_row);

        let root = widgets::page_scroll(&content).upcast();
        Self {
            store: store.clone(),
            root,
            upcoming,
            month,
            categories,
            accounts,
            accounts_total,
            recent,
            budget_donut,
            budget_figures,
        }
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let accounts = self.store.selected_accounts();
        let today = self.store.today();
        let Some(view) = self.store.read(|engine| engine.dashboard(&accounts, today)) else {
            return;
        };

        // Échéances
        widgets::clear(&self.upcoming);
        if view.upcoming.is_empty() {
            self.upcoming
                .append(&widgets::empty_state("CheckCircle2", "Aucune échéance à venir", None));
        } else {
            for item in &view.upcoming {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.set_margin_bottom(10);
                let late = item.days_until < 0;
                row.append(&icons::badge(
                    if late { "AlertCircle" } else { "Clock" },
                    if late { "#ef4444" } else { "#10b981" },
                    32,
                    false,
                ));
                let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
                labels.set_hexpand(true);
                let description = gtk::Label::new(Some(&item.description));
                description.set_xalign(0.0);
                description.set_ellipsize(gtk::pango::EllipsizeMode::End);
                description.add_css_class("heading");
                labels.append(&description);
                labels.append(&widgets::caption(&format::relative(item.days_until)));
                row.append(&labels);
                row.append(&widgets::amount(&format::money(item.amount), None));
                self.upcoming.append(&row);
            }
        }

        // Opérations du mois
        widgets::clear(&self.month);
        let saved = view.month.saved;
        let center = gtk::Box::new(gtk::Orientation::Vertical, 2);
        center.set_halign(gtk::Align::Center);
        center.append(&widgets::section_label("Revenus - Dépenses"));
        let saved_label = gtk::Label::new(Some(&format!(
            "{}{}",
            if saved > 0.0 { "+" } else { "" },
            format::rounded(saved)
        )));
        saved_label.add_css_class("dmx-big-amount");
        saved_label.add_css_class(if saved >= 0.0 { "dmx-income" } else { "dmx-expense" });
        center.append(&saved_label);
        self.month.append(&center);
        self.month.append(&widgets::separator());
        let figures = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        figures.set_homogeneous(true);
        figures.append(&mini_figure(
            "TrendingUp",
            "dmx-income",
            "Revenus",
            &format::rounded(view.month.income),
            None,
        ));
        figures.append(&mini_figure(
            "TrendingDown",
            "dmx-expense",
            "Dépenses",
            &format::rounded(view.month.expenses),
            None,
        ));
        figures.append(&mini_figure(
            "Wallet",
            "dmx-transfer",
            "Épargné",
            &format::rounded(saved),
            Some(if saved >= 0.0 { "dmx-income" } else { "dmx-expense" }),
        ));
        self.month.append(&figures);

        // Catégories
        widgets::clear(&self.categories);
        if view.top_categories.is_empty() {
            self.categories.append(&widgets::caption("Aucune dépense ce mois-ci"));
        } else {
            for share in view.top_categories.iter().take(3) {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row.set_margin_bottom(6);
                let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                dot.set_size_request(8, 8);
                dot.add_css_class("dmx-badge");
                dot.add_css_class(&icons::fill_class(&share.category.color));
                dot.set_valign(gtk::Align::Center);
                row.append(&dot);
                let name = gtk::Label::new(Some(&share.category.name));
                name.set_xalign(0.0);
                name.set_hexpand(true);
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                row.append(&name);
                row.append(&widgets::amount(&format::percent_tight(share.percentage), None));
                self.categories.append(&row);
            }
        }

        // Comptes
        widgets::clear(&self.accounts);
        if view.accounts.is_empty() {
            self.accounts
                .append(&widgets::empty_state("Wallet", "Aucun compte configuré", None));
        }
        for entry in &view.accounts {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.set_margin_top(6);
            row.set_margin_bottom(6);
            row.append(&icons::badge(&entry.account.icon, &entry.account.color, 32, false));
            let name = gtk::Label::new(Some(&entry.account.name));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&name);
            row.append(&widgets::amount(
                &format::money(entry.balance),
                if entry.balance < 0.0 { Some("dmx-expense") } else { None },
            ));
            let store = self.store.clone();
            let id = entry.account.id.clone();
            let button = crate::window::clickable(&row, move || {
                store.set_selected_accounts(vec![id.clone()]);
                store.navigate(Route::Transactions);
            });
            self.accounts.append(&button);
        }
        self.accounts_total.set_text(&format::money(view.accounts_total));

        // Dernières opérations
        widgets::clear(&self.recent);
        for item in &view.recent_transactions {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.set_margin_top(6);
            row.set_margin_bottom(6);
            row.append(&icons::badge(&item.category.icon, &item.category.color, 32, false));
            let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
            labels.set_hexpand(true);
            let description = gtk::Label::new(Some(&item.transaction.description));
            description.set_xalign(0.0);
            description.set_ellipsize(gtk::pango::EllipsizeMode::End);
            description.add_css_class("heading");
            labels.append(&description);
            labels.append(&widgets::caption(&format::day_short(&item.transaction.date)));
            row.append(&labels);
            let is_income = matches!(
                item.transaction.transaction_type,
                dmx_core::models::TransactionType::Income
            );
            row.append(&widgets::amount(
                &format::signed_income_only(item.transaction.amount, item.transaction.transaction_type),
                if is_income { Some("dmx-income") } else { None },
            ));
            self.recent.append(&row);
        }

        // Budget
        let gauge = &view.budget;
        self.budget_donut.set_data(DonutData {
            slices: vec![
                DonutSlice {
                    value: gauge.spent,
                    color: if gauge.remaining >= 0.0 {
                        "#10b981".into()
                    } else {
                        "#ef4444".into()
                    },
                    hidden: false,
                },
                DonutSlice {
                    value: gauge.remaining.max(0.0),
                    color: "#9ca3af".into(),
                    hidden: false,
                },
            ],
            center_title: "Dépenses".into(),
            center_value: format::percent_tight(gauge.progress),
        });
        widgets::clear(&self.budget_figures);
        self.budget_figures.append(&figure(
            "Restant",
            &format::rounded(gauge.remaining),
            if gauge.remaining < 0.0 {
                Some("dmx-expense")
            } else {
                None
            },
        ));
        self.budget_figures
            .append(&figure("Dépenses", &format::rounded(gauge.spent), Some("dmx-income")));
        self.budget_figures
            .append(&figure("Prévu", &format::rounded(gauge.total_budgeted), None));
    }
}

fn mini_figure(icon: &str, icon_class: &str, label: &str, value: &str, value_class: Option<&str>) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 2);
    host.set_halign(gtk::Align::Center);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    header.set_halign(gtk::Align::Center);
    let image = icons::image(icon, 11);
    image.add_css_class(icon_class);
    header.append(&image);
    header.append(&widgets::caption(label));
    host.append(&header);
    let amount = gtk::Label::new(Some(value));
    amount.add_css_class("dmx-amount");
    if let Some(class) = value_class {
        amount.add_css_class(class);
    }
    host.append(&amount);
    host
}

fn figure(label: &str, value: &str, class: Option<&str>) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 2);
    host.set_halign(gtk::Align::Center);
    let caption = widgets::section_label(label);
    caption.set_halign(gtk::Align::Center);
    host.append(&caption);
    let amount = gtk::Label::new(Some(value));
    amount.add_css_class("dmx-amount");
    if let Some(class) = class {
        amount.add_css_class(class);
    }
    host.append(&amount);
    host
}
