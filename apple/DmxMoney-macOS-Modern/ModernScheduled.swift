import DmxKit
import SwiftUI

/// Échéancier : table native, plage d'affichage et suggestions.
struct ModernScheduled: View {
    @ObservedObject var model: ScheduledModel
    @EnvironmentObject private var store: AppStore
    @State private var sort: [KeyPathComparator<ScheduledRow>] = []
    @State private var selection: Set<String> = []

    var body: some View {
        VStack(spacing: 0) {
            filters
            Divider()
            Table(rows, selection: $selection, sortOrder: $sort) {
                TableColumn("Compte", value: \.accountName) { row in
                    HStack(spacing: 6) {
                        Circle().fill(Color(hex: row.accountColor, fallback: .secondary)).frame(width: 7, height: 7)
                        Text(row.accountName).lineLimit(1)
                        if let destination = row.toAccountName {
                            Image(systemName: "arrow.right").foregroundStyle(.secondary)
                            Text(destination).lineLimit(1).foregroundStyle(.secondary)
                        }
                    }
                }
                .width(min: 120, ideal: 180)

                TableColumn("Échéance", value: \.scheduled.nextDate) { row in
                    HStack(spacing: 6) {
                        Text(DayFormat.numeric(row.scheduled.nextDate)).monospacedDigit()
                        if row.isEnded {
                            Text("Terminé")
                                .font(.caption2.weight(.semibold))
                                .padding(.horizontal, 5).padding(.vertical, 1)
                                .background(.quaternary, in: Capsule())
                        }
                    }
                }
                .width(min: 110, ideal: 140)

                TableColumn("Fréquence", value: \.frequencyLabel) { row in
                    Text(row.frequencyLabel).foregroundStyle(.secondary)
                }
                .width(min: 100, ideal: 140)

                TableColumn("Catégorie", value: \.category.name) { row in
                    HStack(spacing: 6) {
                        Image(systemName: Symbols.name(for: row.category.icon))
                            .foregroundStyle(Color(hex: row.category.color, fallback: .secondary))
                            .frame(width: 18, alignment: .center)
                        Text(row.category.name).lineLimit(1)
                        if let budget = row.budgetName {
                            Text(budget)
                                .font(.caption2)
                                .padding(.horizontal, 5).padding(.vertical, 1)
                                .background(.quaternary, in: Capsule())
                        }
                    }
                }
                .width(min: 120, ideal: 180)

                TableColumn("Description", value: \.scheduled.description) { row in
                    Text(row.scheduled.description).lineLimit(1)
                }
                .width(min: 120, ideal: 200)

                TableColumn("Montant", value: \.scheduled.amount) { row in
                    MoneyText(amount: row.scheduled.amount, signed: row.scheduled.transactionType)
                }
                .width(min: 90, ideal: 110)
                .alignment(.trailing)

                TableColumn("Actions") { row in
                    HStack(spacing: 6) {
                        Button { store.present(.scheduled(id: row.scheduled.id)) } label: {
                            Image(systemName: "pencil")
                        }
                        .buttonStyle(.plain)
                        .help("Modifier")
                        Button { delete(row) } label: {
                            Image(systemName: "trash")
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(.red)
                        .help("Supprimer")
                    }
                }
                .width(64)
            }
            .tableStyle(.inset)
            .contextMenu(forSelectionType: String.self) { ids in
                if let id = ids.first {
                    Button("Modifier") { store.present(.scheduled(id: id)) }
                    Button("Supprimer", role: .destructive) {
                        if let row = rows.first(where: { $0.scheduled.id == id }) { delete(row) }
                    }
                }
            } primaryAction: { ids in
                if let id = ids.first { store.present(.scheduled(id: id)) }
            }
            .onExitCommand { selection = [] }
            .overlay {
                if rows.isEmpty {
                    ContentUnavailableView {
                        Label("Aucune échéance", systemImage: "calendar.badge.clock")
                    } description: {
                        Text((model.view?.hasFilters ?? false) ? "Aucune échéance ne correspond aux filtres." : "Programmez un loyer, un abonnement, un salaire…")
                    } actions: {
                        Button("Nouvelle échéance") { store.present(.scheduled(id: nil)) }
                    }
                }
            }
        }
    }

    private var filters: some View {
        FilterBar {
            selectionActions
        } trailing: {
            HStack(spacing: 5) {
                Image(systemName: "calendar")
                Picker("Plage", selection: dueRangeBinding) {
                    ForEach(allDueRanges(), id: \.self) { range in
                        Text(dueRangeLabel(range: range)).tag(range)
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .fixedSize()
            }
            CategoryFilterMenu(selection: $model.categories)
            FilterSelector(
                title: "Fréquences",
                systemImage: "repeat",
                options: allPeriodicities().map {
                    SelectOption(id: String(describing: $0), label: periodicityLabel(frequency: $0), icon: "Repeat", color: "#6366f1")
                },
                selection: $model.frequencies
            )
            if let view = model.view {
                Text("\(view.rows.count) / \(view.totalCount)").foregroundStyle(.secondary).monospacedDigit()
            }
        }
    }

    /// Modifier ou supprimer l'échéance choisie, comme dans le journal.
    @ViewBuilder
    private var selectionActions: some View {
        HStack(spacing: 8) {
            if let view = model.view, !view.suggestions.isEmpty {
                Button("Suggestions (\(view.suggestions.count))", systemImage: "sparkles") {
                    store.present(.scheduledSuggestions)
                }
            }
            if selection.count == 1, let id = selection.first, let row = rows.first(where: { $0.scheduled.id == id }) {
                Divider().frame(height: 16)
                Text(row.scheduled.description.isEmpty ? "Échéance" : row.scheduled.description)
                    .font(.callout.weight(.medium))
                    .lineLimit(1)
                Button("Modifier", systemImage: "pencil") { store.present(.scheduled(id: id)) }
                Button("Supprimer", systemImage: "trash", role: .destructive) { delete(row) }
                Button("Désélectionner", systemImage: "xmark") { selection = [] }
                    .buttonStyle(.link)
            } else if selection.count > 1 {
                Divider().frame(height: 16)
                Text("\(selection.count) échéances").font(.callout.weight(.medium))
                Button("Supprimer", systemImage: "trash", role: .destructive) { deleteSelection() }
                Button("Désélectionner", systemImage: "xmark") { selection = [] }
                    .buttonStyle(.link)
            }
        }
        .font(.callout)
    }

    private func deleteSelection() {
        let ids = Array(selection)
        store.confirm(
            title: ids.count == 1 ? "Supprimer cette échéance ?" : "Supprimer \(ids.count) échéances ?",
            message: "Elles ne généreront plus d'opérations."
        ) {
            store.run("Échéances supprimées") { engine in
                for id in ids {
                    try engine.deleteScheduled(id: id)
                }
            }
            selection = []
        }
    }

    private var dueRangeBinding: Binding<ScheduledDueRange> {
        Binding(
            get: { store.settings.scheduledDueRange },
            set: { store.apply(.setScheduledDueRange(range: $0)) }
        )
    }

    private var rows: [ScheduledRow] {
        let rows = model.view?.rows ?? []
        return sort.isEmpty ? rows : rows.sorted(using: sort)
    }

    private func delete(_ row: ScheduledRow) {
        store.confirm(
            title: "Supprimer « \(row.scheduled.description) » ?",
            message: "L'échéance ne générera plus d'opérations."
        ) {
            store.run("Échéance supprimée") { engine in try engine.deleteScheduled(id: row.scheduled.id) }
        }
    }
}
