import Combine
import Foundation
#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// État partagé de l'application : moteur Rust, réglages, comptes, catégories et filtre global.
///
/// Toutes les valeurs métier viennent du noyau ; le store ne fait que les relayer aux vues.
/// Les lectures sont rapides (instantané en cache côté Rust) et peuvent se faire sur le fil
/// principal ; les opérations longues (pont, import, restauration) passent par `perform`.
public final class AppStore: ObservableObject {
    public let engine: DmxEngine

    @Published public private(set) var dataVersion: Int64 = 0
    @Published public private(set) var settings: AppSettings
    @Published public private(set) var accounts: [Account] = []
    @Published public private(set) var categories: [DmxCategory] = []
    /// Comptes cochés dans le filtre global (vide = tous les comptes).
    @Published public var selectedAccountIds: [String] = [] {
        didSet {
            if oldValue != selectedAccountIds { bumpRevision() }
        }
    }
    /// Incrémenté à chaque changement de données ou de filtre : les pages se recalculent.
    @Published public private(set) var revision: Int = 0
    @Published public var errorMessage: String?
    @Published public var route: AppRoute = .dashboard {
        didSet { if oldValue != route { processDueScheduled() } }
    }
    @Published public var form: FormRequest?
    @Published public var confirmation: ConfirmRequest?
    @Published public var toast: String?
    @Published public private(set) var bridgeStatus: CompanionStatus?
    @Published public private(set) var dueResult: ProcessDueResult?

    private let queue = DispatchQueue(label: "com.dmxmoney.engine", qos: .userInitiated)
    private let listener = ListenerProxy()
    private var dueTimer: Timer?
    private var lifecycleObservers: [NSObjectProtocol] = []
    private var processingDue = false
    private var lastDueCheck: (day: String, version: Int64)?

    /// Install once after the window exists; also refresh date-sensitive pages after midnight.
    public func startScheduledRefresh() {
        guard dueTimer == nil else { return }
        let timer = Timer(timeInterval: 60, repeats: true) { [weak self] _ in self?.processDueScheduled() }
        RunLoop.main.add(timer, forMode: .common)
        dueTimer = timer
        #if os(macOS)
        let active = NSApplication.didBecomeActiveNotification
        #else
        let active = UIApplication.didBecomeActiveNotification
        #endif
        for name in [active, NSNotification.Name.NSCalendarDayChanged] {
            lifecycleObservers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                self?.processDueScheduled()
            })
        }
        processDueScheduled()
    }

    deinit {
        dueTimer?.invalidate()
        lifecycleObservers.forEach(NotificationCenter.default.removeObserver)
    }

    public init(engine: DmxEngine) {
        self.engine = engine
        self.settings = (try? engine.settings()) ?? AppStore.placeholderSettings
        listener.store = self
        engine.setListener(listener: listener)
        reload()
    }

    /// Ouvre la base de l'utilisateur (et reprend la base DmxMoney 1.x au premier lancement sur macOS).
    ///
    /// `DMXMONEY_DATA_DIR` permet de lancer l'app sur un dossier de test, sans reprise de la base 1.x.
    /// `DMXMONEY_LEGACY_DB` (chemins séparés par `:`) y ajoute des bases 1.x factices, pour
    /// éprouver la proposition de reprise sans toucher aux données de l'utilisateur.
    public static func openDefault() throws -> AppStore {
        if let override = ProcessInfo.processInfo.environment["DMXMONEY_DATA_DIR"], !override.isEmpty {
            try FileManager.default.createDirectory(atPath: override, withIntermediateDirectories: true)
            let legacy = (ProcessInfo.processInfo.environment["DMXMONEY_LEGACY_DB"] ?? "")
                .split(separator: ":")
                .map(String.init)
            let store = AppStore(engine: try DmxEngine.open(dataDir: override, legacyDatabasePaths: legacy))
            // Dossier de test : pas de pont PWA (ni d'accès au trousseau de l'utilisateur).
            store.bridgeEnabled = false
            return store
        }
        return AppStore(engine: try DmxEngine.open(dataDir: try dataDirectory().path, legacyDatabasePaths: legacyPaths()))
    }

    public static func dataDirectory() throws -> URL {
        let base = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        let directory = base.appendingPathComponent("com.dmxmoney.app", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    private static func legacyPaths() -> [String] {
        #if os(macOS)
        return defaultLegacyDatabasePaths()
        #else
        return []
        #endif
    }

    // MARK: - Jour courant

    /// Date du jour (`YYYY-MM-DD`) selon le fuseau local, fournie par le noyau.
    public var today: String { DmxCore.today() }

    // MARK: - Rechargement

    public func reload() {
        do {
            let version = try engine.dataVersion()
            settings = try engine.settings()
            accounts = try engine.accountsList()
            categories = try engine.categories()
            let known = Set(accounts.map { $0.id })
            let filtered = selectedAccountIds.filter { known.contains($0) }
            if filtered != selectedAccountIds {
                selectedAccountIds = filtered
            }
            dataVersion = version
            bumpRevision()
        } catch {
            errorMessage = AppStore.message(for: error)
        }
    }

    /// Soldes Pointé / Actuel des comptes filtrés, pour la barre d'outils.
    @Published public private(set) var balances = BalanceSummary(currentBalance: 0, checkedBalance: 0)

    /// Reformulation des demandes de l'assistant par un modèle local, quand l'hôte en a un
    /// (Apple Intelligence sur macOS 26). Appelée depuis un thread du pont : elle doit être
    /// synchrone et sûre à appeler hors du fil principal.
    public var assistantRewriter: (@Sendable (String) -> String?)?

    private func bumpRevision() {
        if let summary = try? engine.dashboard(accounts: selectedAccountIds, today: today).balances {
            balances = summary
        }
        revision &+= 1
    }

    /// Crée les opérations des échéances arrivées à terme (au lancement et au retour au premier plan).
    public func processDueScheduled(force: Bool = false) {
        let day = today
        guard !processingDue, force || lastDueCheck?.day != day || lastDueCheck?.version != dataVersion else { return }
        processingDue = true
        let version = dataVersion
        perform({ engine in try engine.processDueScheduled(today: day) }) { [weak self] result in
            guard let self else { return }
            self.processingDue = false
            self.lastDueCheck = (day, version)
            if result.createdTransactions > 0 || result.updatedScheduled > 0 || result.deletedScheduled > 0 {
                self.dueResult = result
            }
        } failure: { [weak self] message in
            self?.processingDue = false
            self?.errorMessage = message
        }
    }

    // MARK: - Exécution

    /// Exécute une écriture synchrone et renvoie le message d'erreur à afficher, ou `nil` si tout va bien.
    @discardableResult
    public func attempt(_ work: (DmxEngine) throws -> Void) -> String? {
        do {
            try work(engine)
            reload()
            return nil
        } catch {
            return AppStore.message(for: error)
        }
    }

    /// Lecture synchrone ; en cas d'échec l'erreur est publiée et `nil` est renvoyé.
    public func read<T>(_ work: (DmxEngine) throws -> T) -> T? {
        do {
            return try work(engine)
        } catch {
            errorMessage = AppStore.message(for: error)
            return nil
        }
    }

    /// Lecture sans effet de bord, pour l'évaluation d'une vue : publier une erreur depuis un
    /// `body` modifierait l'état en pleine mise à jour SwiftUI, ce qui n'est pas permis.
    public func peek<T>(_ work: (DmxEngine) throws -> T) -> T? {
        try? work(engine)
    }

    /// Exécute un travail sur la file du moteur puis rappelle `completion` sur le fil principal.
    public func perform<T>(_ work: @escaping (DmxEngine) throws -> T, completion: ((T) -> Void)? = nil, failure: ((String) -> Void)? = nil) {
        let engine = self.engine
        queue.async { [weak self] in
            do {
                let value = try work(engine)
                DispatchQueue.main.async {
                    self?.reload()
                    completion?(value)
                }
            } catch {
                let message = AppStore.message(for: error)
                DispatchQueue.main.async {
                    if let failure = failure {
                        failure(message)
                    } else {
                        self?.errorMessage = message
                    }
                }
            }
        }
    }

    public func apply(_ change: SettingsChange) {
        if let message = attempt({ engine in _ = try engine.applySettingsChange(change: change) }) {
            errorMessage = message
        }
    }

    // MARK: - Filtre de comptes

    public var isFiltering: Bool { !selectedAccountIds.isEmpty }

    public func isSelected(accountId: String) -> Bool {
        selectedAccountIds.isEmpty || selectedAccountIds.contains(accountId)
    }

    public func toggleAccountFilter(_ accountId: String) {
        if selectedAccountIds.contains(accountId) {
            selectedAccountIds.removeAll { $0 == accountId }
        } else {
            selectedAccountIds.append(accountId)
            if selectedAccountIds.count == accounts.count {
                selectedAccountIds = []
            }
        }
    }

    public func clearAccountFilter() {
        selectedAccountIds = []
    }

    public func account(id: String?) -> Account? {
        guard let id = id else { return nil }
        return accounts.first { $0.id == id }
    }

    public func category(id: String?) -> DmxCategory? {
        guard let id = id else { return nil }
        return categories.first { $0.id == id }
    }

    // MARK: - Présentation (formulaires, confirmations, messages)

    public func present(_ request: FormRequest) {
        form = request
    }

    public func confirm(title: String, message: String, confirmTitle: String = "Supprimer", destructive: Bool = true, action: @escaping () -> Void) {
        confirmation = ConfirmRequest(title: title, message: message, confirmTitle: confirmTitle, destructive: destructive, action: action)
    }

    public func showToast(_ message: String) {
        toast = message
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) { [weak self] in
            if self?.toast == message { self?.toast = nil }
        }
    }

    /// Exécute une écriture ; affiche `success` en cas de réussite, l'erreur sinon.
    public func run(_ success: String? = nil, _ work: (DmxEngine) throws -> Void) {
        if let message = attempt(work) {
            errorMessage = message
        } else if let success = success {
            showToast(success)
        }
    }

    // MARK: - Pont PWA

    /// Pont PWA autorisé pour ce lancement (désactivé sur un dossier de test).
    public var bridgeEnabled = true

    public var bridgeAvailable: Bool {
        bridgeEnabled && engine.bridgeSupported()
    }

    public func refreshBridgeStatus() {
        guard bridgeAvailable else { return }
        perform({ engine in try engine.bridgeStatus() }, completion: { [weak self] status in
            self?.bridgeStatus = status
        }, failure: { _ in })
    }

    public func updateBridgeStatus(_ status: CompanionStatus) {
        bridgeStatus = status
    }

    // MARK: - Erreurs

    public static func message(for error: Error) -> String {
        if let error = error as? DmxError {
            switch error {
            case let .Database(message), let .Validation(message), let .NotFound(message), let .Import(message),
                 let .Io(message), let .Bridge(message), let .Unsupported(message):
                return message
            }
        }
        return error.localizedDescription
    }

    fileprivate func handleExternalChange(version: Int64) {
        guard version != dataVersion else { return }
        reload()
    }

    private static let placeholderSettings = AppSettings(
        settingsRevision: 0,
        theme: .system,
        primaryColor: "",
        windowPosition: nil,
        windowSize: nil,
        accountGroups: [:],
        customGroups: [],
        customGroupsOrder: nil,
        accountsOrder: nil,
        lastSeenVersion: nil,
        dismissedBudgetSuggestions: [],
        dismissedScheduledSuggestions: [],
        predictionTimeRange: .month,
        predictionCustomEndDate: nil,
        predictionAlertThreshold: 0,
        predictionMonthStartsOnFirst: false,
        predictionFakeTransactions: [],
        analyticsTimeRange: .month,
        analyticsCustomStartDate: nil,
        analyticsCustomEndDate: nil,
        analyticsMonthStartsOnFirst: false,
        analyticsHiddenExpenseCategories: [],
        analyticsHiddenIncomeCategories: [],
        scheduledDueRange: .all,
        accentColor: nil,
        effectiveGroupOrder: []
    )
}

/// Relais des notifications du noyau (écritures faites par le pont PWA ou la synchronisation).
private final class ListenerProxy: EngineListener, @unchecked Sendable {
    weak var store: AppStore?

    func dataChanged(dataVersion: Int64) {
        DispatchQueue.main.async { [weak self] in
            self?.store?.handleExternalChange(version: dataVersion)
        }
    }

    func bridgeStatusChanged() {
        DispatchQueue.main.async { [weak self] in
            self?.store?.refreshBridgeStatus()
        }
    }

    /// Reformulation d'une demande envoyée par la PWA. Appelée depuis un thread du pont, donc
    /// sans passer par le fil principal : l'hôte fournit une fonction synchrone ou rien du tout.
    func rephraseAssistantRequest(text: String) -> String? {
        store?.assistantRewriter?(text)
    }
}
