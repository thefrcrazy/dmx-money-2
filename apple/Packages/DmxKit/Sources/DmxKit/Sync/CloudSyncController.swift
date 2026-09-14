import CloudKit
import Combine
import Foundation

/// Synchronisation iCloud au choix de l'utilisateur (macOS 14+ / iOS 17+, app signée avec CloudKit).
///
/// Le noyau tient le journal des changements (`sync_outbox`) ; ce contrôleur les pousse dans la base
/// privée CloudKit via `CKSyncEngine` et applique les changements reçus (le plus récent l'emporte).
public final class CloudSyncController {
    public static let enabledKey = "DmxICloudSyncEnabled"

    private let store: AppStore
    private var backend: AnyObject?
    private var cancellable: AnyCancellable?
    fileprivate(set) var lastSync: Date?
    fileprivate(set) var lastError: String?

    public init(store: AppStore) {
        self.store = store
        if isEnabled && isAvailable {
            start(initial: false)
        }
    }

    /// Conteneur déclaré dans Info.plist (`DMX_ICLOUD_CONTAINER`), vide sans équipe de signature.
    public var containerIdentifier: String? {
        guard let value = Bundle.main.object(forInfoDictionaryKey: "DmxICloudContainer") as? String,
              !value.isEmpty, !value.hasPrefix("$(")
        else { return nil }
        return value
    }

    public var isAvailable: Bool {
        guard #available(macOS 14.0, iOS 17.0, *) else { return false }
        return containerIdentifier != nil && FileManager.default.ubiquityIdentityToken != nil
    }

    public var unavailableReason: String {
        guard #available(macOS 14.0, iOS 17.0, *) else {
            return "La synchronisation iCloud nécessite macOS 14 Sonoma ou plus récent. Le pont PWA reste disponible."
        }
        if containerIdentifier == nil {
            return "Cette version de DmxMoney n'est pas signée avec les droits iCloud (CloudKit)."
        }
        return "Connectez-vous à iCloud dans les réglages du système pour activer la synchronisation."
    }

    public var isEnabled: Bool {
        UserDefaults.standard.bool(forKey: Self.enabledKey)
    }

    public func setEnabled(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: Self.enabledKey)
        if enabled {
            start(initial: true)
        } else {
            stop()
        }
    }

    public func syncNow() {
        guard #available(macOS 14.0, iOS 17.0, *), let backend = backend as? CloudSyncBackend else { return }
        backend.syncNow()
    }

    public var statusText: String {
        if let error = lastError {
            return "Dernière erreur : \(error)"
        }
        let pending = (try? store.engine.pendingSyncCount()) ?? 0
        if pending > 0 {
            return "\(pending) \(pending > 1 ? "changements" : "changement") en attente d'envoi."
        }
        if let lastSync = lastSync {
            let formatter = DateFormatter()
            formatter.locale = Locale(identifier: "fr_FR")
            formatter.dateStyle = .none
            formatter.timeStyle = .short
            return "Synchronisé à \(formatter.string(from: lastSync))."
        }
        return "En attente de la première synchronisation."
    }

    public var settings: ICloudSettings {
        ICloudSettings(
            isAvailable: { [weak self] in self?.isAvailable ?? false },
            unavailableReason: { [weak self] in self?.unavailableReason ?? "" },
            isEnabled: { [weak self] in self?.isEnabled ?? false },
            setEnabled: { [weak self] enabled in self?.setEnabled(enabled) },
            status: { [weak self] in self?.statusText ?? "" },
            syncNow: { [weak self] in self?.syncNow() }
        )
    }

    private func start(initial: Bool) {
        guard #available(macOS 14.0, iOS 17.0, *), backend == nil, let container = containerIdentifier else { return }
        let stateURL = URL(fileURLWithPath: store.engine.openReport().databasePath)
            .deletingLastPathComponent()
            .appendingPathComponent("cloudkit-sync-state.json")
        let backend = CloudSyncBackend(store: store, containerIdentifier: container, stateURL: stateURL) { [weak self] date, error in
            DispatchQueue.main.async {
                if let date = date { self?.lastSync = date }
                self?.lastError = error
            }
        }
        self.backend = backend
        backend.bootstrap(initial: initial)
        cancellable = store.$dataVersion
            .dropFirst()
            .debounce(for: .milliseconds(800), scheduler: RunLoop.main)
            .sink { [weak backend] _ in backend?.enqueuePending() }
    }

    private func stop() {
        cancellable = nil
        guard #available(macOS 14.0, iOS 17.0, *), let backend = backend as? CloudSyncBackend else { return }
        backend.reset()
        self.backend = nil
        lastError = nil
    }
}

@available(macOS 14.0, iOS 17.0, *)
final class CloudSyncBackend: CKSyncEngineDelegate, @unchecked Sendable {
    static let zoneID = CKRecordZone.ID(zoneName: "DmxMoney", ownerName: CKCurrentUserDefaultName)
    private static let separator = "::"

    private let store: AppStore
    private let engine: DmxEngine
    private let stateURL: URL
    private let report: (Date?, String?) -> Void
    private var syncEngine: CKSyncEngine!
    private let lock = NSLock()
    /// Dernier changement local connu par enregistrement CloudKit.
    private var outgoing: [String: SyncChange] = [:]
    /// Changement réellement envoyé (sert à l'acquittement).
    private var sent: [String: SyncChange] = [:]
    /// Enregistrements serveur connus (conservent l'étiquette de modification).
    private var serverRecords: [String: CKRecord] = [:]

    init(store: AppStore, containerIdentifier: String, stateURL: URL, report: @escaping (Date?, String?) -> Void) {
        self.store = store
        engine = store.engine
        self.stateURL = stateURL
        self.report = report
        let container = CKContainer(identifier: containerIdentifier)
        var configuration = CKSyncEngine.Configuration(
            database: container.privateCloudDatabase,
            stateSerialization: Self.loadState(stateURL),
            delegate: self
        )
        configuration.automaticallySync = true
        syncEngine = CKSyncEngine(configuration)
    }

    func bootstrap(initial: Bool) {
        if initial {
            _ = try? engine.enqueueAllForSync()
            syncEngine.state.add(pendingDatabaseChanges: [.saveZone(CKRecordZone(zoneID: Self.zoneID))])
        }
        enqueuePending()
        syncNow()
    }

    func syncNow() {
        Task {
            do {
                try await syncEngine.fetchChanges()
                try await syncEngine.sendChanges()
                report(Date(), nil)
            } catch {
                report(nil, error.localizedDescription)
            }
        }
    }

    func reset() {
        try? FileManager.default.removeItem(at: stateURL)
        lock.lock()
        outgoing = [:]
        sent = [:]
        serverRecords = [:]
        lock.unlock()
    }

    // MARK: - Envoi

    func enqueuePending() {
        guard let changes = try? engine.pendingSyncChanges(limit: 2000), !changes.isEmpty else { return }
        var additions: [CKSyncEngine.PendingRecordZoneChange] = []
        var removals: [CKSyncEngine.PendingRecordZoneChange] = []
        lock.lock()
        for change in changes {
            let name = Self.recordName(change.entity, change.recordId)
            if outgoing[name]?.seq == change.seq { continue }
            outgoing[name] = change
            let id = CKRecord.ID(recordName: name, zoneID: Self.zoneID)
            additions.append(change.deleted ? .deleteRecord(id) : .saveRecord(id))
            removals.append(change.deleted ? .saveRecord(id) : .deleteRecord(id))
        }
        lock.unlock()
        guard !additions.isEmpty else { return }
        syncEngine.state.remove(pendingRecordZoneChanges: removals)
        syncEngine.state.add(pendingRecordZoneChanges: additions)
    }

    func nextRecordZoneChangeBatch(_ context: CKSyncEngine.SendChangesContext, syncEngine: CKSyncEngine) async -> CKSyncEngine.RecordZoneChangeBatch? {
        let scope = context.options.scope
        let changes = syncEngine.state.pendingRecordZoneChanges.filter { scope.contains($0) }
        guard !changes.isEmpty else { return nil }
        return await CKSyncEngine.RecordZoneChangeBatch(pendingChanges: changes) { [weak self] recordID in
            self?.record(for: recordID)
        }
    }

    private func record(for recordID: CKRecord.ID) -> CKRecord? {
        lock.lock()
        defer { lock.unlock() }
        let name = recordID.recordName
        guard let change = outgoing[name], let payload = change.payload else { return nil }
        let record = serverRecords[name] ?? CKRecord(recordType: Self.recordType(change.entity), recordID: recordID)
        record["entity"] = change.entity
        record["recordId"] = change.recordId
        record["updatedAt"] = change.updatedAt
        record["payload"] = payload
        sent[name] = change
        return record
    }

    // MARK: - Événements

    func handleEvent(_ event: CKSyncEngine.Event, syncEngine: CKSyncEngine) async {
        switch event {
        case let .stateUpdate(update):
            saveState(update.stateSerialization)
        case let .accountChange(change):
            handleAccountChange(change)
        case let .fetchedDatabaseChanges(changes):
            if changes.deletions.contains(where: { $0.zoneID == Self.zoneID }) {
                // Données iCloud effacées depuis un autre appareil : on renvoie tout.
                clearCaches()
                _ = try? engine.enqueueAllForSync()
                syncEngine.state.add(pendingDatabaseChanges: [.saveZone(CKRecordZone(zoneID: Self.zoneID))])
                enqueuePending()
            }
        case let .fetchedRecordZoneChanges(changes):
            await applyFetched(
                modifications: changes.modifications.map { $0.record },
                deletions: changes.deletions.map { $0.recordID }
            )
        case let .sentRecordZoneChanges(result):
            await handleSent(result)
        case .didFetchChanges, .didSendChanges:
            report(Date(), nil)
        default:
            break
        }
    }

    private func handleAccountChange(_ change: CKSyncEngine.Event.AccountChange) {
        switch change.changeType {
        case .signIn:
            _ = try? engine.enqueueAllForSync()
            syncEngine.state.add(pendingDatabaseChanges: [.saveZone(CKRecordZone(zoneID: Self.zoneID))])
            enqueuePending()
        case .signOut, .switchAccounts:
            reset()
            _ = try? engine.enqueueAllForSync()
            enqueuePending()
        @unknown default:
            break
        }
    }

    private func applyFetched(modifications: [CKRecord], deletions: [CKRecord.ID]) async {
        await apply(ingest(modifications: modifications, deletions: deletions))
    }

    /// Les sections critiques restent synchrones : un verrou ne se prend pas dans un contexte
    /// asynchrone (il bloquerait un fil du pool, et Swift 6 l'interdit).
    private func clearCaches() {
        lock.lock()
        outgoing = [:]
        serverRecords = [:]
        lock.unlock()
    }

    private func ingest(modifications: [CKRecord], deletions: [CKRecord.ID]) -> [RemoteChange] {
        var remote: [RemoteChange] = []
        lock.lock()
        for record in modifications where record.recordID.zoneID == Self.zoneID {
            guard let entity = record["entity"] as? String,
                  let recordId = record["recordId"] as? String,
                  let updatedAt = record["updatedAt"] as? String
            else { continue }
            serverRecords[record.recordID.recordName] = record
            remote.append(RemoteChange(entity: entity, recordId: recordId, deleted: false, updatedAt: updatedAt, payload: record["payload"] as? String))
        }
        for recordID in deletions where recordID.zoneID == Self.zoneID {
            guard let (entity, recordId) = Self.parse(recordID.recordName) else { continue }
            serverRecords.removeValue(forKey: recordID.recordName)
            remote.append(RemoteChange(entity: entity, recordId: recordId, deleted: true, updatedAt: Self.now(), payload: nil))
        }
        lock.unlock()
        return remote
    }

    private struct SentOutcome {
        var acknowledged: [SyncChange] = []
        var retry: [CKSyncEngine.PendingRecordZoneChange] = []
        var serverWins: [RemoteChange] = []
        var needsZone = false
        var failure: String?
    }

    private func handleSent(_ result: CKSyncEngine.Event.SentRecordZoneChanges) async {
        let outcome = reconcileSent(result)

        if outcome.needsZone {
            syncEngine.state.add(pendingDatabaseChanges: [.saveZone(CKRecordZone(zoneID: Self.zoneID))])
        }
        if !outcome.retry.isEmpty {
            syncEngine.state.add(pendingRecordZoneChanges: outcome.retry)
        }
        if !outcome.acknowledged.isEmpty {
            try? engine.acknowledgeSyncChanges(changes: outcome.acknowledged)
        }
        await apply(outcome.serverWins)
        report(outcome.failure == nil ? Date() : nil, outcome.failure)
    }

    private func reconcileSent(_ result: CKSyncEngine.Event.SentRecordZoneChanges) -> SentOutcome {
        var acknowledged: [SyncChange] = []
        var retry: [CKSyncEngine.PendingRecordZoneChange] = []
        var serverWins: [RemoteChange] = []
        var needsZone = false
        var failure: String?

        lock.lock()
        for record in result.savedRecords {
            let name = record.recordID.recordName
            serverRecords[name] = record
            if let change = sent.removeValue(forKey: name) {
                acknowledged.append(change)
                if outgoing[name]?.seq == change.seq { outgoing.removeValue(forKey: name) }
            }
        }
        for recordID in result.deletedRecordIDs {
            let name = recordID.recordName
            serverRecords.removeValue(forKey: name)
            if let change = outgoing.removeValue(forKey: name) {
                acknowledged.append(change)
            }
        }
        for failed in result.failedRecordSaves {
            let recordID = failed.record.recordID
            let name = recordID.recordName
            switch failed.error.code {
            case .serverRecordChanged:
                guard let server = failed.error.serverRecord else { continue }
                serverRecords[name] = server
                let serverUpdatedAt = server["updatedAt"] as? String ?? ""
                if let change = outgoing[name], change.updatedAt >= serverUpdatedAt {
                    retry.append(.saveRecord(recordID))
                } else {
                    if let entity = server["entity"] as? String, let recordId = server["recordId"] as? String {
                        serverWins.append(RemoteChange(entity: entity, recordId: recordId, deleted: false, updatedAt: serverUpdatedAt, payload: server["payload"] as? String))
                    }
                    if let change = outgoing.removeValue(forKey: name) {
                        acknowledged.append(change)
                    }
                }
            case .zoneNotFound:
                needsZone = true
                retry.append(.saveRecord(recordID))
            case .unknownItem:
                serverRecords.removeValue(forKey: name)
                retry.append(.saveRecord(recordID))
            case .networkFailure, .networkUnavailable, .zoneBusy, .serviceUnavailable, .requestRateLimited, .notAuthenticated, .operationCancelled:
                break
            default:
                failure = failed.error.localizedDescription
            }
            sent.removeValue(forKey: name)
        }
        for (recordID, error) in result.failedRecordDeletes where error.code == .unknownItem {
            if let change = outgoing.removeValue(forKey: recordID.recordName) {
                acknowledged.append(change)
            }
        }
        lock.unlock()

        return SentOutcome(acknowledged: acknowledged, retry: retry, serverWins: serverWins, needsZone: needsZone, failure: failure)
    }

    private func apply(_ changes: [RemoteChange]) async {
        guard !changes.isEmpty else { return }
        do {
            _ = try engine.applyRemoteChanges(changes: changes)
            await MainActor.run { store.reload() }
        } catch {
            report(nil, AppStore.message(for: error))
        }
    }

    // MARK: - Utilitaires

    private static func recordName(_ entity: String, _ recordId: String) -> String {
        entity + separator + recordId
    }

    private static func parse(_ recordName: String) -> (String, String)? {
        guard let range = recordName.range(of: separator) else { return nil }
        return (String(recordName[..<range.lowerBound]), String(recordName[range.upperBound...]))
    }

    private static func recordType(_ entity: String) -> String {
        switch entity {
        case "accounts": return "Account"
        case "categories": return "Category"
        case "budgets": return "Budget"
        case "scheduled": return "Scheduled"
        case "transactions": return "Transaction"
        default: return "Settings"
        }
    }

    private static func now() -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: Date())
    }

    private static func loadState(_ url: URL) -> CKSyncEngine.State.Serialization? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(CKSyncEngine.State.Serialization.self, from: data)
    }

    private func saveState(_ state: CKSyncEngine.State.Serialization) {
        guard let data = try? JSONEncoder().encode(state) else { return }
        try? data.write(to: stateURL, options: .atomic)
    }
}
