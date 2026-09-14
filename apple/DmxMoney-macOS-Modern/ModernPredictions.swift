import Charts
import DmxKit
import SwiftUI

/// Prédictions : transactions fictives, projection en escalier (Swift Charts), jours à surveiller.
struct ModernPredictions: View {
    @ObservedObject var model: PredictionsModel
    @EnvironmentObject private var store: AppStore
    @State private var hover: Int?

    private struct Step: Identifiable {
        let id: String
        let series: String
        let index: Int
        let label: String
        let value: Double
        var isLow = false
    }

    var body: some View {
        PageBody {
            if let query = model.query, let view = model.view {
                VStack(alignment: .leading, spacing: 16) {
                    rangeBar(query, view)
                    fakeTransactions(view)
                    projection(view)
                    if view.intradayRiskCount > 0 {
                        riskDays(view)
                    }
                    totals(view)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, minHeight: 200)
            }
        }
    }

    /// Plage en menu déroulant, comme l'échéancier : un segmenté changeait de largeur d'une
    /// période à l'autre et écrasait les libellés voisins.
    private func rangeBar(_ query: PredictionQuery, _ view: PredictionView) -> some View {
        HStack(spacing: 12) {
            HStack(spacing: 5) {
                Image(systemName: "calendar.badge.clock")
                Picker("Horizon", selection: Binding(get: { query.range }, set: model.setRange)) {
                    ForEach(allTimeRanges(), id: \.self) { range in
                        Text(timeRangeLabel(range: range)).tag(range)
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .fixedSize()
            }
            if query.range == .custom {
                DatePicker("Jusqu'au", selection: Binding(
                    get: { DayValue.date(from: query.customEndDate ?? view.endDate) ?? Date() },
                    set: { model.setCustomEnd(DayValue.string(from: $0)) }
                ), displayedComponents: .date)
                .fixedSize()
            } else {
                Text("Jusqu'au \(view.endLabel)")
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
            Toggle("Point bas journalier", isOn: $model.showIntradayLow)
                .toggleStyle(.checkbox)
                .fixedSize()
        }
    }

    // MARK: Transactions fictives

    private func fakeTransactions(_ view: PredictionView) -> some View {
        Card("Transactions fictives", systemImage: "wand.and.sparkles", trailing: {
            HStack(spacing: 8) {
                if !view.fakeTransactions.isEmpty {
                    Button("Tout retirer", systemImage: "trash") { model.clearAll() }
                }
                Button("Ajouter", systemImage: "plus") { store.present(.fakeTransaction(id: nil)) }
                    .disabled(store.accounts.isEmpty)
            }
        }) {
            if view.fakeTransactions.isEmpty {
                Text("Elles modifient uniquement cette projection et ne sont pas ajoutées au journal.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("\(view.enabledFakeCount)/\(view.fakeTransactions.count) simulation\(view.fakeTransactions.count > 1 ? "s" : "") active\(view.enabledFakeCount > 1 ? "s" : "")")
                        Spacer()
                        Text("Impact période : \(view.fakeImpact >= 0 ? "+" : "")\(Money.format(view.fakeImpact))")
                            .foregroundStyle(view.fakeImpact >= 0 ? .green : .red)
                            .fontWeight(.semibold)
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    ForEach(view.fakeTransactions, id: \.transaction.id) { row in
                        fakeRow(row)
                    }
                }
            }
        }
    }

    private func fakeRow(_ row: FakeTransactionRow) -> some View {
        HStack(spacing: 10) {
            Toggle("", isOn: Binding(
                get: { row.transaction.enabled },
                set: { _ in model.toggle(row.transaction.id) }
            ))
            .toggleStyle(.checkbox)
            .labelsHidden()
            VStack(alignment: .leading, spacing: 1) {
                Text(row.transaction.description).fontWeight(.medium).lineLimit(1)
                Text("\(DayFormat.medium(row.transaction.date)) • \(row.sourceAccountName)\(row.categoryName.map { " • \($0)" } ?? "")")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 8)
            MoneyText(amount: row.transaction.amount, signed: row.transaction.transactionType)
            Button { store.present(.fakeTransaction(id: row.transaction.id)) } label: {
                Image(systemName: "pencil")
            }
            .buttonStyle(.plain)
            Button { model.remove(row.transaction.id) } label: {
                Image(systemName: "trash")
            }
            .buttonStyle(.plain)
            .foregroundStyle(.red)
        }
        .opacity(row.transaction.enabled ? 1 : 0.6)
        .padding(8)
        .background(Color(nsColor: .textBackgroundColor).opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
    }

    // MARK: Projection

    private func projection(_ view: PredictionView) -> some View {
        Card(view.titleLabel, systemImage: "chart.line.uptrend.xyaxis") {
            Chart {
                ForEach(steps(view)) { step in
                    LineMark(
                        x: .value("Jour", step.index),
                        y: .value("Solde", step.value),
                        series: .value("Compte", step.series)
                    )
                    .interpolationMethod(.stepEnd)
                    .foregroundStyle(by: .value("Compte", step.series))
                    .lineStyle(StrokeStyle(lineWidth: step.isLow ? 1 : 2, dash: step.isLow ? [3, 3] : []))
                }
                if let hover, view.labels.indices.contains(hover) {
                    ChartHoverMark(index: hover) {
                        bubble(view, at: hover)
                    }
                }
                RuleMark(y: .value("Zéro", 0))
                    .lineStyle(StrokeStyle(lineWidth: 1, dash: [4, 3]))
                    .foregroundStyle(.red.opacity(0.7))
                if view.alertThreshold > 0 {
                    RuleMark(y: .value("Seuil d'alerte", view.alertThreshold))
                        .lineStyle(StrokeStyle(lineWidth: 1, dash: [4, 3]))
                        .foregroundStyle(.orange.opacity(0.8))
                }
                ForEach(view.markers, id: \.date) { marker in
                    RuleMark(x: .value("Jour", Int(marker.index)))
                        .lineStyle(StrokeStyle(lineWidth: 1, dash: [2, 3]))
                        .foregroundStyle(Color(hex: marker.strokeColor, fallback: .red).opacity(0.6))
                }
            }
            .chartForegroundStyleScale(domain: seriesDomain(view), range: seriesColors(view))
            .chartXAxis {
                AxisMarks(values: axisIndices(view.labels)) { value in
                    AxisGridLine()
                    AxisValueLabel {
                        if let index = value.as(Int.self), view.labels.indices.contains(index) {
                            Text(view.labels[index])
                        }
                    }
                }
            }
            .chartYAxis { AxisMarks(format: .currency(code: "EUR").precision(.fractionLength(0))) }
            .chartOverlay { proxy in
                ChartHoverArea(proxy: proxy, count: view.labels.count, index: $hover)
            }
            .frame(height: 300)
            markerLegend
        }
    }

    /// Signification des marqueurs, comme en 1.x.
    private var markerLegend: some View {
        HStack(spacing: 16) {
            ForEach(Self.legend, id: \.1) { item in
                HStack(spacing: 5) {
                    Capsule()
                        .fill(Color(hex: item.0, fallback: .secondary))
                        .frame(width: 14, height: 3)
                    Text(item.1)
                }
            }
            Spacer(minLength: 0)
        }
        .font(.caption2)
        .foregroundStyle(.secondary)
    }

    private static let legend: [(String, String)] = [
        ("#ef4444", "Solde négatif en fin de journée"),
        ("#f97316", "Passage sous le seuil d'alerte"),
        ("#a855f7", "Point bas négatif, rattrapé par un revenu"),
        ("#eab308", "Point bas sous le seuil, rattrapé par un revenu"),
    ]

    /// Soldes du jour survolé : une ligne par compte, le point bas en dessous du montant.
    private func bubble(_ view: PredictionView, at index: Int) -> some View {
        HoverBubble {
            Text(view.labels[index]).fontWeight(.semibold)
            ForEach(view.accounts, id: \.id) { series in
                if series.closes.indices.contains(index) {
                    let close = series.closes[index]
                    let low = model.showIntradayLow && series.lows.indices.contains(index) ? series.lows[index] : nil
                    HoverRow(
                        color: Color(hex: series.color, fallback: .accentColor),
                        label: series.name,
                        value: Money.format(close),
                        valueColor: projectionColor(close, threshold: view.alertThreshold),
                        detail: low.map { "point bas \(Money.format($0))" },
                        detailColor: low.map { projectionColor($0, threshold: view.alertThreshold) } ?? .secondary
                    )
                }
            }
        }
    }

    /// Séries déclarées dans l'échelle de couleurs : toute série absente du domaine ferait
    /// échouer Swift Charts (l'application s'arrêtait en activant le point bas journalier).
    private func seriesDomain(_ view: PredictionView) -> [String] {
        var names = view.accounts.map(\.name)
        if model.showIntradayLow {
            names += view.accounts.map { lowSeriesName($0.name) }
        }
        return names
    }

    private func seriesColors(_ view: PredictionView) -> [Color] {
        var colors = view.accounts.map { Color(hex: $0.color, fallback: .accentColor) }
        if model.showIntradayLow {
            colors += view.accounts.map { Color(hex: $0.color, fallback: .accentColor).opacity(0.5) }
        }
        return colors
    }

    private func lowSeriesName(_ account: String) -> String { "\(account) — point bas" }

    private func steps(_ view: PredictionView) -> [Step] {
        var steps: [Step] = view.accounts.flatMap { series in
            series.closes.enumerated().map { index, value in
                Step(id: "\(series.id)-\(index)", series: series.name, index: index,
                     label: view.labels.indices.contains(index) ? view.labels[index] : "", value: value)
            }
        }
        if model.showIntradayLow {
            steps += view.accounts.flatMap { series in
                series.lows.enumerated().map { index, value in
                    Step(id: "\(series.id)-low-\(index)", series: lowSeriesName(series.name), index: index,
                         label: "", value: value, isLow: true)
                }
            }
        }
        return steps
    }

    private func axisIndices(_ labels: [String]) -> [Int] {
        guard labels.count > 1 else { return [0] }
        let step = max(labels.count / 8, 1)
        return Array(stride(from: 0, to: labels.count, by: step))
    }

    // MARK: Jours à surveiller

    private func riskDays(_ view: PredictionView) -> some View {
        Card("Jours à surveiller (\(view.intradayRiskCount))", systemImage: "exclamationmark.triangle") {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(view.markers.filter { $0.intradaySeverity != nil }, id: \.date) { marker in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(marker.fullLabel).fontWeight(.medium)
                            Spacer()
                            Text(marker.intradayNames.joined(separator: ", "))
                                .font(.caption)
                                .foregroundStyle(marker.intradaySeverity == .danger ? .red : .orange)
                        }
                        ForEach(marker.intradayBalances, id: \.name) { balance in
                            HStack(spacing: 8) {
                                Text(balance.name).font(.caption).foregroundStyle(.secondary)
                                Spacer()
                                Text("point bas \(Money.format(balance.low))").font(.caption.monospacedDigit())
                                    .foregroundStyle(balance.low < 0 ? .red : .orange)
                                Text("→ \(Money.format(balance.value))").font(.caption.monospacedDigit())
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .padding(8)
                    .background(Color(nsColor: .textBackgroundColor).opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
                }
            }
        }
    }

    // MARK: Totaux et seuil

    private func totals(_ view: PredictionView) -> some View {
        Grid(horizontalSpacing: 14, verticalSpacing: 14) {
            GridRow {
                total("Solde actuel", value: view.currentTotalBalance, systemImage: "wallet.bifold")
                total("À mi-période", value: view.midpointBalance, systemImage: "chart.line.flattrend.xyaxis")
                total("À l'échéance", value: view.finalBalance, systemImage: "flag.checkered")
                Card {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Seuil d'alerte").font(.caption).textCase(.uppercase).foregroundStyle(.secondary)
                        TextField("0", text: $model.thresholdText)
                            .textFieldStyle(.roundedBorder)
                            .monospacedDigit()
                            .onSubmit { model.commitThreshold() }
                        Text("Marque les jours sous ce solde.").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    private func total(_ label: String, value: Double, systemImage: String) -> some View {
        Card {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(label).font(.caption).textCase(.uppercase).foregroundStyle(.secondary)
                    Spacer()
                    Image(systemName: systemImage).foregroundStyle(.secondary)
                }
                MoneyText(amount: value, weight: .bold, size: .title3)
            }
        }
    }
}
