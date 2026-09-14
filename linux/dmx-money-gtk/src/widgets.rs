//! Briques d'interface communes : cartes, pastilles, états vides, lignes de tableau.

use gtk::prelude::*;

use crate::icons;

/// Carte avec en-tête optionnel. Renvoie (carte, contenu, actions d'en-tête).
pub fn card(title: Option<&str>, icon: Option<&str>) -> (gtk::Box, gtk::Box, gtk::Box) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("dmx-card");
    card.add_css_class("card");

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    actions.set_hexpand(true);

    if title.is_some() || icon.is_some() {
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.set_margin_bottom(12);
        if let Some(icon) = icon {
            header.append(&icons::image(icon, 15));
        }
        if let Some(title) = title {
            let label = gtk::Label::new(Some(title));
            label.add_css_class("dmx-card-title");
            label.set_xalign(0.0);
            header.append(&label);
        }
        header.append(&actions);
        card.append(&header);
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_hexpand(true);
    card.append(&content);
    (card, content, actions)
}

pub fn section_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dmx-section-label");
    label.set_xalign(0.0);
    label
}

pub fn title_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dmx-page-title");
    label.set_xalign(0.0);
    label
}

pub fn caption(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

/// Montant aligné à droite, coloré selon la classe (`dmx-income`, `dmx-expense`…).
pub fn amount(text: &str, class: Option<&str>) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dmx-amount");
    if let Some(class) = class {
        label.add_css_class(class);
    }
    label.set_xalign(1.0);
    label
}

/// Pastille colorée : icône et texte sur fond teinté.
pub fn chip(text: &str, color: &str, icon: Option<&str>) -> gtk::Box {
    let chip = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    chip.add_css_class("dmx-chip");
    chip.add_css_class(&icons::tint_class(color));
    chip.set_halign(gtk::Align::Start);
    if let Some(icon) = icon {
        chip.append(&icons::image(icon, 11));
    }
    let label = gtk::Label::new(Some(text));
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    chip.append(&label);
    chip
}

pub fn empty_state(icon: &str, title: &str, message: Option<&str>) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 8);
    host.set_halign(gtk::Align::Center);
    host.set_valign(gtk::Align::Center);
    host.set_margin_top(24);
    host.set_margin_bottom(24);
    let image = icons::image(icon, 28);
    image.add_css_class("dim-label");
    host.append(&image);
    let label = gtk::Label::new(Some(title));
    label.add_css_class("heading");
    host.append(&label);
    if let Some(message) = message {
        let caption = gtk::Label::new(Some(message));
        caption.add_css_class("dim-label");
        caption.set_wrap(true);
        caption.set_justify(gtk::Justification::Center);
        host.append(&caption);
    }
    host
}

pub fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(&icons::image(icon, 14)));
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("flat");
    button
}

/// Bouton principal avec icône (« + Nouveau compte »).
pub fn action_button(label: &str, icon: Option<&str>, suggested: bool) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if let Some(icon) = icon {
        content.append(&icons::image(icon, 14));
    }
    content.append(&gtk::Label::new(Some(label)));
    let button = gtk::Button::new();
    button.set_child(Some(&content));
    if suggested {
        button.add_css_class("suggested-action");
    }
    button
}

/// Page défilante avec marge et largeur maximale.
pub fn page_scroll(content: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(1280);
    clamp.set_child(Some(content));
    clamp.set_margin_top(20);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(24);
    clamp.set_margin_end(24);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&clamp));
    scroll
}

/// Vide un conteneur (reconstruction d'une liste).
///
/// `GtkListBox` et `GtkFlowBox` encapsulent leurs enfants : les détacher un à un laisse leur
/// état interne incohérent (GTK signale alors des frères invalides). On passe donc par leur
/// propre `remove_all`.
pub fn clear(container: &impl IsA<gtk::Widget>) {
    let container = container.as_ref();
    if let Some(list) = container.downcast_ref::<gtk::ListBox>() {
        list.remove_all();
        return;
    }
    if let Some(flow) = container.downcast_ref::<gtk::FlowBox>() {
        flow.remove_all();
        return;
    }
    while let Some(child) = container.first_child() {
        child.unparent();
    }
}

/// Séparateur horizontal discret.
pub fn separator() -> gtk::Separator {
    gtk::Separator::new(gtk::Orientation::Horizontal)
}

/// Barre de progression colorée selon le dépassement.
pub fn progress(fraction: f64, over: bool) -> gtk::ProgressBar {
    let bar = gtk::ProgressBar::new();
    bar.set_fraction(fraction.clamp(0.0, 1.0));
    bar.add_css_class("dmx-progress");
    bar.add_css_class(if over { "dmx-progress-over" } else { "dmx-progress-ok" });
    bar
}

/// Colonne d'un tableau : libellé en tête, largeur fixe.
pub fn column_header(text: &str, width: i32, align: gtk::Align) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dmx-table-header");
    label.set_halign(align);
    if width > 0 {
        label.set_size_request(width, -1);
    } else {
        label.set_hexpand(true);
    }
    label
}

/// Cellule d'un tableau.
pub fn cell(child: &impl IsA<gtk::Widget>, width: i32, align: gtk::Align) -> gtk::Widget {
    let host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    host.set_halign(align);
    host.set_valign(gtk::Align::Center);
    if width > 0 {
        host.set_size_request(width, -1);
    } else {
        host.set_hexpand(true);
    }
    host.append(child);
    host.upcast()
}
