import SwiftUI

public final class AnalyticsModel: PageModel {
    public private(set) var query: AnalyticsQuery? = nil {
        willSet { objectWillChange.send() }
    }

    public private(set) var view: AnalyticsView? = nil {
        willSet { objectWillChange.send() }
    }

    public override init(store: AppStore) {
        super.init(store: store)
        refresh()
    }

    public override func refresh() {
        let accounts = store.selectedAccountIds
        let today = store.today
        guard let query = store.read({ engine in try engine.analyticsQuery(accounts: accounts) }) else { return }
        self.query = query
        view = store.read { engine in try engine.analytics(query: query, today: today) }
    }

    public func setRange(_ range: TimeRange) {
        store.apply(.setAnalyticsTimeRange(range: range))
    }

    public func setMonthStartsOnFirst(_ enabled: Bool) {
        store.apply(.setAnalyticsMonthStartsOnFirst(enabled: enabled))
    }

    public func setCustomStart(_ date: String) {
        store.apply(.setAnalyticsCustomStartDate(date: date))
    }

    public func setCustomEnd(_ date: String) {
        store.apply(.setAnalyticsCustomEndDate(date: date))
    }

    public func toggle(_ slice: CategorySlice, kind: TransactionType) {
        store.apply(.setAnalyticsCategoryHidden(kind: kind, categoryId: slice.category.id, hidden: !slice.hidden))
    }
}

/// Encadré d'inspection des graphiques.
public struct ChartTooltip: View {
    public struct Line: Identifiable {
        public let id = UUID()
        public let color: Color
        public let name: String
        public let value: String

        public init(color: Color, name: String, value: String) {
            self.color = color
            self.name = name
            self.value = value
        }
    }

    private let title: String
    private let lines: [Line]
    private let footer: String?

    public init(title: String, lines: [Line], footer: String? = nil) {
        self.title = title
        self.lines = lines
        self.footer = footer
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.system(size: 12, weight: .semibold))
            ForEach(lines) { line in
                HStack(spacing: 6) {
                    Circle().fill(line.color).frame(width: 7, height: 7)
                    Text(line.name).lineLimit(1)
                    Spacer(minLength: 8)
                    Text(line.value).fontWeight(.semibold)
                }
                .font(.system(size: 11))
            }
            if let footer = footer {
                Text(footer).font(.system(size: 10)).foregroundColor(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(DmxPalette.cardBackground).shadow(color: Color.black.opacity(0.15), radius: 10, y: 3))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(DmxPalette.separator.opacity(0.6), lineWidth: 0.5))
    }
}

/// Sélecteur de période (boutons sur grand écran, liste déroulante sur iPhone).
struct TimeRangeSelector: View {
    @Environment(\.dmxCompact) private var compact
    let selection: TimeRange
    let onSelect: (TimeRange) -> Void

    var body: some View {
        if compact {
            ChoicePicker(
                options: allTimeRanges().map { SelectOption(id: String(describing: $0), label: timeRangeLabel(range: $0)) },
                selection: Binding(get: { String(describing: selection) }, set: { key in
                    if let range = allTimeRanges().first(where: { String(describing: $0) == key }) { onSelect(range) }
                })
            )
        } else {
            HStack(spacing: 4) {
                ForEach(allTimeRanges(), id: \.self) { range in
                    let isSelected = selection == range
                    Button(action: { onSelect(range) }) {
                        Text(timeRangeLabel(range: range))
                            .font(.system(size: 12, weight: .medium))
                            .foregroundColor(isSelected ? .accentColor : .secondary)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(isSelected ? Color.accentColor.opacity(0.14) : Color.clear))
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(PlainButtonStyle())
                }
            }
        }
    }
}

struct CheckboxLabel: View {
    let title: String
    @Binding var isOn: Bool

    var body: some View {
        Button(action: { isOn.toggle() }) {
            HStack(spacing: 8) {
                DmxIcon(isOn ? "CheckCircle2" : "Circle", size: 15).foregroundColor(isOn ? .accentColor : .secondary)
                Text(title).font(.system(size: 12)).foregroundColor(.primary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(DmxPalette.separator, lineWidth: 0.5))
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
    }
}

/// Analyses financières : historique des soldes, répartition par catégorie, revenus contre dépenses.
public struct AnalyticsPage: View {
    @ObservedObject private var model: AnalyticsModel
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact

    public init(model: AnalyticsModel) {
        self.model = model
    }

    public var body: some View {
        PageScroll {
            if let query = model.query, let view = model.view {
                if compact {
                    TimeRangeSelector(selection: query.range, onSelect: model.setRange)
                } else {
                    HStack {
                        Text("Analyses Financières").font(.system(size: 24, weight: .bold))
                        Spacer()
                        TimeRangeSelector(selection: query.range, onSelect: model.setRange)
                    }
                }

                if query.range == .custom {
                    HStack(spacing: 16) {
                        if !compact { Spacer() }
                        FormField("Du") {
                            DayPicker(day: Binding(get: { query.customStart ?? view.startDate }, set: model.setCustomStart))
                        }
                        .fixedSize(horizontal: true, vertical: false)
                        FormField("Au") {
                            DayPicker(day: Binding(get: { query.customEnd ?? view.endDate }, set: model.setCustomEnd))
                        }
                        .fixedSize(horizontal: true, vertical: false)
                    }
                }

                balanceCard(query, view)

                if compact {
                    VStack(spacing: 16) {
                        donutCard("Dépenses par Catégorie", slices: view.expensesByCategory, kind: .expense)
                        donutCard("Revenus par Catégorie", slices: view.incomeByCategory, kind: .income)
                    }
                } else {
                    HStack(alignment: .top, spacing: 20) {
                        donutCard("Dépenses par Catégorie", slices: view.expensesByCategory, kind: .expense)
                            .frame(maxHeight: .infinity, alignment: .top)
                        donutCard("Revenus par Catégorie", slices: view.incomeByCategory, kind: .income)
                            .frame(maxHeight: .infinity, alignment: .top)
                    }
                    .fixedSize(horizontal: false, vertical: true)
                }

                barsCard(view)
            }
        }
    }

    private func balanceCard(_ query: AnalyticsQuery, _ view: AnalyticsView) -> some View {
        let history = view.balanceHistory
        return DmxCard {
            VStack(alignment: .leading, spacing: 14) {
                HStack {
                    Text("Évolution du Solde").font(.system(size: 16, weight: .semibold))
                    Spacer()
                    if query.range != .custom {
                        CheckboxLabel(title: "Démarrer au 1er du mois", isOn: Binding(get: { query.monthStartsOnFirst }, set: model.setMonthStartsOnFirst))
                    }
                }
                LineChart(
                    lines: history.series.map { ChartLine(id: $0.id, name: $0.name, color: Color(hex: $0.color, fallback: DmxColors.transfer), values: $0.values) },
                    labels: history.labels,
                    tooltip: { index in
                        AnyView(ChartTooltip(
                            title: index < history.fullLabels.count ? history.fullLabels[index] : "",
                            lines: history.series.map { series in
                                ChartTooltip.Line(
                                    color: Color(hex: series.color, fallback: DmxColors.transfer),
                                    name: series.name,
                                    value: index < series.values.count ? Money.format(series.values[index]) : ""
                                )
                            }
                        ))
                    }
                )
                .frame(height: compact ? 240 : 320)
            }
        }
    }

    private func donutCard(_ title: String, slices: [CategorySlice], kind: TransactionType) -> some View {
        let hasVisible = slices.contains { !$0.hidden && $0.value > 0 }
        return DmxCard(title: title) {
            VStack(spacing: 16) {
                if hasVisible {
                    DonutChart(
                        slices: slices.map { DonutSlice(id: $0.category.id, label: $0.category.name, value: $0.value, color: Color(hex: $0.category.color, fallback: DmxColors.muted), hidden: $0.hidden) },
                        thickness: 34
                    ) {
                        VStack(spacing: 2) {
                            SectionLabel("Total")
                            Text(Money.rounded(slices.filter { !$0.hidden }.reduce(0) { $0 + $1.value }))
                                .font(.system(size: 15, weight: .bold))
                        }
                    }
                    .frame(height: 210)
                } else {
                    Text("Aucune donnée à afficher")
                        .font(.system(size: 13))
                        .foregroundColor(.secondary)
                        .frame(maxWidth: .infinity, minHeight: 210)
                }
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(slices.chunked(compact ? 1 : 2).enumerated()), id: \.offset) { row in
                        HStack(spacing: 12) {
                            ForEach(row.element, id: \.category.id) { slice in
                                legendItem(slice, kind: kind)
                            }
                            if row.element.count == 1 && !compact {
                                Color.clear.frame(maxWidth: .infinity, maxHeight: 1)
                            }
                        }
                    }
                }
            }
        }
    }

    private func legendItem(_ slice: CategorySlice, kind: TransactionType) -> some View {
        Button(action: { model.toggle(slice, kind: kind) }) {
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(slice.hidden ? DmxColors.muted : Color(hex: slice.category.color, fallback: DmxColors.muted))
                    .frame(width: 10, height: 10)
                Text(slice.category.name)
                    .strikethrough(slice.hidden)
                    .foregroundColor(slice.hidden ? .secondary : .primary)
                    .lineLimit(1)
                Spacer(minLength: 4)
                if !slice.hidden {
                    Text("\(Int(slice.percentage.rounded())) %").foregroundColor(.secondary)
                }
                Text(Money.format(slice.value)).fontWeight(.medium).foregroundColor(slice.hidden ? .secondary : .primary)
            }
            .font(.system(size: 12))
            .frame(maxWidth: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
    }

    private func barsCard(_ view: AnalyticsView) -> some View {
        let bars = view.incomeVsExpenses
        return DmxCard(title: "Revenus vs Dépenses") {
            BarChart(
                groups: bars.enumerated().map { BarGroup(id: $0.offset, label: $0.element.label, values: [$0.element.income, $0.element.expenses]) },
                colors: [DmxColors.income, DmxColors.expense],
                names: ["Revenus", "Dépenses"],
                tooltip: { index in
                    AnyView(ChartTooltip(title: bars[index].label, lines: [
                        ChartTooltip.Line(color: DmxColors.income, name: "Revenus", value: Money.format(bars[index].income)),
                        ChartTooltip.Line(color: DmxColors.expense, name: "Dépenses", value: Money.format(bars[index].expenses)),
                    ]))
                }
            )
            .frame(height: compact ? 240 : 320)
        }
    }
}
