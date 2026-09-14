//! Contrôles de formulaire : champs, sélecteurs, grilles d'icônes et de couleurs.
//!
//! Aucune règle métier ici : les valeurs partent telles quelles dans les brouillons du noyau.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use chrono::Datelike;
use dmx_core::models::TransactionType;
use gtk::glib;

use crate::{format, icons, widgets};

/// Abonnés d'un contrôle (la valeur choisie est passée telle quelle).
type Listeners<T> = RefCell<Vec<Box<dyn Fn(T)>>>;

/// Champ étiqueté (libellé au-dessus du contrôle).
pub fn field(label: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 4);
    host.set_hexpand(true);
    host.append(&widgets::section_label(label));
    host.append(child);
    host
}

/// Deux champs côte à côte.
pub fn pair(left: &impl IsA<gtk::Widget>, right: &impl IsA<gtk::Widget>) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    host.set_homogeneous(true);
    host.append(left);
    host.append(right);
    host
}

pub fn entry(placeholder: &str, value: &str) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(placeholder));
    entry.set_text(value);
    entry.set_hexpand(true);
    entry
}

/// Champ de montant (virgule ou point, comme en 1.x).
pub fn amount_entry(value: f64) -> gtk::Entry {
    let entry = entry("0,00", &format::amount_input(value));
    entry.set_input_purpose(gtk::InputPurpose::Number);
    entry
}

/// Montant strictement positif (opérations, budgets, échéances).
pub fn required_amount(entry: &gtk::Entry) -> Result<f64, String> {
    match format::parse_amount(&entry.text()) {
        Some(value) if value > 0.0 => Ok(value),
        _ => Err("Saisissez un montant valide".to_string()),
    }
}

/// Montant facultatif : vide vaut zéro (solde initial).
pub fn optional_amount(entry: &gtk::Entry) -> Result<f64, String> {
    let text = entry.text();
    if text.trim().is_empty() {
        return Ok(0.0);
    }
    format::parse_amount(&text).ok_or_else(|| "Saisissez un montant valide".to_string())
}

/// Bouton-calendrier : conserve la date au format `YYYY-MM-DD`.
pub struct DateButton {
    button: gtk::MenuButton,
    value: Rc<RefCell<String>>,
}

impl DateButton {
    pub fn new(initial: &str) -> Rc<Self> {
        let label = gtk::Label::new(Some(&format::day_medium(initial)));
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        content.append(&icons::image("Calendar", 13));
        content.append(&label);
        let button = gtk::MenuButton::new();
        button.set_child(Some(&content));
        button.set_hexpand(true);

        let value = Rc::new(RefCell::new(initial.to_string()));
        let calendar = gtk::Calendar::new();
        if let Some(date) = dmx_core::dates::parse_date(initial) {
            if let Ok(selected) =
                glib::DateTime::from_local(date.year(), date.month() as i32, date.day() as i32, 12, 0, 0.0)
            {
                calendar.select_day(&selected);
            }
        }
        let popover = gtk::Popover::new();
        popover.set_child(Some(&calendar));
        button.set_popover(Some(&popover));

        let value_for_pick = value.clone();
        let label_for_pick = label.clone();
        calendar.connect_day_selected(move |calendar| {
            let date = calendar.date();
            let text = format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day_of_month());
            *value_for_pick.borrow_mut() = text.clone();
            label_for_pick.set_text(&format::day_medium(&text));
        });
        let popover_for_pick = popover.clone();
        calendar.connect_day_selected(move |_| popover_for_pick.popdown());

        Rc::new(Self { button, value })
    }

    pub fn widget(&self) -> gtk::Widget {
        self.button.clone().upcast()
    }

    pub fn value(&self) -> String {
        self.value.borrow().clone()
    }
}

/// Option d'une liste déroulante.
#[derive(Clone)]
pub struct SelectOption {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub color: Option<String>,
}

impl SelectOption {
    pub fn simple(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            icon: None,
            color: None,
        }
    }

    pub fn with_icon(id: &str, label: &str, icon: &str, color: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            icon: Some(icon.to_string()),
            color: Some(color.to_string()),
        }
    }
}

/// Liste déroulante avec recherche, icônes et entrée « aucun » facultative.
pub struct Select {
    button: gtk::MenuButton,
    label: gtk::Label,
    image: gtk::Image,
    options: Vec<SelectOption>,
    placeholder: String,
    value: RefCell<Option<String>>,
    listeners: Listeners<Option<String>>,
    rows: RefCell<Vec<(gtk::ListBoxRow, String)>>,
}

impl Select {
    pub fn new(
        placeholder: &str,
        options: Vec<SelectOption>,
        initial: Option<String>,
        none_label: Option<&str>,
    ) -> Rc<Self> {
        let mut entries = Vec::new();
        if let Some(none_label) = none_label {
            entries.push(SelectOption::simple("", none_label));
        }
        entries.extend(options);

        let image = gtk::Image::new();
        image.set_pixel_size(14);
        image.set_visible(false);
        let label = gtk::Label::new(Some(placeholder));
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        content.append(&image);
        content.append(&label);
        content.append(&gtk::Image::from_icon_name("pan-down-symbolic"));
        let button = gtk::MenuButton::new();
        button.set_child(Some(&content));
        button.set_hexpand(true);

        let select = Rc::new(Self {
            button: button.clone(),
            label,
            image,
            options: entries,
            placeholder: placeholder.to_string(),
            value: RefCell::new(None),
            listeners: RefCell::new(Vec::new()),
            rows: RefCell::new(Vec::new()),
        });

        // Contenu du popover : recherche au-delà de huit entrées.
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        let popover_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        popover_content.set_margin_top(8);
        popover_content.set_margin_bottom(8);
        popover_content.set_margin_start(8);
        popover_content.set_margin_end(8);
        popover_content.set_size_request(280, -1);

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Rechercher..."));
        if select.options.len() > 8 {
            popover_content.append(&search);
        }
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_max_content_height(320);
        scroll.set_propagate_natural_height(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_child(Some(&list));
        popover_content.append(&scroll);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&popover_content));
        button.set_popover(Some(&popover));

        for option in &select.options {
            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row_box.set_margin_top(4);
            row_box.set_margin_bottom(4);
            row_box.set_margin_start(6);
            row_box.set_margin_end(6);
            if let Some(icon) = &option.icon {
                row_box.append(&icons::colored_image(
                    icon,
                    option.color.as_deref().unwrap_or("#9ca3af"),
                    14,
                ));
            }
            let row_label = gtk::Label::new(Some(&option.label));
            row_label.set_xalign(0.0);
            row_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row_box.append(&row_label);
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&row_box));
            list.append(&row);
            select.rows.borrow_mut().push((row, option.label.to_lowercase()));
        }

        {
            let select_for_rows = select.clone();
            let popover = popover.clone();
            list.connect_row_activated(move |_, row| {
                let index = select_for_rows
                    .rows
                    .borrow()
                    .iter()
                    .position(|(candidate, _)| candidate == row);
                if let Some(index) = index {
                    let id = select_for_rows.options[index].id.clone();
                    select_for_rows.set_value(if id.is_empty() { None } else { Some(id) });
                    select_for_rows.notify();
                }
                popover.popdown();
            });
        }
        {
            let select_for_search = select.clone();
            search.connect_search_changed(move |entry| {
                let needle = entry.text().to_lowercase();
                for (row, haystack) in select_for_search.rows.borrow().iter() {
                    row.set_visible(needle.is_empty() || haystack.contains(&needle));
                }
            });
        }

        select.set_value(initial);
        select
    }

    pub fn widget(&self) -> gtk::Widget {
        self.button.clone().upcast()
    }

    pub fn value(&self) -> Option<String> {
        self.value.borrow().clone()
    }

    /// Valeur obligatoire : chaîne vide si rien n'est choisi (validée par le noyau).
    pub fn required_value(&self) -> String {
        self.value.borrow().clone().unwrap_or_default()
    }

    pub fn set_value(&self, value: Option<String>) {
        let value = value.filter(|id| !id.is_empty());
        let option = value
            .as_ref()
            .and_then(|id| self.options.iter().find(|option| &option.id == id))
            .cloned();
        match &option {
            Some(option) => {
                self.label.set_text(&option.label);
                self.label.remove_css_class("dim-label");
                match &option.icon {
                    Some(icon) => {
                        self.image.set_icon_name(Some(&icons::icon_name(icon)));
                        self.image.set_visible(true);
                        if let Some(color) = &option.color {
                            self.image.set_css_classes(&[]);
                            self.image.add_css_class(&icons::color_class(color));
                        }
                    }
                    None => self.image.set_visible(false),
                }
            }
            None => {
                let none_label = self.options.first().filter(|option| option.id.is_empty());
                self.label.set_text(
                    none_label
                        .map(|option| option.label.as_str())
                        .unwrap_or(&self.placeholder),
                );
                self.label.add_css_class("dim-label");
                self.image.set_visible(false);
            }
        }
        *self.value.borrow_mut() = option.map(|option| option.id);
    }

    pub fn connect_changed(&self, listener: impl Fn(Option<String>) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    pub fn set_sensitive(&self, sensitive: bool) {
        self.button.set_sensitive(sensitive);
    }

    fn notify(&self) {
        let value = self.value();
        for listener in self.listeners.borrow().iter() {
            listener(value.clone());
        }
    }
}

/// Sélecteur de type (Dépense / Revenu / Virement), comme le segmenté de 1.x.
pub struct KindPicker {
    host: gtk::Box,
    value: Cell<TransactionType>,
    listeners: Listeners<TransactionType>,
}

impl KindPicker {
    pub fn new(initial: TransactionType) -> Rc<Self> {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        host.add_css_class("linked");
        host.set_homogeneous(true);
        let picker = Rc::new(Self {
            host: host.clone(),
            value: Cell::new(initial),
            listeners: RefCell::new(Vec::new()),
        });

        let kinds = [
            TransactionType::Expense,
            TransactionType::Income,
            TransactionType::Transfer,
        ];
        let icons_names = ["TrendingDown", "TrendingUp", "ArrowRightLeft"];
        let mut first: Option<gtk::ToggleButton> = None;
        for (kind, icon) in kinds.into_iter().zip(icons_names) {
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            content.set_halign(gtk::Align::Center);
            content.append(&icons::image(icon, 13));
            content.append(&gtk::Label::new(Some(kind.label())));
            let button = gtk::ToggleButton::new();
            button.set_child(Some(&content));
            button.set_active(kind == initial);
            match &first {
                Some(first) => button.set_group(Some(first)),
                None => first = Some(button.clone()),
            }
            let picker_for_button = picker.clone();
            button.connect_toggled(move |button| {
                if button.is_active() && picker_for_button.value.get() != kind {
                    picker_for_button.value.set(kind);
                    let listeners = picker_for_button.listeners.borrow();
                    for listener in listeners.iter() {
                        listener(kind);
                    }
                }
            });
            host.append(&button);
        }
        picker
    }

    pub fn widget(&self) -> gtk::Widget {
        self.host.clone().upcast()
    }

    pub fn value(&self) -> TransactionType {
        self.value.get()
    }

    pub fn connect_changed(&self, listener: impl Fn(TransactionType) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }
}

/// Grille de couleurs (palette de 120 couleurs des comptes et catégories).
pub struct ColorGrid {
    scroll: gtk::ScrolledWindow,
    buttons: RefCell<Vec<(String, gtk::Button, gtk::Image)>>,
    value: RefCell<String>,
    listeners: Listeners<String>,
}

impl ColorGrid {
    pub fn new(colors: &[&str], initial: &str, columns: i32, height: i32) -> Rc<Self> {
        let grid = gtk::Grid::new();
        grid.set_row_spacing(6);
        grid.set_column_spacing(6);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_min_content_height(height);
        scroll.set_max_content_height(height);
        scroll.set_child(Some(&grid));

        let picker = Rc::new(Self {
            scroll: scroll.clone(),
            buttons: RefCell::new(Vec::new()),
            value: RefCell::new(initial.to_string()),
            listeners: RefCell::new(Vec::new()),
        });

        for (index, color) in colors.iter().enumerate() {
            let check = icons::image("Check", 11);
            check.set_visible(false);
            let button = gtk::Button::new();
            button.set_child(Some(&check));
            button.set_size_request(24, 24);
            button.add_css_class("circular");
            button.add_css_class(&icons::fill_class(color));
            button.set_tooltip_text(Some(color));
            grid.attach(&button, index as i32 % columns, index as i32 / columns, 1, 1);

            let picker_for_button = picker.clone();
            let color_for_button = color.to_string();
            button.connect_clicked(move |_| {
                picker_for_button.set_value(&color_for_button);
                let listeners = picker_for_button.listeners.borrow();
                for listener in listeners.iter() {
                    listener(color_for_button.clone());
                }
            });
            picker.buttons.borrow_mut().push((color.to_string(), button, check));
        }
        picker.refresh();
        picker
    }

    pub fn widget(&self) -> gtk::Widget {
        self.scroll.clone().upcast()
    }

    pub fn value(&self) -> String {
        self.value.borrow().clone()
    }

    pub fn set_value(&self, color: &str) {
        *self.value.borrow_mut() = color.to_string();
        self.refresh();
    }

    pub fn connect_changed(&self, listener: impl Fn(String) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    fn refresh(&self) {
        let current = self.value.borrow().to_lowercase();
        for (color, _, check) in self.buttons.borrow().iter() {
            check.set_visible(color.to_lowercase() == current);
        }
    }
}

/// Grille d'icônes Lucide (même ordre que le sélecteur 1.x).
pub struct IconGrid {
    scroll: gtk::ScrolledWindow,
    buttons: RefCell<Vec<(String, gtk::ToggleButton)>>,
    value: RefCell<String>,
    listeners: Listeners<String>,
}

impl IconGrid {
    pub fn new(names: &[&str], initial: &str, columns: i32, height: i32) -> Rc<Self> {
        let grid = gtk::Grid::new();
        grid.set_row_spacing(4);
        grid.set_column_spacing(4);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_min_content_height(height);
        scroll.set_max_content_height(height);
        scroll.set_child(Some(&grid));

        let picker = Rc::new(Self {
            scroll: scroll.clone(),
            buttons: RefCell::new(Vec::new()),
            value: RefCell::new(initial.to_string()),
            listeners: RefCell::new(Vec::new()),
        });

        for (index, name) in names.iter().enumerate() {
            let button = gtk::ToggleButton::new();
            button.set_child(Some(&icons::image(name, 15)));
            button.set_size_request(30, 30);
            button.set_tooltip_text(Some(name));
            button.add_css_class("flat");
            grid.attach(&button, index as i32 % columns, index as i32 / columns, 1, 1);

            let picker_for_button = picker.clone();
            let name_for_button = name.to_string();
            button.connect_clicked(move |_| {
                picker_for_button.set_value(&name_for_button);
                let listeners = picker_for_button.listeners.borrow();
                for listener in listeners.iter() {
                    listener(name_for_button.clone());
                }
            });
            picker.buttons.borrow_mut().push((name.to_string(), button));
        }
        picker.refresh();
        picker
    }

    pub fn widget(&self) -> gtk::Widget {
        self.scroll.clone().upcast()
    }

    pub fn value(&self) -> String {
        self.value.borrow().clone()
    }

    pub fn set_value(&self, name: &str) {
        *self.value.borrow_mut() = name.to_string();
        self.refresh();
    }

    pub fn connect_changed(&self, listener: impl Fn(String) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    fn refresh(&self) {
        let current = self.value.borrow().clone();
        for (name, button) in self.buttons.borrow().iter() {
            let active = *name == current;
            if button.is_active() != active {
                button.set_active(active);
            }
        }
    }
}

/// Aperçu d'une pastille (icône + couleur) rafraîchi à chaque changement.
pub struct BadgePreview {
    host: gtk::Box,
    size: i32,
}

impl BadgePreview {
    pub fn new(icon: &str, color: &str, size: i32) -> Rc<Self> {
        let host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        host.set_valign(gtk::Align::Center);
        let preview = Rc::new(Self { host, size });
        preview.set(icon, color);
        preview
    }

    pub fn widget(&self) -> gtk::Widget {
        self.host.clone().upcast()
    }

    pub fn set(&self, icon: &str, color: &str) {
        widgets::clear(&self.host);
        self.host.append(&icons::badge(icon, color, self.size, true));
    }
}

/// Indicateur d'étapes de l'assistant d'import.
pub fn step_indicator(labels: &[&str], current: usize) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    for (index, label) in labels.iter().enumerate() {
        let step = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        step.set_hexpand(index + 1 < labels.len());
        let bullet = gtk::Label::new(Some(&format!("{}", index + 1)));
        bullet.add_css_class("dmx-step-bullet");
        let text = gtk::Label::new(Some(label));
        if index == current {
            bullet.add_css_class("dmx-step-active");
            text.add_css_class("heading");
        } else if index < current {
            bullet.add_css_class("dmx-step-done");
            text.add_css_class("dim-label");
        } else {
            text.add_css_class("dim-label");
        }
        step.append(&bullet);
        step.append(&text);
        host.append(&step);
    }
    host
}
