import SwiftUI

public final class ScheduledModel: PageModel {
    public var search = "" {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var categories: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    /// Clés de périodicité sélectionnées (voir `periodicityKey`).
    public var frequencies: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public private(set) var view: ScheduledView? = nil {
        willSet { objectWillChange.send() }
    }

    public override init(store: AppStore) {
        super.init(store: store)
        refresh()
    }

    public override func refresh() {
        let selected = Set(frequencies)
        let query = ScheduledQuery(
            accounts: store.selectedAccountIds,
            dueRange: store.settings.scheduledDueRange,
            search: search,
            categories: categories,
            frequencies: allPeriodicities().filter { selected.contains(periodicityKey($0)) }
        )
        let today = store.today
        view = store.read { engine in try engine.scheduled(query: query, today: today) }
    }
}

/// Échéancier : transactions récurrentes, plage d'affichage persistée, filtres et suggestions.
public struct ScheduledPage: View {
    @ObservedObject private var model: ScheduledModel
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact

    public init(model: ScheduledModel) {
        self.model = model
    }

    public var body: some View {
        PageScroll {
            PageHeader("Transactions Récurrentes") {
                HStack(spacing: 8) {
                    if let count = model.view?.suggestions.count, count > 0 {
                        Button(action: { store.present(.scheduledSuggestions) }) {
                            HStack(spacing: 6) {
                                DmxIcon("Sparkles", size: 13)
                                Text("Suggestions (\(count))")
                            }
                        }
                        .buttonStyle(DmxButtonStyle(.secondary))
                    }
                    Button(action: { store.present(.scheduled(id: nil)) }) {
                        HStack(spacing: 6) {
                            DmxIcon("Plus", size: 13)
                            Text("Nouvelle transaction")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(.primary))
                }
            }

            rangeSelector

            HStack(spacing: 10) {
                SearchField("Rechercher dans l'échéancier...", text: $model.search)
                    .frame(maxWidth: compact ? .infinity : 360)
                MultiSelectButton("Toutes les catégories", options: store.categoryOptions, selection: $model.categories)
                MultiSelectButton(
                    "Toutes les fréquences",
                    options: allPeriodicities().map { SelectOption(id: periodicityKey($0), label: periodicityLabel(frequency: $0)) },
                    selection: $model.frequencies
                )
                if !compact { Spacer() }
            }

            if let view = model.view {
                if view.rows.isEmpty {
                    DmxCard {
                        EmptyStateView(
                            icon: "Search",
                            title: view.hasFilters ? "Aucune transaction récurrente ne correspond aux filtres." : "Aucune transaction récurrente configurée",
                            message: view.hasFilters ? nil : "Ajoute une transaction récurrente pour commencer."
                        )
                    }
                } else if compact {
                    VStack(spacing: 12) {
                        ForEach(view.rows, id: \.scheduled.id) { row in
                            ScheduledCompactCard(row: row)
                        }
                    }
                } else {
                    table(view.rows)
                }
            }
        }
    }

    // MARK: Plage d'échéance

    @ViewBuilder
    private var rangeSelector: some View {
        if compact {
            ChoicePicker(
                options: allDueRanges().map { SelectOption(id: String(describing: $0), label: dueRangeLabel(range: $0)) },
                selection: Binding(
                    get: { String(describing: store.settings.scheduledDueRange) },
                    set: { key in
                        if let range = allDueRanges().first(where: { String(describing: $0) == key }) {
                            store.apply(.setScheduledDueRange(range: range))
                        }
                    }
                )
            )
        } else {
            HStack(spacing: 6) {
                ForEach(allDueRanges(), id: \.self) { range in
                    let isSelected = store.settings.scheduledDueRange == range
                    Button(action: { store.apply(.setScheduledDueRange(range: range)) }) {
                        Text(dueRangeLabel(range: range))
                            .font(.system(size: 12, weight: .medium))
                            .foregroundColor(isSelected ? .accentColor : .secondary)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(isSelected ? Color.accentColor.opacity(0.14) : Color.clear))
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(PlainButtonStyle())
                }
                Spacer()
            }
        }
    }

    // MARK: Tableau

    private enum Column {
        static let account: CGFloat = 150
        static let due: CGFloat = 170
        static let frequency: CGFloat = 140
        static let category: CGFloat = 150
        static let amount: CGFloat = 110
        static let actions: CGFloat = 64
    }

    private func table(_ rows: [ScheduledRow]) -> some View {
        DmxCard(padded: false) {
            VStack(spacing: 0) {
                HStack(spacing: 12) {
                    SectionLabel("Compte").frame(width: Column.account, alignment: .leading)
                    SectionLabel("Échéance").frame(width: Column.due, alignment: .leading)
                    SectionLabel("Fréquence").frame(width: Column.frequency, alignment: .leading)
                    SectionLabel("Catégorie").frame(width: Column.category, alignment: .leading)
                    SectionLabel("Description").frame(maxWidth: .infinity, alignment: .leading)
                    SectionLabel("Montant").frame(width: Column.amount, alignment: .trailing)
                    SectionLabel("Actions").frame(width: Column.actions, alignment: .trailing)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(Color.primary.opacity(0.03))
                ForEach(rows, id: \.scheduled.id) { row in
                    Divider()
                    tableRow(row)
                }
            }
        }
    }

    private func tableRow(_ row: ScheduledRow) -> some View {
        let item = row.scheduled
        return HStack(spacing: 12) {
            HStack(spacing: 10) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(row.isEnded ? DmxColors.expense : Color(hex: row.accountColor, fallback: DmxColors.transfer))
                    .frame(width: 4, height: 24)
                VStack(alignment: .leading, spacing: 1) {
                    Text(row.accountName).font(.system(size: 13, weight: .medium)).lineLimit(1)
                    if let destination = row.toAccountName {
                        Text("→ \(destination)").font(.system(size: 11)).foregroundColor(.secondary).lineLimit(1)
                    }
                }
            }
            .frame(width: Column.account, alignment: .leading)

            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    DmxIcon("Calendar", size: 13).foregroundColor(.secondary)
                    Text(DayFormat.medium(item.nextDate)).font(.system(size: 12)).lineLimit(1)
                }
                if let end = item.endDate {
                    Text("→ \(DayFormat.medium(end))").font(.system(size: 11)).foregroundColor(.secondary).lineLimit(1)
                }
                if row.isEnded {
                    Pill("Terminé", color: DmxColors.expense)
                } else if item.budgetId != nil {
                    Pill("Budget", color: DmxColors.transfer)
                }
            }
            .frame(width: Column.due, alignment: .leading)

            HStack(spacing: 6) {
                DmxIcon("Clock", size: 13).foregroundColor(.secondary)
                Text(row.frequencyLabel).font(.system(size: 12)).foregroundColor(.secondary).lineLimit(1)
            }
            .frame(width: Column.frequency, alignment: .leading)

            CategoryChip(category: row.category, isTransfer: item.transactionType == .transfer)
                .frame(width: Column.category, alignment: .leading)

            VStack(alignment: .leading, spacing: 2) {
                Text(item.description).font(.system(size: 13)).lineLimit(1)
                if let budget = row.budgetName {
                    HStack(spacing: 4) {
                        DmxIcon("Tag", size: 11)
                        Text(budget).lineLimit(1)
                    }
                    .font(.system(size: 11))
                    .foregroundColor(DmxColors.income)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            ScheduledAmount(item: item).frame(width: Column.amount, alignment: .trailing)

            HStack(spacing: 0) {
                IconButton("Edit2", color: .accentColor) { store.present(.scheduled(id: item.id)) }
                IconButton("Trash2", color: DmxColors.expense) { deleteScheduled(item.id, store: store) }
            }
            .frame(width: Column.actions, alignment: .trailing)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(row.isEnded ? DmxColors.expense.opacity(0.05) : Color.clear)
        .contentShape(Rectangle())
        .contextMenu {
            Button(action: { store.present(.scheduled(id: item.id)) }) { Text("Modifier") }
            Button(action: { deleteScheduled(item.id, store: store) }) { Text("Supprimer") }
        }
    }
}

func deleteScheduled(_ id: String, store: AppStore) {
    store.confirm(title: "Supprimer la transaction récurrente", message: "Êtes-vous sûr de vouloir supprimer cette transaction récurrente ?") {
        store.run("Transaction récurrente supprimée") { engine in try engine.deleteScheduled(id: id) }
    }
}

struct CategoryChip: View {
    let category: CategoryDisplay
    let isTransfer: Bool

    var body: some View {
        let color = isTransfer ? DmxColors.transfer : Color(hex: category.color, fallback: DmxColors.muted)
        return HStack(spacing: 5) {
            DmxIcon(isTransfer ? "ArrowRightLeft" : category.icon, size: 11)
            Text(isTransfer ? "Virement" : category.name).lineLimit(1)
        }
        .font(.system(size: 11, weight: .medium))
        .foregroundColor(color)
        .padding(.horizontal, 9)
        .padding(.vertical, 3)
        .background(Capsule().fill(color.opacity(0.13)))
    }
}

struct ScheduledAmount: View {
    let item: ScheduledTransaction

    var body: some View {
        Text(Money.signed(item.amount, kind: item.transactionType))
            .font(.system(size: 13, weight: .semibold))
            .foregroundColor(item.transactionType == .income ? DmxColors.income : (item.transactionType == .transfer ? DmxColors.transfer : DmxColors.expense))
            .lineLimit(1)
    }
}

struct ScheduledCompactCard: View {
    @EnvironmentObject private var store: AppStore
    let row: ScheduledRow

    var body: some View {
        let item = row.scheduled
        let isTransfer = item.transactionType == .transfer
        return HStack(alignment: .top, spacing: 12) {
            IconBadge(icon: isTransfer ? "ArrowRightLeft" : row.category.icon, colorHex: isTransfer ? "#6366f1" : row.category.color, size: 40)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .top) {
                    Text(item.description).font(.system(size: 15, weight: .semibold)).lineLimit(1)
                    Spacer()
                    ScheduledAmount(item: item)
                }
                HStack(spacing: 6) {
                    Text(row.accountName).lineLimit(1)
                    Text("·")
                    CategoryChip(category: row.category, isTransfer: isTransfer)
                    Text("·")
                    Text(row.frequencyLabel).foregroundColor(.accentColor)
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                Divider()
                HStack {
                    HStack(spacing: 4) {
                        DmxIcon("Calendar", size: 12)
                        Text("Échéance : \(DayFormat.numeric(item.nextDate))")
                    }
                    Spacer()
                    if let end = item.endDate {
                        Text("Fin : \(DayFormat.numeric(end))")
                    }
                    if row.isEnded {
                        Pill("Terminé", color: DmxColors.expense)
                    }
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            }
            VStack(spacing: 6) {
                IconButton("Edit2") { store.present(.scheduled(id: item.id)) }
                IconButton("Trash2", color: DmxColors.expense) { deleteScheduled(item.id, store: store) }
            }
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(DmxPalette.cardBackground))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(row.isEnded ? DmxColors.expense.opacity(0.4) : DmxPalette.separator.opacity(0.5), lineWidth: 0.5))
        .opacity(row.isEnded ? 0.65 : 1)
    }
}

/// Suggestions d'échéances repérées dans le Journal (au moins deux mois consécutifs).
struct ScheduledSuggestionsView: View {
    @EnvironmentObject private var store: AppStore
    private let onClose: () -> Void

    init(onClose: @escaping () -> Void) {
        self.onClose = onClose
    }

    private var suggestions: [ScheduledSuggestion] {
        let query = ScheduledQuery(accounts: store.selectedAccountIds, dueRange: .all, search: "", categories: [], frequencies: [])
        let today = store.today
        return store.read { engine in try engine.scheduled(query: query, today: today).suggestions } ?? []
    }

    var body: some View {
        let items = suggestions
        return VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Suggestions du journal").font(.system(size: 17, weight: .semibold))
                Spacer()
                IconButton("X", action: onClose)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            Divider()
            ScrollView {
                VStack(spacing: 8) {
                    if items.isEmpty {
                        EmptyStateView(icon: "Sparkles", title: "Aucune suggestion pour le moment")
                    }
                    ForEach(items, id: \.key) { suggestion in
                        row(suggestion)
                    }
                }
                .padding(16)
            }
            .frame(minHeight: 200, maxHeight: 480)
        }
    }

    private func row(_ suggestion: ScheduledSuggestion) -> some View {
        let isTransfer = suggestion.transactionType == .transfer
        return HStack(spacing: 12) {
            IconBadge(icon: isTransfer ? "ArrowRightLeft" : suggestion.category.icon, colorHex: isTransfer ? "#6366f1" : suggestion.category.color, size: 40)
            VStack(alignment: .leading, spacing: 3) {
                Text(suggestion.description).font(.system(size: 13, weight: .semibold)).lineLimit(1)
                HStack(spacing: 6) {
                    Text(suggestion.accountName)
                    Text("•")
                    Text(periodicityLabel(frequency: suggestion.frequency))
                    Text("•")
                    Text(plural(Int(suggestion.occurrenceCount), "occurrence"))
                    Text("•")
                    Text("prochaine le \(DayFormat.short(suggestion.nextDate))")
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .lineLimit(1)
            }
            Spacer(minLength: 8)
            Text(Money.signed(suggestion.amount, kind: suggestion.transactionType))
                .font(.system(size: 13, weight: .semibold))
                .foregroundColor(suggestion.transactionType == .income ? DmxColors.income : (isTransfer ? DmxColors.transfer : DmxColors.expense))
            Button(action: { accept(suggestion) }) {
                HStack(spacing: 4) {
                    DmxIcon("Plus", size: 12)
                    Text("Ajouter")
                }
            }
            .buttonStyle(DmxButtonStyle(.secondary))
            Button(action: { dismiss(suggestion) }) {
                HStack(spacing: 4) {
                    DmxIcon("Trash2", size: 12)
                    Text("Supprimer")
                }
                .foregroundColor(DmxColors.expense)
            }
            .buttonStyle(PlainButtonStyle())
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.primary.opacity(0.04)))
    }

    private func accept(_ suggestion: ScheduledSuggestion) {
        let accounts = store.selectedAccountIds
        let today = store.today
        store.run("Suggestion ajoutée à l'échéancier") { engine in
            _ = try engine.acceptScheduledSuggestion(key: suggestion.key, accounts: accounts, today: today)
        }
    }

    private func dismiss(_ suggestion: ScheduledSuggestion) {
        store.run("Suggestion supprimée") { engine in try engine.dismissScheduledSuggestion(key: suggestion.key) }
    }
}
