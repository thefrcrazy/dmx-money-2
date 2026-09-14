import Foundation

/// Journal : filtres, sélection multiple et éditions rapides, partagé par le tableau AppKit et la liste iOS.
public final class JournalModel: PageModel {
    public static let typeOptions = [
        SelectOption(id: "expense", label: "Dépenses", icon: "TrendingDown", color: "#ef4444"),
        SelectOption(id: "income", label: "Revenus", icon: "TrendingUp", color: "#10b981"),
        SelectOption(id: "transfer", label: "Virements", icon: "ArrowRightLeft", color: "#6366f1"),
    ]

    public static let statusOptions = [
        SelectOption(id: "checked", label: "Pointées", icon: "CheckCircle2", color: "#10b981"),
        SelectOption(id: "unchecked", label: "Non pointées", icon: "Circle", color: "#9ca3af"),
    ]

    public static let budgetOptions = [
        SelectOption(id: "budgeted", label: "Avec budget"),
        SelectOption(id: "unbudgeted", label: "Hors budget"),
    ]

    public var search = "" {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var categories: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var types: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var statuses: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var budgetStatuses: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var selection: Set<String> = [] {
        willSet { objectWillChange.send() }
    }

    public private(set) var view: JournalView? = nil {
        willSet { objectWillChange.send() }
    }

    /// Appelé après chaque recalcul (le tableau AppKit recharge ses lignes).
    public var onChange: (() -> Void)?

    public override init(store: AppStore) {
        super.init(store: store)
        refresh()
    }

    public override func refresh() {
        let query = JournalQuery(
            accounts: store.selectedAccountIds,
            search: search,
            categories: categories,
            types: types.compactMap(JournalModel.transactionType),
            statuses: statuses.map { $0 == "checked" ? .checked : .unchecked },
            budgetStatuses: budgetStatuses.map { $0 == "budgeted" ? .budgeted : .unbudgeted }
        )
        view = store.read { engine in try engine.journal(query: query) }
        let visible = Set(view?.rows.map { $0.transaction.id } ?? [])
        let kept = selection.intersection(visible)
        if kept != selection {
            selection = kept
        }
        onChange?()
    }

    public var rows: [JournalRow] { view?.rows ?? [] }

    public var hasFilters: Bool { view?.hasFilters ?? false }

    public func clearFilters() {
        search = ""
        categories = []
        types = []
        statuses = []
        budgetStatuses = []
    }

    private static func transactionType(_ key: String) -> TransactionType? {
        switch key {
        case "expense": return .expense
        case "income": return .income
        case "transfer": return .transfer
        default: return nil
        }
    }

    // MARK: - Actions

    public func toggleChecked(_ id: String) {
        store.run { engine in _ = try engine.toggleTransactionsChecked(ids: [id]) }
    }

    /// Pointer/Dépointer la sélection : tout pointé → tout dépointé, sinon tout pointé (règle du noyau).
    public func toggleCheckedSelection() {
        let ids = Array(selection)
        guard !ids.isEmpty else { return }
        store.run { engine in _ = try engine.toggleTransactionsChecked(ids: ids) }
    }

    public func updateDescription(_ id: String, _ description: String) {
        store.run("Transaction mise à jour") { engine in try engine.updateTransactionDescription(id: id, description: description) }
    }

    /// Renvoie `false` si le montant saisi est invalide.
    @discardableResult
    public func updateAmount(_ id: String, text: String) -> Bool {
        guard let amount = AmountInput.parse(text), amount > 0 else {
            store.errorMessage = "Saisissez un montant valide"
            return false
        }
        store.run("Transaction mise à jour") { engine in try engine.updateTransactionAmount(id: id, amount: amount) }
        return true
    }

    public func delete(_ id: String) {
        store.confirm(title: "Supprimer", message: "Voulez-vous vraiment supprimer cette transaction ?") { [store] in
            store.run("Transaction supprimée") { engine in try engine.deleteTransactions(ids: [id]) }
        }
    }

    public func deleteSelection() {
        let ids = Array(selection)
        guard !ids.isEmpty else { return }
        store.confirm(
            title: "Supprimer la sélection",
            message: "Voulez-vous vraiment supprimer \(plural(ids.count, "transaction")) ?",
            confirmTitle: "Tout supprimer"
        ) { [weak self, store] in
            store.run("Transactions supprimées") { engine in try engine.deleteTransactions(ids: ids) }
            self?.selection = []
        }
    }
}
