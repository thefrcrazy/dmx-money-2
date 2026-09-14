import SwiftUI

/// Gestion des catégories : grille, recherche, création et modification (Virement verrouillée).
public struct CategoriesPage: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact
    @State private var search = ""

    public init() {}

    public var body: some View {
        PageScroll {
            PageHeader("Gestion des Catégories") {
                Button(action: { store.present(.category(id: nil)) }) {
                    HStack(spacing: 6) {
                        DmxIcon("Plus", size: 13)
                        Text("Nouvelle catégorie")
                    }
                }
                .buttonStyle(DmxButtonStyle(.primary))
            }

            SearchField("Rechercher une catégorie...", text: $search)
                .frame(maxWidth: compact ? .infinity : 360)

            if filtered.isEmpty {
                DmxCard {
                    EmptyStateView(icon: "Search", title: "Aucune catégorie ne correspond à la recherche.")
                }
            } else {
                VStack(spacing: 12) {
                    ForEach(Array(filtered.chunked(columns).enumerated()), id: \.offset) { row in
                        HStack(spacing: 12) {
                            ForEach(row.element, id: \.id) { category in
                                CategoryTile(category: category)
                            }
                            ForEach(0..<(columns - row.element.count), id: \.self) { _ in
                                Color.clear.frame(maxWidth: .infinity, maxHeight: 1)
                            }
                        }
                    }
                }
            }
        }
    }

    /// Trois colonnes sur grand écran : à quatre, les noms composés étaient tronqués.
    private var columns: Int { compact ? 1 : 3 }

    private var filtered: [DmxCategory] {
        let query = search.trimmingCharacters(in: .whitespaces)
        guard !query.isEmpty else { return store.categories }
        return store.categories.filter { $0.name.range(of: query, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
    }
}

struct CategoryTile: View {
    @EnvironmentObject private var store: AppStore
    let category: DmxCategory

    private var isLocked: Bool { category.id == "transfer" }

    var body: some View {
        HStack(spacing: 10) {
            IconBadge(icon: category.icon, colorHex: category.color, size: 36)
            Text(category.name).font(.system(size: 13, weight: .medium)).lineLimit(1)
            Spacer(minLength: 4)
            if isLocked {
                DmxIcon("Lock", size: 12).foregroundColor(.secondary)
            } else {
                IconButton("Edit2") { store.present(.category(id: category.id)) }
                IconButton("Trash2", color: DmxColors.expense, action: delete)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(DmxPalette.cardBackground))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(DmxPalette.separator.opacity(0.5), lineWidth: 0.5))
        .contextMenu {
            if !isLocked {
                Button(action: { store.present(.category(id: category.id)) }) { Text("Modifier") }
                Button(action: delete) { Text("Supprimer") }
            }
        }
    }

    private func delete() {
        let id = category.id
        store.confirm(title: "Supprimer la catégorie", message: "Êtes-vous sûr de vouloir supprimer « \(category.name) » ?") { [store] in
            store.run("Catégorie supprimée") { engine in try engine.deleteCategory(id: id) }
        }
    }
}
