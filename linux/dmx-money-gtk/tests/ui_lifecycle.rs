//! Real GTK objects, executed on the process main thread; no user database is opened.
#![allow(dead_code)]
#[path = "../src/format.rs"]
mod format;
#[path = "../src/forms/mod.rs"]
mod forms;
#[path = "../src/icon_names.rs"]
mod icon_names;
#[path = "../src/icons.rs"]
mod icons;
#[path = "../src/store.rs"]
mod store;
#[path = "../src/widgets.rs"]
mod widgets;
use adw::prelude::*;
use forms::controls::{ColorGrid, DateButton, IconGrid, KindPicker, Select, SelectOption};
use std::rc::Rc;

fn releases<T>(control: Rc<T>, widget: gtk::Widget) {
    let weak = Rc::downgrade(&control);
    let widget_weak = widget.downgrade();
    drop(control);
    assert!(weak.upgrade().is_none(), "Control retained itself in a widget callback");
    drop(widget);
    assert!(
        widget_weak.upgrade().is_none(),
        "Widget tree retained a popover or callback cycle"
    );
}

fn main() {
    adw::init().expect("GTK display required (use xvfb-run on Linux)");
    for _ in 0..20 {
        forms::verify_closed_form_releases_its_callbacks();
        let select = Select::new(
            "Fixture",
            vec![SelectOption::simple("A", "Alpha")],
            Some("A".into()),
            None,
        );
        let widget = select.widget();
        releases(select, widget);
        let kind = KindPicker::new(dmx_core::models::TransactionType::Expense);
        let widget = kind.widget();
        releases(kind, widget);
        let colors = ColorGrid::new(&["#6366f1", "#ef4444"], "#6366f1", 2, 50);
        let widget = colors.widget();
        releases(colors, widget);
        let icons = IconGrid::new(&["Wallet", "Tag"], "Wallet", 2, 50);
        let widget = icons.widget();
        releases(icons, widget);
        let date = DateButton::new("2026-10-03");
        let widget = date.widget();
        releases(date, widget);
    }
    println!("GTK: 20 closed-form/control/calendar lifecycle iterations passed with real objects.");
}
