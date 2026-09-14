//! Formulaires de données : restauration `.dmx`, assistant d'import de relevés, nouveautés.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use dmx_core::backup::RestoreMode;
use dmx_core::import::{
    detect_csv_separator, initial_balance_from_final, source_categories, CategoryMatch, CsvColumnMapping, CsvOptions,
    ImportTarget, ParsedStatementTransaction, StatementFormat, StatementImportRequest,
};
use dmx_core::models::ACCOUNT_TYPES;
use dmx_core::settings::SettingsChange;

use super::controls::{amount_entry, entry, field, step_indicator, Select, SelectOption};
use super::FormShell;
use crate::store::Store;
use crate::{format, icons, widgets};

/// Rendu de l'étape courante de l'assistant, appelable depuis ses propres contrôles.
type Render = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// Nouveautés de la version, affichées après une mise à jour.
const RELEASE_NOTES: [&str; 5] = [
    "DmxMoney devient une application native : GTK4 et libadwaita sous Linux, AppKit et SwiftUI sur Mac, WinUI sous Windows.",
    "Vos données DmxMoney 1.x sont reprises au premier lancement ; la base d'origine n'est jamais modifiée.",
    "Le pont PWA sécurisé reste disponible : appairez vos mobiles avec un QR, autant d'appareils que nécessaire.",
    "Budget, échéancier, analyses et prédictions sont calculés par le même noyau sur les trois systèmes.",
    "Distribution Flatpak et AppImage, avec icône symbolique et thème clair/sombre du système.",
];

pub fn whats_new(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>) {
    let shell = FormShell::new("Nouveautés", Some("Continuer"), 520);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.append(&icons::badge("Sparkles", "#6366f1", 44, true));
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let title = gtk::Label::new(Some("Nouveautés"));
    title.set_xalign(0.0);
    title.add_css_class("title-4");
    labels.append(&title);
    labels.append(&widgets::caption(&format!("DmxMoney {}", env!("CARGO_PKG_VERSION"))));
    header.append(&labels);
    shell.body().append(&header);

    for note in RELEASE_NOTES {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_valign(gtk::Align::Start);
        let check = icons::image("CheckCircle2", 14);
        check.set_valign(gtk::Align::Start);
        check.add_css_class("accent");
        row.append(&check);
        let label = widgets::caption(note);
        label.remove_css_class("dim-label");
        row.append(&label);
        shell.body().append(&row);
    }

    {
        let store = store.clone();
        let shell = shell.clone();
        shell.clone().on_submit(move || {
            store.apply(SettingsChange::LastSeenVersion(env!("CARGO_PKG_VERSION").to_string()));
            shell.close();
        });
    }
    shell.present(parent);
}

// --- Restauration d'une sauvegarde ---

pub fn restore_backup(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, content: String, file_name: String) {
    let shell = FormShell::new("Importer une sauvegarde", Some("Remplacer mes données"), 520);

    let summary = match store.read(|engine| engine.inspect_backup(&content)) {
        Some(summary) => summary,
        None => return,
    };

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.append(&icons::badge("Database", "#6366f1", 40, false));
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let name = gtk::Label::new(Some(&file_name));
    name.set_xalign(0.0);
    name.add_css_class("heading");
    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    labels.append(&name);
    labels.append(&widgets::caption(&format!(
        "Sauvegarde du {}",
        format::day_long(&summary.timestamp.chars().take(10).collect::<String>())
    )));
    header.append(&labels);
    shell.body().append(&header);

    let counts = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    counts.set_homogeneous(true);
    for (value, label) in [
        (summary.accounts, "comptes"),
        (summary.transactions, "transactions"),
        (summary.categories, "catégories"),
        (summary.scheduled, "échéances"),
        (summary.budgets, "budgets"),
    ] {
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 2);
        tile.add_css_class("card");
        let count = gtk::Label::new(Some(&value.to_string()));
        count.add_css_class("title-4");
        tile.append(&count);
        let caption = widgets::caption(label);
        caption.set_xalign(0.5);
        tile.append(&caption);
        counts.append(&tile);
    }
    shell.body().append(&counts);

    let mode = Rc::new(RefCell::new(RestoreMode::Replace));
    let mode_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    mode_box.add_css_class("linked");
    mode_box.set_homogeneous(true);
    let explanation = widgets::caption("");
    explanation.add_css_class("dmx-warning");
    let mut first: Option<gtk::ToggleButton> = None;
    for (value, label) in [(RestoreMode::Replace, "Remplacer"), (RestoreMode::Merge, "Fusionner")] {
        let button = gtk::ToggleButton::with_label(label);
        button.set_active(value == RestoreMode::Replace);
        match &first {
            Some(first) => button.set_group(Some(first)),
            None => first = Some(button.clone()),
        }
        let mode = mode.clone();
        let shell_for_mode = shell.clone();
        let explanation = explanation.clone();
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            *mode.borrow_mut() = value;
            shell_for_mode.set_submit_title(match value {
                RestoreMode::Replace => "Remplacer mes données",
                RestoreMode::Merge => "Fusionner",
            });
            set_mode_explanation(&explanation, value);
        });
        mode_box.append(&button);
    }
    shell.body().append(&field("Mode d'import", &mode_box));
    set_mode_explanation(&explanation, RestoreMode::Replace);
    shell.body().append(&explanation);

    {
        let store = store.clone();
        let shell = shell.clone();
        shell.clone().on_submit(move || {
            let mode = *mode.borrow();
            let error = store.attempt(|engine| engine.restore_backup(&content, mode));
            shell.finish(&store, error, "Import réussi");
        });
    }
    shell.present(parent);
}

fn set_mode_explanation(label: &gtk::Label, mode: RestoreMode) {
    match mode {
        RestoreMode::Replace => {
            label.set_text("Toutes les données actuelles seront remplacées par celles de la sauvegarde.");
            label.add_css_class("dmx-warning");
            label.remove_css_class("dim-label");
        }
        RestoreMode::Merge => {
            label.set_text("Les éléments de la sauvegarde sont ajoutés ; les éléments déjà présents sont conservés.");
            label.remove_css_class("dmx-warning");
            label.add_css_class("dim-label");
        }
    }
}

// --- Assistant d'import de relevés ---

const NEW_ACCOUNT: &str = "__new__";
const NEW_CATEGORY: &str = "__new__";

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Columns,
    Account,
    Categories,
    Confirm,
}

impl Step {
    fn label(self) -> &'static str {
        match self {
            Step::Columns => "Colonnes",
            Step::Account => "Compte",
            Step::Categories => "Catégories",
            Step::Confirm => "Confirmation",
        }
    }
}

struct Wizard {
    format: StatementFormat,
    step: Step,
    separator: char,
    has_header: bool,
    mapping: CsvColumnMapping,
    transactions: Vec<ParsedStatementTransaction>,
    sources: Vec<String>,
    categories: HashMap<String, Option<String>>,
    account: Option<String>,
    new_name: String,
    new_type: String,
    final_balance: String,
}

impl Wizard {
    fn steps(&self) -> Vec<Step> {
        if self.format == StatementFormat::Csv {
            vec![Step::Columns, Step::Account, Step::Categories, Step::Confirm]
        } else {
            vec![Step::Account, Step::Categories, Step::Confirm]
        }
    }

    fn csv_options(&self) -> CsvOptions {
        CsvOptions {
            separator: self.separator,
            has_header: self.has_header,
        }
    }
}

pub fn statement_import(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, content: String, file_name: String) {
    let format = StatementFormat::from_file_name(&file_name).unwrap_or_default();
    let title = match format {
        StatementFormat::Csv => "Assistant d'import CSV",
        StatementFormat::Qif => "Import QIF",
        StatementFormat::Ofx => "Import OFX",
    };
    let shell = FormShell::new(title, Some("Suivant"), 720);

    let state = Rc::new(RefCell::new(Wizard {
        format,
        step: if format == StatementFormat::Csv {
            Step::Columns
        } else {
            Step::Account
        },
        separator: detect_csv_separator(&content),
        has_header: true,
        mapping: CsvColumnMapping::default(),
        transactions: Vec::new(),
        sources: Vec::new(),
        categories: HashMap::new(),
        account: None,
        new_name: String::new(),
        new_type: ACCOUNT_TYPES[0].to_string(),
        final_balance: String::new(),
    }));

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.append(&icons::badge("FileText", "#6366f1", 36, false));
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let name = gtk::Label::new(Some(title));
    name.set_xalign(0.0);
    name.add_css_class("heading");
    labels.append(&name);
    let file_label = widgets::caption(&file_name);
    file_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    labels.append(&file_label);
    header.append(&labels);
    shell.body().append(&header);

    let steps_host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    shell.body().append(&steps_host);
    shell.body().append(&widgets::separator());
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    shell.body().append(&page);

    let back = widgets::action_button("Retour", Some("ChevronLeft"), false);
    shell.extra_actions().append(&back);

    // Rendu de l'étape courante ; la fonction s'appelle elle-même après chaque changement.
    let render: Render = Rc::new(RefCell::new(None));
    let render_step: Rc<dyn Fn()> = {
        let store = store.clone();
        let shell = shell.clone();
        let state = state.clone();
        let content = content.clone();
        let steps_host = steps_host.clone();
        let page = page.clone();
        let back = back.clone();
        let render = render.clone();
        Rc::new(move || {
            // Relance le rendu après un changement d'option (séparateur, en-tête, colonnes).
            let again: Rc<dyn Fn()> = {
                let render = render.clone();
                Rc::new(move || {
                    let callback = render.borrow().clone();
                    if let Some(callback) = callback {
                        callback();
                    }
                })
            };
            let steps = state.borrow().steps();
            let step = state.borrow().step;
            let index = steps.iter().position(|candidate| *candidate == step).unwrap_or(0);
            widgets::clear(&steps_host);
            let labels: Vec<&str> = steps.iter().map(|step| step.label()).collect();
            steps_host.append(&step_indicator(&labels, index));
            back.set_visible(index > 0);
            shell.set_submit_title(if step == Step::Confirm {
                "Importer maintenant"
            } else {
                "Suivant"
            });
            widgets::clear(&page);

            match step {
                Step::Columns => columns_step(&store, &shell, &state, &content, &page, again.clone()),
                Step::Account => account_step(&store, &state, &page, again.clone()),
                Step::Categories => categories_step(&store, &state, &page),
                Step::Confirm => confirm_step(&store, &state, &page),
            }
        })
    };
    *render.borrow_mut() = Some(render_step.clone());

    {
        let state = state.clone();
        let render_step = render_step.clone();
        let shell = shell.clone();
        back.connect_clicked(move |_| {
            shell.set_error(None);
            let mut wizard = state.borrow_mut();
            let steps = wizard.steps();
            let index = steps
                .iter()
                .position(|candidate| *candidate == wizard.step)
                .unwrap_or(0);
            if index == 0 {
                return;
            }
            // Sans catégorie dans le fichier, l'étape Catégories est sautée.
            let mut target = index - 1;
            if steps[target] == Step::Categories && wizard.sources.is_empty() {
                target = target.saturating_sub(1);
            }
            wizard.step = steps[target];
            drop(wizard);
            render_step();
        });
    }

    {
        let store = store.clone();
        let shell = shell.clone();
        let state = state.clone();
        let content = content.clone();
        let render_step = render_step.clone();
        shell.clone().on_submit(move || {
            shell.set_error(None);
            let step = state.borrow().step;
            match step {
                Step::Columns => {
                    if let Err(message) = parse_statement(&store, &state, &content) {
                        return shell.set_error(Some(&message));
                    }
                    state.borrow_mut().step = Step::Account;
                    render_step();
                }
                Step::Account => {
                    let choice = state.borrow().account.clone();
                    match choice {
                        None => return shell.set_error(Some("Sélectionnez un compte de destination")),
                        Some(choice) if choice == NEW_ACCOUNT => {
                            let wizard = state.borrow();
                            if wizard.new_name.trim().is_empty() {
                                drop(wizard);
                                return shell.set_error(Some("Saisissez le nom du nouveau compte"));
                            }
                            if !wizard.final_balance.trim().is_empty()
                                && format::parse_amount(&wizard.final_balance).is_none()
                            {
                                drop(wizard);
                                return shell.set_error(Some("Saisissez un montant valide"));
                            }
                        }
                        Some(_) => {}
                    }
                    let sources_empty = state.borrow().sources.is_empty();
                    state.borrow_mut().step = if sources_empty { Step::Confirm } else { Step::Categories };
                    render_step();
                }
                Step::Categories => {
                    state.borrow_mut().step = Step::Confirm;
                    render_step();
                }
                Step::Confirm => {
                    let request = match import_request(&state) {
                        Ok(request) => request,
                        Err(message) => return shell.set_error(Some(&message)),
                    };
                    match store.read(|engine| engine.import_statement(request)) {
                        Some(result) => {
                            let duplicates = if result.duplicates > 0 {
                                format!(" ({} doublons ignorés)", result.duplicates)
                            } else {
                                String::new()
                            };
                            store.reload();
                            store.show_toast(&format!("{} transactions importées{duplicates}", result.imported));
                            shell.close();
                        }
                        None => shell.close(),
                    }
                }
            }
        });
    }

    // QIF et OFX n'ont pas d'étape colonnes : le fichier est analysé tout de suite.
    if format != StatementFormat::Csv {
        if let Err(message) = parse_statement(store, &state, &content) {
            shell.set_error(Some(&message));
        }
    }
    render_step();
    shell.present(parent);
}

/// Analyse le fichier et prépare l'association des catégories.
fn parse_statement(store: &Rc<Store>, state: &Rc<RefCell<Wizard>>, content: &str) -> Result<(), String> {
    let (format, csv, today) = {
        let wizard = state.borrow();
        let csv = (wizard.format == StatementFormat::Csv).then(|| (wizard.csv_options(), wizard.mapping));
        (wizard.format, csv, store.today())
    };
    if let Some((_, mapping)) = &csv {
        if mapping.date.is_none() || mapping.amount.is_none() {
            return Err("Assignez au moins les colonnes Date et Montant.".to_string());
        }
    }
    let transactions = store
        .read(|engine| engine.parse_statement(format, content, csv, today))
        .ok_or_else(|| "Le fichier n'a pas pu être analysé.".to_string())?;
    if transactions.is_empty() {
        return Err("Aucune transaction n'a été trouvée dans le fichier.".to_string());
    }
    let sources = source_categories(&transactions);
    let suggestions = store
        .read(|engine| engine.suggest_category_mapping(&sources))
        .unwrap_or_default();
    let mut mapping = HashMap::new();
    for suggestion in suggestions {
        mapping.insert(suggestion.source, suggestion.category_id);
    }
    let mut wizard = state.borrow_mut();
    wizard.transactions = transactions;
    wizard.sources = sources;
    wizard.categories = mapping;
    Ok(())
}

fn import_request(state: &Rc<RefCell<Wizard>>) -> Result<StatementImportRequest, String> {
    let wizard = state.borrow();
    let choice = wizard
        .account
        .clone()
        .ok_or_else(|| "Sélectionnez un compte".to_string())?;
    let target = if choice == NEW_ACCOUNT {
        ImportTarget::New {
            name: wizard.new_name.trim().to_string(),
            account_type: wizard.new_type.clone(),
            final_balance: format::parse_amount(&wizard.final_balance),
        }
    } else {
        ImportTarget::Existing(choice)
    };
    let category_mapping = wizard
        .sources
        .iter()
        .map(|source| CategoryMatch {
            source: source.clone(),
            category_id: wizard.categories.get(source).cloned().flatten(),
        })
        .collect();
    Ok(StatementImportRequest {
        transactions: wizard.transactions.clone(),
        target,
        category_mapping,
    })
}

/// Étape 1 : séparateur, en-tête et assignation des colonnes.
fn columns_step(
    store: &Rc<Store>,
    shell: &Rc<FormShell>,
    state: &Rc<RefCell<Wizard>>,
    content: &str,
    page: &gtk::Box,
    again: Rc<dyn Fn()>,
) {
    let options = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let separator_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    separator_box.add_css_class("linked");
    let current = state.borrow().separator;
    let mut first: Option<gtk::ToggleButton> = None;
    for (value, label) in [(';', "Point-virgule (;)"), (',', "Virgule (,)"), ('\t', "Tabulation")] {
        let button = gtk::ToggleButton::with_label(label);
        button.set_active(value == current);
        match &first {
            Some(first) => button.set_group(Some(first)),
            None => first = Some(button.clone()),
        }
        let state = state.clone();
        let again = again.clone();
        button.connect_toggled(move |button| {
            if button.is_active() && state.borrow().separator != value {
                state.borrow_mut().separator = value;
                again();
            }
        });
        separator_box.append(&button);
    }
    options.append(&field("Séparateur", &separator_box));

    let header_check = gtk::CheckButton::with_label("La première ligne est un en-tête");
    header_check.set_active(state.borrow().has_header);
    header_check.set_valign(gtk::Align::End);
    {
        let state = state.clone();
        let again = again.clone();
        header_check.connect_toggled(move |button| {
            if state.borrow().has_header != button.is_active() {
                state.borrow_mut().has_header = button.is_active();
                again();
            }
        });
    }
    options.append(&header_check);
    page.append(&options);

    let preview = match store.read(|engine| engine.preview_csv(content, state.borrow().csv_options())) {
        Some(preview) => preview,
        None => return,
    };
    shell.set_error(None);

    let grid = gtk::Grid::new();
    grid.set_column_spacing(6);
    grid.set_row_spacing(4);
    let roles = [
        ("ignore", "Ignorer"),
        ("date", "Date"),
        ("amount", "Montant"),
        ("description", "Description"),
        ("category", "Catégorie"),
    ];
    for column in 0..preview.column_count {
        let role = column_role(&state.borrow().mapping, column);
        let select = Select::new(
            "Ignorer",
            roles
                .iter()
                .map(|(id, label)| SelectOption::simple(id, label))
                .collect(),
            Some(role.to_string()),
            None,
        );
        select.widget().set_size_request(150, -1);
        let state_for_select = state.clone();
        let again_for_select = again.clone();
        select.connect_changed(move |value| {
            let role = value.unwrap_or_else(|| "ignore".to_string());
            {
                let mut wizard = state_for_select.borrow_mut();
                let mapping = assign_role(wizard.mapping, column, &role);
                wizard.mapping = mapping;
            }
            again_for_select();
        });
        grid.attach(&select.widget(), column as i32, 0, 1, 1);
    }
    for (row_index, row) in preview.rows.iter().take(8).enumerate() {
        for column in 0..preview.column_count {
            let text = row.get(column as usize).cloned().unwrap_or_default();
            let label = gtk::Label::new(Some(&text));
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_size_request(150, -1);
            if column_role(&state.borrow().mapping, column) == "ignore" {
                label.add_css_class("dim-label");
            }
            grid.attach(&label, column as i32, row_index as i32 + 1, 1, 1);
        }
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_min_content_height(260);
    scroll.set_max_content_height(300);
    scroll.set_child(Some(&grid));
    page.append(&scroll);
    page.append(&widgets::caption(&format!(
        "{} lignes détectées. Assignez les colonnes avec les listes au-dessus de l'aperçu.",
        preview.row_count
    )));
}

fn column_role(mapping: &CsvColumnMapping, column: u32) -> &'static str {
    if mapping.date == Some(column) {
        "date"
    } else if mapping.amount == Some(column) {
        "amount"
    } else if mapping.description == Some(column) {
        "description"
    } else if mapping.category == Some(column) {
        "category"
    } else {
        "ignore"
    }
}

/// Un rôle n'est porté que par une colonne à la fois, comme en 1.x.
fn assign_role(mapping: CsvColumnMapping, column: u32, role: &str) -> CsvColumnMapping {
    let mut updated = mapping;
    for field in [
        &mut updated.date,
        &mut updated.amount,
        &mut updated.description,
        &mut updated.category,
    ] {
        if *field == Some(column) {
            *field = None;
        }
    }
    match role {
        "date" => updated.date = Some(column),
        "amount" => updated.amount = Some(column),
        "description" => updated.description = Some(column),
        "category" => updated.category = Some(column),
        _ => {}
    }
    updated
}

/// Étape 2 : compte de destination, existant ou nouveau.
fn account_step(store: &Rc<Store>, state: &Rc<RefCell<Wizard>>, page: &gtk::Box, again: Rc<dyn Fn()>) {
    page.append(&widgets::section_label("Vers quel compte importer ?"));
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let selected = state.borrow().account.clone();
    let mut first: Option<gtk::CheckButton> = None;

    for account in store.accounts() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("card");
        let check = gtk::CheckButton::new();
        check.set_active(selected.as_deref() == Some(account.id.as_str()));
        match &first {
            Some(first) => check.set_group(Some(first)),
            None => first = Some(check.clone()),
        }
        row.append(&check);
        row.append(&icons::badge(&account.icon, &account.color, 30, true));
        let label = gtk::Label::new(Some(&account.name));
        label.set_hexpand(true);
        label.set_xalign(0.0);
        row.append(&label);
        let state_for_check = state.clone();
        let id = account.id.clone();
        check.connect_toggled(move |check| {
            if check.is_active() {
                state_for_check.borrow_mut().account = Some(id.clone());
            }
        });
        list.append(&row);
    }

    let new_row = gtk::Box::new(gtk::Orientation::Vertical, 8);
    new_row.add_css_class("card");
    let new_header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let new_check = gtk::CheckButton::new();
    new_check.set_active(selected.as_deref() == Some(NEW_ACCOUNT));
    match &first {
        Some(first) => new_check.set_group(Some(first)),
        None => first = Some(new_check.clone()),
    }
    let _ = first;
    new_header.append(&new_check);
    new_header.append(&icons::badge("Upload", "#9ca3af", 30, false));
    let new_label = gtk::Label::new(Some("Nouveau compte"));
    new_label.set_hexpand(true);
    new_label.set_xalign(0.0);
    new_header.append(&new_label);
    new_row.append(&new_header);

    let details = gtk::Box::new(gtk::Orientation::Vertical, 10);
    details.set_visible(selected.as_deref() == Some(NEW_ACCOUNT));
    let name = entry("Nom du compte", &state.borrow().new_name);
    {
        let state = state.clone();
        name.connect_changed(move |entry| state.borrow_mut().new_name = entry.text().to_string());
    }
    details.append(&field("Nom", &name));
    let type_select = Select::new(
        "Type",
        ACCOUNT_TYPES
            .iter()
            .map(|value| SelectOption::simple(value, value))
            .collect(),
        Some(state.borrow().new_type.clone()),
        None,
    );
    {
        let state = state.clone();
        type_select.connect_changed(move |value| {
            state.borrow_mut().new_type = value.unwrap_or_else(|| ACCOUNT_TYPES[0].to_string());
        });
    }
    details.append(&field("Type", &type_select.widget()));
    let balance = amount_entry(0.0);
    balance.set_text(&state.borrow().final_balance);
    {
        let state = state.clone();
        balance.connect_changed(move |entry| state.borrow_mut().final_balance = entry.text().to_string());
    }
    details.append(&field("Solde final du relevé (optionnel)", &balance));
    details.append(&widgets::caption(
        "Saisissez le solde final du relevé pour calculer automatiquement le solde initial.",
    ));
    new_row.append(&details);
    list.append(&new_row);

    {
        let state = state.clone();
        new_check.connect_toggled(move |check| {
            if check.is_active() {
                state.borrow_mut().account = Some(NEW_ACCOUNT.to_string());
                again();
            }
        });
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_min_content_height(300);
    scroll.set_max_content_height(340);
    scroll.set_child(Some(&list));
    page.append(&scroll);
}

/// Étape 3 : association des catégories du fichier.
fn categories_step(store: &Rc<Store>, state: &Rc<RefCell<Wizard>>, page: &gtk::Box) {
    page.append(&widgets::caption(
        "Associez les catégories du fichier à vos catégories existantes.",
    ));
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let sources = state.borrow().sources.clone();
    for source in sources {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("card");
        let label = gtk::Label::new(Some(&source));
        label.set_hexpand(true);
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&label);
        row.append(&icons::image("ArrowRight", 14));

        let mut options = vec![SelectOption::simple(NEW_CATEGORY, &format!("+ Créer « {source} »"))];
        options.extend(
            store
                .selectable_categories()
                .iter()
                .map(|category| SelectOption::with_icon(&category.id, &category.name, &category.icon, &category.color)),
        );
        let current = state
            .borrow()
            .categories
            .get(&source)
            .cloned()
            .flatten()
            .unwrap_or_else(|| NEW_CATEGORY.to_string());
        let select = Select::new("Sélectionner une catégorie", options, Some(current), None);
        select.widget().set_size_request(280, -1);
        let state_for_select = state.clone();
        let source_for_select = source.clone();
        select.connect_changed(move |value| {
            let category_id = value.filter(|id| id != NEW_CATEGORY);
            state_for_select
                .borrow_mut()
                .categories
                .insert(source_for_select.clone(), category_id);
        });
        row.append(&select.widget());
        list.append(&row);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_min_content_height(280);
    scroll.set_max_content_height(340);
    scroll.set_child(Some(&list));
    page.append(&scroll);
}

/// Étape 4 : récapitulatif avant écriture.
fn confirm_step(store: &Rc<Store>, state: &Rc<RefCell<Wizard>>, page: &gtk::Box) {
    let wizard = state.borrow();
    let host = gtk::Box::new(gtk::Orientation::Vertical, 10);
    host.set_halign(gtk::Align::Center);
    host.append(&icons::badge("Check", "#10b981", 60, false));
    let title = gtk::Label::new(Some("Prêt à importer"));
    title.add_css_class("title-3");
    host.append(&title);

    let account_name = match wizard.account.as_deref() {
        Some(NEW_ACCOUNT) => wizard.new_name.trim().to_string(),
        Some(id) => store.account_name(id).unwrap_or_default(),
        None => String::new(),
    };
    let summary = gtk::Label::new(Some(&format!(
        "{} transactions seront importées dans le compte {account_name}.",
        wizard.transactions.len()
    )));
    summary.set_wrap(true);
    summary.set_justify(gtk::Justification::Center);
    host.append(&summary);

    if wizard.account.as_deref() == Some(NEW_ACCOUNT) {
        if let Some(final_balance) = format::parse_amount(&wizard.final_balance) {
            host.append(&widgets::caption(&format!(
                "Solde initial calculé : {}",
                format::money(initial_balance_from_final(&wizard.transactions, final_balance))
            )));
        }
    }
    page.append(&host);

    if wizard.format == StatementFormat::Csv && wizard.mapping.category.is_none() {
        let warning = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        warning.add_css_class("card");
        warning.append(&icons::colored_image("AlertTriangle", "#f97316", 15));
        let label = widgets::caption(
            "Aucune colonne catégorie n'a été sélectionnée. Toutes les transactions seront classées dans « Divers ».",
        );
        warning.append(&label);
        page.append(&warning);
    }
}
