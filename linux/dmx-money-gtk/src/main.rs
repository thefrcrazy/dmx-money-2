//! DmxMoney pour Linux : GTK4 et libadwaita, sur le même noyau Rust que macOS et Windows.

mod bridge;
mod charts;
mod format;
mod forms;
mod icon_names;
mod icons;
mod import_file;
mod pages;
mod snapshot;
mod store;
mod tray;
mod widgets;
mod window;

use adw::prelude::*;
use gtk::glib;

const APP_ID: &str = "com.dmxmoney.app";

fn main() -> glib::ExitCode {
    env_logger::init();
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gtk::gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let active_store = std::rc::Rc::new(std::cell::RefCell::new(None::<std::rc::Rc<store::Store>>));
    application.connect_startup(|_| {
        icons::install_search_paths();
        icons::load_stylesheet();
        icons::preload_palette();
    });
    application.connect_shutdown(|_| bridge::shutdown());
    let store_for_activate = active_store.clone();
    application.connect_activate(move |application| {
        if let Some(existing) = application.active_window() {
            existing.present();
            return;
        }
        match store::Store::open() {
            Ok(store) => {
                *store_for_activate.borrow_mut() = Some(store.clone());
                window::present(application, store);
            }
            Err(error) => {
                let dialog = adw::AlertDialog::new(Some("Impossible d'ouvrir la base DmxMoney"), Some(&error));
                dialog.add_response("close", "Fermer");
                let window = gtk::ApplicationWindow::new(application);
                let application_for_quit = application.clone();
                dialog.connect_response(None, move |_, _| application_for_quit.quit());
                window.present();
                dialog.present(Some(&window));
            }
        }
    });
    application.connect_open(move |application, files, _| {
        application.activate();
        let Some(store) = active_store.borrow().clone() else {
            return;
        };
        if files.len() != 1 {
            store.show_error("Ouvrez un seul fichier à la fois.");
            return;
        }
        import_file::open(&store, files[0].clone());
    });
    application.run()
}
