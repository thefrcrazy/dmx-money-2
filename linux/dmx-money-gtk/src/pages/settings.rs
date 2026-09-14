//! Paramètres : apparence, pont PWA (QR d'appairage), données, à propos.

use std::rc::Rc;

use adw::prelude::*;
use dmx_core::models::Theme;
use dmx_core::settings::SettingsChange;
use gtk::{gio, glib};
use qrcode::QrCode;

use crate::store::{FormRequest, Store};
use crate::{bridge, format, icons, widgets};

/// Modules du QR d'appairage : taille du côté et cases sombres.
type QrModules = (usize, Vec<bool>);

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    theme_box: gtk::Box,
    accent_box: gtk::Box,
    bridge_section: gtk::Box,
    bridge_switch: gtk::Switch,
    bridge_badge: gtk::Label,
    bridge_detail: gtk::Label,
    app_url: gtk::Label,
    api_url: gtk::Label,
    steps: gtk::Box,
    bridge_error: gtk::Label,
    qr_area: gtk::DrawingArea,
    qr_empty: gtk::Label,
    pairing_button: gtk::Button,
    copy_button: gtk::Button,
    passkeys_title: gtk::Label,
    passkeys: gtk::Box,
    version_label: gtk::Label,
    qr_modules: Rc<std::cell::RefCell<Option<QrModules>>>,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 28);
        content.append(&widgets::title_label("Paramètres"));

        // --- Apparence ---
        let (appearance_card, appearance_content, _) = widgets::card(None, None);
        content.append(&section("Apparence", "Palette", &appearance_card));

        let theme_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let theme_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        theme_labels.set_hexpand(true);
        let theme_title = gtk::Label::new(Some("Thème de l'interface"));
        theme_title.set_xalign(0.0);
        theme_title.add_css_class("heading");
        theme_labels.append(&theme_title);
        theme_labels.append(&widgets::caption("Choisissez l'aspect visuel de l'application"));
        theme_row.append(&theme_labels);
        let theme_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        theme_box.add_css_class("linked");
        theme_row.append(&theme_box);
        appearance_content.append(&theme_row);
        appearance_content.append(&widgets::separator());

        let accent_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let accent_title = gtk::Label::new(Some("Couleur d'accentuation"));
        accent_title.set_xalign(0.0);
        accent_title.add_css_class("heading");
        accent_labels.append(&accent_title);
        accent_labels.append(&widgets::caption("Personnalisez la couleur principale"));
        appearance_content.append(&accent_labels);
        let accent_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        appearance_content.append(&accent_box);

        // --- Pont PWA ---
        let (bridge_card, bridge_content, _) = widgets::card(None, None);
        let bridge_section = section("Mode compagnon mobile", "Smartphone", &bridge_card);
        content.append(&bridge_section);

        let bridge_header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        bridge_header.append(&icons::badge("Lock", "#10b981", 34, false));
        let bridge_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        bridge_labels.set_hexpand(true);
        let bridge_title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let bridge_title = gtk::Label::new(Some("Accès mobile local + PWA"));
        bridge_title.add_css_class("heading");
        bridge_title_row.append(&bridge_title);
        let bridge_badge = gtk::Label::new(None);
        bridge_badge.add_css_class("dmx-chip");
        bridge_title_row.append(&bridge_badge);
        bridge_labels.append(&bridge_title_row);
        let bridge_detail = widgets::caption("");
        bridge_labels.append(&bridge_detail);
        bridge_header.append(&bridge_labels);
        let bridge_switch = gtk::Switch::new();
        bridge_switch.set_valign(gtk::Align::Center);
        bridge_header.append(&bridge_switch);
        bridge_content.append(&bridge_header);

        let urls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        urls.set_homogeneous(true);
        let (app_url_card, app_url) = url_tile("PWA mobile", "Globe2");
        let (api_url_card, api_url) = url_tile("API locale sécurisée", "Server");
        urls.append(&app_url_card);
        urls.append(&api_url_card);
        bridge_content.append(&urls);
        bridge_content.append(&widgets::separator());

        let bridge_body = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let left = gtk::Box::new(gtk::Orientation::Vertical, 8);
        left.set_hexpand(true);
        let steps = gtk::Box::new(gtk::Orientation::Vertical, 6);
        left.append(&steps);
        let bridge_error = widgets::caption("");
        bridge_error.add_css_class("dmx-warning");
        bridge_error.set_visible(false);
        left.append(&bridge_error);
        bridge_body.append(&left);

        let right = gtk::Box::new(gtk::Orientation::Vertical, 8);
        right.set_size_request(200, -1);
        let qr_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        qr_frame.add_css_class("card");
        qr_frame.set_size_request(140, 140);
        let qr_area = gtk::DrawingArea::new();
        qr_area.set_content_width(128);
        qr_area.set_content_height(128);
        qr_area.set_margin_top(6);
        qr_area.set_margin_bottom(6);
        qr_area.set_margin_start(6);
        qr_area.set_margin_end(6);
        qr_frame.append(&qr_area);
        let qr_empty = widgets::caption("");
        qr_empty.set_justify(gtk::Justification::Center);
        qr_empty.set_halign(gtk::Align::Center);
        qr_frame.append(&qr_empty);
        right.append(&qr_frame);
        let pairing_button = widgets::action_button("Nouveau QR", Some("KeyRound"), true);
        right.append(&pairing_button);
        let copy_button = widgets::action_button("Copier", Some("Copy"), false);
        copy_button.set_visible(false);
        right.append(&copy_button);
        right.append(&widgets::caption(
            "Ouvre la PWA sur mobile, puis appaire le téléphone avec ce QR. Génère un QR par appareil : plusieurs mobiles peuvent rester appairés.",
        ));
        bridge_body.append(&right);
        bridge_content.append(&bridge_body);
        bridge_content.append(&widgets::separator());

        let passkeys_title = widgets::section_label("Mobiles appairés");
        bridge_content.append(&passkeys_title);
        let passkeys = gtk::Box::new(gtk::Orientation::Vertical, 6);
        bridge_content.append(&passkeys);

        // --- Données ---
        let (data_card, data_content, _) = widgets::card(None, None);
        content.append(&section("Données & Stockage", "HardDrive", &data_card));
        let export = navigation_row("Download", "Exporter les données", "Créer une sauvegarde locale (.dmx)");
        data_content.append(&export);
        data_content.append(&widgets::separator());
        let import = navigation_row(
            "Upload",
            "Importer ou Restaurer",
            "Depuis un backup ou un fichier CSV/OFX/QIF",
        );
        data_content.append(&import);

        // --- À propos ---
        let (about_card, about_content, _) = widgets::card(None, None);
        content.append(&section("À propos", "Info", &about_card));
        let about_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let about_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        about_labels.set_hexpand(true);
        let name = gtk::Label::new(Some("DmxMoney"));
        name.set_xalign(0.0);
        name.add_css_class("heading");
        about_labels.append(&name);
        let version_label = widgets::caption("");
        about_labels.append(&version_label);
        about_row.append(&about_labels);
        let whats_new = gtk::Button::with_label("Nouveautés");
        let store_for_whats_new = store.clone();
        whats_new.connect_clicked(move |_| store_for_whats_new.present(FormRequest::WhatsNew));
        about_row.append(&whats_new);
        about_content.append(&about_row);

        let root = widgets::page_scroll(&content).upcast();
        let page = Self {
            store: store.clone(),
            root,
            theme_box: theme_box.clone(),
            accent_box: accent_box.clone(),
            bridge_section,
            bridge_switch: bridge_switch.clone(),
            bridge_badge,
            bridge_detail,
            app_url,
            api_url,
            steps,
            bridge_error,
            qr_area: qr_area.clone(),
            qr_empty,
            pairing_button: pairing_button.clone(),
            copy_button: copy_button.clone(),
            passkeys_title,
            passkeys,
            version_label,
            qr_modules: Rc::new(std::cell::RefCell::new(None)),
        };

        // Thème
        for (theme, icon, tooltip) in [
            (Theme::Light, "Sun", "Clair"),
            (Theme::Dark, "Moon", "Sombre"),
            (Theme::System, "Monitor", "Système"),
        ] {
            let button = gtk::ToggleButton::new();
            button.set_child(Some(&icons::image(icon, 15)));
            button.set_tooltip_text(Some(tooltip));
            let store_for_theme = store.clone();
            button.connect_clicked(move |button| {
                if button.is_active() {
                    store_for_theme.apply(SettingsChange::Theme(theme));
                }
            });
            theme_box.append(&button);
        }

        // Couleur d'accentuation
        let default_button = gtk::Button::with_label("Déf");
        default_button.set_size_request(32, 32);
        let store_for_default = store.clone();
        default_button
            .connect_clicked(move |_| store_for_default.apply(SettingsChange::PrimaryColor("default".into())));
        accent_box.append(&default_button);
        for color in dmx_core::settings::ACCENT_COLORS {
            let button = gtk::Button::new();
            button.set_size_request(28, 28);
            button.add_css_class("circular");
            button.add_css_class(&icons::fill_class(color));
            let store_for_accent = store.clone();
            let color = color.to_string();
            button.connect_clicked(move |_| store_for_accent.apply(SettingsChange::PrimaryColor(color.clone())));
            accent_box.append(&button);
        }

        // Pont
        {
            let store = store.clone();
            bridge_switch.connect_state_set(move |switch, state| {
                if switch.is_sensitive() {
                    match bridge::set_enabled(state) {
                        Ok(_) => store.reload(),
                        Err(error) => store.show_error(&format!("Pont sécurisé indisponible : {error}")),
                    }
                }
                glib::Propagation::Proceed
            });
        }
        {
            let store = store.clone();
            pairing_button.connect_clicked(move |_| match bridge::regenerate_pairing() {
                Ok(_) => store.reload(),
                Err(error) => store.show_error(&format!("Pairing impossible : {error}")),
            });
        }
        {
            let store = store.clone();
            copy_button.connect_clicked(move |button| {
                if let Some(url) = bridge::status()
                    .and_then(|status| status.secure_bridge)
                    .and_then(|bridge| bridge.pairing_url)
                {
                    button.clipboard().set_text(&url);
                    store.show_toast("URL copiée");
                }
            });
        }

        // QR
        {
            let modules = page.qr_modules.clone();
            qr_area.set_draw_func(move |_, context, width, height| {
                let Some((size, cells)) = modules.borrow().clone() else {
                    return;
                };
                let scale = (width.min(height) as f64) / size as f64;
                context.set_source_rgb(1.0, 1.0, 1.0);
                let _ = context.paint();
                context.set_source_rgb(0.0, 0.0, 0.0);
                for (index, dark) in cells.iter().enumerate() {
                    if *dark {
                        let x = (index % size) as f64 * scale;
                        let y = (index / size) as f64 * scale;
                        context.rectangle(x, y, scale, scale);
                    }
                }
                let _ = context.fill();
            });
        }

        // Données
        {
            let store = store.clone();
            export.connect_clicked(move |button| export_backup(&store, button));
        }
        {
            let store = store.clone();
            import.connect_clicked(move |button| import_file(&store, button));
        }

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let settings = self.store.settings();

        // Thème
        let mut child = self.theme_box.first_child();
        let themes = [Theme::Light, Theme::Dark, Theme::System];
        let mut index = 0usize;
        while let Some(widget) = child {
            if let Some(button) = widget.downcast_ref::<gtk::ToggleButton>() {
                let is_current = themes.get(index) == Some(&settings.theme);
                if button.is_active() != is_current {
                    button.set_active(is_current);
                }
            }
            child = widget.next_sibling();
            index += 1;
        }

        // Accent sélectionné
        let accent = settings.accent_color();
        let mut child = self.accent_box.first_child();
        while let Some(widget) = child {
            widget.remove_css_class("suggested-action");
            child = widget.next_sibling();
        }
        if let Some(accent) = accent {
            let mut child = self.accent_box.first_child();
            let mut position = 0usize;
            while let Some(widget) = child {
                if position > 0 {
                    if let Some(color) = dmx_core::settings::ACCENT_COLORS.get(position - 1) {
                        if color.eq_ignore_ascii_case(accent) {
                            widget.add_css_class("suggested-action");
                        }
                    }
                }
                child = widget.next_sibling();
                position += 1;
            }
        } else if let Some(first) = self.accent_box.first_child() {
            first.add_css_class("suggested-action");
        }

        self.version_label
            .set_text(&format!("Version {}", env!("CARGO_PKG_VERSION")));

        // Pont PWA
        self.bridge_section.set_visible(self.store.bridge_available());
        if !self.store.bridge_available() {
            return;
        }
        let status = bridge::status();
        let secure = status.as_ref().and_then(|status| status.secure_bridge.clone());
        let enabled = secure.as_ref().map(|bridge| bridge.enabled).unwrap_or(false);
        let active = secure.as_ref().map(|bridge| bridge.active).unwrap_or(false);
        let certificate_ready = secure.as_ref().map(|bridge| bridge.certificate_ready).unwrap_or(false);

        self.bridge_switch.set_sensitive(false);
        self.bridge_switch.set_active(enabled);
        self.bridge_switch.set_sensitive(true);

        let (badge, detail) = if !enabled {
            (
                "Désactivé",
                "Active le mode pour préparer le pont HTTPS et le QR mobile.",
            )
        } else if active {
            ("Prêt à appairer", "La PWA peut se connecter à l’API locale sécurisée.")
        } else if certificate_ready {
            (
                "Démarrage local",
                "Le certificat est prêt, le serveur local termine son démarrage.",
            )
        } else {
            (
                "Préparation HTTPS",
                "DNS et certificat sont préparés automatiquement en arrière-plan.",
            )
        };
        self.bridge_badge.set_text(badge);
        self.bridge_detail.set_text(detail);
        self.app_url.set_text(
            secure
                .as_ref()
                .and_then(|bridge| bridge.app_url.as_deref())
                .unwrap_or("Provisionnement automatique en attente"),
        );
        self.api_url.set_text(
            secure
                .as_ref()
                .and_then(|bridge| bridge.api_url.as_deref())
                .unwrap_or("Non active"),
        );

        let local_label = if status.as_ref().map(|status| status.active).unwrap_or(false) {
            "Actif"
        } else if enabled {
            "Démarrage"
        } else {
            "Inactif"
        };
        let provisioning_ready = secure.as_ref().map(|bridge| bridge.configured).unwrap_or(false);
        let dns_ready = secure
            .as_ref()
            .and_then(|bridge| bridge.dns_record_id.clone())
            .is_some();
        let steps = [
            (
                "PWA publique",
                if secure.as_ref().and_then(|bridge| bridge.app_url.clone()).is_some() {
                    "Disponible"
                } else {
                    "En attente"
                },
                secure.as_ref().and_then(|bridge| bridge.app_url.clone()).is_some(),
                "Globe2",
            ),
            (
                "Provisionnement",
                if provisioning_ready {
                    "Prêt"
                } else if !enabled {
                    "En attente d’activation"
                } else {
                    "En cours ou indisponible"
                },
                provisioning_ready,
                "KeyRound",
            ),
            (
                "DNS local",
                if dns_ready {
                    "Configuré"
                } else if secure
                    .as_ref()
                    .map(|bridge| bridge.managed_credential_ready)
                    .unwrap_or(false)
                {
                    "Prêt"
                } else if enabled {
                    "En attente"
                } else {
                    "En attente d’activation"
                },
                dns_ready,
                "Wifi",
            ),
            (
                "Certificat HTTPS",
                if certificate_ready {
                    "Prêt"
                } else if enabled {
                    "En génération"
                } else {
                    "Absent"
                },
                certificate_ready,
                "ShieldCheck",
            ),
            (
                "API locale",
                if secure.as_ref().and_then(|bridge| bridge.api_url.clone()).is_some() {
                    local_label
                } else {
                    "Non active"
                },
                active,
                "Server",
            ),
        ];
        widgets::clear(&self.steps);
        for (label, value, ready, icon) in steps {
            self.steps.append(&step_row(label, value, ready, icon, enabled));
        }

        let error = secure.as_ref().and_then(|bridge| bridge.last_error.clone());
        self.bridge_error.set_visible(enabled && error.is_some());
        self.bridge_error.set_text(error.as_deref().unwrap_or(""));

        // QR d'appairage
        let pairing_url = secure.as_ref().and_then(|bridge| bridge.pairing_url.clone());
        self.pairing_button.set_sensitive(active);
        if let Some(content) = self.pairing_button.child().and_downcast::<gtk::Box>() {
            if let Some(label) = content.last_child().and_downcast::<gtk::Label>() {
                label.set_text(if active {
                    "Nouveau QR"
                } else if enabled {
                    "Préparation HTTPS"
                } else {
                    "Activer d’abord"
                });
            }
        }
        self.copy_button.set_visible(pairing_url.is_some());
        match pairing_url.as_deref() {
            Some(url) => {
                if let Ok(code) = QrCode::new(url.as_bytes()) {
                    let size = code.width();
                    let cells = code
                        .to_colors()
                        .into_iter()
                        .map(|color| color == qrcode::Color::Dark)
                        .collect::<Vec<bool>>();
                    *self.qr_modules.borrow_mut() = Some((size, cells));
                    self.qr_area.set_visible(true);
                    self.qr_empty.set_visible(false);
                    self.qr_area.queue_draw();
                }
            }
            None => {
                *self.qr_modules.borrow_mut() = None;
                self.qr_area.set_visible(false);
                self.qr_empty.set_visible(true);
                self.qr_empty.set_text(if !enabled {
                    "Le QR sera disponible après activation."
                } else if !certificate_ready {
                    "Certificat HTTPS en cours de génération."
                } else if !active {
                    "Serveur local en démarrage."
                } else {
                    "Génère un QR pour appairer un mobile."
                });
            }
        }

        // Mobiles appairés
        let passkeys: Vec<_> = secure
            .as_ref()
            .map(|bridge| {
                bridge
                    .passkeys
                    .iter()
                    .filter(|passkey| passkey.revoked_at.is_none())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        self.passkeys_title
            .set_text(&format!("MOBILES APPAIRÉS ({})", passkeys.len()));
        widgets::clear(&self.passkeys);
        if passkeys.is_empty() {
            self.passkeys
                .append(&widgets::caption("Aucun mobile appairé pour l’instant."));
        }
        for passkey in passkeys {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("card");
            row.set_margin_top(2);
            row.append(&icons::badge("Smartphone", "#9ca3af", 30, false));
            let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
            labels.set_hexpand(true);
            let name = gtk::Label::new(Some(passkey.device_label.as_deref().unwrap_or("Mobile")));
            name.set_xalign(0.0);
            labels.append(&name);
            let mut meta = vec![format!("Appairé le {}", format::day_medium(&passkey.created_at))];
            if let Some(used) = &passkey.last_used_at {
                meta.push(format!("utilisé le {}", format::day_medium(used)));
            }
            labels.append(&widgets::caption(&meta.join(" · ")));
            row.append(&labels);
            let revoke = gtk::Button::with_label("Désappairer");
            revoke.add_css_class("destructive-action");
            let store = self.store.clone();
            let id = passkey.id.clone();
            let label = passkey.device_label.clone().unwrap_or_else(|| "Mobile".to_string());
            revoke.connect_clicked(move |_| {
                let store_for_action = store.clone();
                let id = id.clone();
                store.confirm(
                    "Désappairer ce mobile ?",
                    &format!("« {label} » devra être appairé de nouveau pour accéder à vos données."),
                    "Désappairer",
                    move || match bridge::revoke_passkey(&id) {
                        Ok(_) => {
                            store_for_action.show_toast("Mobile désappairé");
                            store_for_action.reload();
                        }
                        Err(_) => store_for_action.show_error("Le mobile n’a pas pu être désappairé."),
                    },
                );
            });
            row.append(&revoke);
            self.passkeys.append(&row);
        }
    }
}

fn section(title: &str, icon: &str, card: &gtk::Box) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let image = icons::image(icon, 14);
    image.add_css_class("dim-label");
    header.append(&image);
    header.append(&widgets::section_label(title));
    host.append(&header);
    host.append(card);
    host
}

fn url_tile(label: &str, icon: &str) -> (gtk::Box, gtk::Label) {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 2);
    host.add_css_class("card");
    host.set_margin_top(4);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    header.append(&icons::image(icon, 11));
    header.append(&widgets::section_label(label));
    host.append(&header);
    let value = gtk::Label::new(None);
    value.set_xalign(0.0);
    value.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    host.append(&value);
    (host, value)
}

fn step_row(label: &str, value: &str, ready: bool, icon: &str, enabled: bool) -> gtk::Box {
    let host = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    host.add_css_class("card");
    let color = if ready {
        "#10b981"
    } else if enabled {
        "#f97316"
    } else {
        "#9ca3af"
    };
    host.append(&icons::badge(
        if ready { "CheckCircle2" } else { icon },
        color,
        24,
        false,
    ));
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 0);
    labels.set_hexpand(true);
    let title = gtk::Label::new(Some(label));
    title.set_xalign(0.0);
    labels.append(&title);
    labels.append(&widgets::caption(value));
    host.append(&labels);
    host
}

fn navigation_row(icon: &str, title: &str, subtitle: &str) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.append(&icons::badge(icon, "#9ca3af", 36, false));
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let name = gtk::Label::new(Some(title));
    name.set_xalign(0.0);
    name.add_css_class("heading");
    labels.append(&name);
    labels.append(&widgets::caption(subtitle));
    row.append(&labels);
    row.append(&icons::image("ChevronRight", 16));
    let button = gtk::Button::new();
    button.set_child(Some(&row));
    button.add_css_class("flat");
    button
}

/// Export `.dmx` via le sélecteur de fichiers natif.
fn export_backup(store: &Rc<Store>, widget: &impl IsA<gtk::Widget>) {
    let Some(content) = store.read(|engine| engine.export_backup()) else {
        return;
    };
    let today = store.today();
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Exporter les données");
    dialog.set_initial_name(Some(&dmx_core::backup::default_backup_file_name(today)));
    let window = widget.as_ref().root().and_downcast::<gtk::Window>();
    let store = store.clone();
    dialog.save(window.as_ref(), gio::Cancellable::NONE, move |result| {
        if let Ok(file) = result {
            if let Some(path) = file.path() {
                match std::fs::write(path, content) {
                    Ok(()) => store.show_toast("Vos données ont été exportées avec succès."),
                    Err(error) => store.show_error(&format!("Impossible de créer la sauvegarde. ({error})")),
                }
            }
        }
    });
}

/// Import d'une sauvegarde ou d'un relevé bancaire.
fn import_file(store: &Rc<Store>, widget: &impl IsA<gtk::Widget>) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Importer ou restaurer");
    let window = widget.as_ref().root().and_downcast::<gtk::Window>();
    let store = store.clone();
    dialog.open(window.as_ref(), gio::Cancellable::NONE, move |result| {
        let Ok(file) = result else {
            return;
        };
        let Some(path) = file.path() else {
            return;
        };
        let Some(content) = read_text(&path) else {
            store.show_error("Le fichier n'a pas pu être lu.");
            return;
        };
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "import".to_string());
        let extension = path
            .extension()
            .map(|extension| extension.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if extension == "dmx" || extension == "json" {
            store.present(FormRequest::RestoreBackup { content, file_name });
        } else {
            store.present(FormRequest::StatementImport { content, file_name });
        }
    });
}

/// Les relevés bancaires sont souvent encodés en Windows-1252.
fn read_text(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    match String::from_utf8(bytes.clone()) {
        Ok(text) => Some(text.trim_start_matches('\u{feff}').to_string()),
        Err(_) => Some(bytes.iter().map(|byte| *byte as char).collect()),
    }
}
