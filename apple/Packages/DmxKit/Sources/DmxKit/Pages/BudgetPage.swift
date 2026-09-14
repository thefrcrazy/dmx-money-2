import SwiftUI

func plural(_ count: Int, _ singular: String, _ pluralForm: String? = nil) -> String {
    "\(count) " + (count > 1 ? (pluralForm ?? singular + "s") : singular)
}

public final class BudgetModel: PageModel {
    public var search = "" {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var categories: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public private(set) var view: BudgetOverview? = nil {
        willSet { objectWillChange.send() }
    }

    public override init(store: AppStore) {
        super.init(store: store)
        refresh()
    }

    public override func refresh() {
        let query = BudgetQuery(accounts: store.selectedAccountIds, search: search, categories: categories)
        let today = store.today
        view = store.read { engine in try engine.budget(query: query, today: today) }
    }
}

/// Budget du mois : indicateurs, consommation, enveloppes par catégorie et suggestions.
public struct BudgetPage: View {
    @ObservedObject private var model: BudgetModel
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact

    public init(model: BudgetModel) {
        self.model = model
    }

    public var body: some View {
        PageScroll {
            if let view = model.view {
                PageHeader("Budget", subtitle: "\(view.monthLabel) · budgets configurés et dépenses du Journal") {
                    HStack(spacing: 8) {
                        if !view.suggestions.isEmpty {
                            Button(action: { store.present(.budgetSuggestions) }) {
                                HStack(spacing: 6) {
                                    DmxIcon("Sparkles", size: 13)
                                    Text("Suggestions (\(view.suggestions.count))")
                                }
                            }
                            .buttonStyle(DmxButtonStyle(.secondary))
                        }
                        Button(action: { store.present(.budget(id: nil)) }) {
                            HStack(spacing: 6) {
                                DmxIcon("Plus", size: 13)
                                Text("Nouveau budget")
                            }
                        }
                        .buttonStyle(DmxButtonStyle(.primary))
                    }
                }
                kpis(view)
                consumption(view)
                categoriesCard(view)
            }
        }
    }

    // MARK: Indicateurs

    private func kpis(_ view: BudgetOverview) -> some View {
        let stateColor: Color
        let stateIcon: String
        switch view.state {
        case .toConfigure:
            stateColor = .secondary
            stateIcon = "CalendarClock"
        case .underControl:
            stateColor = DmxColors.income
            stateIcon = "CheckCircle2"
        case .overrun:
            stateColor = DmxColors.expense
            stateIcon = "AlertCircle"
        }
        let pace = view.paceDelta > 0
            ? "\(Money.rounded(view.paceDelta)) au-dessus du rythme"
            : "\(Money.rounded(abs(view.paceDelta))) sous le rythme"

        let tiles = [
            AnyView(kpi("Prévu", icon: "CalendarClock", iconColor: .accentColor, value: Money.rounded(view.totalBudgeted), valueColor: .primary,
                        caption: "\(plural(Int(view.budgetCount), "budget")) \(view.budgetCount > 1 ? "configurés" : "configuré")")),
            AnyView(kpi("Dépensé", icon: "TrendingDown", iconColor: DmxColors.expense, value: Money.rounded(view.totalSpent), valueColor: .primary,
                        caption: "\(plural(Int(view.expenseCount), "dépense")) ce mois-ci")),
            AnyView(kpi("Restant", icon: "Wallet", iconColor: view.remaining >= 0 ? DmxColors.income : DmxColors.expense,
                        value: Money.rounded(view.remaining), valueColor: view.remaining >= 0 ? DmxColors.income : DmxColors.expense,
                        caption: "\(Money.rounded(view.remainingPerDay)) / jour restant")),
            AnyView(kpi("État", icon: stateIcon, iconColor: stateColor, value: budgetStateLabel(state: view.state), valueColor: stateColor, caption: pace)),
        ]
        return VStack(spacing: 16) {
            ForEach(Array(tiles.chunked(compact ? 2 : 4).enumerated()), id: \.offset) { row in
                HStack(alignment: .top, spacing: 16) {
                    ForEach(0..<row.element.count, id: \.self) { index in
                        row.element[index].frame(maxHeight: .infinity, alignment: .top)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func kpi(_ label: String, icon: String, iconColor: Color, value: String, valueColor: Color, caption: String) -> some View {
        DmxCard {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    SectionLabel(label)
                    Spacer()
                    DmxIcon(icon, size: 15).foregroundColor(iconColor)
                }
                Text(value)
                    .font(.system(size: compact ? 18 : 22, weight: .bold))
                    .foregroundColor(valueColor)
                    .lineLimit(1)
                    .minimumScaleFactor(0.6)
                Text(caption).font(.system(size: 11)).foregroundColor(.secondary).lineLimit(2)
            }
        }
    }

    // MARK: Consommation

    private func consumption(_ view: BudgetOverview) -> some View {
        DmxCard {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .bottom) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Consommation du mois").font(.system(size: 16, weight: .semibold))
                        Text("Le budget suit les enveloppes configurées et les dépenses du Journal.")
                            .font(.system(size: 12)).foregroundColor(.secondary)
                    }
                    Spacer()
                    (Text("\(Int(view.progress.rounded()))%").bold() + Text(" utilisé"))
                        .font(.system(size: 13))
                        .foregroundColor(.secondary)
                }
                ProgressBar(progress: view.progress, color: view.progress > 100 ? DmxColors.expense : DmxColors.income, height: 12)
                consumptionStats(view)
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
        }
    }

    /// Détail de la consommation : en ligne sur grand écran, empilé sur iPhone.
    @ViewBuilder
    private func consumptionStats(_ view: BudgetOverview) -> some View {
        let stats: [(String, Color)] = {
            var values: [(String, Color)] = [
                ("\(Money.format(view.totalSpent)) dépensés", .secondary),
                ("\(Money.format(view.totalBudgeted)) prévus", .secondary),
                ("\(Money.format(view.expectedSpend)) théoriques au \(view.todayLabel)", .secondary),
            ]
            if view.overBudgetCount > 0 {
                values.append((plural(Int(view.overBudgetCount), "dépassement"), DmxColors.expense))
            }
            if view.unbudgetedCount > 0 {
                let label = "\(plural(Int(view.unbudgetedCount), "catégorie")) non \(view.unbudgetedCount > 1 ? "budgétées" : "budgétée")"
                values.append((label, .accentColor))
            }
            return values
        }()

        if compact {
            VStack(alignment: .leading, spacing: 3) {
                ForEach(Array(stats.enumerated()), id: \.offset) { item in
                    Text(item.element.0).foregroundColor(item.element.1)
                }
            }
        } else {
            HStack(spacing: 18) {
                ForEach(Array(stats.enumerated()), id: \.offset) { item in
                    Text(item.element.0).foregroundColor(item.element.1)
                }
                Spacer(minLength: 0)
            }
        }
    }

    // MARK: Budget par catégorie

    private func categoriesCard(_ view: BudgetOverview) -> some View {
        DmxCard(padded: false) {
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Budget par catégorie").font(.system(size: 16, weight: .semibold))
                        Text("Catégories configurées, enveloppes et suivi détaillé.").font(.system(size: 11)).foregroundColor(.secondary)
                    }
                    Spacer()
                    Text(categoryCountLabel(view)).font(.system(size: 11, weight: .semibold)).foregroundColor(.secondary)
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 14)
                Divider()
                HStack(spacing: 10) {
                    SearchField("Rechercher un budget...", text: $model.search).frame(maxWidth: compact ? .infinity : 360)
                    MultiSelectButton("Toutes les catégories", options: store.categoryOptions, selection: $model.categories)
                    if !compact { Spacer() }
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 10)
                Divider()
                if view.categories.isEmpty {
                    emptyState(view)
                } else {
                    ForEach(Array(view.categories.enumerated()), id: \.element.category.id) { item in
                        if item.offset > 0 { Divider() }
                        categoryRow(item.element)
                    }
                }
            }
        }
    }

    private func categoryCountLabel(_ view: BudgetOverview) -> String {
        let total = Int(view.budgetedCategoryCount)
        let visible = view.categories.filter { !$0.isUnbudgeted }.count
        let prefix = visible != total ? "\(visible) / \(total)" : "\(total)"
        return "\(prefix) \(total > 1 ? "catégories budgétées" : "catégorie budgétée")"
    }

    private func emptyState(_ view: BudgetOverview) -> some View {
        VStack(spacing: 6) {
            if view.budgetedCategoryCount == 0 {
                EmptyStateView(icon: "Wallet", title: "Aucun budget configuré", message: "Crée une enveloppe mensuelle, même sans échéance liée.")
                Button(action: { store.present(.budget(id: nil)) }) {
                    HStack(spacing: 6) {
                        DmxIcon("Plus", size: 13)
                        Text("Nouveau budget")
                    }
                }
                .buttonStyle(DmxButtonStyle(.secondary))
            } else {
                EmptyStateView(icon: "Search", title: "Aucun budget ne correspond aux filtres.", message: "Modifie la recherche ou les catégories sélectionnées.")
            }
        }
        .padding(.vertical, 20)
    }

    private func categoryRow(_ row: BudgetCategoryRow) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                IconBadge(icon: row.category.icon, colorHex: row.category.color, size: 36)
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.category.name).font(.system(size: 14, weight: .semibold)).lineLimit(1)
                    Text(row.isUnbudgeted
                         ? "\(Money.format(row.spent)) dépensés sans budget ce mois-ci"
                         : "\(plural(row.envelopes.count, "enveloppe")) · \(plural(Int(row.linkedScheduledCount), "échéance")) \(row.linkedScheduledCount > 1 ? "liées" : "liée")")
                        .font(.system(size: 11)).foregroundColor(.secondary)
                }
                Spacer()
                if row.isUnbudgeted {
                    Button(action: { store.present(.newBudget(categoryId: row.category.id)) }) {
                        HStack(spacing: 4) {
                            DmxIcon("Plus", size: 12)
                            Text("Créer un budget")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(.subtle))
                } else if row.isOverBudget {
                    status("Dépassé", icon: "AlertCircle", color: DmxColors.expense)
                } else {
                    status("OK", icon: "CheckCircle2", color: DmxColors.income)
                }
            }
            if !row.envelopes.isEmpty {
                VStack(spacing: 0) {
                    if !compact {
                        HStack(spacing: 12) {
                            SectionLabel("Budget").frame(maxWidth: .infinity, alignment: .leading)
                            SectionLabel("Prévu").frame(width: 100, alignment: .leading)
                            SectionLabel("Dépensé").frame(width: 100, alignment: .leading)
                            SectionLabel("Restant").frame(width: 100, alignment: .leading)
                            SectionLabel("Actions").frame(width: 64, alignment: .trailing)
                        }
                        .padding(.horizontal, 14)
                        .padding(.vertical, 8)
                        .background(Color.primary.opacity(0.03))
                    }
                    ForEach(Array(row.envelopes.enumerated()), id: \.element.budget.id) { item in
                        if item.offset > 0 || !compact { Divider() }
                        envelopeRow(item.element)
                    }
                }
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(DmxPalette.separator.opacity(0.6), lineWidth: 0.5))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
    }

    private func status(_ label: String, icon: String, color: Color) -> some View {
        HStack(spacing: 4) {
            DmxIcon(icon, size: 13)
            Text(label).font(.system(size: 11, weight: .semibold))
        }
        .foregroundColor(color)
    }

    private func envelopeRow(_ envelope: BudgetEnvelope) -> some View {
        let isOver = envelope.remaining < 0
        return VStack(alignment: .leading, spacing: 8) {
            if compact {
                HStack {
                    envelopeTitle(envelope)
                    Spacer()
                    envelopeActions(envelope)
                }
                HStack {
                    figure("Prévu", Money.format(envelope.budget.amount), .primary)
                    figure("Dépensé", Money.format(envelope.spent), .primary)
                    figure("Restant", Money.format(envelope.remaining), isOver ? DmxColors.expense : DmxColors.income)
                }
            } else {
                HStack(spacing: 12) {
                    envelopeTitle(envelope).frame(maxWidth: .infinity, alignment: .leading)
                    Text(Money.format(envelope.budget.amount)).font(.system(size: 13, weight: .semibold)).frame(width: 100, alignment: .leading)
                    Text(Money.format(envelope.spent)).font(.system(size: 13, weight: .semibold)).frame(width: 100, alignment: .leading)
                    Text(Money.format(envelope.remaining)).font(.system(size: 13, weight: .semibold))
                        .foregroundColor(isOver ? DmxColors.expense : DmxColors.income)
                        .frame(width: 100, alignment: .leading)
                    envelopeActions(envelope).frame(width: 64, alignment: .trailing)
                }
            }
            ProgressBar(progress: envelope.progress, color: isOver ? DmxColors.expense : DmxColors.income)
            HStack {
                Text("\(Int(envelope.progress.rounded()))% utilisé")
                Spacer()
                if isOver {
                    Text("\(Money.format(abs(envelope.remaining))) au-dessus").foregroundColor(DmxColors.expense)
                }
            }
            .font(.system(size: 11))
            .foregroundColor(.secondary)
            if !envelope.linkedScheduled.isEmpty {
                Divider()
                linkedScheduled(envelope.linkedScheduled)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private func envelopeTitle(_ envelope: BudgetEnvelope) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(envelope.budget.name).font(.system(size: 13, weight: .medium)).lineLimit(1)
            Text(envelope.accountName).font(.system(size: 11)).foregroundColor(.secondary).lineLimit(1)
        }
    }

    private func envelopeActions(_ envelope: BudgetEnvelope) -> some View {
        HStack(spacing: 0) {
            IconButton("Edit2") { store.present(.budget(id: envelope.budget.id)) }
            IconButton("Trash2", color: DmxColors.expense) { deleteBudget(envelope.budget.id) }
        }
    }

    private func figure(_ label: String, _ value: String, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(.system(size: 10)).foregroundColor(.secondary)
            Text(value).font(.system(size: 13, weight: .semibold)).foregroundColor(color).lineLimit(1).minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func linkedScheduled(_ items: [LinkedScheduled]) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 4) {
                DmxIcon("CalendarClock", size: 12).foregroundColor(.accentColor)
                Text("Échéances").font(.system(size: 11, weight: .medium))
            }
            ForEach(items, id: \.scheduledId) { item in
                HStack(spacing: 6) {
                    Text(item.description).lineLimit(1)
                    Text(Money.signed(item.amount, kind: item.transactionType))
                        .foregroundColor(item.transactionType == .income ? DmxColors.income : (item.transactionType == .transfer ? DmxColors.transfer : DmxColors.expense))
                    Text("·")
                    Text(DayFormat.short(item.nextDate))
                    Spacer(minLength: 0)
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            }
        }
    }

    private func deleteBudget(_ id: String) {
        store.confirm(title: "Supprimer le budget", message: "Ce budget sera supprimé et les échéances liées seront simplement déliées.") { [store] in
            store.run("Budget supprimé") { engine in try engine.deleteBudget(id: id) }
        }
    }
}

/// Suggestions de budgets déduites du Journal (6 derniers mois).
struct BudgetSuggestionsView: View {
    @EnvironmentObject private var store: AppStore
    private let onClose: () -> Void

    init(onClose: @escaping () -> Void) {
        self.onClose = onClose
    }

    private var suggestions: [BudgetSuggestion] {
        let query = BudgetQuery(accounts: store.selectedAccountIds, search: "", categories: [])
        let today = store.today
        return store.read { engine in try engine.budget(query: query, today: today).suggestions } ?? []
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

    private func row(_ suggestion: BudgetSuggestion) -> some View {
        HStack(spacing: 12) {
            IconBadge(icon: suggestion.category.icon, colorHex: suggestion.category.color, size: 40)
            VStack(alignment: .leading, spacing: 3) {
                Text(suggestion.name).font(.system(size: 13, weight: .semibold)).lineLimit(1)
                HStack(spacing: 6) {
                    Text(suggestion.accountName)
                    Text("•")
                    Text("\(suggestion.monthCount) mois \(suggestion.monthCount > 1 ? "observés" : "observé")")
                    if suggestion.currentMonthSpent > 0 {
                        Text("•")
                        Text("\(Money.format(suggestion.currentMonthSpent)) ce mois-ci")
                    }
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .lineLimit(1)
            }
            Spacer(minLength: 8)
            Text(Money.format(suggestion.amount)).font(.system(size: 13, weight: .semibold)).foregroundColor(.accentColor)
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

    private func accept(_ suggestion: BudgetSuggestion) {
        let accounts = store.selectedAccountIds
        let today = store.today
        store.run("Budget ajouté") { engine in _ = try engine.acceptBudgetSuggestion(key: suggestion.key, accounts: accounts, today: today) }
    }

    private func dismiss(_ suggestion: BudgetSuggestion) {
        store.run("Suggestion supprimée") { engine in try engine.dismissBudgetSuggestion(key: suggestion.key) }
    }
}
