//! Analyses : historique des soldes, répartition par catégorie, revenus contre dépenses.

use std::rc::Rc;

use adw::prelude::*;
use dmx_core::analytics::{AnalyticsQuery, CategorySlice};
use dmx_core::models::{TimeRange, TransactionType};
use dmx_core::settings::SettingsChange;

use crate::charts::{BarChart, BarChartData, DonutChart, DonutData, DonutSlice, LineChart, LineChartData, Series};
use crate::store::Store;
use crate::{format, icons, widgets};

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    range_box: gtk::Box,
    custom_dates: gtk::Box,
    start_button: gtk::MenuButton,
    end_button: gtk::MenuButton,
    month_start: gtk::CheckButton,
    balance_chart: LineChart,
    balance_legend: gtk::Box,
    expense_donut: DonutChart,
    expense_legend: gtk::Box,
    expense_empty: gtk::Label,
    income_donut: DonutChart,
    income_legend: gtk::Box,
    income_empty: gtk::Label,
    bars: BarChart,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = widgets::title_label("Analyses Financières");
        title.set_hexpand(true);
        header.append(&title);
        let range_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        range_box.add_css_class("linked");
        header.append(&range_box);
        content.append(&header);

        let custom_dates = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        custom_dates.set_halign(gtk::Align::End);
        custom_dates.set_visible(false);
        let start_button = gtk::MenuButton::new();
        start_button.set_label("Du");
        let end_button = gtk::MenuButton::new();
        end_button.set_label("Au");
        custom_dates.append(&start_button);
        custom_dates.append(&end_button);
        content.append(&custom_dates);

        let (balance_card, balance_content, balance_actions) = widgets::card(Some("Évolution du Solde"), None);
        let month_start = gtk::CheckButton::with_label("Démarrer au 1er du mois");
        balance_actions.append(&month_start);
        let balance_chart = LineChart::new(320);
        balance_content.append(balance_chart.widget());
        let balance_legend = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        balance_content.append(&balance_legend);
        content.append(&balance_card);

        let donuts = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        donuts.set_homogeneous(true);
        let (expense_card, expense_content, _) = widgets::card(Some("Dépenses par Catégorie"), None);
        let expense_donut = DonutChart::new(210, 30.0);
        expense_content.append(expense_donut.widget());
        let expense_empty = gtk::Label::new(Some("Aucune donnée à afficher"));
        expense_empty.add_css_class("dim-label");
        expense_content.append(&expense_empty);
        let expense_legend = gtk::Box::new(gtk::Orientation::Vertical, 4);
        expense_content.append(&expense_legend);
        donuts.append(&expense_card);

        let (income_card, income_content, _) = widgets::card(Some("Revenus par Catégorie"), None);
        let income_donut = DonutChart::new(210, 30.0);
        income_content.append(income_donut.widget());
        let income_empty = gtk::Label::new(Some("Aucune donnée à afficher"));
        income_empty.add_css_class("dim-label");
        income_content.append(&income_empty);
        let income_legend = gtk::Box::new(gtk::Orientation::Vertical, 4);
        income_content.append(&income_legend);
        donuts.append(&income_card);
        content.append(&donuts);

        let (bars_card, bars_content, _) = widgets::card(Some("Revenus vs Dépenses"), None);
        let bars = BarChart::new(320);
        bars_content.append(bars.widget());
        let legend = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        legend.append(&widgets::chip("Revenus", "#10b981", None));
        legend.append(&widgets::chip("Dépenses", "#ef4444", None));
        bars_content.append(&legend);
        content.append(&bars_card);

        let root = widgets::page_scroll(&content).upcast();
        let page = Self {
            store: store.clone(),
            root,
            range_box: range_box.clone(),
            custom_dates,
            start_button: start_button.clone(),
            end_button: end_button.clone(),
            month_start: month_start.clone(),
            balance_chart,
            balance_legend,
            expense_donut,
            expense_legend,
            expense_empty,
            income_donut,
            income_legend,
            income_empty,
            bars,
        };

        for range in TimeRange::ALL {
            let range = *range;
            let button = gtk::ToggleButton::with_label(range.label());
            let store_for_range = store.clone();
            button.connect_clicked(move |button| {
                if button.is_active() {
                    store_for_range.apply(SettingsChange::AnalyticsTimeRange(range));
                }
            });
            range_box.append(&button);
        }

        {
            let store = store.clone();
            month_start.connect_toggled(move |check| {
                store.apply(SettingsChange::AnalyticsMonthStartsOnFirst(check.is_active()));
            });
        }

        attach_calendar(&start_button, {
            let store = store.clone();
            move |date| store.apply(SettingsChange::AnalyticsCustomStartDate(date))
        });
        attach_calendar(&end_button, {
            let store = store.clone();
            move |date| store.apply(SettingsChange::AnalyticsCustomEndDate(date))
        });

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let accounts = self.store.selected_accounts();
        let settings = self.store.settings();
        let query = AnalyticsQuery::from_settings(&settings, accounts);
        let today = self.store.today();
        let Some(view) = self.store.read(|engine| engine.analytics(&query, today)) else {
            return;
        };

        // Plage sélectionnée
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
        self.custom_dates.set_visible(query.range == TimeRange::Custom);
        self.month_start.set_visible(query.range != TimeRange::Custom);
        if self.month_start.is_active() != query.month_starts_on_first {
            self.month_start.set_active(query.month_starts_on_first);
        }
        self.start_button.set_label(&format!(
            "Du {}",
            format::day_numeric(query.custom_start.as_deref().unwrap_or(&view.start_date))
        ));
        self.end_button.set_label(&format!(
            "Au {}",
            format::day_numeric(query.custom_end.as_deref().unwrap_or(&view.end_date))
        ));

        // Historique des soldes
        self.balance_chart.set_data(LineChartData {
            series: view
                .balance_history
                .series
                .iter()
                .map(|series| Series {
                    name: series.name.clone(),
                    color: series.color.clone(),
                    values: series.values.clone(),
                    dashed: false,
                    filled: true,
                })
                .collect(),
            labels: view.balance_history.labels.clone(),
            references: Vec::new(),
            markers: Vec::new(),
            stepped: false,
        });
        widgets::clear(&self.balance_legend);
        for series in &view.balance_history.series {
            self.balance_legend
                .append(&widgets::chip(&series.name, &series.color, None));
        }

        // Camemberts
        self.fill_donut(
            &self.expense_donut,
            &self.expense_legend,
            &self.expense_empty,
            &view.expenses_by_category,
            TransactionType::Expense,
        );
        self.fill_donut(
            &self.income_donut,
            &self.income_legend,
            &self.income_empty,
            &view.income_by_category,
            TransactionType::Income,
        );

        // Revenus contre dépenses
        self.bars.set_data(BarChartData {
            labels: view.income_vs_expenses.iter().map(|bar| bar.label.clone()).collect(),
            groups: view
                .income_vs_expenses
                .iter()
                .map(|bar| vec![bar.income, bar.expenses])
                .collect(),
            colors: vec!["#10b981".into(), "#ef4444".into()],
        });
    }

    fn fill_donut(
        &self,
        donut: &DonutChart,
        legend: &gtk::Box,
        empty: &gtk::Label,
        slices: &[CategorySlice],
        kind: TransactionType,
    ) {
        let visible: Vec<&CategorySlice> = slices
            .iter()
            .filter(|slice| !slice.hidden && slice.value > 0.0)
            .collect();
        let total: f64 = visible.iter().map(|slice| slice.value).sum();
        donut.widget().set_visible(!visible.is_empty());
        empty.set_visible(visible.is_empty());
        donut.set_data(DonutData {
            slices: slices
                .iter()
                .map(|slice| DonutSlice {
                    value: slice.value,
                    color: slice.category.color.clone(),
                    hidden: slice.hidden,
                })
                .collect(),
            center_title: "Total".into(),
            center_value: format::rounded(total),
        });

        widgets::clear(legend);
        for slice in slices {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.set_size_request(10, 10);
            dot.add_css_class("dmx-badge");
            dot.add_css_class(&icons::fill_class(if slice.hidden {
                "#9ca3af"
            } else {
                &slice.category.color
            }));
            dot.set_valign(gtk::Align::Center);
            row.append(&dot);
            let name = gtk::Label::new(Some(&slice.category.name));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if slice.hidden {
                name.add_css_class("dim-label");
                let attributes = gtk::pango::AttrList::new();
                attributes.insert(gtk::pango::AttrInt::new_strikethrough(true));
                name.set_attributes(Some(&attributes));
            }
            row.append(&name);
            if !slice.hidden {
                row.append(&widgets::caption(&format::percent(slice.percentage)));
            }
            row.append(&widgets::amount(&format::money(slice.value), None));

            let store = self.store.clone();
            let category_id = slice.category.id.clone();
            let hidden = slice.hidden;
            let button = crate::window::clickable(&row, move || {
                store.apply(SettingsChange::AnalyticsCategoryHidden {
                    kind,
                    category_id: category_id.clone(),
                    hidden: !hidden,
                });
            });
            legend.append(&button);
        }
    }
}

/// Bouton avec calendrier : renvoie la date choisie au format `YYYY-MM-DD`.
pub fn attach_calendar(button: &gtk::MenuButton, on_pick: impl Fn(String) + 'static) {
    let popover = gtk::Popover::new();
    let calendar = gtk::Calendar::new();
    popover.set_child(Some(&calendar));
    button.set_popover(Some(&popover));
    let popover_for_pick = popover.clone();
    calendar.connect_day_selected(move |calendar| {
        let date = calendar.date();
        on_pick(format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month(),
            date.day_of_month()
        ));
        popover_for_pick.popdown();
    });
}
