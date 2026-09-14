import DmxKit
import SwiftUI
import UniformTypeIdentifiers

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

struct RootView: View {
    @ObservedObject var store: AppStore
    let cloud: CloudSyncController?
    @StateObject private var models: PageModels
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var isExporting = false
    @State private var exportDocument: BackupDocument?
    @State private var isImporting = false
    @State private var snapshotsStarted = false

    init(store: AppStore, cloud: CloudSyncController?) {
        self.store = store
        self.cloud = cloud
        _models = StateObject(wrappedValue: PageModels(store: store))
    }

    var body: some View {
        Group {
            if sizeClass == .compact {
                CompactShell(models: models, actions: actions)
            } else {
                RegularShell(models: models, actions: actions)
            }
        }
        .modifier(StorePresentation(store: store))
        .preferredColorScheme(colorScheme)
        .sheet(item: Binding(get: { store.form }, set: { store.form = $0 })) { request in
            FormHost(request: request, onClose: { store.form = nil })
                .modifier(StorePresentation(store: store))
                .environment(\.dmxCompact, sizeClass == .compact)
                .presentationDetents([.large])
                .presentationDragIndicator(.visible)
        }
        .fileExporter(isPresented: $isExporting, document: exportDocument, contentType: .dmxBackup, defaultFilename: backupFileName(today: store.today)) { result in
            switch result {
            case .success: store.showToast("Vos données ont été exportées avec succès.")
            case let .failure(error): store.errorMessage = "Impossible de créer la sauvegarde. (\(error.localizedDescription))"
            }
        }
        .fileImporter(isPresented: $isImporting, allowedContentTypes: [.dmxBackup, .commaSeparatedText, .json, .plainText, .data]) { result in
            switch result {
            case let .success(url): openImport(url)
            case let .failure(error): store.errorMessage = error.localizedDescription
            }
        }
        .onOpenURL { url in openImport(url) }
        .onAppear {
            guard !snapshotsStarted, let directory = SnapshotRunner.directory else { return }
            snapshotsStarted = true
            SnapshotRunner.run(store: store, directory: directory)
        }
    }

    private var colorScheme: ColorScheme? {
        switch store.settings.theme {
        case .light: return .light
        case .dark: return .dark
        case .system: return nil
        }
    }

    private var actions: SettingsActions {
        SettingsActions(
            exportBackup: { exportBackup() },
            importFile: { isImporting = true },
            copyToClipboard: { UIPasteboard.general.string = $0 },
            iCloud: cloud?.settings
        )
    }

    private func exportBackup() {
        store.perform({ engine in try engine.exportBackup() }, completion: { content in
            exportDocument = BackupDocument(text: content)
            isExporting = true
        })
    }

    private func openImport(_ url: URL) {
        let access = url.startAccessingSecurityScopedResource()
        defer {
            if access { url.stopAccessingSecurityScopedResource() }
        }
        guard let content = FileText.read(url) else {
            store.errorMessage = "Le fichier n'a pas pu être lu."
            return
        }
        let name = url.lastPathComponent
        if ["dmx", "json"].contains(url.pathExtension.lowercased()) {
            store.present(.restoreBackup(content: content, fileName: name))
        } else {
            store.present(.statementImport(content: content, fileName: name))
        }
    }
}

/// Confirmations, erreurs, messages et couleur d'accentuation (appliqués aussi dans les feuilles).
struct StorePresentation: ViewModifier {
    @ObservedObject var store: AppStore

    func body(content: Content) -> some View {
        content
            .environmentObject(store)
            .tint(DmxColors.accent(store.settings.accentColor))
            .accentColor(DmxColors.accent(store.settings.accentColor))
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
                    ToastView(toast)
                        .padding(.bottom, 96)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
            .animation(.spring(duration: 0.3), value: store.toast)
    }
}

/// Contenu d'une page, commun à l'iPhone et à l'iPad.
struct PageContent: View {
    let route: AppRoute
    @ObservedObject var models: PageModels
    let actions: SettingsActions

    var body: some View {
        switch route {
        case .dashboard: DashboardPage(model: models.dashboard)
        case .accounts: AccountsPage(model: models.accounts)
        case .transactions: JournalListPage(model: models.journal)
        case .budget: BudgetPage(model: models.budget)
        case .scheduled: ScheduledPage(model: models.scheduled)
        case .analytics: AnalyticsPage(model: models.analytics)
        case .predictions: PredictionsPage(model: models.predictions)
        case .categories: CategoriesPage()
        case .settings: SettingsPage(actions: actions)
        }
    }
}

// MARK: - iPhone

struct CompactShell: View {
    private enum Tab: Hashable {
        case dashboard, accounts, transactions, budget, more

        var route: AppRoute? {
            switch self {
            case .dashboard: return .dashboard
            case .accounts: return .accounts
            case .transactions: return .transactions
            case .budget: return .budget
            case .more: return nil
            }
        }

        init(route: AppRoute) {
            switch route {
            case .dashboard: self = .dashboard
            case .accounts: self = .accounts
            case .transactions: self = .transactions
            case .budget: self = .budget
            default: self = .more
            }
        }
    }

    @EnvironmentObject private var store: AppStore
    @ObservedObject var models: PageModels
    let actions: SettingsActions
    @State private var tab: Tab = .dashboard
    @State private var morePath: [AppRoute] = []

    var body: some View {
        TabView(selection: $tab) {
            tabPage(.dashboard, tab: .dashboard)
            tabPage(.accounts, tab: .accounts)
            tabPage(.transactions, tab: .transactions)
            tabPage(.budget, tab: .budget)
            NavigationStack(path: $morePath) {
                MoreMenu()
                    .navigationDestination(for: AppRoute.self) { route in
                        CompactPage(route: route, models: models, actions: actions)
                    }
            }
            .tabItem {
                Label { Text("Plus") } icon: { DmxIcon.swiftUIImage("MoreHorizontal") }
            }
            .tag(Tab.more)
        }
        .environment(\.dmxCompact, true)
        .onChange(of: store.route) { _, route in
            let target = Tab(route: route)
            if target == .more {
                if morePath.last != route { morePath = [route] }
            }
            if tab != target { tab = target }
            models.activate(route)
        }
        .onChange(of: tab) { _, newTab in
            if let route = newTab.route {
                if store.route != route { store.route = route }
                models.activate(route)
            } else {
                models.activate(morePath.last)
            }
        }
        .onChange(of: morePath) { _, path in
            if tab == .more {
                models.activate(path.last)
                if let route = path.last, store.route != route { store.route = route }
            }
        }
    }

    private func tabPage(_ route: AppRoute, tab: Tab) -> some View {
        NavigationStack {
            CompactPage(route: route, models: models, actions: actions)
        }
        .tabItem {
            Label { Text(route.shortTitle) } icon: { DmxIcon.swiftUIImage(route.icon) }
        }
        .tag(tab)
    }
}

struct CompactPage: View {
    @EnvironmentObject private var store: AppStore
    let route: AppRoute
    @ObservedObject var models: PageModels
    let actions: SettingsActions

    var body: some View {
        VStack(spacing: 0) {
            if route.usesAccountFilter || route.showsBalances {
                MobileHeader(route: route)
            }
            PageContent(route: route, models: models, actions: actions)
        }
        .background(DmxPalette.pageBackground)
        .navigationTitle(route.title)
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct MoreMenu: View {
    var body: some View {
        List {
            Section {
                ForEach([AppRoute.scheduled, .analytics, .predictions]) { route in
                    row(route)
                }
            }
            Section {
                ForEach(AppRoute.footerRoutes) { route in
                    row(route)
                }
            }
            Section {
                HStack {
                    Spacer()
                    Text("DmxMoney • v\(AppInfo.version)").font(.caption2.weight(.bold)).foregroundStyle(.tertiary)
                    Spacer()
                }
                .listRowBackground(Color.clear)
            }
        }
        .navigationTitle("Plus")
    }

    private func row(_ route: AppRoute) -> some View {
        NavigationLink(value: route) {
            Label {
                Text(route.title)
            } icon: {
                DmxIcon(route.icon, size: 18).foregroundColor(.accentColor)
            }
        }
    }
}

// MARK: - iPad

struct RegularShell: View {
    @EnvironmentObject private var store: AppStore
    @ObservedObject var models: PageModels
    let actions: SettingsActions
    @State private var columnVisibility = NavigationSplitViewVisibility.all

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            List(selection: Binding(get: { Optional(store.route) }, set: { if let route = $0 { store.route = route } })) {
                ForEach(AppRoute.sidebarSections) { section in
                    Section(section.title) {
                        ForEach(section.routes) { route in sidebarRow(route) }
                    }
                }
                Section {
                    ForEach(AppRoute.footerRoutes) { route in sidebarRow(route) }
                }
            }
            .navigationTitle("DmxMoney")
        } detail: {
            NavigationStack {
                PageContent(route: store.route, models: models, actions: actions)
                    .background(DmxPalette.pageBackground)
                    .navigationTitle(store.route.title)
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar {
                        if store.route.usesAccountFilter {
                            ToolbarItem(placement: .topBarLeading) { AccountFilterButton() }
                        }
                        if store.route.showsBalances {
                            ToolbarItem(placement: .topBarTrailing) { BalancesToolbar() }
                        }
                    }
            }
        }
        .environment(\.dmxCompact, false)
        .onChange(of: store.route, initial: true) { _, route in
            models.activate(route)
        }
    }

    private func sidebarRow(_ route: AppRoute) -> some View {
        NavigationLink(value: route) {
            Label {
                Text(route.title)
            } icon: {
                DmxIcon(route.icon, size: 18)
            }
        }
    }
}

struct BalancesToolbar: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        HStack(spacing: 16) {
            VStack(alignment: .trailing, spacing: 0) {
                SectionLabel("Pointé")
                Text(Money.format(store.balances.checkedBalance)).font(.subheadline.weight(.bold)).foregroundColor(DmxColors.income)
            }
            VStack(alignment: .trailing, spacing: 0) {
                SectionLabel("Actuel")
                Text(Money.format(store.balances.currentBalance)).font(.headline.weight(.bold))
            }
        }
    }
}

// MARK: - Fichiers

extension UTType {
    static let dmxBackup = UTType(exportedAs: "com.dmxmoney.backup")
}

struct BackupDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.dmxBackup] }

    var text: String

    init(text: String) {
        self.text = text
    }

    init(configuration: ReadConfiguration) throws {
        text = String(decoding: configuration.file.regularFileContents ?? Data(), as: UTF8.self)
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}
