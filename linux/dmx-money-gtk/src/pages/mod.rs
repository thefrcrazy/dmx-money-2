//! Pages de l'application. Chaque page se reconstruit depuis les vues calculées par le noyau.

pub mod accounts;
pub mod analytics;
pub mod budget;
pub mod categories;
pub mod dashboard;
pub mod journal;
pub mod predictions;
pub mod scheduled;
pub mod settings;

use std::rc::Rc;

use crate::store::{Route, Store};

pub struct Pages {
    dashboard: dashboard::Page,
    accounts: accounts::Page,
    journal: journal::Page,
    budget: budget::Page,
    scheduled: scheduled::Page,
    analytics: analytics::Page,
    predictions: predictions::Page,
    categories: categories::Page,
    settings: settings::Page,
}

impl Pages {
    pub fn new(store: &Rc<Store>, stack: &adw::ViewStack) -> Self {
        let pages = Self {
            dashboard: dashboard::Page::new(store),
            accounts: accounts::Page::new(store),
            journal: journal::Page::new(store),
            budget: budget::Page::new(store),
            scheduled: scheduled::Page::new(store),
            analytics: analytics::Page::new(store),
            predictions: predictions::Page::new(store),
            categories: categories::Page::new(store),
            settings: settings::Page::new(store),
        };
        let entries: [(Route, gtk::Widget); 9] = [
            (Route::Dashboard, pages.dashboard.root()),
            (Route::Accounts, pages.accounts.root()),
            (Route::Transactions, pages.journal.root()),
            (Route::Budget, pages.budget.root()),
            (Route::Scheduled, pages.scheduled.root()),
            (Route::Analytics, pages.analytics.root()),
            (Route::Predictions, pages.predictions.root()),
            (Route::Categories, pages.categories.root()),
            (Route::Settings, pages.settings.root()),
        ];
        for (route, widget) in entries {
            stack.add_titled(&widget, Some(route.id()), route.title());
        }
        pages
    }

    /// Recalcule la page visible (les autres se mettront à jour en devenant visibles).
    pub fn refresh(&self, route: Route) {
        match route {
            Route::Dashboard => self.dashboard.refresh(),
            Route::Accounts => self.accounts.refresh(),
            Route::Transactions => self.journal.refresh(),
            Route::Budget => self.budget.refresh(),
            Route::Scheduled => self.scheduled.refresh(),
            Route::Analytics => self.analytics.refresh(),
            Route::Predictions => self.predictions.refresh(),
            Route::Categories => self.categories.refresh(),
            Route::Settings => self.settings.refresh(),
        }
    }
}
