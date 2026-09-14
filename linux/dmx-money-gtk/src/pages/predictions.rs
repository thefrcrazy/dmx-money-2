//! Prédictions : transactions fictives, projection journalière, jours à surveiller, seuil d'alerte.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use dmx_core::models::{TimeRange, TransactionType};
use dmx_core::predictions::{FakeTransactionRow, PredictionQuery, Severity};
use dmx_core::settings::SettingsChange;

use crate::charts::{LineChart, LineChartData, Marker, Reference, Series};
use crate::pages::analytics::attach_calendar;
use crate::store::{FormRequest, Store};
use crate::{format, icons, widgets};

/// Légende des marqueurs, identique aux autres plateformes.
const MARKER_LEGEND: [(&str, &str); 4] = [
    ("#ef4444", "Solde négatif en fin de journée"),
    ("#f97316", "Passage sous le seuil d’alerte"),
    ("#a855f7", "Point bas négatif, rattrapé par un revenu"),
    ("#eab308", "Point bas sous le seuil, rattrapé par un revenu"),
];

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    range_box: gtk::Box,
    end_button: gtk::MenuButton,
    fake_summary: gtk::Box,
    fake_count: gtk::Label,
    fake_impact: gtk::Label,
    fake_list: gtk::Box,
    fake_empty: gtk::Box,
    clear_fake: gtk::Button,
    projection_title: gtk::Label,
    month_start: gtk::CheckButton,
    chart: LineChart,
    risk_card: gtk::Box,
    risk_title: gtk::Label,
    risk_days: gtk::Box,
    risk_more: gtk::Label,
    totals: gtk::Box,
    threshold_entry: gtk::Entry,
    show_intraday: Rc<Cell<bool>>,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = widgets::title_label("Prédictions Financières");
        title.set_hexpand(true);
        header.append(&title);
        let range_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        range_box.add_css_class("linked");
        header.append(&range_box);
        let end_button = gtk::MenuButton::new();
        end_button.set_label("Jusqu'au");
        end_button.set_visible(false);
        header.append(&end_button);
        content.append(&header);

        // Transactions fictives
        let (fake_card, fake_content, fake_actions) = widgets::card(Some("Transactions fictives"), None);
        let clear_fake = widgets::action_button("Tout retirer", Some("Trash2"), false);
        clear_fake.set_visible(false);
        let store_for_clear = store.clone();
        clear_fake.connect_clicked(move |_| {
            let accounts = store_for_clear.selected_accounts();
            store_for_clear.run(Some("Transactions fictives retirées"), move |engine| {
                engine.clear_applied_fake_transactions(&accounts)
            });
        });
        fake_actions.append(&clear_fake);
        let add_fake = widgets::action_button("Ajouter", Some("Plus"), true);
        let store_for_add = store.clone();
        add_fake.connect_clicked(move |_| store_for_add.present(FormRequest::FakeTransaction(None)));
        fake_actions.append(&add_fake);
        fake_content.append(&widgets::caption(
            "Elles modifient uniquement cette projection et ne sont pas ajoutées au journal.",
        ));
        let fake_summary = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        fake_summary.set_visible(false);
        let fake_count = widgets::caption("");
        fake_count.set_hexpand(true);
        fake_summary.append(&fake_count);
        let fake_impact = gtk::Label::new(None);
        fake_impact.add_css_class("dmx-amount");
        fake_summary.append(&fake_impact);
        fake_content.append(&fake_summary);
        let fake_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        fake_content.append(&fake_list);
        let fake_empty = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        fake_empty.add_css_class("dmx-dashed");
        let fake_empty_label = widgets::caption("Aucune transaction fictive appliquée à cette projection.");
        fake_empty_label.set_halign(gtk::Align::Center);
        fake_empty_label.set_hexpand(true);
        fake_empty_label.set_margin_top(16);
        fake_empty_label.set_margin_bottom(16);
        fake_empty.append(&fake_empty_label);
        fake_content.append(&fake_empty);
        content.append(&fake_card);

        // Projection
        let (projection_card, projection_content, projection_actions) = widgets::card(None, None);
        let projection_header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 2);
        titles.set_hexpand(true);
        let projection_title = gtk::Label::new(None);
        projection_title.add_css_class("dmx-card-title");
        projection_title.set_xalign(0.0);
        titles.append(&projection_title);
        titles.append(&widgets::caption("Visualisation de la trésorerie jour par jour."));
        projection_header.append(&titles);
        let month_start = gtk::CheckButton::with_label("Démarrer au 1er du mois");
        projection_header.append(&month_start);
        let intraday = gtk::CheckButton::with_label("Point bas journalier");
        intraday.set_tooltip_text(Some(
            "Trace le solde de chaque compte une fois les retraits du jour passés, avant l’arrivée des revenus.",
        ));
        projection_header.append(&intraday);
        projection_content.append(&projection_header);
        let chart = LineChart::new(380);
        projection_content.append(chart.widget());
        let legend = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        for (color, label) in MARKER_LEGEND {
            let item = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            line.set_size_request(16, 2);
            line.add_css_class("dmx-badge");
            line.add_css_class(&icons::fill_class(color));
            line.set_valign(gtk::Align::Center);
            item.append(&line);
            item.append(&widgets::caption(label));
            legend.append(&item);
        }
        projection_content.append(&legend);
        let _ = projection_actions;
        content.append(&projection_card);

        // Jours à surveiller
        let (risk_card, risk_content, _) = widgets::card(None, None);
        risk_card.set_visible(false);
        let risk_header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        risk_header.append(&icons::colored_image("ShieldAlert", "#f97316", 18));
        let risk_title = gtk::Label::new(None);
        risk_title.add_css_class("dmx-card-title");
        risk_header.append(&risk_title);
        risk_content.append(&risk_header);
        risk_content.append(&widgets::caption(
            "Ces jours cumulent un retrait et un revenu. Le solde de fin de journée reste au-dessus du seuil, mais il passe en dessous avant l’arrivée du revenu — de quoi déclencher des frais côté banque.",
        ));
        let risk_days = gtk::Box::new(gtk::Orientation::Vertical, 0);
        risk_content.append(&risk_days);
        let risk_more = widgets::caption("");
        risk_more.set_visible(false);
        risk_content.append(&risk_more);
        content.append(&risk_card);

        // Totaux
        let totals = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        totals.set_homogeneous(true);
        content.append(&totals);

        // Seuil d'alerte
        let (threshold_card, threshold_content, _) = widgets::card(None, None);
        let threshold_row = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        let threshold_labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
        threshold_labels.set_hexpand(true);
        let threshold_title = gtk::Label::new(Some("Seuil d'alerte"));
        threshold_title.add_css_class("dmx-card-title");
        threshold_title.set_xalign(0.0);
        threshold_labels.append(&threshold_title);
        threshold_labels.append(&widgets::caption(
            "Le seuil personnalisé s'affiche en orange. Le rouge reste réservé aux soldes négatifs. Les jours où un retrait passe avant un revenu sont signalés à part, même si la journée se termine au-dessus du seuil.",
        ));
        threshold_row.append(&threshold_labels);
        let threshold_entry = gtk::Entry::new();
        threshold_entry.set_placeholder_text(Some("0"));
        threshold_entry.set_width_request(160);
        threshold_entry.set_valign(gtk::Align::Center);
        threshold_row.append(&threshold_entry);
        threshold_content.append(&threshold_row);
        content.append(&threshold_card);

        let root = widgets::page_scroll(&content).upcast();
        let page = Self {
            store: store.clone(),
            root,
            range_box: range_box.clone(),
            end_button: end_button.clone(),
            fake_summary,
            fake_count,
            fake_impact,
            fake_list,
            fake_empty,
            clear_fake,
            projection_title,
            month_start: month_start.clone(),
            chart,
            risk_card,
            risk_title,
            risk_days,
            risk_more,
            totals,
            threshold_entry: threshold_entry.clone(),
            show_intraday: Rc::new(Cell::new(false)),
        };

        for range in TimeRange::ALL {
            let range = *range;
            let button = gtk::ToggleButton::with_label(range.label());
            let store_for_range = store.clone();
            button.connect_clicked(move |button| {
                if button.is_active() {
                    store_for_range.apply(SettingsChange::PredictionTimeRange(range));
                }
            });
            range_box.append(&button);
        }

        attach_calendar(&end_button, {
            let store = store.clone();
            move |date| store.apply(SettingsChange::PredictionCustomEndDate(date))
        });

        {
            let store = store.clone();
            month_start.connect_toggled(move |check| {
                store.apply(SettingsChange::PredictionMonthStartsOnFirst(check.is_active()));
            });
        }

        {
            let show_intraday = page.show_intraday.clone();
            let store = store.clone();
            intraday.connect_toggled(move |check| {
                show_intraday.set(check.is_active());
                store.navigate(crate::store::Route::Predictions);
            });
        }

        {
            let store = store.clone();
            let entry = threshold_entry.clone();
            let commit = move || {
                let text = entry.text().to_string();
                let value = if text.trim().is_empty() {
                    0.0
                } else {
                    match format::parse_amount(&text) {
                        Some(value) => value,
                        None => return,
                    }
                };
                store.apply(SettingsChange::PredictionAlertThreshold(value));
            };
            let commit_for_activate = commit.clone();
            threshold_entry.connect_activate(move |_| commit_for_activate());
            let focus = gtk::EventControllerFocus::new();
            focus.connect_leave(move |_| commit());
            threshold_entry.add_controller(focus);
        }

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let accounts = self.store.selected_accounts();
        let settings = self.store.settings();
        let query = PredictionQuery::from_settings(&settings, accounts);
        let today = self.store.today();
        let Some(view) = self.store.read(|engine| engine.predictions(&query, today)) else {
            return;
        };

        // Plage
        let mut child = self.range_box.first_child();
        let mut index = 0usize;
        while let Some(widget) = child {
            if let Some(button) = widget.downcast_ref::<gtk::ToggleButton>() {
                let is_current = TimeRange::ALL.get(index) == Some(&query.range);
                if button.is_active() != is_current {
                    button.set_active(is_current);
                }
            }
            child = widget.next_sibling();
            index += 1;
        }
        self.end_button.set_visible(query.range == TimeRange::Custom);
        self.end_button.set_label(&format!(
            "Jusqu'au {}",
            format::day_numeric(query.custom_end_date.as_deref().unwrap_or(&view.end_date))
        ));
        self.month_start.set_visible(query.range != TimeRange::Custom);
        if self.month_start.is_active() != query.month_starts_on_first {
            self.month_start.set_active(query.month_starts_on_first);
        }
        if !self.threshold_entry.has_focus() {
            self.threshold_entry
                .set_text(&format::amount_input(query.alert_threshold));
        }

        // Transactions fictives
        widgets::clear(&self.fake_list);
        let has_fake = !view.fake_transactions.is_empty();
        self.fake_empty.set_visible(!has_fake);
        self.fake_summary.set_visible(has_fake);
        self.clear_fake.set_visible(has_fake);
        self.fake_count.set_text(&format!(
            "{}/{} {} {}",
            view.enabled_fake_count,
            view.fake_transactions.len(),
            if view.fake_transactions.len() > 1 {
                "simulations"
            } else {
                "simulation"
            },
            if view.enabled_fake_count == 1 {
                "active"
            } else {
                "actives"
            }
        ));
        self.fake_impact.set_text(&format!(
            "Impact période : {}{}",
            if view.fake_impact >= 0.0 { "+" } else { "" },
            format::money(view.fake_impact)
        ));
        self.fake_impact.remove_css_class("dmx-income");
        self.fake_impact.remove_css_class("dmx-expense");
        self.fake_impact.add_css_class(if view.fake_impact >= 0.0 {
            "dmx-income"
        } else {
            "dmx-expense"
        });
        for row in &view.fake_transactions {
            self.fake_list.append(&self.fake_row(row));
            self.fake_list.append(&widgets::separator());
        }

        // Projection
        self.projection_title
            .set_text(&format!("Projection sur {} (Journalière)", view.title_label));
        let mut series: Vec<Series> = view
            .accounts
            .iter()
            .map(|account| Series {
                name: account.name.clone(),
                color: account.color.clone(),
                values: account.closes.clone(),
                dashed: false,
                filled: true,
            })
            .collect();
        if self.show_intraday.get() {
            series.extend(view.accounts.iter().map(|account| Series {
                name: format!("{} — point bas", account.name),
                color: account.color.clone(),
                values: account.lows.clone(),
                dashed: true,
                filled: false,
            }));
        }
        let mut references = vec![Reference {
            value: 0.0,
            color: "#ef4444".into(),
            dashed: false,
        }];
        if view.alert_threshold > 0.0 {
            references.push(Reference {
                value: view.alert_threshold,
                color: "#f97316".into(),
                dashed: true,
            });
        }
        self.chart.set_data(LineChartData {
            series,
            labels: view.labels.clone(),
            references,
            markers: view
                .markers
                .iter()
                .map(|marker| Marker {
                    index: marker.index as usize,
                    color: marker.stroke_color.clone(),
                })
                .collect(),
            stepped: true,
        });

        // Jours à surveiller
        let risks: Vec<_> = view
            .markers
            .iter()
            .filter(|marker| marker.intraday_severity.is_some())
            .collect();
        self.risk_card.set_visible(view.intraday_risk_count > 0);
        self.risk_title
            .set_text(&format!("Jours à surveiller ({})", view.intraday_risk_count));
        widgets::clear(&self.risk_days);
        for marker in risks.iter().take(8) {
            let block = gtk::Box::new(gtk::Orientation::Vertical, 4);
            block.set_margin_top(8);
            block.set_margin_bottom(8);
            let date = gtk::Label::new(Some(&marker.full_label));
            date.set_xalign(0.0);
            date.add_css_class("heading");
            block.append(&date);
            for balance in &marker.intraday_balances {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let name = widgets::caption(&balance.name);
                name.set_hexpand(true);
                row.append(&name);
                let low = widgets::amount(
                    &format!("Point bas {}", format::money(balance.low)),
                    Some(if marker.intraday_severity == Some(Severity::Danger) {
                        "dmx-transfer"
                    } else {
                        "dmx-warning"
                    }),
                );
                row.append(&low);
                row.append(&widgets::amount(
                    &format!("Fin de journée {}", format::money(balance.value)),
                    Some("dmx-income"),
                ));
                block.append(&row);
            }
            self.risk_days.append(&block);
            self.risk_days.append(&widgets::separator());
        }
        let others = risks.len().saturating_sub(8);
        self.risk_more.set_visible(others > 0);
        if others > 0 {
            self.risk_more.set_text(&format!(
                "+{others} {} sur la période.",
                if others > 1 { "autres jours" } else { "autre jour" }
            ));
        }

        // Totaux
        widgets::clear(&self.totals);
        self.totals.append(&total_card(
            "Solde Actuel Total",
            &format::money(view.current_total_balance),
            None,
        ));
        self.totals.append(&total_card(
            "Projection mi-période",
            &format::money(view.midpoint_balance),
            Some(if view.midpoint_balance >= view.current_total_balance {
                "dmx-income"
            } else {
                "dmx-expense"
            }),
        ));
        self.totals.append(&total_card(
            &format!("Projection au {}", view.end_label),
            &format::money(view.final_balance),
            Some(if view.final_balance >= view.current_total_balance {
                "dmx-income"
            } else {
                "dmx-expense"
            }),
        ));
    }

    fn fake_row(&self, row: &FakeTransactionRow) -> gtk::Box {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        host.set_margin_top(10);
        host.set_margin_bottom(10);

        let check = gtk::CheckButton::new();
        check.set_active(row.transaction.enabled);
        check.set_tooltip_text(Some(if row.transaction.enabled {
            "Désactiver cette transaction fictive"
        } else {
            "Activer cette transaction fictive"
        }));
        let store = self.store.clone();
        let id = row.transaction.id.clone();
        check.connect_toggled(move |_| {
            let id = id.clone();
            store.run(None, move |engine| engine.toggle_fake_transaction(&id));
        });
        host.append(&check);

        let (icon, color) = match row.transaction.transaction_type {
            TransactionType::Income => ("TrendingUp", "#10b981"),
            TransactionType::Transfer => ("ArrowRightLeft", "#6366f1"),
            TransactionType::Expense => ("TrendingDown", "#ef4444"),
        };
        host.append(&icons::badge(icon, color, 34, false));

        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let description = gtk::Label::new(Some(&row.transaction.description));
        description.set_xalign(0.0);
        description.add_css_class("heading");
        title_row.append(&description);
        title_row.append(&widgets::chip(&row.type_label, "#9ca3af", None));
        if !row.transaction.enabled {
            title_row.append(&widgets::chip("Désactivée", "#9ca3af", None));
        }
        labels.append(&title_row);
        let mut details = vec![
            format::day_medium(&row.transaction.date),
            row.source_account_name.clone(),
        ];
        if let Some(destination) = &row.destination_account_name {
            details.push(format!("→ {destination}"));
        } else if let Some(category) = &row.category_name {
            details.push(category.clone());
        }
        labels.append(&widgets::caption(&details.join(" • ")));
        host.append(&labels);

        host.append(&widgets::amount(
            &format::signed(row.transaction.amount, row.transaction.transaction_type),
            Some(match row.transaction.transaction_type {
                TransactionType::Income => "dmx-income",
                TransactionType::Transfer => "dmx-transfer",
                TransactionType::Expense => "dmx-expense",
            }),
        ));

        let edit = widgets::icon_button("Edit2", "Modifier cette transaction fictive");
        let store = self.store.clone();
        let id = row.transaction.id.clone();
        edit.connect_clicked(move |_| store.present(FormRequest::FakeTransaction(Some(id.clone()))));
        host.append(&edit);

        let remove = widgets::icon_button("Trash2", "Retirer cette transaction fictive");
        remove.add_css_class("dmx-expense");
        let store = self.store.clone();
        let id = row.transaction.id.clone();
        remove.connect_clicked(move |_| {
            let ids = vec![id.clone()];
            store.run(Some("Transaction fictive retirée"), move |engine| {
                engine.delete_fake_transactions(ids)
            });
        });
        host.append(&remove);

        host
    }
}

fn total_card(label: &str, value: &str, class: Option<&str>) -> gtk::Box {
    let (card, content, _) = widgets::card(None, None);
    content.append(&widgets::section_label(label));
    let amount = gtk::Label::new(Some(value));
    amount.set_xalign(0.0);
    amount.add_css_class("title-2");
    if let Some(class) = class {
        amount.add_css_class(class);
    }
    content.append(&amount);
    card
}
