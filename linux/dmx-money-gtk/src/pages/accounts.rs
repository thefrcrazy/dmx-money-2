//! Mes Comptes : groupes personnalisés, cartes de soldes, glisser-déposer.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;

use crate::store::{FormRequest, Route, Store};
use crate::{format, icons, widgets};

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    groups: gtk::Box,
    empty: gtk::Box,
    empty_title: gtk::Label,
    empty_message: gtk::Label,
    empty_action: gtk::Button,
    types_button: gtk::MenuButton,
    types: Rc<RefCell<Vec<String>>>,
    query: Rc<RefCell<String>>,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = widgets::title_label("Mes Comptes");
        title.set_hexpand(true);
        header.append(&title);
        let manage = widgets::action_button("Gérer les groupes", Some("Settings"), false);
        let store_for_groups = store.clone();
        manage.connect_clicked(move |_| store_for_groups.present(FormRequest::AccountGroups));
        header.append(&manage);
        let new_account = widgets::action_button("Nouveau compte", Some("Plus"), true);
        let store_for_new = store.clone();
        new_account.connect_clicked(move |_| store_for_new.present(FormRequest::Account(None)));
        header.append(&new_account);
        content.append(&header);

        let filters = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Rechercher un compte..."));
        search.set_width_request(320);
        filters.append(&search);
        let types_button = gtk::MenuButton::new();
        types_button.set_label("Tous les types");
        filters.append(&types_button);
        content.append(&filters);

        let groups = gtk::Box::new(gtk::Orientation::Vertical, 24);
        content.append(&groups);

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.add_css_class("dmx-card");
        empty.add_css_class("card");
        empty.set_visible(false);
        let empty_title = gtk::Label::new(None);
        empty_title.add_css_class("heading");
        let empty_message = gtk::Label::new(None);
        empty_message.add_css_class("dim-label");
        let empty_action = widgets::action_button("Créer votre premier compte", Some("Plus"), false);
        empty_action.set_halign(gtk::Align::Center);
        let store_for_empty = store.clone();
        empty_action.connect_clicked(move |_| store_for_empty.present(FormRequest::Account(None)));
        empty.append(&empty_title);
        empty.append(&empty_message);
        empty.append(&empty_action);
        content.append(&empty);

        let root = widgets::page_scroll(&content).upcast();
        let page = Self {
            store: store.clone(),
            root,
            groups,
            empty,
            empty_title,
            empty_message,
            empty_action,
            types_button: types_button.clone(),
            types: Rc::new(RefCell::new(Vec::new())),
            query: Rc::new(RefCell::new(String::new())),
        };

        // Filtre par type (cases à cocher dans un popover).
        let popover = gtk::Popover::new();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        list.set_margin_start(8);
        list.set_margin_end(8);
        for account_type in dmx_core::models::ACCOUNT_TYPES {
            let check = gtk::CheckButton::with_label(account_type);
            let types = page.types.clone();
            let store_for_check = store.clone();
            let account_type = account_type.to_string();
            check.connect_toggled(move |check| {
                let mut types = types.borrow_mut();
                if check.is_active() {
                    if !types.contains(&account_type) {
                        types.push(account_type.clone());
                    }
                } else {
                    types.retain(|candidate| candidate != &account_type);
                }
                drop(types);
                store_for_check.navigate(Route::Accounts);
            });
            list.append(&check);
        }
        popover.set_child(Some(&list));
        types_button.set_popover(Some(&popover));

        let store_for_search = store.clone();
        let query = page.query.clone();
        search.connect_search_changed(move |entry| {
            *query.borrow_mut() = entry.text().to_string();
            store_for_search.navigate(Route::Accounts);
        });

        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let query = dmx_core::accounts_view::AccountsQuery {
            search: self.query.borrow().clone(),
            types: self.types.borrow().clone(),
        };
        let Some(view) = self.store.read(|engine| engine.accounts(&query)) else {
            return;
        };

        self.types_button.set_label(&if self.types.borrow().is_empty() {
            "Tous les types".to_string()
        } else {
            format!("Types ({})", self.types.borrow().len())
        });

        widgets::clear(&self.groups);
        let has_accounts = view.total_count > 0;
        let has_visible = view.visible_count > 0;
        self.empty.set_visible(!has_visible);
        self.groups.set_visible(has_visible);
        self.empty_title.set_text(if has_accounts {
            "Aucun compte ne correspond aux filtres."
        } else {
            "Aucun compte configuré"
        });
        self.empty_message.set_text(if has_accounts {
            "Modifie la recherche ou les types sélectionnés."
        } else {
            ""
        });
        self.empty_action.set_visible(!has_accounts);

        for section in &view.groups {
            let block = gtk::Box::new(gtk::Orientation::Vertical, 14);

            let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            if !section.is_ungrouped {
                let handle = icons::image("GripVertical", 14);
                handle.add_css_class("dim-label");
                header.append(&handle);
            }
            let name = gtk::Label::new(Some(&section.name));
            name.add_css_class("title-4");
            header.append(&name);
            header.append(&widgets::chip(&section.accounts.len().to_string(), "#9ca3af", None));
            block.append(&header);

            // Le déposé sur l'en-tête déplace un compte dans ce groupe.
            let group_target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);
            let store_for_drop = self.store.clone();
            let group_name = if section.is_ungrouped {
                None
            } else {
                Some(section.name.clone())
            };
            group_target.connect_drop(move |_, value, _, _| {
                if let Ok(payload) = value.get::<String>() {
                    if let Some(account_id) = payload.strip_prefix("account:") {
                        let account_id = account_id.to_string();
                        let group = group_name.clone();
                        store_for_drop.run(None, move |engine| {
                            engine.move_account(&account_id, dmx_core::ops::AccountDropTarget::Group(group))
                        });
                        return true;
                    }
                    if let (Some(group), Some(over)) = (payload.strip_prefix("group:"), group_name.clone()) {
                        let group = group.to_string();
                        store_for_drop.run(None, move |engine| engine.move_group(&group, &over));
                        return true;
                    }
                }
                false
            });
            header.add_controller(group_target);

            if !section.is_ungrouped {
                let source = gtk::DragSource::new();
                source.set_actions(gdk::DragAction::MOVE);
                let payload = format!("group:{}", section.name);
                source.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&payload.to_value())));
                header.add_controller(source);
            }

            let grid = gtk::FlowBox::new();
            grid.set_selection_mode(gtk::SelectionMode::None);
            grid.set_column_spacing(16);
            grid.set_row_spacing(16);
            grid.set_min_children_per_line(1);
            grid.set_max_children_per_line(3);
            grid.set_homogeneous(true);
            for card in &section.accounts {
                grid.append(&self.account_card(card));
            }
            if section.accounts.is_empty() {
                let placeholder = widgets::caption("Glissez un compte ici");
                placeholder.set_halign(gtk::Align::Center);
                let host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                host.add_css_class("dmx-dashed");
                host.set_size_request(-1, 64);
                host.set_halign(gtk::Align::Fill);
                host.append(&placeholder);
                block.append(&host);
            } else {
                block.append(&grid);
            }
            self.groups.append(&block);
        }
    }

    fn account_card(&self, card: &dmx_core::accounts_view::AccountCard) -> gtk::Box {
        let host = gtk::Box::new(gtk::Orientation::Vertical, 12);
        host.add_css_class("dmx-card");
        host.add_css_class("card");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.append(&icons::badge(&card.account.icon, &card.account.color, 46, false));
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let name = gtk::Label::new(Some(&card.account.name));
        name.set_xalign(0.0);
        name.add_css_class("heading");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        labels.append(&name);
        labels.append(&widgets::caption(&card.account.account_type));
        header.append(&labels);

        let edit = widgets::icon_button("Edit2", "Modifier");
        let store_for_edit = self.store.clone();
        let id = card.account.id.clone();
        edit.connect_clicked(move |_| store_for_edit.present(FormRequest::Account(Some(id.clone()))));
        header.append(&edit);

        let delete = widgets::icon_button("Trash2", "Supprimer");
        delete.add_css_class("dmx-expense");
        let store_for_delete = self.store.clone();
        let id = card.account.id.clone();
        delete.connect_clicked(move |_| {
            let store = store_for_delete.clone();
            let id = id.clone();
            store_for_delete.confirm(
                "Supprimer le compte",
                "Êtes-vous sûr de vouloir supprimer ce compte ? Cette action est irréversible et supprimera toutes les transactions associées.",
                "Supprimer",
                move || {
                    let id = id.clone();
                    store.run(Some("Compte supprimé"), move |engine| engine.delete_account(&id));
                },
            );
        });
        header.append(&delete);
        host.append(&header);

        let balances = gtk::Box::new(gtk::Orientation::Vertical, 4);
        balances.append(&widgets::caption("Solde actuel"));
        let current = gtk::Label::new(Some(&format::money(card.current_balance)));
        current.set_xalign(0.0);
        current.add_css_class("dmx-big-amount");
        if card.current_balance < 0.0 {
            current.add_css_class("dmx-expense");
        }
        balances.append(&current);
        balances.append(&widgets::separator());
        let checked_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let checked_label = widgets::caption("Solde pointé");
        checked_label.set_hexpand(true);
        checked_row.append(&checked_label);
        checked_row.append(&widgets::amount(
            &format::money(card.checked_balance),
            Some("dim-label"),
        ));
        balances.append(&checked_row);
        host.append(&balances);

        // Glisser un compte vers un autre compte ou un groupe.
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::MOVE);
        let payload = format!("account:{}", card.account.id);
        source.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&payload.to_value())));
        host.add_controller(source);

        let target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);
        let store_for_drop = self.store.clone();
        let target_id = card.account.id.clone();
        let target_group = card.group.clone();
        target.connect_drop(move |_, value, _, _| {
            if let Ok(payload) = value.get::<String>() {
                if let Some(account_id) = payload.strip_prefix("account:") {
                    if account_id == target_id {
                        return false;
                    }
                    let account_id = account_id.to_string();
                    let target_id = target_id.clone();
                    store_for_drop.run(None, move |engine| {
                        engine.move_account(&account_id, dmx_core::ops::AccountDropTarget::Account(target_id))
                    });
                    return true;
                }
                if let (Some(group), Some(over)) = (payload.strip_prefix("group:"), target_group.clone()) {
                    let group = group.to_string();
                    store_for_drop.run(None, move |engine| engine.move_group(&group, &over));
                    return true;
                }
            }
            false
        });
        host.add_controller(target);

        host
    }
}

use gtk::glib;
