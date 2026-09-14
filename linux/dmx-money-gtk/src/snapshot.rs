//! Mode capture (`DMXMONEY_SNAPSHOT_DIR`) : rend chaque page et chaque formulaire en PNG, puis
//! quitte. Même usage que le `SnapshotRunner` de l'app macOS : vérifier l'interface sans
//! piloter l'écran.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::store::{FormRequest, Route, Store};

pub fn directory() -> Option<PathBuf> {
    match std::env::var("DMXMONEY_SNAPSHOT_DIR") {
        Ok(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => None,
    }
}

/// Étapes : les neuf pages, puis les formulaires présentés par-dessus la page concernée.
fn steps() -> Vec<(String, Route, Option<FormRequest>)> {
    let mut steps: Vec<(String, Route, Option<FormRequest>)> = Route::sections()
        .iter()
        .flat_map(|(_, routes)| routes.iter())
        .chain(Route::FOOTER.iter())
        .map(|route| (route.id().to_string(), *route, None))
        .collect();
    let forms = [
        ("form-transaction", Route::Transactions, FormRequest::Transaction(None)),
        ("form-account", Route::Accounts, FormRequest::Account(None)),
        ("form-account-groups", Route::Accounts, FormRequest::AccountGroups),
        ("form-category", Route::Categories, FormRequest::Category(None)),
        ("form-budget", Route::Budget, FormRequest::Budget(None, None)),
        ("form-scheduled", Route::Scheduled, FormRequest::Scheduled(None)),
        (
            "form-fake-transaction",
            Route::Predictions,
            FormRequest::FakeTransaction(None),
        ),
        ("form-budget-suggestions", Route::Budget, FormRequest::BudgetSuggestions),
        ("form-whats-new", Route::Dashboard, FormRequest::WhatsNew),
    ];
    steps.extend(
        forms
            .into_iter()
            .map(|(name, route, request)| (name.to_string(), route, Some(request))),
    );
    steps
}

pub fn run(store: &Rc<Store>, window: &adw::ApplicationWindow, directory: PathBuf) {
    if let Err(error) = std::fs::create_dir_all(&directory) {
        log::error!("dossier de capture : {error}");
        return;
    }
    perform(store.clone(), window.clone(), directory, steps(), 0);
}

fn perform(
    store: Rc<Store>,
    window: adw::ApplicationWindow,
    directory: PathBuf,
    steps: Vec<(String, Route, Option<FormRequest>)>,
    index: usize,
) {
    let Some((name, route, request)) = steps.get(index).cloned() else {
        if let Some(application) = window.application() {
            application.quit();
        }
        return;
    };

    log::info!("capture {name} ({})", route.id());
    store.navigate(route);
    if let Some(request) = request {
        store.present(request);
    }

    // Laisse GTK terminer la mise en page et le rendu avant la capture.
    glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
        if let Err(error) = capture(&window, &directory.join(format!("{name}.png"))) {
            log::error!("capture {name} : {error}");
        }
        if let Some(dialog) = window.visible_dialog() {
            dialog.force_close();
        }
        glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            perform(store, window, directory, steps, index + 1);
        });
    });
}

/// Rend le contenu de la fenêtre dans une texture, via le moteur de rendu déjà réalisé.
fn capture(window: &adw::ApplicationWindow, path: &Path) -> Result<(), String> {
    let widget = window.upcast_ref::<gtk::Widget>();
    let (width, height) = (widget.width().max(1), widget.height().max(1));
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width as f64, height as f64);
    let node = snapshot.to_node().ok_or("rien à rendre")?;
    let renderer = widget
        .native()
        .and_then(|native| native.renderer())
        .ok_or("fenêtre sans moteur de rendu")?;
    let texture = renderer.render_texture(&node, None);
    let bytes = texture.save_to_png_bytes();
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}
