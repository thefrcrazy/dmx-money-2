import CloudKit
import Combine
import Foundation

/// Synchronisation iCloud au choix de l'utilisateur (macOS 14+ / iOS 17+, app signée avec CloudKit).
///
/// Le noyau tient le journal des changements (`sync_outbox`) ; ce contrôleur les pousse dans la base
/// privée CloudKit via `CKSyncEngine` et applique les changements reçus (le plus récent l'emporte).
public final class CloudSyncController: ObservableObject {
    public static let enabledKey = "DmxICloudSyncEnabled"

    private let store: AppStore
    private var backend: AnyObject?
    private var cancellable: AnyCancellable?
    private var backendGeneration: UInt64 = 0
    private let isolated = ProcessInfo.processInfo.environment["DMXMONEY_DATA_DIR"] != nil
    private lazy var defaults: UserDefaults = isolated
        ? UserDefaults(suiteName: "DmxMoney.CloudKit.fixture.\(Bundle.main.bundleIdentifier ?? "audit")")!
        : .standard
    @Published fileprivate(set) var lastSync: Date?
    @Published fileprivate(set) var lastError: String?

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
        return !isolated && containerIdentifier != nil && FileManager.default.ubiquityIdentityToken != nil
    }

    public var unavailableReason: String {
        if isolated { return "iCloud est désactivé dans ce dossier de données fictives." }
        guard #available(macOS 14.0, iOS 17.0, *) else {
            return "La synchronisation iCloud nécessite macOS 14 Sonoma ou plus récent. Le pont PWA reste disponible."
        }
        if containerIdentifier == nil {
            return "Cette version de DmxMoney n'est pas signée avec les droits iCloud (CloudKit)."
        }
        return "Connectez-vous à iCloud dans les réglages du système pour activer la synchronisation."
    }

    public var isEnabled: Bool {
        !isolated && defaults.bool(forKey: Self.enabledKey)
    }

    public func setEnabled(_ enabled: Bool) {
        guard !isolated else { return }
        objectWillChange.send()
        defaults.set(enabled, forKey: Self.enabledKey)
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
            syncNow: { [weak self] in self?.syncNow() },
            changes: objectWillChange.eraseToAnyPublisher()
        )
    }

    private func start(initial: Bool) {
        guard !isolated, #available(macOS 14.0, iOS 17.0, *), backend == nil, let container = containerIdentifier else { return }
        backendGeneration &+= 1
        let generation = backendGeneration
        let stateURL = URL(fileURLWithPath: store.engine.openReport().databasePath)
            .deletingLastPathComponent()
            .appendingPathComponent("cloudkit-sync-state.json")
        let backend = CloudSyncBackend(store: store, containerIdentifier: container, stateURL: stateURL, report: { [weak self] date, error in
            DispatchQueue.main.async {
                guard let self, self.backendGeneration == generation, self.isEnabled else { return }
                if let date = date { self.lastSync = date }
                self.lastError = error
            }
        }, suspended: { [weak self] reason in
            DispatchQueue.main.async {
                guard let self, self.backendGeneration == generation else { return }
                self.setEnabled(false)
                self.lastError = reason
            }
        })
        self.backend = backend
        backend.bootstrap(initial: initial)
        cancellable = store.$dataVersion
            .dropFirst()
            .debounce(for: .milliseconds(800), scheduler: RunLoop.main)
            .sink { [weak backend] _ in backend?.enqueuePending() }
    }

    private func stop() {
        backendGeneration &+= 1
        cancellable = nil
        guard #available(macOS 14.0, iOS 17.0, *), let backend = backend as? CloudSyncBackend else { return }
        backend.stop()
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
    private let suspended: (String) -> Void
    private var syncEngine: CKSyncEngine!
    private let lock = NSLock()
    private var stopped = false
    /// Dernier changement local connu par enregistrement CloudKit.
    private var outgoing: [String: SyncChange] = [:]
    /// Changement réellement envoyé (sert à l'acquittement).
    private var sent: [String: SyncChange] = [:]
    /// Enregistrements serveur connus (conservent l'étiquette de modification).
    private var serverRecords: [String: CKRecord] = [:]

    init(store: AppStore, containerIdentifier: String, stateURL: URL, report: @escaping (Date?, String?) -> Void, suspended: @escaping (String) -> Void) {
        self.store = store
        engine = store.engine
        self.stateURL = stateURL
        self.report = report
        self.suspended = suspended
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
        // Legacy queued physical deletes have no deletion clock and must never be sent.
        let legacyDeletes = syncEngine.state.pendingRecordZoneChanges.filter {
            if case .deleteRecord = $0 { return true }; return false
        }
        syncEngine.state.remove(pendingRecordZoneChanges: legacyDeletes)
        if initial {
            _ = try? engine.enqueueAllForSync()
            syncEngine.state.add(pendingDatabaseChanges: [.saveZone(CKRecordZone(zoneID: Self.zoneID))])
        }
        enqueuePending()
        syncNow()
    }

    func syncNow() {
        guard isRunning else { return }
        Task {
            do {
                try await syncEngine.fetchChanges()
                guard isRunning else { return }
                try await syncEngine.sendChanges()
                report(Date(), nil)
            } catch {
                report(nil, error.localizedDescription)
            }
        }
    }

    private var isRunning: Bool {
        lock.lock()
        defer { lock.unlock() }
        return !stopped
    }

    func stop() {
        lock.lock()
        stopped = true
        lock.unlock()
        syncEngine.state.remove(pendingRecordZoneChanges: syncEngine.state.pendingRecordZoneChanges)
        syncEngine.state.remove(pendingDatabaseChanges: syncEngine.state.pendingDatabaseChanges)
        reset()
        let currentEngine = syncEngine!
        Task { await currentEngine.cancelOperations() }
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
        guard isRunning else { return }
        guard let changes = try? engine.pendingSyncChanges(limit: 2000), !changes.isEmpty else { return }
        let queued = syncEngine.state.pendingRecordZoneChanges
        let saves = Set(queued.compactMap { pending -> String? in
            if case let .saveRecord(id) = pending { return id.recordName }
            return nil
        })
        var additions: [CKSyncEngine.PendingRecordZoneChange] = []
        var removals: [CKSyncEngine.PendingRecordZoneChange] = []
        lock.lock()
        for change in changes {
            let name = Self.recordName(change.entity, change.recordId)
            let isQueued = saves.contains(name)
            if outgoing[name]?.seq == change.seq && isQueued { continue }
            outgoing[name] = change
            let id = CKRecord.ID(recordName: name, zoneID: Self.zoneID)
            // A deletion is a versioned record, never a physical delete without its date.
            additions.append(.saveRecord(id))
            removals.append(.deleteRecord(id))
        }
        lock.unlock()
        guard !additions.isEmpty else { return }
        syncEngine.state.remove(pendingRecordZoneChanges: removals)
        syncEngine.state.add(pendingRecordZoneChanges: additions)
    }

    func nextRecordZoneChangeBatch(_ context: CKSyncEngine.SendChangesContext, syncEngine: CKSyncEngine) async -> CKSyncEngine.RecordZoneChangeBatch? {
        guard isRunning else { return nil }
        let scope = context.options.scope
        let changes = Array(syncEngine.state.pendingRecordZoneChanges.filter {
            guard case .saveRecord = $0 else { return false }
            return scope.contains($0)
        }.prefix(200))
        guard !changes.isEmpty else { return nil }
        return await CKSyncEngine.RecordZoneChangeBatch(pendingChanges: changes) { [weak self] recordID in
            self?.record(for: recordID)
        }
    }

    private func record(for recordID: CKRecord.ID) -> CKRecord? {
        lock.lock()
        defer { lock.unlock() }
        let name = recordID.recordName
        guard !stopped else { return nil }
        guard let change = outgoing[name], change.deleted || change.payload != nil else { return nil }
        let record = serverRecords[name] ?? CKRecord(recordType: Self.recordType(change.entity), recordID: recordID)
        Self.write(change, to: record)
        sent[name] = change
        return record
    }

    // Reuses the deployed payload String field: no production schema change is required.
    static let tombstonePayload = "{\"$dmxTombstone\":1}"

    /// Preserves the author's deletion clock for the same LWW comparison as an edit.
    static func write(_ change: SyncChange, to record: CKRecord) {
        record["entity"] = change.entity
        record["recordId"] = change.recordId
        record["updatedAt"] = change.updatedAt
        if record["deleted"] != nil { record["deleted"] = nil }
        record["payload"] = change.deleted ? tombstonePayload : change.payload
    }

    static func remoteChange(from record: CKRecord) -> RemoteChange? {
        guard let entity = record["entity"] as? String,
              let recordId = record["recordId"] as? String,
              let updatedAt = record["updatedAt"] as? String,
              !entity.isEmpty, !recordId.isEmpty, !updatedAt.isEmpty,
              record.recordID.zoneID == zoneID,
              record.recordID.recordName == recordName(entity, recordId) else { return nil }
        let deleted = ((record["deleted"] as? NSNumber)?.boolValue ?? false)
            || record["payload"] as? String == tombstonePayload
        guard deleted || record["payload"] is String else { return nil }
        return RemoteChange(entity: entity, recordId: recordId, deleted: deleted, updatedAt: updatedAt,
                            payload: deleted ? nil : record["payload"] as? String)
    }

    // MARK: - Événements

    func handleEvent(_ event: CKSyncEngine.Event, syncEngine: CKSyncEngine) async {
        guard isRunning else { return }
        switch event {
        case let .stateUpdate(update):
            saveState(update.stateSerialization)
        case let .accountChange(change):
            handleAccountChange(change)
        case let .fetchedDatabaseChanges(changes):
            if changes.deletions.contains(where: { $0.zoneID == Self.zoneID }) {
                stop()
                suspended("Les données iCloud ont été effacées. Vos données locales sont conservées. Réactivez la synchronisation uniquement si vous souhaitez les republier dans iCloud.")
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
            // Les données locales ne sont jamais envoyées automatiquement à un autre compte.
            stop()
            suspended("Compte iCloud modifié : vérifiez le compte connecté avant de réactiver la synchronisation.")
        @unknown default:
            break
        }
    }

    private func applyFetched(modifications: [CKRecord], deletions: [CKRecord.ID]) async {
        if deletions.contains(where: { $0.zoneID == Self.zoneID }) {
            // Old clients physically deleted records; their original deletion time is lost.
            stop()
            suspended("Une suppression provenant d'une ancienne version iCloud a été détectée. Vos données locales sont conservées et la synchronisation est suspendue. Mettez tous vos appareils à jour avant de la réactiver.")
            return
        }
        guard let changes = ingest(modifications: modifications) else {
            stop()
            suspended("Un enregistrement iCloud est illisible. Vos données locales et les changements en attente sont conservés ; la synchronisation est suspendue.")
            return
        }
        await apply(changes)
    }

    /// Les sections critiques restent synchrones : un verrou ne se prend pas dans un contexte
    /// asynchrone (il bloquerait un fil du pool, et Swift 6 l'interdit).
    private func ingest(modifications: [CKRecord]) -> [RemoteChange]? {
        var remote: [RemoteChange] = []
        lock.lock()
        defer { lock.unlock() }
        for record in modifications where record.recordID.zoneID == Self.zoneID {
            guard let change = Self.remoteChange(from: record) else { return nil }
            serverRecords[record.recordID.recordName] = record
            remote.append(change)
        }
        return remote
    }

    private struct SentOutcome {
        var acknowledged: [SyncChange] = []
        var retry: [CKSyncEngine.PendingRecordZoneChange] = []
        var serverWins: [RemoteChange] = []
        var needsZone = false
        var invalidServerRecord = false
        var failure: String?
    }

    private func handleSent(_ result: CKSyncEngine.Event.SentRecordZoneChanges) async {
        let outcome = reconcileSent(result)

        if outcome.needsZone {
            stop()
            suspended("La zone iCloud a disparu. Vos données locales sont conservées. Réactivez la synchronisation uniquement pour les republier.")
            return
        }
        if outcome.invalidServerRecord {
            stop()
            suspended("Un conflit iCloud contient un enregistrement illisible. Vos données locales et les changements en attente sont conservés ; la synchronisation est suspendue.")
            return
        }
        if !outcome.retry.isEmpty {
            syncEngine.state.add(pendingRecordZoneChanges: outcome.retry)
        }
        guard await apply(outcome.serverWins) else { return }
        if !outcome.acknowledged.isEmpty {
            try? engine.acknowledgeSyncChanges(changes: outcome.acknowledged)
            enqueuePending()
        }
        report(outcome.failure == nil ? Date() : nil, outcome.failure)
    }

    private func reconcileSent(_ result: CKSyncEngine.Event.SentRecordZoneChanges) -> SentOutcome {
        var acknowledged: [SyncChange] = []
        var retry: [CKSyncEngine.PendingRecordZoneChange] = []
        var serverWins: [RemoteChange] = []
        var needsZone = false
        var invalidServerRecord = false
        var failure: String?

        lock.lock()
        for record in result.savedRecords {
            let name = record.recordID.recordName
            serverRecords[name] = record
            Self.acknowledgeSentRecord(record.recordID, sent: &sent, outgoing: &outgoing, acknowledged: &acknowledged, retry: &retry)
        }
        for recordID in result.deletedRecordIDs {
            let name = recordID.recordName
            serverRecords.removeValue(forKey: name)
            Self.acknowledgeSentRecord(recordID, sent: &sent, outgoing: &outgoing, acknowledged: &acknowledged, retry: &retry)
        }
        for failed in result.failedRecordSaves {
            let recordID = failed.record.recordID
            let name = recordID.recordName
            switch failed.error.code {
            case .serverRecordChanged:
                if !Self.reconcileServerConflict(failed.error.serverRecord, recordID: recordID,
                                                outgoing: &outgoing, acknowledged: &acknowledged,
                                                retry: &retry, serverWins: &serverWins) {
                    invalidServerRecord = true
                    break
                }
                serverRecords[name] = failed.error.serverRecord
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
            Self.acknowledgeSentRecord(recordID, sent: &sent, outgoing: &outgoing, acknowledged: &acknowledged, retry: &retry)
        }
        lock.unlock()

        return SentOutcome(acknowledged: acknowledged, retry: retry, serverWins: serverWins,
                           needsZone: needsZone, invalidServerRecord: invalidServerRecord, failure: failure)
    }

    /// Une réponse serveur illisible ne doit jamais acquitter un changement local.
    static func reconcileServerConflict(
        _ server: CKRecord?, recordID: CKRecord.ID,
        outgoing: inout [String: SyncChange], acknowledged: inout [SyncChange],
        retry: inout [CKSyncEngine.PendingRecordZoneChange], serverWins: inout [RemoteChange]
    ) -> Bool {
        guard let server = server, server.recordID == recordID,
              let remote = remoteChange(from: server) else { return false }
        let name = recordID.recordName
        if let change = outgoing[name], change.updatedAt >= remote.updatedAt {
            retry.append(.saveRecord(recordID))
        } else {
            serverWins.append(remote)
            if let change = outgoing.removeValue(forKey: name) { acknowledged.append(change) }
        }
        return true
    }

    /// Acquitte la version réellement envoyée et conserve une écriture arrivée pendant l'envoi.
    static func acknowledgeSentRecord(
        _ recordID: CKRecord.ID,
        sent: inout [String: SyncChange],
        outgoing: inout [String: SyncChange],
        acknowledged: inout [SyncChange],
        retry: inout [CKSyncEngine.PendingRecordZoneChange]
    ) {
        let name = recordID.recordName
        guard let change = sent.removeValue(forKey: name) else { return }
        acknowledged.append(change)
        if outgoing[name]?.seq == change.seq {
            outgoing.removeValue(forKey: name)
        } else if outgoing[name] != nil {
            retry.append(.saveRecord(recordID))
        }
    }

    @discardableResult
    private func apply(_ changes: [RemoteChange]) async -> Bool {
        guard isRunning else { return false }
        guard !changes.isEmpty else { return true }
        do {
            _ = try engine.applyRemoteChanges(changes: changes)
            await MainActor.run { store.reload() }
            return true
        } catch {
            stop()
            suspended("Un changement iCloud n'a pas pu être appliqué : \(AppStore.message(for: error)). La copie locale est conservée.")
            return false
        }
    }

    // MARK: - Utilitaires

    private static func recordName(_ entity: String, _ recordId: String) -> String {
        entity + separator + recordId
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

    private static func loadState(_ url: URL) -> CKSyncEngine.State.Serialization? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(CKSyncEngine.State.Serialization.self, from: data)
    }

    private func saveState(_ state: CKSyncEngine.State.Serialization) {
        guard let data = try? JSONEncoder().encode(state) else { return }
        try? data.write(to: stateURL, options: .atomic)
    }
}
