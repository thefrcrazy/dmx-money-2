//! Icône de zone de notification (SNI). Disponible sous Linux ; ailleurs, sans effet.

use std::rc::Rc;

use crate::store::Store;

#[cfg(target_os = "linux")]
pub fn start(store: &Rc<Store>, window: &adw::ApplicationWindow) {
    use adw::prelude::*;

    use crate::format;
    use crate::store::Route;
    use gtk::glib;

    struct Tray {
        accounts: Vec<(String, String, f64)>,
        total: f64,
        sender: async_channel::Sender<TrayAction>,
    }

    enum TrayAction {
        Open,
        Account(String),
        Navigate(Route),
        NewTransaction,
        Sync,
        CheckUpdates,
        Quit,
    }

    impl ksni::Tray for Tray {
        fn icon_name(&self) -> String {
            "com.dmxmoney.app".into()
        }

        fn title(&self) -> String {
            "DmxMoney".into()
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::{MenuItem, StandardItem};
            let mut items: Vec<MenuItem<Self>> = Vec::new();
            for (id, name, balance) in self.accounts.iter().take(8) {
                let id = id.clone();
                items.push(
                    StandardItem {
                        label: format!("{name} — {}", format::money(*balance)),
                        activate: Box::new(move |tray: &mut Self| {
                            let _ = tray.sender.send_blocking(TrayAction::Account(id.clone()));
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            items.push(
                StandardItem {
                    label: format!("Total — {}", format::money(self.total)),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Ouvrir DmxMoney".into(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send_blocking(TrayAction::Open);
                    }),
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: "Nouvelle transaction…".into(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send_blocking(TrayAction::NewTransaction);
                    }),
                    ..Default::default()
                }
                .into(),
            );
            for route in [
                Route::Dashboard,
                Route::Transactions,
                Route::Budget,
                Route::Scheduled,
                Route::Predictions,
            ] {
                items.push(
                    StandardItem {
                        label: route.title().into(),
                        activate: Box::new(move |tray: &mut Self| {
                            let _ = tray.sender.send_blocking(TrayAction::Navigate(route));
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Synchroniser".into(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send_blocking(TrayAction::Sync);
                    }),
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: "Rechercher les mises à jour…".into(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send_blocking(TrayAction::CheckUpdates);
                    }),
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: "Quitter DmxMoney".into(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send_blocking(TrayAction::Quit);
                    }),
                    ..Default::default()
                }
                .into(),
            );
            items
        }
    }

    let (sender, receiver) = async_channel::unbounded::<TrayAction>();
    let summary = store.read(|engine| engine.tray_summary());
    let tray = Tray {
        accounts: summary
            .as_ref()
            .map(|summary| {
                summary
                    .accounts
                    .iter()
                    .map(|account| (account.account_id.clone(), account.name.clone(), account.balance))
                    .collect()
            })
            .unwrap_or_default(),
        total: summary.map(|summary| summary.total).unwrap_or_default(),
        sender,
    };
    let service = ksni::TrayService::new(tray);
    let handle = service.handle();
    service.spawn();

    let store_for_actions = store.clone();
    let window_for_actions = window.clone();
    glib::spawn_future_local(async move {
        while let Ok(action) = receiver.recv().await {
            match action {
                TrayAction::Open => window_for_actions.present(),
                TrayAction::Account(id) => {
                    store_for_actions.set_selected_accounts(vec![id]);
                    store_for_actions.navigate(Route::Transactions);
                    window_for_actions.present();
                }
                TrayAction::Navigate(route) => store_for_actions.navigate(route),
                TrayAction::NewTransaction => store_for_actions.present(crate::store::FormRequest::Transaction(None)),
                TrayAction::Sync => store_for_actions.reload(),
                TrayAction::CheckUpdates => {
                    let _ = gtk::gio::AppInfo::launch_default_for_uri(
                        "https://github.com/thefrcrazy/dmx-money-2/releases",
                        Option::<&gtk::gio::AppLaunchContext>::None,
                    );
                    store_for_actions.show_toast("Ouverture des versions de DmxMoney…");
                }
                TrayAction::Quit => std::process::exit(0),
            }
        }
    });

    // Le menu suit les soldes.
    let store_for_tray = store.clone();
    store.subscribe(move |_| {
        if let Some(summary) = store_for_tray.read(|engine| engine.tray_summary()) {
            handle.update(|tray: &mut Tray| {
                tray.accounts = summary
                    .accounts
                    .iter()
                    .map(|account| (account.account_id.clone(), account.name.clone(), account.balance))
                    .collect();
                tray.total = summary.total;
            });
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn start(_store: &Rc<Store>, _window: &adw::ApplicationWindow) {
    // L'icône de zone de notification n'existe que sous Linux (protocole SNI).
}
