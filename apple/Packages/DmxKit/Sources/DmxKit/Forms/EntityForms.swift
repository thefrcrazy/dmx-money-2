import SwiftUI

/// Construit le formulaire demandé (brouillon préparé par le noyau).
public struct FormHost: View {
    @EnvironmentObject private var store: AppStore
    private let request: FormRequest
    private let onClose: () -> Void

    public init(request: FormRequest, onClose: @escaping () -> Void) {
        self.request = request
        self.onClose = onClose
    }

    #if os(macOS)
    public var body: some View {
        content
            .frame(minWidth: CGFloat(request.preferredWidth))
            .background(DmxPalette.pageBackground)
    }
    #else
    // Sur iPhone la feuille occupe tout l'écran : contenu ancré en haut, pas de largeur imposée.
    public var body: some View {
        content
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .background(DmxPalette.pageBackground)
    }
    #endif

    @ViewBuilder
    private var content: some View {
        switch request {
        case let .account(id):
            if let draft = store.read({ try $0.accountDraft(id: id) }) {
                AccountForm(draft: draft, onClose: onClose)
            }
        case .accountGroups:
            AccountGroupsForm(onClose: onClose)
        case let .category(id):
            if let draft = store.read({ try $0.categoryDraft(id: id) }) {
                CategoryForm(draft: draft, onClose: onClose)
            }
        case let .transaction(id):
            if let draft = store.read({ try $0.transactionDraft(id: id, accounts: store.selectedAccountIds, today: store.today) }) {
                TransactionForm(draft: draft, onClose: onClose)
            }
        case let .budget(id):
            if let draft = store.read({ try $0.budgetDraft(id: id, accounts: store.selectedAccountIds) }) {
                BudgetForm(draft: draft, onClose: onClose)
            }
        case let .newBudget(categoryId):
            if let draft = newBudgetDraft(categoryId) {
                BudgetForm(draft: draft, onClose: onClose)
            }
        case .budgetSuggestions:
            BudgetSuggestionsView(onClose: onClose)
        case .scheduledSuggestions:
            ScheduledSuggestionsView(onClose: onClose)
        case let .scheduled(id):
            if let draft = store.read({ try $0.scheduledDraft(id: id, today: store.today) }) {
                ScheduledForm(draft: draft, onClose: onClose)
            }
        case let .fakeTransaction(id):
            if let draft = store.read({ try $0.fakeTransactionDraft(id: id, accounts: store.selectedAccountIds, today: store.today) }) {
                FakeTransactionForm(draft: draft, onClose: onClose)
            }
        case let .restoreBackup(content, fileName):
            RestoreBackupForm(content: content, fileName: fileName, onClose: onClose)
        case let .statementImport(content, fileName):
            StatementImportWizard(content: content, fileName: fileName, onClose: onClose)
        case .whatsNew:
            WhatsNewView(onClose: onClose)
        }
    }

    private func newBudgetDraft(_ categoryId: String) -> BudgetDraft? {
        guard var draft = store.read({ try $0.budgetDraft(id: nil, accounts: store.selectedAccountIds) }) else { return nil }
        draft.categoryId = categoryId
        if draft.name.isEmpty, let name = store.category(id: categoryId)?.name {
            draft.name = name
        }
        return draft
    }
}

// MARK: - Compte

struct AccountForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var draft: AccountDraft
    @State private var amount: String
    @State private var error: String?
    private let onClose: () -> Void

    init(draft: AccountDraft, onClose: @escaping () -> Void) {
        _draft = State(initialValue: draft)
        _amount = State(initialValue: AmountInput.text(draft.initialBalance))
        self.onClose = onClose
    }

    private var isEditing: Bool { draft.id != nil }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier le compte" : "Nouveau compte",
            submitTitle: isEditing ? "Mettre à jour" : "Créer",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            FormField("Nom du compte") {
                DmxTextField("Ex: Compte Courant", text: $draft.name)
            }
            HStack(alignment: .top, spacing: 14) {
                FormField("Type") {
                    ChoicePicker(
                        options: accountTypes().map { SelectOption(id: $0, label: $0) },
                        selection: Binding(get: { draft.accountType }, set: changeType)
                    )
                }
                FormField("Solde initial") {
                    AmountField(text: $amount)
                }
            }
            FormField("Groupe") {
                SearchableSelect(
                    "Aucun groupe",
                    options: store.settings.customGroups.map { SelectOption(id: $0, label: $0) },
                    selection: $draft.group,
                    noneLabel: "Aucun groupe"
                )
            }
            FormField("Couleur") {
                HStack(alignment: .top, spacing: 14) {
                    IconBadge(icon: draft.icon, colorHex: draft.color, size: 40, filled: true)
                    ColorGridPicker(selection: $draft.color, columns: 14)
                }
            }
        }
    }

    private func changeType(_ type: String) {
        let defaults = accountTypeDefaults(accountType: type)
        draft.accountType = type
        draft.icon = defaults.icon
        draft.color = defaults.color
    }

    private func submit() {
        guard let value = amount.trimmingCharacters(in: .whitespaces).isEmpty ? 0 : AmountInput.parse(amount) else {
            error = "Saisissez un montant valide"
            return
        }
        var draft = self.draft
        draft.initialBalance = value
        error = store.attempt { engine in _ = try engine.saveAccount(draft: draft) }
        if error == nil {
            store.showToast(isEditing ? "Compte mis à jour" : "Compte créé")
            onClose()
        }
    }
}

// MARK: - Groupes de comptes

struct AccountGroupsForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var newGroup = ""
    @State private var renaming: String?
    @State private var renameText = ""
    private let onClose: () -> Void

    init(onClose: @escaping () -> Void) {
        self.onClose = onClose
    }

    private var groups: [String] {
        store.settings.effectiveGroupOrder.filter { $0 != "Non groupés" }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Gérer les groupes").font(.system(size: 17, weight: .semibold))
                Spacer()
                IconButton("X", action: onClose)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            Divider()
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 8) {
                    TextField("Nouveau groupe...", text: $newGroup, onCommit: add)
                        .textFieldStyle(PlainTextFieldStyle())
                        .font(.system(size: 13))
                        .modifier(FieldBackground())
                    Button(action: add) { DmxIcon("Plus", size: 14) }
                        .buttonStyle(DmxButtonStyle(.primary))
                        .disabled(newGroup.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                if groups.isEmpty {
                    EmptyStateView(icon: "Users", title: "Aucun groupe", message: "Les comptes sans groupe apparaissent dans « Non groupés ».")
                }
                ForEach(Array(groups.enumerated()), id: \.element) { item in
                    groupRow(item.element, index: item.offset)
                }
            }
            .padding(20)
            Divider()
            HStack {
                Spacer()
                Button(action: onClose) { Text("Fermer") }.buttonStyle(DmxButtonStyle(.secondary))
            }
            .padding(16)
        }
    }

    private func groupRow(_ group: String, index: Int) -> some View {
        HStack(spacing: 6) {
            if renaming == group {
                TextField(group, text: $renameText, onCommit: { commitRename(group) })
                    .textFieldStyle(PlainTextFieldStyle())
                    .font(.system(size: 13))
                    .modifier(FieldBackground())
                IconButton("Check", color: DmxColors.income) { commitRename(group) }
                IconButton("X") { renaming = nil }
            } else {
                Text(group).font(.system(size: 13, weight: .medium)).lineLimit(1)
                Spacer()
                IconButton("ChevronUp") { move(index, by: -1) }.disabled(index == 0)
                IconButton("ChevronDown") { move(index, by: 1) }.disabled(index == groups.count - 1)
                IconButton("Edit2") {
                    renameText = group
                    renaming = group
                }
                IconButton("Trash2", color: DmxColors.expense) { delete(group) }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color.primary.opacity(0.03)))
    }

    private func add() {
        let name = newGroup.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        store.run { engine in _ = try engine.applySettingsChange(change: .addCustomGroup(name: name)) }
        newGroup = ""
    }

    private func commitRename(_ group: String) {
        let name = renameText.trimmingCharacters(in: .whitespaces)
        renaming = nil
        guard !name.isEmpty, name != group else { return }
        store.run { engine in _ = try engine.applySettingsChange(change: .renameCustomGroup(oldName: group, newName: name)) }
    }

    private func move(_ index: Int, by offset: Int) {
        var order = groups
        let target = index + offset
        guard order.indices.contains(index), order.indices.contains(target) else { return }
        order.swapAt(index, target)
        store.run { engine in _ = try engine.applySettingsChange(change: .setCustomGroupsOrder(order: order)) }
    }

    private func delete(_ group: String) {
        store.confirm(title: "Supprimer le groupe", message: "Les comptes du groupe « \(group) » seront déplacés dans « Non groupés ».") { [store] in
            store.run { engine in _ = try engine.applySettingsChange(change: .deleteCustomGroup(name: group)) }
        }
    }
}

// MARK: - Catégorie

struct CategoryForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var draft: CategoryDraft
    @State private var error: String?
    private let onClose: () -> Void

    init(draft: CategoryDraft, onClose: @escaping () -> Void) {
        _draft = State(initialValue: draft)
        self.onClose = onClose
    }

    private var isEditing: Bool { draft.id != nil }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier la catégorie" : "Nouvelle catégorie",
            submitTitle: isEditing ? "Modifier" : "Ajouter",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            HStack(alignment: .bottom, spacing: 14) {
                IconBadge(icon: draft.icon, colorHex: draft.color, size: 40, filled: true)
                FormField("Nom") {
                    DmxTextField("Ex: Loisirs", text: $draft.name)
                }
            }
            FormField("Icône") {
                IconGridPicker(selection: $draft.icon, color: Color(hex: draft.color), columns: 12)
            }
            FormField("Couleur") {
                ColorGridPicker(selection: $draft.color, columns: 17)
            }
        }
    }

    private func submit() {
        let draft = self.draft
        error = store.attempt { engine in _ = try engine.saveCategory(draft: draft) }
        if error == nil {
            store.showToast(isEditing ? "Catégorie modifiée" : "Catégorie ajoutée")
            onClose()
        }
    }
}

// MARK: - Transaction

struct TransactionForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var draft: TransactionDraft
    @State private var amount: String
    @State private var error: String?
    private let onClose: () -> Void

    init(draft: TransactionDraft, onClose: @escaping () -> Void) {
        _draft = State(initialValue: draft)
        _amount = State(initialValue: AmountInput.text(draft.amount))
        self.onClose = onClose
    }

    private var isEditing: Bool { draft.id != nil }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier la transaction" : "Nouvelle transaction",
            submitTitle: "Enregistrer",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            FormField("Type") {
                TransactionKindPicker(kind: $draft.kind)
            }
            FormField("Description") {
                DmxTextField("Ex: Loyer", text: $draft.description)
            }
            HStack(alignment: .top, spacing: 14) {
                FormField("Montant") { AmountField(text: $amount) }
                FormField("Date") { DayPicker(day: $draft.date) }
            }
            FormField(draft.kind == .transfer ? "Compte source" : "Compte") {
                SearchableSelect("Sélectionner un compte", options: store.accountOptions, required: $draft.accountId)
            }
            if draft.kind == .transfer {
                FormField("Compte destination") {
                    SearchableSelect("Sélectionner un compte", options: store.accountOptions, selection: $draft.toAccountId)
                }
            } else {
                FormField("Catégorie") {
                    SearchableSelect("Sélectionner une catégorie", options: store.categoryOptions, required: $draft.categoryId)
                }
            }
        }
    }

    private func submit() {
        guard let value = AmountInput.parse(amount), value > 0 else {
            error = "Saisissez un montant valide"
            return
        }
        var draft = self.draft
        draft.amount = value
        error = store.attempt { engine in _ = try engine.saveTransaction(draft: draft) }
        if error == nil {
            store.showToast(isEditing ? "Transaction mise à jour" : (draft.kind == .transfer ? "Virement ajouté" : "Transaction ajoutée"))
            onClose()
        }
    }
}

// MARK: - Budget

struct BudgetForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var draft: BudgetDraft
    @State private var amount: String
    @State private var error: String?
    private let onClose: () -> Void

    init(draft: BudgetDraft, onClose: @escaping () -> Void) {
        _draft = State(initialValue: draft)
        _amount = State(initialValue: AmountInput.text(draft.amount))
        self.onClose = onClose
    }

    private var isEditing: Bool { draft.id != nil }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier le budget" : "Nouveau budget",
            submitTitle: isEditing ? "Modifier" : "Créer",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            FormField("Nom") {
                DmxTextField("Ex: Courses, Essence, Loisirs", text: $draft.name)
            }
            FormField("Montant mensuel") {
                AmountField(text: $amount)
            }
            FormField("Catégorie") {
                SearchableSelect("Sélectionner", options: store.categoryOptions, required: $draft.categoryId)
            }
            FormField("Compte") {
                SearchableSelect("Tous les comptes", options: store.accountOptions, selection: $draft.accountId, noneLabel: "Tous les comptes")
            }
        }
    }

    private func submit() {
        guard let value = AmountInput.parse(amount), value > 0 else {
            error = "Saisissez un montant valide"
            return
        }
        var draft = self.draft
        draft.amount = value
        error = store.attempt { engine in _ = try engine.saveBudget(draft: draft) }
        if error == nil {
            store.showToast(isEditing ? "Budget modifié" : "Budget ajouté")
            onClose()
        }
    }
}

// MARK: - Échéance

struct ScheduledForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var draft: ScheduledDraft
    @State private var amount: String
    @State private var hasEndDate: Bool
    @State private var error: String?
    private let budgets: [Budget]
    private let onClose: () -> Void

    init(draft: ScheduledDraft, onClose: @escaping () -> Void, budgets: [Budget] = []) {
        _draft = State(initialValue: draft)
        _amount = State(initialValue: AmountInput.text(draft.amount))
        _hasEndDate = State(initialValue: draft.endDate != nil)
        self.budgets = budgets
        self.onClose = onClose
    }

    private var isEditing: Bool { draft.id != nil }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier la transaction" : "Nouvelle transaction récurrente",
            submitTitle: isEditing ? "Modifier" : "Ajouter",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            FormField("Type") {
                TransactionKindPicker(kind: $draft.kind)
            }
            FormField("Description") {
                DmxTextField("Ex: Loyer", text: $draft.description)
            }
            HStack(alignment: .top, spacing: 14) {
                FormField("Montant") { AmountField(text: $amount) }
                FormField("Fréquence") {
                    ChoicePicker(
                        options: allPeriodicities().map { SelectOption(id: periodicityKey($0), label: periodicityFormLabel(frequency: $0)) },
                        selection: Binding(get: { periodicityKey(draft.frequency) }, set: { key in
                            if let value = allPeriodicities().first(where: { periodicityKey($0) == key }) { draft.frequency = value }
                        })
                    )
                }
            }
            HStack(alignment: .top, spacing: 14) {
                FormField("Date de début") { DayPicker(day: $draft.nextDate) }
                FormField("Date de fin (optionnel)") {
                    HStack(spacing: 6) {
                        Toggle("", isOn: Binding(get: { hasEndDate }, set: { enabled in
                            hasEndDate = enabled
                            draft.endDate = enabled ? (draft.endDate ?? draft.nextDate) : nil
                        }))
                        .labelsHidden()
                        if hasEndDate {
                            DayPicker(day: Binding(get: { draft.endDate ?? draft.nextDate }, set: { draft.endDate = $0 }))
                        }
                    }
                }
            }
            if draft.kind == .expense {
                FormField("Budget lié") {
                    SearchableSelect(
                        "Hors budget",
                        options: linkableBudgets.map { SelectOption(id: $0.id, label: $0.name) },
                        selection: Binding(get: { draft.budgetId }, set: linkBudget),
                        noneLabel: "Hors budget"
                    )
                }
            }
            FormField(draft.kind == .transfer ? "Compte source" : "Compte") {
                SearchableSelect("Sélectionner un compte", options: store.accountOptions, required: $draft.accountId)
                    .disabled(linkedBudget?.accountId != nil)
            }
            if draft.kind == .transfer {
                FormField("Compte destination") {
                    SearchableSelect("Sélectionner un compte", options: store.accountOptions, selection: $draft.toAccountId)
                }
            } else {
                FormField("Catégorie") {
                    SearchableSelect("Sélectionner une catégorie", options: store.categoryOptions, required: $draft.categoryId)
                        .disabled(linkedBudget != nil)
                }
            }
        }
    }

    private var linkableBudgets: [Budget] {
        budgets.isEmpty ? (store.peek { try $0.budgetsList() } ?? []) : budgets
    }

    private var linkedBudget: Budget? {
        guard draft.kind == .expense, let id = draft.budgetId else { return nil }
        return linkableBudgets.first { $0.id == id }
    }

    /// Un budget lié impose sa catégorie et, s'il en a un, son compte (règle appliquée aussi par le noyau).
    private func linkBudget(_ id: String?) {
        draft.budgetId = id
        if let budget = linkableBudgets.first(where: { $0.id == id }) {
            draft.categoryId = budget.category
            if let accountId = budget.accountId {
                draft.accountId = accountId
            }
        }
    }

    private func submit() {
        guard let value = AmountInput.parse(amount), value > 0 else {
            error = "Saisissez un montant valide"
            return
        }
        var draft = self.draft
        draft.amount = value
        if draft.kind != .expense {
            draft.budgetId = nil
        }
        error = store.attempt { engine in _ = try engine.saveScheduled(draft: draft) }
        if error == nil {
            store.showToast(isEditing ? "Échéance modifiée" : "Échéance ajoutée")
            onClose()
        }
    }
}

func periodicityKey(_ value: Periodicity) -> String {
    String(describing: value)
}

// MARK: - Transaction fictive

struct FakeTransactionForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var draft: FakeTransactionDraft
    @State private var amount: String
    @State private var error: String?
    private let onClose: () -> Void

    init(draft: FakeTransactionDraft, onClose: @escaping () -> Void) {
        _draft = State(initialValue: draft)
        _amount = State(initialValue: AmountInput.text(draft.amount))
        self.onClose = onClose
    }

    private var isEditing: Bool { draft.id != nil }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier la transaction fictive" : "Nouvelle transaction fictive",
            submitTitle: isEditing ? "Enregistrer" : "Ajouter",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            FormField("Type") {
                TransactionKindPicker(kind: $draft.kind)
            }
            FormField("Description") {
                DmxTextField("Ex: Réparation voiture", text: $draft.description)
            }
            HStack(alignment: .top, spacing: 14) {
                FormField("Montant") { AmountField(text: $amount) }
                FormField("Date") { DayPicker(day: $draft.date) }
            }
            FormField(draft.kind == .transfer ? "Compte source" : "Compte") {
                SearchableSelect("Sélectionner un compte", options: store.accountOptions, required: $draft.accountId)
            }
            if draft.kind == .transfer {
                FormField("Compte destination") {
                    SearchableSelect("Sélectionner un compte", options: store.accountOptions, selection: $draft.toAccountId)
                }
            } else {
                FormField("Catégorie") {
                    SearchableSelect("Sélectionner une catégorie", options: store.categoryOptions, required: $draft.categoryId)
                }
            }
        }
    }

    private func submit() {
        guard let value = AmountInput.parse(amount), value > 0 else {
            error = "Saisissez un montant valide"
            return
        }
        var draft = self.draft
        draft.amount = value
        let today = store.today
        error = store.attempt { engine in _ = try engine.saveFakeTransaction(draft: draft, today: today) }
        if error == nil {
            store.showToast(isEditing ? "Transaction fictive mise à jour" : "Transaction fictive ajoutée")
            onClose()
        }
    }
}
