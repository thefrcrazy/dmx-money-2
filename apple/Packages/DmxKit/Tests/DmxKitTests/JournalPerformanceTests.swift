import Foundation
import XCTest
@testable import DmxKit

final class JournalPerformanceTests: XCTestCase {
    func testFormatCacheReusesExactResultsAndEvictsAtCapacity() {
        let cache = BoundedFormatCache<Int>(capacity: 2)
        var calls = 0
        func formatted(_ key: Int) -> String {
            cache.value(for: key) {
                calls += 1
                return "\(key):\(calls)"
            }
        }
        XCTAssertEqual(formatted(1), "1:1")
        XCTAssertEqual(formatted(2), "2:2")
        XCTAssertEqual(formatted(1), "1:1")
        XCTAssertEqual(formatted(3), "3:3")
        XCTAssertEqual(formatted(2), "2:2")
        XCTAssertEqual(formatted(1), "1:4")
        XCTAssertEqual(calls, 4)
    }

    func testConcurrentCacheMissComputesOnlyOnce() {
        let cache = BoundedFormatCache<Int>(capacity: 2)
        let calls = LockedCounter()
        DispatchQueue.concurrentPerform(iterations: 200) { _ in
            _ = cache.value(for: 1) { "result:\(calls.increment())" }
        }
        XCTAssertEqual(calls.value, 1)
        XCTAssertEqual(cache.value(for: 1) { "unexpected" }, "result:1")
    }

    func testCachedFinancialAndDateFormatsMatchTheKernel() {
        let amounts = [0.0, -0.0, 1.005, 2.675, -1234.567, 1234567890.12, Double.nan, Double.infinity]
        for amount in amounts {
            let exact = formatCurrency(amount: amount)
            XCTAssertEqual(Money.format(amount), exact)
            XCTAssertEqual(Money.format(amount), exact)
            XCTAssertEqual(Money.rounded(amount), formatCurrencyRounded(amount: amount))
            XCTAssertEqual(Money.signed(amount, display: .transfer, stored: .income), "+" + exact)
            XCTAssertEqual(Money.signed(amount, display: .transfer, stored: .expense), "-" + exact)
        }
        for date in ["2026-09-14", "2024-02-29", "2026-12-31", "invalid", ""] {
            for _ in 0..<2 {
                XCTAssertEqual(DayFormat.numeric(date), formatDateNumeric(date: date))
                XCTAssertEqual(DayFormat.short(date), formatDateShort(date: date))
                XCTAssertEqual(DayFormat.medium(date), formatDateMedium(date: date))
                XCTAssertEqual(DayFormat.long(date), formatDateLong(date: date))
            }
        }
    }

    @MainActor
    func testUnchangedReloadDoesNotInvalidatePagesButWritesDo() throws {
        let store = AppStore(engine: try DmxEngine.openInMemory())
        let revision = store.revision
        for _ in 0..<100 { store.reload() }
        XCTAssertEqual(store.revision, revision)

        var draft = try store.engine.accountDraft(id: nil)
        draft.name = "Fixture"
        XCTAssertNil(store.attempt { engine in _ = try engine.saveAccount(draft: draft) })
        XCTAssertGreaterThan(store.revision, revision)
        XCTAssertEqual(store.accounts.map(\.name), ["Fixture"])

        let accountRevision = store.revision
        let theme: Theme = store.settings.theme == .dark ? .light : .dark
        store.apply(.setTheme(theme: theme))
        XCTAssertEqual(store.settings.theme, theme)
        XCTAssertGreaterThan(store.revision, accountRevision)
        let settingsRevision = store.revision
        store.reload()
        XCTAssertEqual(store.revision, settingsRevision)
    }

    @MainActor
    func testJournalKeepsVisibleSelectionAndDropsFilteredRows() async throws {
        let engine = try DmxEngine.openInMemory()
        var account = try engine.accountDraft(id: nil)
        account.name = "Fixture"
        let accountID = try engine.saveAccount(draft: account)
        var draft = try engine.transactionDraft(id: nil, accounts: [], today: "2026-09-14")
        draft.kind = .expense
        draft.amount = 12.5
        draft.description = "Fixture sélection"
        draft.accountId = accountID
        draft.categoryId = "28"
        let transactionID = try XCTUnwrap(engine.saveTransaction(draft: draft).first)
        let journal = JournalModel(store: AppStore(engine: engine))
        await waitForRefresh(journal) { journal.setNeedsRefresh() }
        journal.selection = [transactionID]
        await waitForRefresh(journal) { journal.search = "Fixture" }
        XCTAssertEqual(journal.selection, [transactionID])
        await waitForRefresh(journal) { journal.search = "aucune correspondance" }
        XCTAssertTrue(journal.selection.isEmpty)
        await waitForRefresh(journal) { journal.clearFilters() }
        XCTAssertFalse(journal.rows.isEmpty)
        XCTAssertTrue(journal.selection.isEmpty)
    }
    @MainActor
    func testScheduledPreparationRevisionAdvancesOnlyForVisibleQueryResults() async throws {
        let model = ScheduledModel(store: AppStore(engine: try DmxEngine.openInMemory()), active: false)
        XCTAssertEqual(model.rowsRevision, 0)
        XCTAssertNil(model.view)
        model.isActive = true
        await waitForScheduledRevision(model, atLeast: 1)
        XCTAssertNotNil(model.view)
        let visibleRevision = model.rowsRevision
        model.isActive = false
        model.search = "Fictif"
        try await Task.sleep(nanoseconds: 200_000_000)
        XCTAssertEqual(model.rowsRevision, visibleRevision)
        model.isActive = true
        await waitForScheduledRevision(model, atLeast: visibleRevision + 1)
        XCTAssertNotNil(model.view)
    }

    @MainActor
    private func waitForScheduledRevision(_ model: ScheduledModel, atLeast revision: UInt64) async {
        let deadline = Date().addingTimeInterval(3)
        while model.rowsRevision < revision && Date() < deadline {
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTAssertGreaterThanOrEqual(model.rowsRevision, revision)
    }

    @MainActor
    private func waitForRefresh(_ journal: JournalModel, change: () -> Void) async {
        let loaded = expectation(description: "Journal background result")
        journal.onChange = { loaded.fulfill() }
        change()
        await fulfillment(of: [loaded], timeout: 3)
        journal.onChange = nil
    }

}

private final class LockedCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0

    var value: Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }

    func increment() -> Int {
        lock.lock()
        defer { lock.unlock() }
        count += 1
        return count
    }
}
