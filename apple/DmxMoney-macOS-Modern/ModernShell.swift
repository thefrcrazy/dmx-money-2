import DmxKit
import SwiftUI

/// Fenêtre principale : `NavigationSplitView`, barre latérale système, barre d'outils unifiée.
struct ModernShell: View {
    @ObservedObject var store: AppStore
    @StateObject private var models: PageModels
    @State private var columns: NavigationSplitViewVisibility = .all
    private let actions: SettingsActions

    init(store: AppStore, actions: SettingsActions = SettingsActions(exportBackup: {}, importFile: {}, copyToClipboard: { _ in })) {
        self.store = store
        self.actions = actions
        _models = StateObject(wrappedValue: PageModels(store: store))
    }

    var body: some View {
        NavigationSplitView(columnVisibility: $columns) {
            Sidebar(store: store, route: routeBinding, actions: actions)
                .navigationSplitViewColumnWidth(min: 210, ideal: 240, max: 320)
        } detail: {
            detail
                .navigationTitle(store.route.title)
                .toolbar { toolbar }
        }
        .modifier(ModernPresentation(store: store))
        .modifier(ModernLegacyAdoption(store: store))
        .onChange(of: store.route) { _, route in models.activate(route) }
        .onAppear { models.activate(store.route) }
    }

    /// La page, précédée du filtre de comptes quand elle en dépend, et de la recherche
    /// seulement là où elle sert à quelque chose.
    @ViewBuilder
    private var detail: some View {
        let page = ModernPage(route: store.route, models: models, actions: actions)
        page
    }

    private var routeBinding: Binding<AppRoute> {
        Binding(get: { store.route }, set: { store.route = $0 })
    }

    // MARK: - Barre d'outils

    /// Barre d'outils : le filtre de comptes et les deux soldes y vivent, sur la même ligne que
    /// l'ajout, le rechargement et la recherche — donc visibles sur toutes les pages.
    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        // Après le titre : le filtre de comptes (seulement là où il change quelque chose) puis
        // les deux soldes. L'ajout, le rechargement et la recherche restent à droite.
        ToolbarItem(placement: .principal) {
            HStack(spacing: 10) {
                if store.route.usesAccountFilter || !store.selectedAccountIds.isEmpty {
                    AccountSelector(store: store)
                }
                ToolbarBalances(store: store)
            }
        }
        ToolbarItemGroup(placement: .primaryAction) {
            if let prompt = searchPrompt {
                ToolbarSearch(prompt: prompt, text: searchBinding)
            }
            if let action = primaryAction {
                Button(action: action.perform) {
                    Label(action.title, systemImage: "plus")
                }
                .help(action.title)
            }
            if store.route.showsSyncButton {
                Button {
                    store.reload()
                    store.refreshBridgeStatus()
                } label: {
                    Label("Synchroniser", systemImage: "arrow.trianglehead.2.clockwise")
                }
                .help("Recharger les données")
            }
        }
    }

    private struct PrimaryAction {
        let title: String
        let perform: () -> Void
    }

    /// Pas de bouton « + » sur la vue d'ensemble : c'est une page de lecture.
    private var primaryAction: PrimaryAction? {
        switch store.route {
        case .transactions:
            return PrimaryAction(title: "Nouvelle transaction") { store.present(.transaction(id: nil)) }
        case .accounts:
            return PrimaryAction(title: "Nouveau compte") { store.present(.account(id: nil)) }
        case .budget:
            return PrimaryAction(title: "Nouveau budget") { store.present(.budget(id: nil)) }
        case .scheduled:
            return PrimaryAction(title: "Nouvelle échéance") { store.present(.scheduled(id: nil)) }
        case .predictions:
            return PrimaryAction(title: "Transaction fictive") { store.present(.fakeTransaction(id: nil)) }
        case .categories:
            return PrimaryAction(title: "Nouvelle catégorie") { store.present(.category(id: nil)) }
        case .dashboard, .analytics, .settings:
            return nil
        }
    }

    // MARK: - Recherche

    private var searchPrompt: String? {
        switch store.route {
        case .transactions: return "Rechercher une opération"
        case .accounts: return "Rechercher un compte"
        case .budget: return "Rechercher un budget"
        case .scheduled: return "Rechercher dans l'échéancier"
        case .categories: return "Rechercher une catégorie"
        case .dashboard, .analytics, .predictions, .settings: return nil
        }
    }

    private var searchBinding: Binding<String> {
        switch store.route {
        case .transactions:
            return Binding(get: { models.journal.search }, set: { models.journal.search = $0 })
        case .accounts:
            return Binding(get: { models.accounts.search }, set: { models.accounts.search = $0 })
        case .budget:
            return Binding(get: { models.budget.search }, set: { models.budget.search = $0 })
        case .scheduled:
            return Binding(get: { models.scheduled.search }, set: { models.scheduled.search = $0 })
        case .categories:
            return Binding(get: { models.categorySearch }, set: { models.categorySearch = $0 })
        default:
            return .constant("")
        }
    }
}

extension AppRoute {
    /// Pages dont le contenu dépend du filtre de comptes. La vue d'ensemble et « Mes Comptes »
    /// montrent déjà tous les comptes, la liste n'y apporte rien.
    var usesAccountFilter: Bool {
        switch self {
        case .transactions, .budget, .scheduled, .analytics, .predictions: return true
        case .dashboard, .accounts, .categories, .settings: return false
        }
    }

    var showsSyncButton: Bool {
        switch self {
        case .dashboard, .settings: return false
        default: return true
        }
    }
}

/// Barre latérale native : sections, SF Symbols, et le pied de la 1.x (réglages, mise à jour,
/// quitter).
private struct Sidebar: View {
    @ObservedObject var store: AppStore
    @Binding var route: AppRoute
    let actions: SettingsActions

    var body: some View {
        List(selection: $route) {
            Section("Général") {
                row(.dashboard)
                row(.accounts)
                row(.transactions)
            }
            Section("Finances") {
                row(.budget)
                row(.scheduled)
            }
            Section("Analyses") {
                row(.analytics)
                row(.predictions)
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .bottom, spacing: 0) { footer }
    }

    /// Pied ancré en bas de la barre latérale, comme en 1.x. C'est une liste de barre latérale
    /// elle aussi : les lignes gardent exactement la même géométrie que celles du dessus.
    private var footer: some View {
        VStack(spacing: 0) {
            List(selection: $route) {
                row(.categories)
                row(.settings)
                Button {
                    // Pas de confirmation : la fenêtre peut se fermer sans quitter, quitter est
                    // donc un choix explicite.
                    NSApp.terminate(nil)
                } label: {
                    Label("Quitter", systemImage: "power")
                        .foregroundStyle(Color.red)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
                // L'icône d'un `Label` de barre latérale suit la teinte, pas le style de premier
                // plan : les deux sont donc fixées au rouge.
                .tint(Color.red)
            }
            .listStyle(.sidebar)
            .scrollDisabled(true)
            .frame(height: 104)
            Text("DmxMoney • v\(AppInfo.version)")
                .font(.caption2)
                .foregroundStyle(.tertiary)
                .padding(.bottom, 8)
        }
    }

    private func row(_ route: AppRoute) -> some View {
        Label(route.title, systemImage: Symbols.route(route)).tag(route)
    }
}

/// Feuilles, confirmations, erreurs et messages.
struct ModernPresentation: ViewModifier {
    @ObservedObject var store: AppStore

    func body(content: Content) -> some View {
        content
            .environmentObject(store)
            .tint(DmxColors.accent(store.settings.accentColor))
            .sheet(item: Binding(get: { store.form }, set: { store.form = $0 })) { request in
                ModernFormHost(request: request, onClose: { store.form = nil })
                    .environmentObject(store)
                    .tint(DmxColors.accent(store.settings.accentColor))
            }
            .alert(
                store.confirmation?.title ?? "",
                isPresented: Binding(get: { store.confirmation != nil }, set: { if !$0 { store.confirmation = nil } }),
                presenting: store.confirmation
            ) { request in
                Button(request.confirmTitle, role: request.destructive ? .destructive : nil) { request.action() }
                Button("Annuler", role: .cancel) {}
            } message: { request in
                Text(request.message)
            }
            .alert("DmxMoney", isPresented: Binding(get: { store.errorMessage != nil }, set: { if !$0 { store.errorMessage = nil } })) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(store.errorMessage ?? "")
            }
            .overlay(alignment: .bottom) {
                if let toast = store.toast {
                    Text(toast)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 10)
                        .background(.regularMaterial, in: Capsule())
                        .padding(.bottom, 28)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
            .animation(.spring(duration: 0.3), value: store.toast)
    }
}
