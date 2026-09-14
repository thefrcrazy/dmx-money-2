import Charts
import DmxKit
import SwiftUI

/// Modèles des pages, créés une fois ; seule la page visible se recalcule.
@MainActor
final class PageModels: ObservableObject {
    let dashboard: DashboardModel
    let accounts: AccountsModel
    let journal: JournalModel
    let budget: BudgetModel
    let scheduled: ScheduledModel
    let analytics: AnalyticsModel
    let predictions: PredictionsModel
    /// La page Catégories lit le store directement : sa recherche vit ici.
    @Published var categorySearch = ""

    init(store: AppStore) {
        dashboard = DashboardModel(store: store)
        accounts = AccountsModel(store: store)
        journal = JournalModel(store: store)
        budget = BudgetModel(store: store)
        scheduled = ScheduledModel(store: store)
        analytics = AnalyticsModel(store: store)
        predictions = PredictionsModel(store: store)
        activate(.dashboard)
    }

    func activate(_ route: AppRoute?) {
        let all: [(AppRoute, PageModel)] = [
            (.dashboard, dashboard), (.accounts, accounts), (.transactions, journal), (.budget, budget),
            (.scheduled, scheduled), (.analytics, analytics), (.predictions, predictions),
        ]
        for (candidate, model) in all {
            model.isActive = candidate == route
        }
    }
}

/// Contenu de la fenêtre pour la page sélectionnée.
struct ModernPage: View {
    let route: AppRoute
    @ObservedObject var models: PageModels
    let actions: SettingsActions
    @EnvironmentObject private var store: AppStore

    var body: some View {
        switch route {
        case .dashboard:
            ModernDashboard(model: models.dashboard)
        case .accounts:
            ModernAccounts(model: models.accounts)
        case .transactions:
            ModernJournal(model: models.journal)
        case .categories:
            ModernCategories(search: $models.categorySearch)
        case .budget:
            ModernBudget(model: models.budget)
        case .scheduled:
            ModernScheduled(model: models.scheduled)
        case .analytics:
            ModernAnalytics(model: models.analytics)
        case .predictions:
            ModernPredictions(model: models.predictions)
        case .settings:
            ModernSettings(store: store, actions: actions)
        }
    }
}

// MARK: - Briques natives

/// Carte de contenu : matériau système, coins arrondis, titre optionnel.
struct Card<Content: View>: View {
    private let title: String?
    private let systemImage: String?
    private let trailing: AnyView?
    private let content: Content

    init(_ title: String? = nil, systemImage: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.systemImage = systemImage
        trailing = nil
        self.content = content()
    }

    init<Trailing: View>(
        _ title: String?,
        systemImage: String? = nil,
        @ViewBuilder trailing: () -> Trailing,
        @ViewBuilder content: () -> Content
    ) {
        self.title = title
        self.systemImage = systemImage
        self.trailing = AnyView(trailing())
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if title != nil || trailing != nil {
                HStack(spacing: 8) {
                    if let title {
                        if let systemImage {
                            Label(title, systemImage: systemImage).font(.headline)
                        } else {
                            Text(title).font(.headline)
                        }
                    }
                    Spacer(minLength: 8)
                    trailing
                }
            }
            content
        }
        .padding(16)
        // `maxHeight` : dans une grille ou une rangée, toutes les cartes prennent la hauteur de
        // la plus haute, comme en 1.x. Dans une page défilante la hauteur proposée est libre,
        // donc la carte garde sa taille naturelle.
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        // Surface opaque : les matériaux ne se rendent pas dans les captures hors écran, et les
        // apps système utilisent aussi une surface opaque pour le contenu.
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(.separator, lineWidth: 0.5)
        }
        // Sans ombre, une carte claire sur un fond clair disparaît.
        .shadow(color: .black.opacity(0.10), radius: 5, y: 1)
    }
}

/// Montant monospacé, coloré selon le signe. Toujours avec les centimes : un solde arrondi
/// à l'euro cache l'information qu'on vient chercher.
struct MoneyText: View {
    let amount: Double
    var rounded = false
    var signed: TransactionType?
    /// Type stocké en base : pour un virement, il donne le signe de la jambe.
    var stored: TransactionType?
    var weight: Font.Weight = .semibold
    var size: Font.TextStyle = .body

    var body: some View {
        Text(text)
            .font(.system(size, design: .default).weight(weight).monospacedDigit())
            .foregroundStyle(color)
    }

    private var text: String {
        if let signed {
            if let stored {
                return Money.signed(amount, display: signed, stored: stored)
            }
            return Money.signed(amount, kind: signed)
        }
        return rounded ? Money.rounded(amount) : Money.format(amount)
    }

    private var color: Color {
        switch signed {
        case .income: return .green
        case .expense: return .red
        case .transfer: return .indigo
        case nil: return amount < 0 ? .red : .primary
        }
    }
}

/// Pastille de catégorie : SF Symbol dans la couleur stockée en base.
struct CategoryBadge: View {
    let icon: String
    let colorHex: String
    var size: CGFloat = 28

    var body: some View {
        Image(systemName: Symbols.name(for: icon))
            .font(.system(size: size * 0.46, weight: .medium))
            .foregroundStyle(Color(hex: colorHex, fallback: .secondary))
            .frame(width: size, height: size)
            .background(Color(hex: colorHex, fallback: .secondary).opacity(0.14), in: Circle())
    }
}

/// Page défilante, avec la marge et la largeur maximale du système.
struct PageBody<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        ScrollView {
            content
                .padding(20)
                .frame(maxWidth: 1180, alignment: .leading)
                .frame(maxWidth: .infinity)
        }
        .scrollEdgeEffectStyle(.soft, for: .top)
    }
}

// MARK: - Survol des graphiques

/// Index survolé dans un graphique indexé (0…count-1), ou `nil` hors de la zone de tracé.
///
/// Les graphiques tracent l'index du jour en abscisse : le noyau fournit les libellés.
func chartHoverIndex(_ proxy: ChartProxy, _ geometry: GeometryProxy, at location: CGPoint, count: Int) -> Int? {
    guard count > 0, let plot = proxy.plotFrame else { return nil }
    let frame = geometry[plot]
    guard frame.contains(location) else { return nil }
    guard let raw: Double = proxy.value(atX: location.x - frame.minX) else { return nil }
    return min(max(Int(raw.rounded()), 0), count - 1)
}

/// Bulle d'information affichée au survol d'un graphique.
struct HoverBubble<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            content
        }
        .font(.caption)
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        // Matériau translucide, le même sur toutes les courbes : le graphique reste perçu sous la
        // bulle, comme une info-bulle système.
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(.separator, lineWidth: 0.5)
        }
        .shadow(color: .black.opacity(0.12), radius: 6, y: 2)
    }
}

/// Ligne d'une bulle : pastille de couleur, libellé, valeur alignée à droite, et un détail
/// facultatif sur une seconde ligne (le point bas de la journée, par exemple).
struct HoverRow: View {
    let color: Color
    let label: String
    let value: String
    var valueColor: Color = .primary
    var detail: String?
    var detailColor: Color = .secondary

    var body: some View {
        HStack(alignment: .top, spacing: 6) {
            Circle().fill(color).frame(width: 7, height: 7).padding(.top, 4)
            Text(label).lineLimit(1)
            Spacer(minLength: 10)
            VStack(alignment: .trailing, spacing: 0) {
                Text(value).monospacedDigit().fontWeight(.semibold).foregroundStyle(valueColor)
                if let detail {
                    Text(detail).monospacedDigit().font(.caption2).foregroundStyle(detailColor)
                }
            }
        }
    }
}

/// Couleur d'un montant projeté : rouge dans le négatif, orange sous le seuil d'alerte.
func projectionColor(_ value: Double, threshold: Double) -> Color {
    if value < 0 { return .red }
    if threshold > 0, value < threshold { return .orange }
    return .primary
}

/// Repère de survol : bande translucide sous le trait, comme la sélection système des
/// graphiques à barres. Les deux pages (analyses et prédictions) présentent ainsi la même chose.
///
/// La bulle est rattachée au seul trait fin : annoter l'ensemble la dessinait une fois par marque,
/// sous la bande pour l'une d'elles — d'où un rendu différent d'un graphique à l'autre.
struct ChartHoverMark<Bubble: View>: ChartContent {
    let index: Int
    var label = "Jour"
    @ViewBuilder let bubble: () -> Bubble

    var body: some ChartContent {
        RuleMark(x: .value(label, index))
            .lineStyle(StrokeStyle(lineWidth: 22))
            .foregroundStyle(.secondary.opacity(0.10))
        RuleMark(x: .value(label, index))
            .lineStyle(StrokeStyle(lineWidth: 1))
            .foregroundStyle(.secondary.opacity(0.5))
            .annotation(
                position: .topTrailing,
                spacing: 6,
                overflowResolution: .init(x: .fit(to: .chart), y: .fit(to: .chart))
            ) {
                bubble()
            }
    }
}

/// Zone transparente qui suit le pointeur au-dessus d'un graphique.
struct ChartHoverArea: View {
    let proxy: ChartProxy
    let count: Int
    @Binding var index: Int?

    var body: some View {
        GeometryReader { geometry in
            Rectangle()
                .fill(.clear)
                .contentShape(Rectangle())
                .onContinuousHover { phase in
                    switch phase {
                    case let .active(location):
                        index = chartHoverIndex(proxy, geometry, at: location, count: count)
                    case .ended:
                        index = nil
                    }
                }
        }
    }
}

/// Barre de filtres d'une page : le contenu libre à gauche, les filtres alignés à droite.
struct FilterBar<Leading: View, Trailing: View>: View {
    @ViewBuilder var leading: Leading
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 10) {
            leading
            Spacer(minLength: 12)
            trailing
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
    }
}

/// Filtre multi-sélection par catégorie, partagé par le budget et l'échéancier.
struct CategoryFilterMenu: View {
    @EnvironmentObject private var store: AppStore
    @Binding var selection: [String]

    var body: some View {
        FilterSelector(
            title: "Catégories",
            systemImage: "tag",
            options: store.categories.map { SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color) },
            selection: $selection
        )
    }
}
