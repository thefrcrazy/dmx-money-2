//! Catégories : grille, recherche, création et modification (« Virement » verrouillée).

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use crate::store::{FormRequest, Store};
use crate::{icons, widgets};

pub struct Page {
    store: Rc<Store>,
    root: gtk::Widget,
    grid: gtk::FlowBox,
    empty: gtk::Box,
    search: gtk::SearchEntry,
    query: Rc<RefCell<String>>,
}

impl Page {
    pub fn new(store: &Rc<Store>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = widgets::title_label("Gestion des Catégories");
        title.set_hexpand(true);
        header.append(&title);
        let new_category = widgets::action_button("Nouvelle catégorie", Some("Plus"), true);
        let store_for_new = store.clone();
        new_category.connect_clicked(move |_| store_for_new.present(FormRequest::Category(None)));
        header.append(&new_category);
        content.append(&header);

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Rechercher une catégorie..."));
        search.set_hexpand(false);
        search.set_width_request(320);
        search.set_halign(gtk::Align::Start);
        content.append(&search);

        let grid = gtk::FlowBox::new();
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_column_spacing(12);
        grid.set_row_spacing(12);
        grid.set_min_children_per_line(1);
        grid.set_max_children_per_line(4);
        grid.set_homogeneous(true);
        content.append(&grid);

        let empty = widgets::empty_state("Search", "Aucune catégorie ne correspond à la recherche.", None);
        empty.set_visible(false);
        content.append(&empty);

        let root = widgets::page_scroll(&content).upcast();
        let page = Self {
            store: store.clone(),
            root,
            grid,
            empty,
            search: search.clone(),
            query: Rc::new(RefCell::new(String::new())),
        };

        let store_for_search = store.clone();
        let query = page.query.clone();
        search.connect_search_changed(move |entry| {
            *query.borrow_mut() = entry.text().to_string();
            store_for_search.navigate(crate::store::Route::Categories);
        });
        page
    }

    pub fn root(&self) -> gtk::Widget {
        self.root.clone()
    }

    pub fn refresh(&self) {
        let query = self.query.borrow().trim().to_lowercase();
        let categories: Vec<_> = self
            .store
            .categories()
            .into_iter()
            .filter(|category| {
                query.is_empty()
                    || dmx_core::text::normalize_search(&category.name)
                        .contains(&dmx_core::text::normalize_search(&query))
            })
            .collect();

        widgets::clear(&self.grid);
        self.empty.set_visible(categories.is_empty());
        self.grid.set_visible(!categories.is_empty());
        let _ = &self.search;

        for category in categories {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("dmx-card");
            row.add_css_class("card");
            row.append(&icons::badge(&category.icon, &category.color, 36, false));
            let name = gtk::Label::new(Some(&category.name));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&name);

            if category.id == dmx_core::models::TRANSFER_CATEGORY_ID {
                let locked = icons::image("Lock", 14);
                locked.add_css_class("dim-label");
                row.append(&locked);
            } else {
                let edit = widgets::icon_button("Edit2", "Modifier");
                let store_for_edit = self.store.clone();
                let id = category.id.clone();
                edit.connect_clicked(move |_| store_for_edit.present(FormRequest::Category(Some(id.clone()))));
                row.append(&edit);

                let delete = widgets::icon_button("Trash2", "Supprimer");
                delete.add_css_class("dmx-expense");
                let store_for_delete = self.store.clone();
                let id = category.id.clone();
                let name = category.name.clone();
                delete.connect_clicked(move |_| {
                    let store = store_for_delete.clone();
                    let id = id.clone();
                    store_for_delete.confirm(
                        "Supprimer la catégorie",
                        &format!("Êtes-vous sûr de vouloir supprimer « {name} » ?"),
                        "Supprimer",
                        move || {
                            let id = id.clone();
                            store.run(Some("Catégorie supprimée"), move |engine| engine.delete_category(&id));
                        },
                    );
                });
                row.append(&delete);
            }
            self.grid.append(&row);
        }
    }
}
