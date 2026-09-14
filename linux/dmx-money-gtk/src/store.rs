//! État partagé de l'application : moteur, réglages, comptes, filtre global.
//!
//! Aucun calcul métier ici : les pages demandent leurs vues au noyau et se réabonnent
//! aux changements via [`Store::subscribe`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dmx_core::metrics::BalanceSummary;
use dmx_core::models::{Account, Category};
use dmx_core::settings::{AppSettings, SettingsChange};
use dmx_core::{Engine, EngineConfig};

/// Pages de l'application (barre latérale identique aux autres plateformes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Dashboard,
    Accounts,
    Transactions,
    Budget,
    Scheduled,
    Analytics,
    Predictions,
    Categories,
    Settings,
}

impl Route {
    pub fn id(self) -> &'static str {
        match self {
            Route::Dashboard => "dashboard",
            Route::Accounts => "accounts",
            Route::Transactions => "transactions",
            Route::Budget => "budget",
            Route::Scheduled => "scheduled",
            Route::Analytics => "analytics",
            Route::Predictions => "predictions",
            Route::Categories => "categories",
            Route::Settings => "settings",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Route::Dashboard => "Vue d'ensemble",
            Route::Accounts => "Mes Comptes",
            Route::Transactions => "Journal",
            Route::Budget => "Budget",
            Route::Scheduled => "Échéancier",
            Route::Analytics => "Analyses",
            Route::Predictions => "Prédictions",
            Route::Categories => "Catégories",
            Route::Settings => "Paramètres",
        }
    }

    /// Nom d'icône Lucide, installé comme icône symbolique GTK (voir `icons.rs`).
    pub fn icon(self) -> &'static str {
        match self {
            Route::Dashboard => "LayoutDashboard",
            Route::Accounts => "Wallet",
            Route::Transactions => "Receipt",
            Route::Budget => "Calculator",
            Route::Scheduled => "CalendarClock",
            Route::Analytics => "PieChart",
            Route::Predictions => "TrendingUp",
            Route::Categories => "Tag",
            Route::Settings => "Settings",
        }
    }

    pub fn uses_account_filter(self) -> bool {
        matches!(
            self,
            Route::Dashboard
                | Route::Transactions
                | Route::Budget
                | Route::Scheduled
                | Route::Analytics
                | Route::Predictions
        )
    }

    pub fn shows_balances(self) -> bool {
        matches!(self, Route::Dashboard | Route::Transactions)
    }

    pub fn sections() -> [(&'static str, &'static [Route]); 3] {
        [
            ("Général", &[Route::Dashboard, Route::Accounts, Route::Transactions]),
            ("Finances", &[Route::Budget, Route::Scheduled]),
            ("Analyses", &[Route::Analytics, Route::Predictions]),
        ]
    }

    pub const FOOTER: [Route; 2] = [Route::Categories, Route::Settings];
}

/// Formulaire à présenter (boîte de dialogue libadwaita).
#[derive(Debug, Clone, PartialEq)]
pub enum FormRequest {
    Account(Option<String>),
    AccountGroups,
    Category(Option<String>),
    Transaction(Option<String>),
    Budget(Option<String>, Option<String>),
    Scheduled(Option<String>),
    FakeTransaction(Option<String>),
    BudgetSuggestions,
    ScheduledSuggestions,
    RestoreBackup { content: String, file_name: String },
    StatementImport { content: String, file_name: String },
    WhatsNew,
}

/// Demande de confirmation avant une action destructive.
pub struct ConfirmRequest {
    pub title: String,
    pub message: String,
    pub confirm_title: String,
    pub action: Box<dyn Fn()>,
}

/// Abonné aux changements de données (une page, l'en-tête, le tray).
type Listener = Rc<dyn Fn(&Rc<Store>)>;

/// Message affiché par la fenêtre (toast ou boîte d'erreur).
type MessageHook = Option<Box<dyn Fn(&str)>>;

/// Points d'entrée fournis par la fenêtre (dialogues, messages, navigation).
#[derive(Default)]
pub struct UiHooks {
    pub toast: MessageHook,
    pub error: MessageHook,
    pub confirm: Option<Box<dyn Fn(ConfirmRequest)>>,
    pub form: Option<Box<dyn Fn(FormRequest)>>,
    pub route: Option<Box<dyn Fn(Route)>>,
}

struct State {
    settings: AppSettings,
    accounts: Vec<Account>,
    categories: Vec<Category>,
    balances: BalanceSummary,
    selected: Vec<String>,
}

pub struct Store {
    engine: Engine,
    state: RefCell<State>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next_listener: Cell<u64>,
    hooks: RefCell<UiHooks>,
    bridge_enabled: Cell<bool>,
}

impl Store {
    /// Ouvre la base de l'utilisateur (et reprend au besoin celle de DmxMoney 1.x).
    ///
    /// `DMXMONEY_DATA_DIR` permet de travailler sur un dossier de test, sans pont PWA.
    pub fn open() -> Result<Rc<Self>, String> {
        let (config, bridge_enabled) = match std::env::var("DMXMONEY_DATA_DIR") {
            Ok(directory) if !directory.is_empty() => (
                EngineConfig {
                    data_dir: directory.into(),
                    legacy_database_paths: Vec::new(),
                },
                false,
            ),
            _ => (EngineConfig::new(Self::data_directory()), true),
        };
        let engine = Engine::open(config).map_err(|error| error.to_string())?;
        let settings = engine.settings().map_err(|error| error.to_string())?;
        let snapshot = engine.snapshot().map_err(|error| error.to_string())?;
        let store = Rc::new(Self {
            state: RefCell::new(State {
                settings,
                accounts: snapshot.accounts.clone(),
                categories: snapshot.categories.clone(),
                balances: BalanceSummary::default(),
                selected: Vec::new(),
            }),
            engine,
            listeners: RefCell::new(Vec::new()),
            next_listener: Cell::new(0),
            hooks: RefCell::new(UiHooks::default()),
            bridge_enabled: Cell::new(bridge_enabled),
        });
        store.refresh_balances();
        Ok(store)
    }

    pub fn data_directory() -> std::path::PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("com.dmxmoney.app")
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn today(&self) -> chrono::NaiveDate {
        dmx_core::dates::today_local()
    }

    pub fn bridge_available(&self) -> bool {
        self.bridge_enabled.get()
    }

    // --- Accès à l'état ---

    pub fn settings(&self) -> AppSettings {
        self.state.borrow().settings.clone()
    }

    pub fn accounts(&self) -> Vec<Account> {
        self.state.borrow().accounts.clone()
    }

    pub fn categories(&self) -> Vec<Category> {
        self.state.borrow().categories.clone()
    }

    /// Catégories choisissables (sans « Virement », réservée aux virements).
    pub fn selectable_categories(&self) -> Vec<Category> {
        self.state
            .borrow()
            .categories
            .iter()
            .filter(|category| category.id != dmx_core::models::TRANSFER_CATEGORY_ID)
            .cloned()
            .collect()
    }

    pub fn balances(&self) -> BalanceSummary {
        self.state.borrow().balances
    }

    pub fn selected_accounts(&self) -> Vec<String> {
        self.state.borrow().selected.clone()
    }

    pub fn account_name(&self, id: &str) -> Option<String> {
        self.state
            .borrow()
            .accounts
            .iter()
            .find(|account| account.id == id)
            .map(|account| account.name.clone())
    }

    pub fn category_name(&self, id: &str) -> Option<String> {
        self.state
            .borrow()
            .categories
            .iter()
            .find(|category| category.id == id)
            .map(|category| category.name.clone())
    }

    // --- Abonnements ---

    pub fn subscribe(self: &Rc<Self>, listener: impl Fn(&Rc<Store>) + 'static) -> u64 {
        let id = self.next_listener.get() + 1;
        self.next_listener.set(id);
        self.listeners.borrow_mut().push((id, Rc::new(listener)));
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(candidate, _)| *candidate != id);
    }

    fn notify(self: &Rc<Self>) {
        let listeners: Vec<_> = self
            .listeners
            .borrow()
            .iter()
            .map(|(_, listener)| listener.clone())
            .collect();
        for listener in listeners {
            listener(self);
        }
    }

    pub fn set_hooks(&self, hooks: UiHooks) {
        *self.hooks.borrow_mut() = hooks;
    }

    // --- Rechargement et écritures ---

    pub fn reload(self: &Rc<Self>) {
        let settings = self.engine.settings();
        let snapshot = self.engine.snapshot();
        if let (Ok(settings), Ok(snapshot)) = (settings, snapshot) {
            let mut state = self.state.borrow_mut();
            state.settings = settings;
            state.accounts = snapshot.accounts.clone();
            state.categories = snapshot.categories.clone();
            let known: Vec<String> = state.accounts.iter().map(|account| account.id.clone()).collect();
            state.selected.retain(|id| known.contains(id));
        }
        self.refresh_balances();
        self.notify();
    }

    fn refresh_balances(&self) {
        let selected = self.state.borrow().selected.clone();
        if let Ok(view) = self.engine.dashboard(&selected, dmx_core::dates::today_local()) {
            self.state.borrow_mut().balances = view.balances;
        }
    }

    /// Écriture ; renvoie le message d'erreur à afficher, ou `None`.
    pub fn attempt<T>(self: &Rc<Self>, work: impl FnOnce(&Engine) -> dmx_core::CoreResult<T>) -> Option<String> {
        match work(&self.engine) {
            Ok(_) => {
                self.reload();
                None
            }
            Err(error) => Some(error.to_string()),
        }
    }

    /// Écriture avec message de réussite ; l'erreur éventuelle est affichée.
    pub fn run<T>(self: &Rc<Self>, success: Option<&str>, work: impl FnOnce(&Engine) -> dmx_core::CoreResult<T>) {
        match self.attempt(work) {
            Some(message) => self.show_error(&message),
            None => {
                if let Some(message) = success {
                    self.show_toast(message);
                }
            }
        }
    }

    pub fn apply(self: &Rc<Self>, change: SettingsChange) {
        self.run(None, |engine| engine.apply_settings_change(change));
    }

    /// Lecture ; l'erreur éventuelle est affichée et `None` renvoyé.
    pub fn read<T>(self: &Rc<Self>, work: impl FnOnce(&Engine) -> dmx_core::CoreResult<T>) -> Option<T> {
        match work(&self.engine) {
            Ok(value) => Some(value),
            Err(error) => {
                self.show_error(&error.to_string());
                None
            }
        }
    }

    // --- Filtre de comptes ---

    pub fn set_selected_accounts(self: &Rc<Self>, accounts: Vec<String>) {
        {
            let mut state = self.state.borrow_mut();
            if state.selected == accounts {
                return;
            }
            state.selected = accounts;
        }
        self.refresh_balances();
        self.notify();
    }

    pub fn toggle_account(self: &Rc<Self>, id: &str) {
        let mut selected = self.selected_accounts();
        if let Some(index) = selected.iter().position(|candidate| candidate == id) {
            selected.remove(index);
        } else {
            selected.push(id.to_string());
        }
        if selected.len() == self.state.borrow().accounts.len() {
            selected.clear();
        }
        self.set_selected_accounts(selected);
    }

    pub fn clear_account_filter(self: &Rc<Self>) {
        self.set_selected_accounts(Vec::new());
    }

    // --- Interface ---

    pub fn show_toast(&self, message: &str) {
        if let Some(toast) = self.hooks.borrow().toast.as_ref() {
            toast(message);
        }
    }

    pub fn show_error(&self, message: &str) {
        if let Some(error) = self.hooks.borrow().error.as_ref() {
            error(message);
        } else {
            log::error!("{message}");
        }
    }

    pub fn confirm(&self, title: &str, message: &str, confirm_title: &str, action: impl Fn() + 'static) {
        if let Some(confirm) = self.hooks.borrow().confirm.as_ref() {
            confirm(ConfirmRequest {
                title: title.to_string(),
                message: message.to_string(),
                confirm_title: confirm_title.to_string(),
                action: Box::new(action),
            });
        }
    }

    pub fn present(&self, request: FormRequest) {
        if let Some(form) = self.hooks.borrow().form.as_ref() {
            form(request);
        }
    }

    pub fn navigate(&self, route: Route) {
        if let Some(navigate) = self.hooks.borrow().route.as_ref() {
            navigate(route);
        }
    }

    /// Crée les opérations des échéances arrivées à terme.
    pub fn process_due(self: &Rc<Self>) {
        let today = self.today();
        if let Some(result) = self.read(|engine| engine.process_due_scheduled(today)) {
            if result.created_transactions > 0 {
                self.show_toast(&format!(
                    "{} échéance{} ajoutée{} au journal",
                    result.created_transactions,
                    if result.created_transactions > 1 { "s" } else { "" },
                    if result.created_transactions > 1 { "s" } else { "" }
                ));
            }
            self.reload();
        }
    }
}
