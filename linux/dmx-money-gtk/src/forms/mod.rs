//! Formulaires présentés en `AdwDialog`. Les brouillons viennent du noyau et y retournent
//! sans transformation : l'interface ne calcule aucun montant.

pub mod controls;
mod data;
mod entities;

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! { static OPEN_FORMS: RefCell<Vec<Rc<FormShell>>> = const { RefCell::new(Vec::new()) }; }

use adw::prelude::*;

use crate::store::{FormRequest, Store};
use crate::widgets;

pub fn present(store: &Rc<Store>, parent: &impl IsA<gtk::Widget>, request: FormRequest) {
    match request {
        FormRequest::Account(id) => entities::account(store, parent, id),
        FormRequest::AccountGroups => entities::account_groups(store, parent),
        FormRequest::Category(id) => entities::category(store, parent, id),
        FormRequest::Transaction(id) => entities::transaction(store, parent, id),
        FormRequest::Budget(id, category_id) => entities::budget(store, parent, id, category_id),
        FormRequest::Scheduled(id) => entities::scheduled(store, parent, id),
        FormRequest::FakeTransaction(id) => entities::fake_transaction(store, parent, id),
        FormRequest::BudgetSuggestions => entities::budget_suggestions(store, parent),
        FormRequest::ScheduledSuggestions => entities::scheduled_suggestions(store, parent),
        FormRequest::RestoreBackup { content, file_name } => data::restore_backup(store, parent, content, file_name),
        FormRequest::StatementImport { content, file_name } => {
            data::statement_import(store, parent, content, file_name)
        }
        FormRequest::WhatsNew => data::whats_new(store, parent),
    }
}

/// Coquille commune : en-tête, corps défilant, message d'erreur et boutons.
pub struct FormShell {
    dialog: adw::Dialog,
    body: gtk::Box,
    error: gtk::Label,
    submit: gtk::Button,
    submit_label: gtk::Label,
    header_extra: gtk::Box,
    submit_handler: RefCell<Option<gtk::glib::SignalHandlerId>>,
    held_controls: RefCell<Vec<Rc<dyn Any>>>,
    cleanup: RefCell<Vec<Box<dyn Fn()>>>,
    closed: Cell<bool>,
    busy: Cell<bool>,
    progress: gtk::Spinner,
}

impl FormShell {
    pub fn new(title: &str, submit_title: Option<&str>, width: i32) -> Rc<Self> {
        let dialog = adw::Dialog::new();
        dialog.set_title(title);
        dialog.set_content_width(width);
        dialog.set_follows_content_size(true);

        let header = adw::HeaderBar::new();
        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
        body.set_margin_top(18);
        body.set_margin_bottom(18);
        body.set_margin_start(20);
        body.set_margin_end(20);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(620);
        scroll.set_child(Some(&body));
        view.set_content(Some(&scroll));

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(10);
        actions.set_margin_bottom(14);
        actions.set_margin_start(16);
        actions.set_margin_end(16);
        let error = widgets::caption("");
        error.add_css_class("dmx-expense");
        error.set_hexpand(true);
        error.set_visible(false);
        actions.append(&error);
        let header_extra = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.append(&header_extra);
        let progress = gtk::Spinner::new();
        progress.set_visible(false);
        progress.set_tooltip_text(Some("Traitement en cours"));
        actions.append(&progress);
        let cancel = gtk::Button::with_label(if submit_title.is_some() { "Annuler" } else { "Fermer" });
        actions.append(&cancel);
        let submit_label = gtk::Label::new(Some(submit_title.unwrap_or("Enregistrer")));
        let submit = gtk::Button::new();
        submit.set_child(Some(&submit_label));
        submit.add_css_class("suggested-action");
        submit.set_visible(submit_title.is_some());
        actions.append(&submit);
        view.add_bottom_bar(&actions);

        dialog.set_child(Some(&view));
        let dialog_for_cancel = dialog.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(dialog) = dialog_for_cancel.upgrade() {
                dialog.close();
            }
        });

        let shell = Rc::new(Self {
            dialog,
            body,
            error,
            submit,
            submit_label,
            header_extra,
            submit_handler: RefCell::new(None),
            held_controls: RefCell::new(Vec::new()),
            cleanup: RefCell::new(Vec::new()),
            closed: Cell::new(false),
            busy: Cell::new(false),
            progress,
        });
        let weak = Rc::downgrade(&shell);
        shell.dialog.connect_closed(move |_| {
            if let Some(shell) = weak.upgrade() {
                shell.closed.set(true);
                if let Some(handler) = shell.submit_handler.borrow_mut().take() {
                    shell.submit.disconnect(handler);
                }
                for cleanup in shell.cleanup.borrow_mut().drain(..) {
                    cleanup();
                }
                shell.held_controls.borrow_mut().clear();
                widgets::clear(&shell.body);
                widgets::clear(&shell.header_extra);
                shell.dialog.set_child(gtk::Widget::NONE);
                OPEN_FORMS.with(|forms| forms.borrow_mut().retain(|candidate| !Rc::ptr_eq(candidate, &shell)));
            }
        });
        shell
    }

    pub fn body(&self) -> &gtk::Box {
        &self.body
    }

    /// Boutons additionnels (retour de l'assistant, actions secondaires).
    pub fn extra_actions(&self) -> &gtk::Box {
        &self.header_extra
    }

    pub fn set_error(&self, message: Option<&str>) {
        if self.is_closed() {
            return;
        }
        self.error.set_text(message.unwrap_or(""));
        self.error.set_visible(message.is_some());
    }

    pub fn set_submit_title(&self, title: &str) {
        self.submit_label.set_text(title);
    }

    pub fn on_submit(&self, action: impl Fn() + 'static) {
        if let Some(handler) = self.submit_handler.borrow_mut().take() {
            self.submit.disconnect(handler);
        }
        *self.submit_handler.borrow_mut() = Some(self.submit.connect_clicked(move |_| action()));
    }

    /// Reconstruit le contenu après chaque écriture, et se désabonne à la fermeture :
    /// sans cela, l'abonnement survivrait au dialogue et manipulerait des widgets détruits.
    pub fn follow(&self, store: &Rc<Store>, refresh: Rc<dyn Fn()>) {
        let id = store.subscribe(move |_| refresh());
        let store = store.clone();
        self.dialog.connect_closed(move |_| store.unsubscribe(id));
    }

    pub fn present(self: &Rc<Self>, parent: &impl IsA<gtk::Widget>) {
        OPEN_FORMS.with(|forms| forms.borrow_mut().push(self.clone()));
        self.dialog.present(Some(parent.as_ref()));
    }

    pub fn hold<T: Any>(&self, control: Rc<T>) {
        self.held_controls.borrow_mut().push(control);
    }
    pub fn release_controls(&self) {
        self.held_controls.borrow_mut().clear();
    }
    pub fn on_close(&self, cleanup: impl Fn() + 'static) {
        self.cleanup.borrow_mut().push(Box::new(cleanup));
    }
    pub fn is_closed(&self) -> bool {
        self.closed.get()
    }
    pub fn is_busy(&self) -> bool {
        self.busy.get()
    }
    pub fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        self.progress.set_spinning(busy);
        self.progress.set_visible(busy);
        self.submit.set_sensitive(!busy);
        self.body.set_sensitive(!busy);
        self.header_extra.set_sensitive(!busy);
        self.dialog.set_can_close(!busy);
    }

    pub fn close(&self) {
        self.dialog.close();
    }

    /// Applique le résultat d'une écriture : ferme et signale, ou affiche l'erreur.
    pub fn finish(&self, store: &Rc<Store>, error: Option<String>, success: &str) {
        match error {
            Some(message) => self.set_error(Some(&message)),
            None => {
                store.show_toast(success);
                self.close();
            }
        }
    }
}

#[cfg(all(test, feature = "ui-tests"))]
pub fn verify_closed_form_releases_its_callbacks() {
    let shell = FormShell::new("Fixture", Some("Enregistrer"), 400);
    let weak = Rc::downgrade(&shell);
    let captured = shell.clone();
    shell.on_submit(move || captured.set_error(None));
    let dialog = shell.dialog.clone();
    drop(shell);
    assert!(
        weak.upgrade().is_some(),
        "Fixture must reproduce the retained submit callback"
    );
    dialog.emit_by_name::<()>("closed", &[]);
    assert!(weak.upgrade().is_none(), "Closed form retained its submit callback");
}
