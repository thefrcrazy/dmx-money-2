import XCTest
@testable import DmxKit

final class EngineSmokeTests: XCTestCase {
    private let today = "2026-09-11"

    func testInMemoryEngineSeedsDefaultsAndComputesDashboard() throws {
        let engine = try DmxEngine.openInMemory()
        XCTAssertFalse(coreVersion().isEmpty)
        XCTAssertTrue(try engine.categories().contains { $0.id == "transfer" })

        var account = try engine.accountDraft(id: nil)
        account.name = "Compte courant"
        account.initialBalance = 1200
        let accountId = try engine.saveAccount(draft: account)

        var transaction = try engine.transactionDraft(id: nil, accounts: [], today: today)
        transaction.kind = .expense
        transaction.amount = 45.5
        transaction.description = "Courses"
        transaction.accountId = accountId
        transaction.categoryId = "28"
        XCTAssertEqual(try engine.saveTransaction(draft: transaction).count, 1)

        let dashboard = try engine.dashboard(accounts: [], today: today)
        XCTAssertEqual(dashboard.balances.currentBalance, 1154.5, accuracy: 0.001)
        XCTAssertEqual(dashboard.month.expenses, 45.5, accuracy: 0.001)
        XCTAssertEqual(formatCurrency(amount: 1154.5), "1\u{202F}154,50\u{00A0}€")
    }

    /// Chemin emprunté par les intentions Siri : phrase ou paramètres structurés, réponse du noyau.
    func testAssistantAnswersThroughTheFfi() throws {
        let engine = try DmxEngine.openInMemory()
        var account = try engine.accountDraft(id: nil)
        account.name = "Compte Courant"
        account.initialBalance = 800
        let accountId = try engine.saveAccount(draft: account)

        let spoken = try engine.assistant(text: "ajoute 12,50 € en alimentation", today: today, apply: true)
        XCTAssertTrue(spoken.changed)
        XCTAssertTrue(spoken.understood)
        XCTAssertTrue(spoken.summary.contains("12,50"), spoken.summary)
        XCTAssertEqual(spoken.draft?.amount, 12.5)

        let balance = try engine.assistantBalance(account: "Compte Courant", today: today)
        XCTAssertFalse(balance.changed)
        XCTAssertTrue(balance.summary.contains("787,50"), balance.summary)

        let structured = try engine.assistantAddTransaction(
            amount: 18.9,
            kind: .expense,
            category: "Pharmacie",
            account: "Compte Courant",
            description: "Ordonnance",
            today: today
        )
        XCTAssertTrue(structured.changed)
        XCTAssertEqual(structured.draft?.accountId, accountId)
        XCTAssertEqual(structured.draft?.categoryId, "14")

        let unknown = try engine.assistant(text: "raconte une blague", today: today, apply: true)
        XCTAssertFalse(unknown.understood)
        XCTAssertFalse(unknown.changed)
    }

    func testValidationErrorsCarryFrenchMessages() throws {
        let engine = try DmxEngine.openInMemory()
        var draft = try engine.accountDraft(id: nil)
        draft.name = "   "
        XCTAssertThrowsError(try engine.saveAccount(draft: draft)) { error in
            guard case .Validation = error as? DmxError else {
                return XCTFail("erreur inattendue : \(error)")
            }
            XCTAssertFalse(AppStore.message(for: error).isEmpty)
        }
    }

    func testStorePublishesAccountsAndFilter() throws {
        let store = AppStore(engine: try DmxEngine.openInMemory())
        var draft = try store.engine.accountDraft(id: nil)
        draft.name = "Livret A"
        XCTAssertNil(store.attempt { engine in _ = try engine.saveAccount(draft: draft) })
        XCTAssertEqual(store.accounts.map(\.name), ["Livret A"])

        let id = store.accounts[0].id
        store.toggleAccountFilter(id)
        XCTAssertTrue(store.selectedAccountIds.isEmpty, "tous les comptes cochés = aucun filtre")
        XCTAssertTrue(store.isSelected(accountId: id))
    }

    func testVersionComparison() {
        XCTAssertTrue(AppInfo.isVersion("2.0.1", newerThan: "2.0.0"))
        XCTAssertTrue(AppInfo.isVersion("2.0.10", newerThan: "2.0.9"))
        XCTAssertTrue(AppInfo.isVersion("2.1", newerThan: "2.0.9"))
        XCTAssertFalse(AppInfo.isVersion("2.0.0", newerThan: "2.0.0"))
        XCTAssertFalse(AppInfo.isVersion("2.0.0", newerThan: "2.0.1"))
        // Une version affichée avec un suffixe reste comparable.
        XCTAssertTrue(AppInfo.isVersion("2.1.0-beta", newerThan: "2.0.9"))
    }

    func testIconsAreBundled() {
        XCTAssertTrue(DmxIcon.exists("Wallet"))
        XCTAssertEqual(DmxIcon.resolvedName("NoSuchIcon"), "Tag")
    }
}
