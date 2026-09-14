import DmxKit
import SwiftUI

/// Catégories : table native, la catégorie « Virement » reste verrouillée.
struct ModernCategories: View {
    @Binding var search: String
    @EnvironmentObject private var store: AppStore
    @State private var sort = [KeyPathComparator(\DmxCategory.name)]
    @State private var selection: Set<String> = []

    var body: some View {
        VStack(spacing: 0) {
            FilterBar {
                selectionActions
            } trailing: {
                Text("\(filtered.count) / \(store.categories.count)")
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
            Divider()
            table
        }
    }

    /// Modifier ou supprimer la catégorie choisie, comme dans le journal.
    @ViewBuilder
    private var selectionActions: some View {
        if let id = selection.first, selection.count == 1, let category = store.categories.first(where: { $0.id == id }) {
            HStack(spacing: 8) {
                CategoryBadge(icon: category.icon, colorHex: category.color, size: 20)
                Text(category.name).font(.callout.weight(.medium))
                if category.id == "transfer" {
                    Text("réservée aux virements").foregroundStyle(.secondary)
                } else {
                    Button("Modifier", systemImage: "pencil") { store.present(.category(id: id)) }
                    Button("Supprimer", systemImage: "trash", role: .destructive) { delete(category) }
                }
                Button("Désélectionner", systemImage: "xmark") { selection = [] }
                    .buttonStyle(.link)
            }
            .font(.callout)
        } else {
            Text("Cliquez une catégorie pour la modifier ou la supprimer.")
                .font(.callout)
                .foregroundStyle(.secondary)
        }
    }

    private var table: some View {
        Table(filtered, selection: $selection, sortOrder: $sort) {
            TableColumn("Catégorie", value: \.name) { category in
                HStack(spacing: 8) {
                    CategoryBadge(icon: category.icon, colorHex: category.color, size: 24)
                    Text(category.name)
                    if category.id == "transfer" {
                        Image(systemName: "lock").foregroundStyle(.secondary).help("Catégorie réservée aux virements")
                    }
                }
            }
            TableColumn("Couleur", value: \.color) { category in
                HStack(spacing: 6) {
                    RoundedRectangle(cornerRadius: 4)
                        .fill(Color(hex: category.color, fallback: .secondary))
                        .frame(width: 22, height: 14)
                    Text(category.color.uppercased()).font(.caption.monospaced()).foregroundStyle(.secondary)
                }
            }
            .width(min: 110, ideal: 130)
            TableColumn("Actions") { category in
                if category.id != "transfer" {
                    HStack(spacing: 6) {
                        Button { store.present(.category(id: category.id)) } label: {
                            Image(systemName: "pencil")
                        }
                        .buttonStyle(.plain)
                        .help("Modifier")
                        Button { delete(category) } label: {
                            Image(systemName: "trash")
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(.red)
                        .help("Supprimer")
                    }
                }
            }
            .width(64)
        }
        .tableStyle(.inset)
        .contextMenu(forSelectionType: String.self) { ids in
            if let id = ids.first, id != "transfer" {
                Button("Modifier") { store.present(.category(id: id)) }
                Button("Supprimer", role: .destructive) {
                    if let category = store.categories.first(where: { $0.id == id }) { delete(category) }
                }
            }
        } primaryAction: { ids in
            if let id = ids.first, id != "transfer" { store.present(.category(id: id)) }
        }
        .onExitCommand { selection = [] }
        .overlay {
            if filtered.isEmpty {
                ContentUnavailableView.search(text: search)
            }
        }
    }

    private var filtered: [DmxCategory] {
        let query = search.trimmingCharacters(in: .whitespaces)
        let categories = query.isEmpty
            ? store.categories
            : store.categories.filter { $0.name.range(of: query, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
        return categories.sorted(using: sort)
    }

    private func delete(_ category: DmxCategory) {
        store.confirm(
            title: "Supprimer « \(category.name) » ?",
            message: "Les opérations de cette catégorie seront reclassées dans « Divers »."
        ) {
            store.run("Catégorie supprimée") { engine in try engine.deleteCategory(id: category.id) }
            selection = []
        }
    }
}
