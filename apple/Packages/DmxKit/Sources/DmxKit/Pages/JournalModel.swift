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
        didSet { if oldValue != search { setNeedsRefresh(debounce: true) } }
    }

    public var categories: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { if oldValue != categories { filtersChanged() } }
    }

    public var types: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { if oldValue != types { filtersChanged() } }
    }

    public var statuses: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { if oldValue != statuses { filtersChanged() } }
    }

    public var budgetStatuses: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { if oldValue != budgetStatuses { filtersChanged() } }
    }

    public var selection: Set<String> = [] {
        willSet { objectWillChange.send() }
    }

    public private(set) var view: JournalView? = nil {
        willSet { objectWillChange.send() }
    }

    /// Invalide le tri des vues uniquement quand les données affichées ont été recalculées.
    public private(set) var rowsRevision: UInt64 = 0 {
        willSet { objectWillChange.send() }
    }

    private var updatingFilters = false

    /// Appelé après chaque recalcul (le tableau AppKit recharge ses lignes).
    public var onChange: (() -> Void)?

    public init(store: AppStore, active: Bool = true) {
        super.init(store: store)
        isActive = active
        if active { refresh() } else { setNeedsRefresh() }
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
        load({ try $0.journal(query: query) }) { [weak self] view in
            guard let self else { return }
            self.view = view
            self.rowsRevision &+= 1
            if !self.selection.isEmpty {
                let visible = Set(view.rows.map { $0.transaction.id })
                let kept = self.selection.intersection(visible)
                if kept != self.selection { self.selection = kept }
            }
            self.onChange?()
        }
    }

    public var rows: [JournalRow] { view?.rows ?? [] }

    public var hasFilters: Bool { view?.hasFilters ?? false }

    public func clearFilters() {
        guard !search.isEmpty || !categories.isEmpty || !types.isEmpty || !statuses.isEmpty || !budgetStatuses.isEmpty else { return }
        updatingFilters = true
        search = ""
        categories = []
        types = []
        statuses = []
        budgetStatuses = []
        updatingFilters = false
        setNeedsRefresh()
    }

    private func filtersChanged() {
        if !updatingFilters { setNeedsRefresh() }
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

    public func updateDescription(_ id: String, _ description: String, baseDescription: String? = nil) {
        let baseline = baseDescription ?? rows.first(where: { $0.transaction.id == id })?.transaction.description
        guard let baseline else { return }
        store.run("Transaction mise à jour") { engine in
            try engine.updateTransactionDescriptionWithBase(id: id, description: description, baseDescription: baseline)
        }
    }

    /// Renvoie `false` si le montant saisi est invalide.
    @discardableResult
    public func updateAmount(_ id: String, text: String, baseAmount: Double? = nil) -> Bool {
        guard let amount = AmountInput.parse(text), amount > 0 else {
            store.errorMessage = "Saisissez un montant valide"
            return false
        }
        guard let baseline = baseAmount ?? rows.first(where: { $0.transaction.id == id })?.transaction.amount else { return false }
        store.run("Transaction mise à jour") { engine in
            try engine.updateTransactionAmountWithBase(id: id, amount: amount, baseAmount: baseline)
        }
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
