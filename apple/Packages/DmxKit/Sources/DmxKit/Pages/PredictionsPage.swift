import SwiftUI

public final class PredictionsModel: PageModel {
    public private(set) var query: PredictionQuery? = nil {
        willSet { objectWillChange.send() }
    }

    public private(set) var view: PredictionView? = nil {
        willSet { objectWillChange.send() }
    }

    /// Affichage du point bas journalier (préférence locale, comme en 1.x) : activé par défaut,
    /// c'est l'information qui dit si un compte passe dans le rouge en cours de journée.
    public var showIntradayLow = true {
        willSet { objectWillChange.send() }
    }

    public var thresholdText = "" {
        willSet { objectWillChange.send() }
    }

    public override init(store: AppStore) {
        super.init(store: store)
        refresh()
    }

    public override func refresh() {
        let accounts = store.selectedAccountIds
        let today = store.today
        guard let query = store.read({ engine in try engine.predictionQuery(accounts: accounts) }) else { return }
        self.query = query
        view = store.read { engine in try engine.predictions(query: query, today: today) }
        thresholdText = AmountInput.text(query.alertThreshold)
    }

    public func setRange(_ range: TimeRange) {
        store.apply(.setPredictionTimeRange(range: range))
    }

    public func setCustomEnd(_ date: String) {
        store.apply(.setPredictionCustomEndDate(date: date))
    }

    public func setMonthStartsOnFirst(_ enabled: Bool) {
        store.apply(.setPredictionMonthStartsOnFirst(enabled: enabled))
    }

    public func commitThreshold() {
        let trimmed = thresholdText.trimmingCharacters(in: .whitespaces)
        let value = trimmed.isEmpty ? 0 : (AmountInput.parse(trimmed) ?? query?.alertThreshold ?? 0)
        guard value != query?.alertThreshold else { return }
        store.apply(.setPredictionAlertThreshold(threshold: value))
    }

    public func toggle(_ id: String) {
        store.run { engine in try engine.toggleFakeTransaction(id: id) }
    }

    public func remove(_ id: String) {
        store.run("Transaction fictive retirée") { engine in try engine.deleteFakeTransactions(ids: [id]) }
    }

    public func clearAll() {
        let accounts = store.selectedAccountIds
        store.run("Transactions fictives retirées") { engine in try engine.clearAppliedFakeTransactions(accounts: accounts) }
    }
}

/// Prédictions : transactions fictives, projection journalière en escalier, jours à surveiller, seuil d'alerte.
public struct PredictionsPage: View {
    @ObservedObject private var model: PredictionsModel
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact

    private static let markerLegend: [(String, String)] = [
        ("#ef4444", "Solde négatif en fin de journée"),
        ("#f97316", "Passage sous le seuil d’alerte"),
        ("#a855f7", "Point bas négatif, rattrapé par un revenu"),
        ("#eab308", "Point bas sous le seuil, rattrapé par un revenu"),
    ]

    public init(model: PredictionsModel) {
        self.model = model
    }

    public var body: some View {
        PageScroll {
            if let query = model.query, let view = model.view {
                if compact {
                    TimeRangeSelector(selection: query.range, onSelect: model.setRange)
                } else {
                    HStack {
                        Text("Prédictions Financières").font(.system(size: 24, weight: .bold))
                        Spacer()
                        TimeRangeSelector(selection: query.range, onSelect: model.setRange)
                    }
                }

                if query.range == .custom {
                    HStack {
                        if !compact { Spacer() }
                        FormField("Jusqu'au") {
                            DayPicker(day: Binding(get: { query.customEndDate ?? view.endDate }, set: model.setCustomEnd))
                        }
                        .fixedSize(horizontal: true, vertical: false)
                    }
                }

                fakeTransactionsCard(view)
                projectionCard(query, view)
                if view.intradayRiskCount > 0 {
                    riskDaysCard(view)
                }
                totals(view)
                thresholdCard
            }
        }
    }

    // MARK: Transactions fictives

    private func fakeTransactionsCard(_ view: PredictionView) -> some View {
        DmxCard {
            VStack(alignment: .leading, spacing: 14) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Transactions fictives").font(.system(size: 16, weight: .semibold))
                        Text("Elles modifient uniquement cette projection et ne sont pas ajoutées au journal.")
                            .font(.system(size: 12)).foregroundColor(.secondary)
                    }
                    Spacer()
                    if !view.fakeTransactions.isEmpty {
                        Button(action: model.clearAll) {
                            HStack(spacing: 6) {
                                DmxIcon("Trash2", size: 13)
                                if !compact {
                                    Text("Tout retirer")
                                }
                            }
                        }
                        .buttonStyle(DmxButtonStyle(.secondary))
                    }
                    Button(action: { store.present(.fakeTransaction(id: nil)) }) {
                        HStack(spacing: 6) {
                            DmxIcon("Plus", size: 13)
                            Text("Ajouter")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(.primary))
                    .disabled(store.accounts.isEmpty)
                }

                if view.fakeTransactions.isEmpty {
                    Text("Aucune transaction fictive appliquée à cette projection.")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 20)
                        .overlay(RoundedRectangle(cornerRadius: 10).stroke(DmxPalette.separator, style: StrokeStyle(lineWidth: 1, dash: [5, 4])))
                } else {
                    let total = view.fakeTransactions.count
                    HStack {
                        Text("\(view.enabledFakeCount)/\(total) \(total > 1 ? "simulations" : "simulation") \(view.enabledFakeCount == 1 ? "active" : "actives")")
                        Spacer()
                        Text("Impact période : \(view.fakeImpact >= 0 ? "+" : "")\(Money.format(view.fakeImpact))")
                            .fontWeight(.semibold)
                            .foregroundColor(view.fakeImpact >= 0 ? DmxColors.income : DmxColors.expense)
                    }
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
                    VStack(spacing: 0) {
                        ForEach(Array(view.fakeTransactions.enumerated()), id: \.element.transaction.id) { item in
                            if item.offset > 0 { Divider() }
                            fakeRow(item.element)
                        }
                    }
                    .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(DmxPalette.separator.opacity(0.6), lineWidth: 0.5))
                }
            }
        }
    }

    private func metaLine(_ row: FakeTransactionRow) -> String {
        let transaction = row.transaction
        var parts = [DayFormat.medium(transaction.date), row.sourceAccountName]
        if transaction.transactionType == .transfer {
            parts.append("→ " + (row.destinationAccountName ?? "Compte supprimé"))
        } else if let category = row.categoryName {
            parts.append(category)
        }
        return parts.joined(separator: " • ")
    }

    private func fakeRow(_ row: FakeTransactionRow) -> some View {
        let transaction = row.transaction
        let meta: (icon: String, color: Color) = {
            switch transaction.transactionType {
            case .income: return ("TrendingUp", DmxColors.income)
            case .expense: return ("TrendingDown", DmxColors.expense)
            case .transfer: return ("ArrowRightLeft", DmxColors.transfer)
            }
        }()
        return HStack(spacing: 12) {
            Button(action: { model.toggle(transaction.id) }) {
                DmxIcon(transaction.enabled ? "CheckCircle2" : "Circle", size: 17)
                    .foregroundColor(transaction.enabled ? .accentColor : .secondary)
            }
            .buttonStyle(PlainButtonStyle())
            // Sur iPhone, la pastille de type et l'icône laisseraient la description illisible.
            if !compact {
                IconCircle(icon: meta.icon, color: meta.color, size: 34)
            }
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(transaction.description).font(.system(size: 13, weight: .semibold)).lineLimit(1)
                    if !compact {
                        Pill(row.typeLabel)
                    }
                    if !transaction.enabled {
                        Pill("Désactivée")
                    }
                }
                // Une seule ligne tronquée à la fin : sur iPhone, trois textes séparés se
                // réduisaient chacun à deux caractères.
                Text(metaLine(row))
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            Spacer(minLength: 8)
            Text(Money.signed(transaction.amount, kind: transaction.transactionType))
                .font(.system(size: 13, weight: .semibold))
                .foregroundColor(transaction.enabled ? meta.color : .secondary)
                .lineLimit(1)
                .layoutPriority(1)
            if !compact {
                IconButton("Edit2") { store.present(.fakeTransaction(id: transaction.id)) }
            }
            IconButton("Trash2", color: DmxColors.expense) { model.remove(transaction.id) }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .contentShape(Rectangle())
        .onTapGesture {
            // Sur iPhone, la ligne entière ouvre le formulaire (le crayon est masqué).
            if compact { store.present(.fakeTransaction(id: transaction.id)) }
        }
        .background(transaction.enabled ? Color.clear : Color.primary.opacity(0.03))
        .opacity(transaction.enabled ? 1 : 0.75)
    }

    // MARK: Projection

    private func projectionCard(_ query: PredictionQuery, _ view: PredictionView) -> some View {
        var lines = view.accounts.map { series in
            ChartLine(id: series.id, name: series.name, color: Color(hex: series.color, fallback: DmxColors.transfer), values: series.closes)
        }
        if model.showIntradayLow {
            lines += view.accounts.map { series in
                ChartLine(id: series.id + "-low", name: "\(series.name) — point bas", color: Color(hex: series.color, fallback: DmxColors.transfer).opacity(0.85),
                          values: series.lows, dashed: true, filled: false, showsInLegend: false)
            }
        }
        var references = [ChartReferenceLine(value: 0, color: DmxColors.expense)]
        if view.alertThreshold > 0 {
            references.append(ChartReferenceLine(value: view.alertThreshold, color: DmxColors.warning, dashed: true))
        }

        return DmxCard {
            VStack(alignment: .leading, spacing: 16) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Projection sur \(view.titleLabel) (Journalière)").font(.system(size: 16, weight: .semibold))
                        Text("Visualisation de la trésorerie jour par jour.").font(.system(size: 12)).foregroundColor(.secondary)
                    }
                    Spacer()
                    if query.range != .custom {
                        CheckboxLabel(title: "Démarrer au 1er du mois", isOn: Binding(get: { query.monthStartsOnFirst }, set: model.setMonthStartsOnFirst))
                    }
                    CheckboxLabel(title: "Point bas journalier", isOn: $model.showIntradayLow)
                }
                LineChart(
                    lines: lines,
                    labels: view.labels,
                    stepped: true,
                    referenceLines: references,
                    markers: view.markers.map { ChartMarker(index: Int($0.index), color: Color(hex: $0.strokeColor, fallback: DmxColors.expense)) },
                    tooltip: { index in AnyView(tooltip(view, index: index)) }
                )
                .frame(height: compact ? 260 : 384)
                HStack(spacing: 14) {
                    ForEach(Self.markerLegend, id: \.1) { item in
                        HStack(spacing: 6) {
                            RoundedRectangle(cornerRadius: 1).fill(Color(hex: item.0)).frame(width: 16, height: 2)
                            Text(item.1)
                        }
                    }
                    Spacer(minLength: 0)
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
            }
        }
    }

    private func tooltip(_ view: PredictionView, index: Int) -> ChartTooltip {
        let marker = view.markers.first { Int($0.index) == index }
        var notes: [String] = []
        if let marker = marker {
            if !marker.crossingNames.isEmpty {
                let label = marker.severity == .danger ? "Solde négatif" : "Sous le seuil d’alerte"
                notes.append("\(label) : \(marker.crossingNames.joined(separator: ", "))")
            }
            if !marker.intradayNames.isEmpty {
                let label = marker.intradaySeverity == .danger ? "Point bas négatif" : "Point bas sous le seuil"
                notes.append("\(label) : \(marker.intradayNames.joined(separator: ", "))")
            }
        }
        return ChartTooltip(
            title: index < view.fullLabels.count ? view.fullLabels[index] : "",
            lines: view.accounts.map { series in
                ChartTooltip.Line(
                    color: Color(hex: series.color, fallback: DmxColors.transfer),
                    name: series.name,
                    value: index < series.closes.count ? Money.format(series.closes[index]) : ""
                )
            },
            footer: notes.isEmpty ? nil : notes.joined(separator: "\n")
        )
    }

    // MARK: Jours à surveiller

    private func riskDaysCard(_ view: PredictionView) -> some View {
        let days = view.markers.filter { $0.intradaySeverity != nil }
        return DmxCard {
            HStack(alignment: .top, spacing: 12) {
                IconCircle(icon: "ShieldAlert", color: DmxColors.intradayWarning, size: 36)
                VStack(alignment: .leading, spacing: 10) {
                    Text("Jours à surveiller (\(view.intradayRiskCount))").font(.system(size: 16, weight: .semibold))
                    Text("Ces jours cumulent un retrait et un revenu. Le solde de fin de journée reste au-dessus du seuil, mais il passe en dessous avant l’arrivée du revenu — de quoi déclencher des frais côté banque.")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    VStack(spacing: 0) {
                        ForEach(Array(days.prefix(8).enumerated()), id: \.element.date) { item in
                            if item.offset > 0 { Divider() }
                            VStack(alignment: .leading, spacing: 4) {
                                Text(item.element.fullLabel).font(.system(size: 13, weight: .semibold))
                                ForEach(item.element.intradayBalances, id: \.name) { balance in
                                    HStack(spacing: 6) {
                                        Text(balance.name).foregroundColor(.secondary)
                                        Spacer()
                                        Text("Point bas \(Money.format(balance.low))")
                                            .fontWeight(.semibold)
                                            .foregroundColor(item.element.intradaySeverity == .danger ? DmxColors.intradayDanger : DmxColors.intradayWarning)
                                        Text("→").foregroundColor(.secondary)
                                        Text("Fin de journée \(Money.format(balance.value))").fontWeight(.semibold).foregroundColor(DmxColors.income)
                                    }
                                    .font(.system(size: 11))
                                }
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 9)
                        }
                    }
                    .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(DmxPalette.separator.opacity(0.6), lineWidth: 0.5))
                    if days.count > 8 {
                        let others = days.count - 8
                        Text("+\(others) \(others > 1 ? "autres jours" : "autre jour") sur la période.")
                            .font(.system(size: 11)).foregroundColor(.secondary)
                    }
                }
            }
        }
    }

    // MARK: Totaux et seuil

    private func totals(_ view: PredictionView) -> some View {
        let tiles = [
            AnyView(StatTile("Solde Actuel Total", value: Money.format(view.currentTotalBalance))),
            AnyView(StatTile("Projection mi-période", value: Money.format(view.midpointBalance),
                             valueColor: view.midpointBalance >= view.currentTotalBalance ? DmxColors.income : DmxColors.expense)),
            AnyView(StatTile("Projection au \(view.endLabel)", value: Money.format(view.finalBalance),
                             valueColor: view.finalBalance >= view.currentTotalBalance ? DmxColors.income : DmxColors.expense)),
        ]
        return Group {
            if compact {
                VStack(spacing: 16) {
                    ForEach(0..<tiles.count, id: \.self) { tiles[$0] }
                }
            } else {
                HStack(alignment: .top, spacing: 20) {
                    ForEach(0..<tiles.count, id: \.self) { tiles[$0].frame(maxHeight: .infinity, alignment: .top) }
                }
                .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var thresholdCard: some View {
        DmxCard {
            HStack(alignment: .center, spacing: 20) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Seuil d'alerte").font(.system(size: 16, weight: .semibold))
                    Text("Le seuil personnalisé s'affiche en orange. Le rouge reste réservé aux soldes négatifs. Les jours où un retrait passe avant un revenu sont signalés à part, même si la journée se termine au-dessus du seuil.")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 8)
                FormField("Montant") {
                    HStack(spacing: 4) {
                        TextField("0", text: $model.thresholdText, onEditingChanged: { editing in
                            if !editing { model.commitThreshold() }
                        }, onCommit: model.commitThreshold)
                        .textFieldStyle(PlainTextFieldStyle())
                        .font(.system(size: 13))
                        Text("€").font(.system(size: 13)).foregroundColor(.secondary)
                    }
                    .modifier(FieldBackground())
                }
                .frame(width: compact ? 130 : 200)
            }
        }
    }
}
