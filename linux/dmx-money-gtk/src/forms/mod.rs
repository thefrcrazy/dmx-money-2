//! Formulaires présentés en `AdwDialog`. Les brouillons viennent du noyau et y retournent
//! sans transformation : l'interface ne calcule aucun montant.

pub mod controls;
mod data;
mod entities;

use std::rc::Rc;

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
        let dialog_for_cancel = dialog.clone();
        cancel.connect_clicked(move |_| {
            dialog_for_cancel.close();
        });

        Rc::new(Self {
            dialog,
            body,
            error,
            submit,
            submit_label,
            header_extra,
        })
    }

    pub fn body(&self) -> &gtk::Box {
        &self.body
    }

    /// Boutons additionnels (retour de l'assistant, actions secondaires).
    pub fn extra_actions(&self) -> &gtk::Box {
        &self.header_extra
    }

    pub fn set_error(&self, message: Option<&str>) {
        self.error.set_text(message.unwrap_or(""));
        self.error.set_visible(message.is_some());
    }

    pub fn set_submit_title(&self, title: &str) {
        self.submit_label.set_text(title);
    }

    pub fn on_submit(&self, action: impl Fn() + 'static) {
        self.submit.connect_clicked(move |_| action());
    }

    /// Reconstruit le contenu après chaque écriture, et se désabonne à la fermeture :
    /// sans cela, l'abonnement survivrait au dialogue et manipulerait des widgets détruits.
    pub fn follow(&self, store: &Rc<Store>, refresh: Rc<dyn Fn()>) {
        let id = store.subscribe(move |_| refresh());
        let store = store.clone();
        self.dialog.connect_closed(move |_| store.unsubscribe(id));
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent.as_ref()));
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
