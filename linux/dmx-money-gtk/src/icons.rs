//! Icônes natives (thème Adwaita et GNOME Icon Development Kit), recolorées par le thème,
//! et couleurs des comptes et catégories appliquées par CSS.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

thread_local! {
    static COLOR_RULES: RefCell<BTreeMap<String, String>> = const { RefCell::new(BTreeMap::new()) };
    static COLOR_PROVIDER: gtk::CssProvider = gtk::CssProvider::new();
    static FLUSH_QUEUED: Cell<bool> = const { Cell::new(false) };
}

/// Icône native d'un nom stocké en base : icône du thème Adwaita, ou icône embarquée du GNOME
/// Icon Development Kit (table partagée shared/icons/native.json).
pub fn icon_name(name: &str) -> String {
    crate::icon_names::native(name).to_string()
}

/// Ajoute les dossiers d'icônes possibles : installation système, AppImage, ou sources.
pub fn install_search_paths() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    for path in candidate_paths() {
        if path.exists() {
            theme.add_search_path(&path);
        }
    }
    COLOR_PROVIDER.with(|provider| {
        gtk::style_context_add_provider_for_display(&display, provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    });
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            paths.push(directory.join("../share/icons"));
            paths.push(directory.join("data/icons"));
        }
    }
    paths.push(PathBuf::from("/app/share/icons"));
    paths.push(PathBuf::from("/usr/share/icons"));
    paths.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/icons"));
    paths
}

/// Classe CSS qui colore une icône ou un texte (`color`).
pub fn color_class(hex: &str) -> String {
    let clean = normalize(hex);
    let class = format!("dmx-c-{clean}");
    add_rule(&class, &format!(".{class} {{ color: #{clean}; }}"));
    class
}

/// Classe CSS d'un fond légèrement teinté (pastilles).
pub fn tint_class(hex: &str) -> String {
    let clean = normalize(hex);
    let class = format!("dmx-t-{clean}");
    add_rule(
        &class,
        &format!(".{class} {{ background-color: alpha(#{clean}, 0.16); color: #{clean}; }}"),
    );
    class
}

/// Classe CSS d'un fond plein (pastille remplie).
pub fn fill_class(hex: &str) -> String {
    let clean = normalize(hex);
    let class = format!("dmx-f-{clean}");
    add_rule(
        &class,
        &format!(".{class} {{ background-color: #{clean}; color: #ffffff; }}"),
    );
    class
}

fn normalize(hex: &str) -> String {
    let clean: String = hex
        .trim_start_matches('#')
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect();
    if clean.len() == 3 {
        clean.chars().flat_map(|character| [character, character]).collect()
    } else if clean.len() >= 6 {
        clean[..6].to_lowercase()
    } else {
        "9ca3af".to_string()
    }
}

fn add_rule(class: &str, rule: &str) {
    let changed = COLOR_RULES.with(|rules| {
        let mut rules = rules.borrow_mut();
        if rules.contains_key(class) {
            false
        } else {
            rules.insert(class.to_string(), rule.to_string());
            true
        }
    });
    if changed {
        queue_flush();
    }
}

/// Recharger la feuille de style au milieu de la construction d'une grille relance le calcul
/// des styles sur des widgets encore en cours d'insertion (GTK émet alors des avertissements).
/// Les nouvelles règles sont donc publiées une seule fois, avant le prochain rendu.
fn queue_flush() {
    if FLUSH_QUEUED.with(|queued| queued.replace(true)) {
        return;
    }
    glib::idle_add_local_full(glib::Priority::HIGH_IDLE, || {
        FLUSH_QUEUED.with(|queued| queued.set(false));
        flush();
        glib::ControlFlow::Break
    });
}

fn flush() {
    let css = COLOR_RULES.with(|rules| rules.borrow().values().cloned().collect::<Vec<_>>().join("\n"));
    COLOR_PROVIDER.with(|provider| provider.load_from_string(&css));
}

/// Enregistre les couleurs connues (palette, accents, types) au démarrage : les pages et les
/// formulaires n'ont alors plus aucune règle à ajouter pendant leur construction.
pub fn preload_palette() {
    let semantic = [
        "#10b981", "#ef4444", "#6366f1", "#f97316", "#f59e0b", "#9ca3af", "#3b82f6", "#8b5cf6",
    ];
    for color in dmx_core::palette::CATEGORY_COLORS
        .iter()
        .chain(dmx_core::palette::ACCENT_COLORS.iter())
        .chain(semantic.iter())
    {
        let _ = color_class(color);
        let _ = tint_class(color);
        let _ = fill_class(color);
    }
    flush();
}

/// Icône symbolique native.
pub fn image(lucide: &str, size: i32) -> gtk::Image {
    let image = gtk::Image::from_icon_name(&icon_name(lucide));
    image.set_pixel_size(size);
    image
}

/// Icône teintée de la couleur donnée.
pub fn colored_image(lucide: &str, hex: &str, size: i32) -> gtk::Image {
    let image = image(lucide, size);
    image.add_css_class(&color_class(hex));
    image
}

/// Pastille arrondie : icône sur fond teinté (ou plein).
pub fn badge(lucide: &str, hex: &str, size: i32, filled: bool) -> gtk::Widget {
    let image = image(lucide, (size as f32 * 0.55) as i32);
    let host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    host.set_halign(gtk::Align::Center);
    host.set_valign(gtk::Align::Center);
    host.append(&image);
    host.set_size_request(size, size);
    host.add_css_class("dmx-badge");
    let class = if filled { fill_class(hex) } else { tint_class(hex) };
    host.add_css_class(&class);
    host.upcast()
}

/// Feuille de style de l'application, chargée au démarrage.
pub fn load_stylesheet() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../data/style.css"));
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1);
}
