import DmxKit
import SwiftUI

/// Journal en tableau AppKit à cellules réutilisables : colonnes triables, sélection multiple, édition en ligne,
/// menus contextuels et actions groupées.
struct ModernJournal: View {
    @ObservedObject var model: JournalModel
    @EnvironmentObject private var store: AppStore

    var body: some View {
        VStack(spacing: 0) {
            filters
            Divider()
            table
                .overlay {
                    if model.isLoading && model.view == nil { ProgressView("Chargement du journal…") }
                }
        }
    }

    // MARK: Filtres

    /// Actions de la sélection à gauche, filtres alignés à droite comme sur les autres pages.
    private var filters: some View {
        FilterBar {
            selectionActions
        } trailing: {
            FilterSelector(
                title: "Catégories",
                systemImage: "tag",
                options: store.categories.map {
                    SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color)
                },
                selection: $model.categories
            )
            FilterSelector(
                title: "Types",
                systemImage: "arrow.left.arrow.right",
                options: JournalModel.typeOptions,
                selection: $model.types
            )
            FilterSelector(
                title: "États",
                systemImage: "checkmark.circle",
                options: JournalModel.statusOptions,
                selection: $model.statuses
            )
            FilterSelector(
                title: "Budgets",
                systemImage: "wallet.bifold",
                options: Self.budgetOptions,
                selection: $model.budgetStatuses
            )
            if model.hasFilters {
                Button("Réinitialiser", systemImage: "xmark.circle") { model.clearFilters() }
                    .buttonStyle(.link)
            }
        }
    }

    /// Pointer, modifier et supprimer la sélection, visibles dès qu'une ligne est choisie.
    @ViewBuilder
    private var selectionActions: some View {
        if model.selection.isEmpty {
            Text("Cliquez une ligne pour la pointer, la modifier ou la supprimer.")
                .font(.callout)
                .foregroundStyle(.secondary)
        } else {
            HStack(spacing: 8) {
                Text(model.selection.count == 1 ? "1 opération" : "\(model.selection.count) opérations")
                    .font(.callout.weight(.medium))
                Button("Pointer", systemImage: "checkmark.circle") { model.toggleCheckedSelection() }
                if model.selection.count == 1, let id = model.selection.first {
                    Button("Modifier", systemImage: "pencil") { store.present(.transaction(id: id)) }
                }
                Button("Supprimer", systemImage: "trash", role: .destructive) { model.deleteSelection() }
                Button("Désélectionner", systemImage: "xmark") { model.selection = [] }
                    .buttonStyle(.link)
            }
            .font(.callout)
        }
    }

    /// Les options de budget arrivent sans icône du noyau : on les habille ici.
    private static let budgetOptions = JournalModel.budgetOptions.map { option in
        SelectOption(
            id: option.id,
            label: option.label,
            icon: option.id == "budgeted" ? "Wallet" : "CircleOff",
            color: option.id == "budgeted" ? "#6366f1" : "#9ca3af"
        )
    }

    // MARK: Tableau

    private var table: some View {
        NativeJournalTable(store: store, model: model)
            .overlay {
                if model.rows.isEmpty {
                    ContentUnavailableView {
                        Label("Aucune opération", systemImage: "list.bullet.rectangle.portrait")
                    } description: {
                        Text(
                            model.hasFilters
                                ? "Aucune opération ne correspond aux filtres." : "Ajoutez votre première opération.")
                    } actions: {
                        if model.hasFilters {
                            Button("Réinitialiser les filtres") { model.clearFilters() }
                        } else {
                            Button("Nouvelle transaction") { store.present(.transaction(id: nil)) }
                        }
                    }
                }
            }
    }

}

/// Native reusable cells keep fast scrolling out of SwiftUI's per-cell hosting graph.
private struct NativeJournalTable: NSViewControllerRepresentable {
    let store: AppStore
    let model: JournalModel

    func makeNSViewController(context: Context) -> JournalViewController {
        JournalViewController(store: store, model: model, includesHeader: false)
    }

    func updateNSViewController(_ controller: JournalViewController, context: Context) {}

    static func dismantleNSViewController(_ controller: JournalViewController, coordinator: ()) {
        controller.stopUpdates()
    }
}
