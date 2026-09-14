import Charts
import DmxKit
import SwiftUI

/// Analyses : historique des soldes et répartitions, en Swift Charts.
struct ModernAnalytics: View {
    @ObservedObject var model: AnalyticsModel
    @EnvironmentObject private var store: AppStore
    @State private var hoverBalance: Int?
    @State private var hoverBars: String?

    private struct Point: Identifiable {
        let id: String
        let series: String
        let color: String
        let label: String
        let index: Int
        let value: Double
    }

    var body: some View {
        PageBody {
            if let query = model.query, let view = model.view {
                VStack(alignment: .leading, spacing: 16) {
                    rangeBar(query, view)
                    balanceCard(view)
                    HStack(alignment: .top, spacing: 16) {
                        donut("Dépenses par catégorie", slices: view.expensesByCategory, kind: .expense)
                        donut("Revenus par catégorie", slices: view.incomeByCategory, kind: .income)
                    }
                    barsCard(view)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, minHeight: 200)
            }
        }
    }

    /// Plage en menu déroulant, comme l'échéancier : un segmenté changeait de largeur d'une
    /// période à l'autre, et les options voisines se retrouvaient écrasées.
    private func rangeBar(_ query: AnalyticsQuery, _ view: AnalyticsView) -> some View {
        HStack(spacing: 12) {
            HStack(spacing: 5) {
                Image(systemName: "calendar")
                Picker("Période", selection: Binding(get: { query.range }, set: model.setRange)) {
                    ForEach(allTimeRanges(), id: \.self) { range in
                        Text(timeRangeLabel(range: range)).tag(range)
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .fixedSize()
            }
            if query.range == .custom {
                DatePicker("Du", selection: dateBinding(query.customStart ?? view.startDate, model.setCustomStart), displayedComponents: .date)
                    .fixedSize()
                DatePicker("au", selection: dateBinding(query.customEnd ?? view.endDate, model.setCustomEnd), displayedComponents: .date)
                    .fixedSize()
            } else {
                Text("\(DayFormat.medium(view.startDate)) → \(DayFormat.medium(view.endDate))")
                    .foregroundStyle(.secondary)
                    .fixedSize()
            }
            Spacer(minLength: 12)
            Toggle("Démarrer au 1er du mois", isOn: Binding(
                get: { query.monthStartsOnFirst },
                set: model.setMonthStartsOnFirst
            ))
            .toggleStyle(.checkbox)
            .fixedSize()
        }
    }

    private func dateBinding(_ value: String, _ setter: @escaping (String) -> Void) -> Binding<Date> {
        Binding(
            get: { DayValue.date(from: value) ?? Date() },
            set: { setter(DayValue.string(from: $0)) }
        )
    }

    // MARK: Évolution du solde

    private func balanceCard(_ view: AnalyticsView) -> some View {
        Card("Évolution du solde", systemImage: "chart.xyaxis.line") {
            let points = balancePoints(view)
            Chart {
                ForEach(points) { point in
                    LineMark(
                        x: .value("Date", point.index),
                        y: .value("Solde", point.value)
                    )
                    .foregroundStyle(by: .value("Compte", point.series))
                    .interpolationMethod(.monotone)
                }
                if let hoverBalance, view.balanceHistory.labels.indices.contains(hoverBalance) {
                    ChartHoverMark(index: hoverBalance, label: "Date") {
                        balanceBubble(view, at: hoverBalance)
                    }
                }
            }
            .chartForegroundStyleScale(
                domain: view.balanceHistory.series.map(\.name),
                range: colorRange(view.balanceHistory.series)
            )
            .chartXAxis {
                AxisMarks(values: axisIndices(view.balanceHistory.labels)) { value in
                    AxisGridLine()
                    AxisValueLabel {
                        if let index = value.as(Int.self), view.balanceHistory.labels.indices.contains(index) {
                            Text(view.balanceHistory.labels[index])
                        }
                    }
                }
            }
            .chartYAxis { AxisMarks(format: .currency(code: "EUR").precision(.fractionLength(0))) }
            .chartOverlay { proxy in
                ChartHoverArea(proxy: proxy, count: view.balanceHistory.labels.count, index: $hoverBalance)
            }
            .frame(height: 260)
        }
    }

    private func balancePoints(_ view: AnalyticsView) -> [Point] {
        view.balanceHistory.series.flatMap { series in
            series.values.enumerated().map { index, value in
                Point(
                    id: "\(series.id)-\(index)",
                    series: series.name,
                    color: series.color,
                    label: view.balanceHistory.labels.indices.contains(index) ? view.balanceHistory.labels[index] : "",
                    index: index,
                    value: value
                )
            }
        }
    }

    /// Soldes du point survolé, compte par compte.
    private func balanceBubble(_ view: AnalyticsView, at index: Int) -> some View {
        HoverBubble {
            Text(view.balanceHistory.labels[index]).fontWeight(.semibold)
            ForEach(view.balanceHistory.series, id: \.id) { series in
                if series.values.indices.contains(index) {
                    let value = series.values[index]
                    HoverRow(
                        color: Color(hex: series.color, fallback: .accentColor),
                        label: series.name,
                        value: Money.format(value),
                        valueColor: value < 0 ? .red : .primary
                    )
                }
            }
        }
    }

    /// Les couleurs des comptes viennent du noyau : on les donne à l'échelle du graphique.
    private func colorRange(_ series: [ChartSeries]) -> [Color] {
        series.map { Color(hex: $0.color, fallback: .accentColor) }
    }

    private func axisIndices(_ labels: [String]) -> [Int] {
        guard labels.count > 1 else { return [0] }
        let step = max(labels.count / 6, 1)
        return Array(stride(from: 0, to: labels.count, by: step))
    }

    // MARK: Répartitions

    private func donut(_ title: String, slices: [CategorySlice], kind: TransactionType) -> some View {
        let visible = slices.filter { !$0.hidden }
        let total = visible.reduce(0) { $0 + $1.value }
        return Card(title, systemImage: "chart.pie.fill") {
            VStack(spacing: 12) {
                Chart(visible, id: \.category.id) { slice in
                    SectorMark(
                        angle: .value("Montant", slice.value),
                        innerRadius: .ratio(0.62),
                        angularInset: 1.5
                    )
                    .foregroundStyle(Color(hex: slice.category.color, fallback: .accentColor))
                }
                .chartLegend(.hidden)
                .frame(height: 210)
                .overlay {
                    VStack(spacing: 0) {
                        Text("Total").font(.caption2).textCase(.uppercase).foregroundStyle(.secondary)
                        Text(Money.format(total)).font(.headline.monospacedDigit())
                    }
                }
                VStack(spacing: 6) {
                    ForEach(slices, id: \.category.id) { slice in
                        Button {
                            model.toggle(slice, kind: kind)
                        } label: {
                            HStack(spacing: 8) {
                                Circle()
                                    .fill(Color(hex: slice.category.color, fallback: .secondary))
                                    .frame(width: 8, height: 8)
                                    .opacity(slice.hidden ? 0.3 : 1)
                                Text(slice.category.name).lineLimit(1)
                                Spacer(minLength: 8)
                                Text("\(Int(slice.percentage.rounded())) %").monospacedDigit().foregroundStyle(.secondary)
                                Text(Money.format(slice.value)).monospacedDigit()
                            }
                            .font(.caption)
                            .opacity(slice.hidden ? 0.45 : 1)
                        }
                        .buttonStyle(.plain)
                        .help(slice.hidden ? "Afficher cette catégorie" : "Masquer cette catégorie")
                    }
                }
            }
        }
    }

    // MARK: Revenus et dépenses

    private func barsCard(_ view: AnalyticsView) -> some View {
        Card("Revenus et dépenses", systemImage: "chart.bar") {
            Chart {
                ForEach(view.incomeVsExpenses, id: \.label) { bar in
                    BarMark(x: .value("Période", bar.label), y: .value("Montant", bar.income))
                        .foregroundStyle(by: .value("Type", "Revenus"))
                        .position(by: .value("Type", "Revenus"))
                    BarMark(x: .value("Période", bar.label), y: .value("Montant", bar.expenses))
                        .foregroundStyle(by: .value("Type", "Dépenses"))
                        .position(by: .value("Type", "Dépenses"))
                }
                if let hoverBars, let bar = view.incomeVsExpenses.first(where: { $0.label == hoverBars }) {
                    RuleMark(x: .value("Période", bar.label))
                        .lineStyle(StrokeStyle(lineWidth: 1))
                        .foregroundStyle(.secondary.opacity(0.5))
                        .annotation(
                            position: .top,
                            spacing: 6,
                            overflowResolution: .init(x: .fit(to: .chart), y: .fit(to: .chart))
                        ) {
                            HoverBubble {
                                Text(bar.label).fontWeight(.semibold)
                                HoverRow(color: .green, label: "Revenus", value: Money.format(bar.income), valueColor: .green)
                                HoverRow(color: .red, label: "Dépenses", value: Money.format(bar.expenses), valueColor: .red)
                                HoverRow(
                                    color: .secondary,
                                    label: "Solde",
                                    value: Money.format(bar.income - bar.expenses),
                                    valueColor: bar.income - bar.expenses < 0 ? .red : .primary
                                )
                            }
                        }
                }
            }
            .chartForegroundStyleScale(["Revenus": Color.green, "Dépenses": Color.red])
            .chartYAxis { AxisMarks(format: .currency(code: "EUR").precision(.fractionLength(0))) }
            .chartXSelection(value: $hoverBars)
            .frame(height: 240)
        }
    }
}

/// Conversion entre les dates `YYYY-MM-DD` du noyau et `Date`, pour les `DatePicker` natifs.
enum DayValue {
    private static let formatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()

    static func date(from value: String) -> Date? {
        formatter.date(from: value)
    }

    static func string(from date: Date) -> String {
        formatter.string(from: date)
    }
}
