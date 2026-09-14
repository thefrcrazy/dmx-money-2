//! DmxMoney pour Linux : GTK4 et libadwaita, sur le même noyau Rust que macOS et Windows.

mod bridge;
mod charts;
mod format;
mod forms;
mod icons;
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
    let application = adw::Application::builder().application_id(APP_ID).build();
    application.connect_startup(|_| {
        icons::install_search_paths();
        icons::load_stylesheet();
        icons::preload_palette();
    });
    application.connect_shutdown(|_| bridge::shutdown());
    application.connect_activate(|application| {
        if let Some(existing) = application.active_window() {
            existing.present();
            return;
        }
        match store::Store::open() {
            Ok(store) => window::present(application, store),
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
    application.run()
}
