import Foundation
import XCTest
@testable import DmxKit

final class AppleAuditRegressionTests: XCTestCase {
    func testImportReadIsBoundedAndBackupHasSeparateLimit() throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let statement = folder.appendingPathComponent("fixture.csv")
        let data = Data(repeating: 65, count: FileText.maximumBytes + 1)
        try data.write(to: statement)
        XCTAssertThrowsError(try FileText.readChecked(statement)) { error in
            XCTAssertEqual(error.localizedDescription, "Le fichier dépasse la limite de 16 Mio.")
        }
        let backup = folder.appendingPathComponent("fixture.dmx")
        try FileManager.default.moveItem(at: statement, to: backup)
        XCTAssertEqual(try FileText.readChecked(backup).utf8.count, data.count)
        let file = try FileHandle(forWritingTo: backup)
        file.truncateFile(atOffset: UInt64(FileText.maximumBackupBytes + 1))
        file.closeFile()
        XCTAssertThrowsError(try FileText.readChecked(backup)) { error in
            XCTAssertEqual(error.localizedDescription, "Le fichier dépasse la limite de 64 Mio.")
        }
        XCTAssertThrowsError(try FileText.readChecked(folder))
        XCTAssertThrowsError(try FileText.readChecked(URL(string: "https://example.invalid/file.csv")!))
    }

    @MainActor
    func testBusyRestoreCannotBeDismissedOrReplacedAndLateCloseCannotCloseNewForm() throws {
        let store = AppStore(engine: try DmxEngine.openInMemory())
        store.present(.restoreBackup(content: "fixture", fileName: "fixture.dmx"))
        let original = store.formGeneration
        let work = store.beginFormWork()
        store.closeForm(generation: original)
        store.present(.account(id: nil))
        XCTAssertEqual(store.formGeneration, original)
        XCTAssertTrue(store.formBusy)
        store.finishFormWork(generation: work)
        store.closeForm(generation: original)
        XCTAssertNil(store.form)
        store.present(.account(id: nil))
        let next = store.formGeneration
        store.closeForm(generation: original)
        XCTAssertNotNil(store.form)
        XCTAssertEqual(store.formGeneration, next)
    }

    @MainActor
    func testCancelledReadCannotDeliverLateResult() async throws {
        let store = AppStore(engine: try DmxEngine.openInMemory())
        let delivered = expectation(description: "Cancelled work is ignored")
        delivered.isInverted = true
        let task = store.fetch({ _ in 123 }, completion: { _ in delivered.fulfill() }, failure: { _ in delivered.fulfill() })
        task.cancel()
        await fulfillment(of: [delivered], timeout: 0.15)
    }

    @MainActor
    func testSearchDebouncePublishesOnlyLatestResultAndDoesNotReadWhileHidden() async throws {
        let store = AppStore(engine: try DmxEngine.openInMemory())
        let model = ProbePageModel(store: store)
        let done = expectation(description: "One final debounced query")
        model.finished = { done.fulfill() }
        for _ in 0..<100 { model.setNeedsRefresh(debounce: true) }
        await fulfillment(of: [done], timeout: 2)
        XCTAssertEqual(model.refreshes, 1)
        model.isActive = false
        for _ in 0..<100 { model.setNeedsRefresh(debounce: true) }
        XCTAssertEqual(model.refreshes, 1)
        let active = expectation(description: "One query at activation")
        model.finished = { active.fulfill() }
        model.isActive = true
        await fulfillment(of: [active], timeout: 2)
        XCTAssertEqual(model.refreshes, 2)
    }

    @MainActor
    func testHidingDuringDebounceDoesNotStartADuplicateQueryAfterActivation() async throws {
        let model = ProbePageModel(store: AppStore(engine: try DmxEngine.openInMemory()))
        model.setNeedsRefresh(debounce: true)
        model.isActive = false
        let loaded = expectation(description: "Reactivation queries once")
        model.finished = { loaded.fulfill() }
        model.isActive = true
        await fulfillment(of: [loaded], timeout: 2)
        let duplicate = expectation(description: "Cancelled timer cannot query again")
        duplicate.isInverted = true
        model.finished = { duplicate.fulfill() }
        await fulfillment(of: [duplicate], timeout: 0.2)
        XCTAssertEqual(model.refreshes, 1)
    }

    @MainActor
    func testPageReadsExecuteOutsideTheMainThread() async throws {
        let store = AppStore(engine: try DmxEngine.openInMemory())
        let completed = expectation(description: "Background engine queue")
        store.fetch({ _ in Thread.isMainThread }, completion: { onMain in
            XCTAssertFalse(onMain)
            XCTAssertTrue(Thread.isMainThread)
            completed.fulfill()
        }, failure: { _ in XCTFail("Unexpected read failure"); completed.fulfill() })
        await fulfillment(of: [completed], timeout: 2)
    }

    @MainActor
    func testInactivePageDoesNotStartAQueryUntilActivated() async throws {
        let journal = JournalModel(store: AppStore(engine: try DmxEngine.openInMemory()), active: false)
        XCTAssertNil(journal.view)
        XCTAssertFalse(journal.isLoading)
        let ready = expectation(description: "First query starts at activation")
        journal.onChange = { ready.fulfill() }
        journal.isActive = true
        await fulfillment(of: [ready], timeout: 2)
        XCTAssertNotNil(journal.view)
    }

    @MainActor
    func testFictitiousDataModeNeverEnablesCloudKit() throws {
        let previous = ProcessInfo.processInfo.environment["DMXMONEY_DATA_DIR"]
        setenv("DMXMONEY_DATA_DIR", "/private/tmp/dmx-audit-fixture-unit-test", 1)
        defer {
            if let previous { setenv("DMXMONEY_DATA_DIR", previous, 1) }
            else { unsetenv("DMXMONEY_DATA_DIR") }
        }
        let cloud = CloudSyncController(store: AppStore(engine: try DmxEngine.openInMemory()))
        XCTAssertFalse(cloud.isAvailable)
        XCTAssertFalse(cloud.isEnabled)
        cloud.setEnabled(true)
        cloud.syncNow()
        XCTAssertFalse(cloud.isEnabled)
        XCTAssertEqual(cloud.unavailableReason, "iCloud est désactivé dans ce dossier de données fictives.")
    }

    func testVersionParsingRejectsMalformedFeedVersionsAndIgnoresBuildMetadata() {
        XCTAssertFalse(AppInfo.isVersion("", newerThan: "2.1.0"))
        XCTAssertFalse(AppInfo.isVersion("garbage", newerThan: "2.1.0"))
        XCTAssertFalse(AppInfo.isVersion("2.1.0+123", newerThan: "2.1.0"))
        XCTAssertTrue(AppInfo.isVersion("2.1.0-rc.10+123", newerThan: "2.1.0-rc.2+999"))
    }
}

private final class ProbePageModel: PageModel {
    var refreshes = 0
    var finished: (() -> Void)?
    override func refresh() {
        refreshes += 1
        load({ _ in true }) { [weak self] _ in self?.finished?() }
    }
}
