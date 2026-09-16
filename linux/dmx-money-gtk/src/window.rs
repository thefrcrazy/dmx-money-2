//! Fenêtre principale : barre latérale, en-tête (filtre de comptes et soldes), pages, messages.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::pages::Pages;
use crate::store::{FormRequest, Route, Store, UiHooks};
use crate::{format, forms, icons, tray, widgets};

pub fn present(application: &adw::Application, store: Rc<Store>) {
    let window = adw::ApplicationWindow::new(application);
    window.set_title(Some("DmxMoney"));
    window.set_default_size(1320, 860);
    window.set_size_request(1000, 640);

    let toasts = adw::ToastOverlay::new();
    let stack = adw::ViewStack::new();
    let pages = Rc::new(Pages::new(&store, &stack));
    let route = Rc::new(Cell::new(Route::Dashboard));

    // --- En-tête : filtre global de comptes et soldes ---
    let filter_label = gtk::Label::new(Some("Tous les comptes"));
    filter_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let filter_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    filter_content.append(&icons::image("Wallet", 14));
    filter_content.append(&filter_label);
    filter_content.append(&gtk::Image::from_icon_name("pan-down-symbolic"));
    let filter_button = gtk::MenuButton::new();
    filter_button.set_child(Some(&filter_content));
    filter_button.set_tooltip_text(Some("Filtrer par compte"));

    let filter_list = gtk::ListBox::new();
    filter_list.set_selection_mode(gtk::SelectionMode::None);
    filter_list.add_css_class("boxed-list");
    let clear_button = gtk::Button::with_label("Tous les comptes");
    let popover_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    popover_content.set_margin_top(8);
    popover_content.set_margin_bottom(8);
    popover_content.set_margin_start(8);
    popover_content.set_margin_end(8);
    popover_content.set_size_request(280, -1);
    let filter_scroll = gtk::ScrolledWindow::new();
    filter_scroll.set_max_content_height(340);
    filter_scroll.set_propagate_natural_height(true);
    filter_scroll.set_child(Some(&filter_list));
    popover_content.append(&filter_scroll);
    popover_content.append(&clear_button);
    let filter_popover = gtk::Popover::new();
    filter_popover.set_child(Some(&popover_content));
    filter_button.set_popover(Some(&filter_popover));

    let checked_value = widgets::amount("", Some("dmx-income"));
    checked_value.add_css_class("title-4");
    let current_value = widgets::amount("", None);
    current_value.add_css_class("title-3");
    let checked_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    checked_box.append(&widgets::section_label("Pointé"));
    checked_box.append(&checked_value);
    let current_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    current_box.append(&widgets::section_label("Actuel"));
    current_box.append(&current_value);
    let balances = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    balances.append(&checked_box);
    balances.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    balances.append(&current_box);

    let header = adw::HeaderBar::new();
    header.pack_start(&filter_button);
    header.pack_end(&balances);

    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&header);
    content_view.set_content(Some(&stack));
    toasts.set_child(Some(&content_view));

    // --- Barre latérale ---
    let sidebar_list = gtk::ListBox::new();
    sidebar_list.set_selection_mode(gtk::SelectionMode::Single);
    sidebar_list.add_css_class("navigation-sidebar");
    let mut rows: Vec<(Route, gtk::ListBoxRow)> = Vec::new();
    for (title, routes) in Route::sections() {
        let header_row = gtk::ListBoxRow::new();
        header_row.set_selectable(false);
        header_row.set_activatable(false);
        let label = widgets::section_label(title);
        label.set_margin_top(10);
        label.set_margin_start(6);
        header_row.set_child(Some(&label));
        sidebar_list.append(&header_row);
        for candidate in routes {
            let row = sidebar_row(*candidate);
            sidebar_list.append(&row);
            rows.push((*candidate, row));
        }
    }

    let footer = gtk::Box::new(gtk::Orientation::Vertical, 2);
    footer.set_margin_top(8);
    footer.set_margin_bottom(10);
    footer.set_margin_start(6);
    footer.set_margin_end(6);
    let mut footer_buttons: Vec<(Route, gtk::Button)> = Vec::new();
    for candidate in Route::FOOTER {
        let button = widgets::action_button(candidate.title(), Some(candidate.icon()), false);
        button.add_css_class("flat");
        button.set_halign(gtk::Align::Fill);
        let store_for_button = store.clone();
        button.connect_clicked(move |_| store_for_button.navigate(candidate));
        footer.append(&button);
        footer_buttons.push((candidate, button));
    }
    let check_updates = widgets::action_button("Vérifier les mises à jour", Some("RefreshCw"), false);
    check_updates.add_css_class("flat");
    let store_for_updates = store.clone();
    check_updates.connect_clicked(move |_| {
        let _ = gtk::gio::AppInfo::launch_default_for_uri(
            "https://github.com/thefrcrazy/dmx-money-2/releases",
            Option::<&gtk::gio::AppLaunchContext>::None,
        );
        store_for_updates.show_toast("Ouverture des versions de DmxMoney…");
    });
    footer.append(&check_updates);
    let quit = widgets::action_button("Quitter", Some("Power"), false);
    quit.add_css_class("flat");
    quit.add_css_class("dmx-expense");
    let application_for_quit = application.clone();
    quit.connect_clicked(move |_| application_for_quit.quit());
    footer.append(&quit);
    let version = widgets::section_label(&format!("DmxMoney • v{}", env!("CARGO_PKG_VERSION")));
    version.set_halign(gtk::Align::Center);
    version.set_margin_top(8);
    footer.append(&version);

    let sidebar_scroll = gtk::ScrolledWindow::new();
    sidebar_scroll.set_vexpand(true);
    sidebar_scroll.set_child(Some(&sidebar_list));
    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_box.append(&sidebar_scroll);
    sidebar_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    sidebar_box.append(&footer);

    let sidebar_view = adw::ToolbarView::new();
    sidebar_view.add_top_bar(&adw::HeaderBar::new());
    sidebar_view.set_content(Some(&sidebar_box));

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&adw::NavigationPage::new(&sidebar_view, "DmxMoney")));
    split.set_content(Some(&adw::NavigationPage::new(&toasts, "DmxMoney")));
    split.set_min_sidebar_width(220.0);
    split.set_max_sidebar_width(280.0);
    window.set_content(Some(&split));

    // --- Rafraîchissement de l'en-tête et du filtre ---
    let filter_rows: Rc<RefCell<Vec<gtk::CheckButton>>> = Rc::new(RefCell::new(Vec::new()));
    let syncing = Rc::new(Cell::new(false));

    let refresh_header = {
        let store = store.clone();
        let filter_label = filter_label.clone();
        let checked_value = checked_value.clone();
        let current_value = current_value.clone();
        let filter_button = filter_button.clone();
        let balances = balances.clone();
        let route = route.clone();
        move || {
            let selected = store.selected_accounts();
            filter_label.set_text(&match selected.len() {
                0 => "Tous les comptes".to_string(),
                1 => store
                    .account_name(&selected[0])
                    .unwrap_or_else(|| "1 compte".to_string()),
                count => format!("{count} comptes"),
            });
            let summary = store.balances();
            checked_value.set_text(&format::money(summary.checked_balance));
            current_value.set_text(&format::money(summary.current_balance));
            filter_button.set_visible(route.get().uses_account_filter());
            balances.set_visible(route.get().shows_balances());
        }
    };

    let rebuild_filter = {
        let store = store.clone();
        let filter_list = filter_list.clone();
        let filter_rows = filter_rows.clone();
        let syncing = syncing.clone();
        move || {
            syncing.set(true);
            widgets::clear(&filter_list);
            filter_rows.borrow_mut().clear();
            let selected = store.selected_accounts();
            for account in store.accounts() {
                let check = gtk::CheckButton::new();
                check.set_active(selected.contains(&account.id));
                let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row_box.append(&check);
                row_box.append(&icons::colored_image(&account.icon, &account.color, 14));
                let label = gtk::Label::new(Some(&account.name));
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                row_box.append(&label);
                let row = gtk::ListBoxRow::new();
                row.set_child(Some(&row_box));
                row.set_activatable(false);
                filter_list.append(&row);

                let store_for_check = store.clone();
                let id = account.id.clone();
                let syncing_for_check = syncing.clone();
                check.connect_toggled(move |_| {
                    if !syncing_for_check.get() {
                        store_for_check.toggle_account(&id);
                    }
                });
                filter_rows.borrow_mut().push(check);
            }
            syncing.set(false);
        }
    };

    {
        let store = store.clone();
        clear_button.connect_clicked(move |_| store.clear_account_filter());
    }

    // --- Navigation ---
    let select_route = {
        let stack = stack.clone();
        let rows = rows.clone();
        let sidebar_list = sidebar_list.clone();
        let footer_buttons = footer_buttons.clone();
        let route = route.clone();
        let pages = pages.clone();
        let syncing = syncing.clone();
        move |target: Route| {
            route.set(target);
            stack.set_visible_child_name(target.id());
            syncing.set(true);
            match rows.iter().find(|(candidate, _)| *candidate == target) {
                Some((_, row)) => sidebar_list.select_row(Some(row)),
                // Catégories et Paramètres vivent dans le pied de page : la barre latérale
                // ne doit pas garder la page précédente en surbrillance.
                None => sidebar_list.unselect_all(),
            }
            syncing.set(false);
            for (candidate, button) in &footer_buttons {
                if *candidate == target {
                    button.add_css_class("dmx-active");
                } else {
                    button.remove_css_class("dmx-active");
                }
            }
            pages.refresh(target);
        }
    };

    {
        let rows = rows.clone();
        let select_route = select_route.clone();
        let syncing = syncing.clone();
        sidebar_list.connect_row_selected(move |_, row| {
            if syncing.get() {
                return;
            }
            if let Some(row) = row {
                if let Some((candidate, _)) = rows.iter().find(|(_, existing)| existing == row) {
                    select_route(*candidate);
                }
            }
        });
    }

    // --- Hooks vers l'interface ---
    let hooks = UiHooks {
        toast: Some(Box::new({
            let toasts = toasts.clone();
            move |message: &str| toasts.add_toast(adw::Toast::new(message))
        })),
        error: Some(Box::new({
            let window = window.clone();
            move |message: &str| {
                let dialog = adw::AlertDialog::new(Some("DmxMoney"), Some(message));
                dialog.add_response("ok", "OK");
                dialog.present(Some(&window));
            }
        })),
        confirm: Some(Box::new({
            let window = window.clone();
            move |request| {
                let dialog = adw::AlertDialog::new(Some(&request.title), Some(&request.message));
                dialog.add_response("cancel", "Annuler");
                dialog.add_response("confirm", &request.confirm_title);
                dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                let action = request.action;
                dialog.connect_response(None, move |_, response| {
                    if response == "confirm" {
                        action();
                    }
                });
                dialog.present(Some(&window));
            }
        })),
        form: Some(Box::new({
            let window = window.clone();
            let store = store.clone();
            move |request: FormRequest| forms::present(&store, &window, request)
        })),
        route: Some(Box::new({
            let select_route = select_route.clone();
            move |target: Route| select_route(target)
        })),
    };
    store.set_hooks(hooks);

    // --- Abonnement : en-tête, filtre, page visible ---
    {
        let refresh_header = refresh_header.clone();
        let rebuild_filter = rebuild_filter.clone();
        let pages = pages.clone();
        let route = route.clone();
        store.subscribe(move |_| {
            rebuild_filter();
            refresh_header();
            pages.refresh(route.get());
        });
    }

    rebuild_filter();
    refresh_header();
    select_route(Route::Dashboard);

    apply_theme(&store);
    window.present();

    // Mode capture : rend les pages et les formulaires en PNG, puis quitte.
    if let Some(directory) = crate::snapshot::directory() {
        crate::snapshot::run(&store, &window, directory);
        return;
    }

    store.process_due();
    start_bridge(&store);
    present_whats_new(&store);
    tray::start(&store, &window);
}

fn sidebar_row(route: Route) -> gtk::ListBoxRow {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(6);
    content.set_margin_bottom(6);
    content.set_margin_start(6);
    content.append(&icons::image(route.icon(), 16));
    content.append(&gtk::Label::new(Some(route.title())));
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    row
}

fn apply_theme(store: &Rc<Store>) {
    use dmx_core::models::Theme;
    let manager = adw::StyleManager::default();
    manager.set_color_scheme(match store.settings().theme {
        Theme::Light => adw::ColorScheme::ForceLight,
        Theme::Dark => adw::ColorScheme::ForceDark,
        Theme::System => adw::ColorScheme::Default,
    });
    let store_for_theme = store.clone();
    store.subscribe(move |_| {
        let manager = adw::StyleManager::default();
        manager.set_color_scheme(match store_for_theme.settings().theme {
            Theme::Light => adw::ColorScheme::ForceLight,
            Theme::Dark => adw::ColorScheme::ForceDark,
            Theme::System => adw::ColorScheme::Default,
        });
    });
}

/// Démarre le pont PWA (il ne sert la PWA que si l'utilisateur l'a activé).
fn start_bridge(store: &Rc<Store>) {
    crate::bridge::start(store, pwa_assets_directory());
}

fn pwa_assets_directory() -> Option<std::path::PathBuf> {
    let candidates = [
        std::path::PathBuf::from("/app/share/dmx-money/pwa"),
        std::path::PathBuf::from("/usr/share/dmx-money/pwa"),
    ];
    candidates.into_iter().find(|path| path.exists())
}

fn present_whats_new(store: &Rc<Store>) {
    let version = env!("CARGO_PKG_VERSION");
    let settings = store.settings();
    if settings.last_seen_version.as_deref() == Some(version) {
        return;
    }
    if settings.last_seen_version.is_none() && store.accounts().is_empty() {
        store.apply(dmx_core::settings::SettingsChange::LastSeenVersion(version.to_string()));
        return;
    }
    store.present(FormRequest::WhatsNew);
}

/// Utilitaire partagé par les pages : construit une ligne cliquable.
pub fn clickable(child: &impl IsA<gtk::Widget>, on_click: impl Fn() + 'static) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(child));
    button.add_css_class("flat");
    button.connect_clicked(move |_| on_click());
    button
}

/// Petite aide de mise en page pour les grilles de cartes (trois colonnes).
pub fn card_row(children: Vec<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    row.set_homogeneous(true);
    for child in children {
        row.append(&child);
    }
    row
}

/// Évite un avertissement quand `glib` n'est utilisé que par les macros.
pub fn _unused() {
    let _ = glib::user_data_dir();
}
