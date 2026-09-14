import DmxKit
import SwiftUI

/// Formulaires natifs de la variante modern : `Form` groupés dans une feuille dimensionnée,
/// avec les contrôles système (`TextField`, `Picker`, `DatePicker`, `ColorPicker`, `Stepper`).
/// Les brouillons viennent du noyau et y retournent sans transformation.
struct ModernFormHost: View {
    @EnvironmentObject private var store: AppStore
    let request: FormRequest
    let onClose: () -> Void

    var body: some View {
        // Les listes et l'assistant sont larges : ils se dimensionnent sur leur contenu,
        // les formulaires gardent la largeur standard d'une feuille de réglages.
        switch request {
        case .budgetSuggestions, .scheduledSuggestions, .statementImport, .restoreBackup, .whatsNew:
            form.presentationSizing(.fitted)
        default:
            form.presentationSizing(.form)
        }
    }

    @ViewBuilder
    private var form: some View {
        switch request {
        case let .account(id):
            draft({ try $0.accountDraft(id: id) }) { AccountForm(draft: $0, onClose: onClose) }
        case .accountGroups:
            AccountGroupsForm(onClose: onClose)
        case let .category(id):
            draft({ try $0.categoryDraft(id: id) }) { CategoryForm(draft: $0, onClose: onClose) }
        case let .transaction(id):
            draft({ try $0.transactionDraft(id: id, accounts: store.selectedAccountIds, today: store.today) }) {
                TransactionForm(draft: $0, onClose: onClose)
            }
        case let .budget(id):
            draft({ try $0.budgetDraft(id: id, accounts: store.selectedAccountIds) }) { BudgetForm(draft: $0, onClose: onClose) }
        case let .newBudget(categoryId):
            draft({ engine in
                var draft = try engine.budgetDraft(id: nil, accounts: store.selectedAccountIds)
                draft.categoryId = categoryId
                if draft.name.isEmpty { draft.name = store.category(id: categoryId)?.name ?? "" }
                return draft
            }) { BudgetForm(draft: $0, onClose: onClose) }
        case let .scheduled(id):
            draft({ try $0.scheduledDraft(id: id, today: store.today) }) { ScheduledForm(draft: $0, onClose: onClose) }
        case let .fakeTransaction(id):
            draft({ try $0.fakeTransactionDraft(id: id, accounts: store.selectedAccountIds, today: store.today) }) {
                FakeTransactionForm(draft: $0, onClose: onClose)
            }
        case .budgetSuggestions:
            ModernBudgetSuggestions(onClose: onClose)
        case .scheduledSuggestions:
            ModernScheduledSuggestions(onClose: onClose)
        case let .restoreBackup(content, fileName):
            ModernRestoreBackup(content: content, fileName: fileName, onClose: onClose)
        case let .statementImport(content, fileName):
            ModernStatementImport(content: content, fileName: fileName, onClose: onClose)
        case .whatsNew:
            ModernWhatsNew(onClose: onClose)
        }
    }

    /// Charge un brouillon par le noyau ; une erreur ferme la feuille avec un message.
    @ViewBuilder
    private func draft<T, Content: View>(
        _ load: (DmxEngine) throws -> T,
        @ViewBuilder content: (T) -> Content
    ) -> some View {
        if let draft = store.peek(load) {
            content(draft)
        } else {
            Color.clear.onAppear(perform: onClose)
        }
    }
}

// MARK: - Coquille commune

/// Feuille de formulaire : titre, `Form` groupé, message d'erreur et boutons.
struct FormSheet<Content: View>: View {
    let title: String
    let submitTitle: String
    var submitDisabled = false
    let error: String?
    let onCancel: () -> Void
    let onSubmit: () -> Void
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) {
            Form { content }
                .formStyle(.grouped)
            Divider()
            HStack {
                if let error {
                    Label(error, systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.red)
                        .font(.callout)
                }
                Spacer()
                Button("Annuler", role: .cancel, action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(submitTitle, action: onSubmit)
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .disabled(submitDisabled)
            }
            .padding(16)
        }
        .navigationTitle(title)
        .frame(minWidth: 460, minHeight: 320)
    }
}

/// Champ de montant : clavier numérique, alignement à droite, virgule ou point acceptés.
struct AmountField: View {
    let label: String
    @Binding var text: String

    var body: some View {
        TextField(label, text: $text, prompt: Text("0,00"))
            .multilineTextAlignment(.trailing)
            .monospacedDigit()
    }
}

/// Sélecteur de compte natif, avec pastille de couleur.
struct AccountPicker: View {
    @EnvironmentObject private var store: AppStore
    let label: String
    @Binding var selection: String
    var noneTitle: String?
    var isEnabled = true

    var body: some View {
        Picker(label, selection: $selection) {
            if let noneTitle {
                Text(noneTitle).tag("")
            }
            ForEach(store.accounts, id: \.id) { account in
                Label(account.name, systemImage: Symbols.name(for: account.icon)).tag(account.id)
            }
        }
        .disabled(!isEnabled)
    }
}

/// Sélecteur de catégorie natif (sans « Virement », réservée aux virements).
struct CategoryPicker: View {
    @EnvironmentObject private var store: AppStore
    let label: String
    @Binding var selection: String
    var isEnabled = true

    var body: some View {
        Picker(label, selection: $selection) {
            Text("Sélectionner une catégorie").tag("")
            ForEach(store.categories.filter { $0.id != "transfer" }, id: \.id) { category in
                Label(category.name, systemImage: Symbols.name(for: category.icon)).tag(category.id)
            }
        }
        .disabled(!isEnabled)
    }
}

/// Sélecteur de type d'opération.
private struct KindPicker: View {
    @Binding var kind: TransactionType

    var body: some View {
        Picker("Type", selection: $kind) {
            Text("Dépense").tag(TransactionType.expense)
            Text("Revenu").tag(TransactionType.income)
            Text("Virement").tag(TransactionType.transfer)
        }
        .pickerStyle(.segmented)
    }
}

/// Date du noyau (`YYYY-MM-DD`) éditée par un `DatePicker`.
private struct DayField: View {
    @EnvironmentObject private var store: AppStore
    let label: String
    @Binding var day: String
    @State private var showsCalendar = false

    private var date: Binding<Date> {
        Binding(
            get: { DayValue.date(from: day) ?? Date() },
            set: { day = DayValue.string(from: $0) }
        )
    }

    var body: some View {
        LabeledContent(label) {
            HStack(spacing: 6) {
                DatePicker("", selection: date, displayedComponents: .date)
                    .labelsHidden()
                Button {
                    showsCalendar.toggle()
                } label: {
                    Image(systemName: "calendar")
                }
                .buttonStyle(.borderless)
                .help("Choisir dans le calendrier")
                .popover(isPresented: $showsCalendar, arrowEdge: .bottom) {
                    VStack(spacing: 8) {
                        DatePicker("", selection: date, displayedComponents: .date)
                            .datePickerStyle(.graphical)
                            .labelsHidden()
                            .frame(width: 264)
                        HStack {
                            Button("Aujourd'hui") { day = store.today }
                            Spacer()
                            Button("Fermer") { showsCalendar = false }
                        }
                    }
                    .padding(12)
                }
            }
        }
    }
}

// MARK: - Compte

private struct AccountForm: View {
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
            submitDisabled: draft.name.trimmingCharacters(in: .whitespaces).isEmpty,
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            Section {
                TextField("Nom", text: $draft.name, prompt: Text("Ex : Compte courant"))
                Picker("Type", selection: Binding(get: { draft.accountType }, set: changeType)) {
                    ForEach(accountTypes(), id: \.self) { type in
                        Text(type).tag(type)
                    }
                }
                AmountField(label: "Solde initial", text: $amount)
            }
            Section("Apparence") {
                LabeledContent("Icône") {
                    HStack(spacing: 8) {
                        Image(systemName: Symbols.name(for: draft.icon))
                            .foregroundStyle(Color(hex: draft.color, fallback: .accentColor))
                        Text(draft.icon).foregroundStyle(.secondary)
                    }
                }
                ColorPicker("Couleur", selection: Binding(
                    get: { Color(hex: draft.color, fallback: .accentColor) },
                    set: { draft.color = $0.dmxHex ?? draft.color }
                ), supportsOpacity: false)
            }
            Section("Groupe") {
                Picker("Groupe", selection: Binding(
                    get: { draft.group ?? "" },
                    set: { draft.group = $0.isEmpty ? nil : $0 }
                )) {
                    Text("Aucun groupe").tag("")
                    ForEach(store.settings.customGroups, id: \.self) { group in
                        Text(group).tag(group)
                    }
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
        let trimmed = amount.trimmingCharacters(in: .whitespaces)
        guard let value = trimmed.isEmpty ? 0 : AmountInput.parse(trimmed) else {
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

private struct AccountGroupsForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var newGroup = ""
    @State private var editing: String?
    @State private var draft = ""
    private let onClose: () -> Void

    init(onClose: @escaping () -> Void) {
        self.onClose = onClose
    }

    private var groups: [String] {
        store.settings.effectiveGroupOrder.filter { $0 != "Non groupés" }
    }

    var body: some View {
        VStack(spacing: 0) {
            Form {
                Section("Nouveau groupe") {
                    HStack {
                        TextField("Nom du groupe", text: $newGroup, prompt: Text("Ex : Épargne"))
                            .onSubmit(add)
                        Button("Ajouter", action: add)
                            .disabled(newGroup.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
                Section("Groupes") {
                    if groups.isEmpty {
                        Text("Les comptes sans groupe apparaissent dans « Non groupés ».")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(Array(groups.enumerated()), id: \.element) { item in
                        HStack(spacing: 8) {
                            Image(systemName: "folder").foregroundStyle(.secondary)
                            if editing == item.element {
                                // Renommage en place : pas de panneau modal dans une feuille.
                                TextField("Nom du groupe", text: $draft)
                                    .textFieldStyle(.roundedBorder)
                                    .onSubmit { commitRename(item.element) }
                                Button("Renommer") { commitRename(item.element) }
                                    .disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty)
                                Button("Annuler") { editing = nil }
                            } else {
                                Text(item.element)
                                Spacer()
                                Button { move(item.offset, by: -1) } label: { Image(systemName: "chevron.up") }
                                    .buttonStyle(.plain)
                                    .disabled(item.offset == 0)
                                Button { move(item.offset, by: 1) } label: { Image(systemName: "chevron.down") }
                                    .buttonStyle(.plain)
                                    .disabled(item.offset == groups.count - 1)
                                Button { startRename(item.element) } label: { Image(systemName: "pencil") }
                                    .buttonStyle(.plain)
                                    .help("Renommer")
                                Button { delete(item.element) } label: { Image(systemName: "trash") }
                                    .buttonStyle(.plain)
                                    .foregroundStyle(.red)
                                    .help("Supprimer")
                            }
                        }
                    }
                }
            }
            .formStyle(.grouped)
            Divider()
            HStack {
                Spacer()
                Button("Fermer", action: onClose).keyboardShortcut(.defaultAction)
            }
            .padding(16)
        }
        .navigationTitle("Gérer les groupes")
        .frame(minWidth: 460, minHeight: 320)
    }

    private func add() {
        let name = newGroup.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        store.run { engine in _ = try engine.applySettingsChange(change: .addCustomGroup(name: name)) }
        newGroup = ""
    }

    private func move(_ index: Int, by offset: Int) {
        var order = groups
        let target = index + offset
        guard order.indices.contains(index), order.indices.contains(target) else { return }
        order.swapAt(index, target)
        store.run { engine in _ = try engine.applySettingsChange(change: .setCustomGroupsOrder(order: order)) }
    }

    private func startRename(_ group: String) {
        draft = group
        editing = group
    }

    private func commitRename(_ group: String) {
        let name = draft.trimmingCharacters(in: .whitespaces)
        editing = nil
        guard !name.isEmpty, name != group else { return }
        store.run("Groupe renommé") { engine in
            _ = try engine.applySettingsChange(change: .renameCustomGroup(oldName: group, newName: name))
        }
    }

    private func delete(_ group: String) {
        store.confirm(
            title: "Supprimer le groupe « \(group) » ?",
            message: "Ses comptes seront déplacés dans « Non groupés »."
        ) {
            store.run("Groupe supprimé") { engine in
                _ = try engine.applySettingsChange(change: .deleteCustomGroup(name: group))
            }
        }
    }
}

// MARK: - Catégorie

private struct CategoryForm: View {
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
            submitDisabled: draft.name.trimmingCharacters(in: .whitespaces).isEmpty,
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            Section {
                TextField("Nom", text: $draft.name, prompt: Text("Ex : Loisirs"))
                ColorPicker("Couleur", selection: Binding(
                    get: { Color(hex: draft.color, fallback: .accentColor) },
                    set: { draft.color = $0.dmxHex ?? draft.color }
                ), supportsOpacity: false)
            }
            Section("Icône") {
                IconGrid(selection: $draft.icon, color: Color(hex: draft.color, fallback: .accentColor))
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

/// Grille d'icônes : SF Symbols correspondant aux noms Lucide stockés en base.
private struct IconGrid: View {
    @Binding var selection: String
    let color: Color

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 34), spacing: 6)], spacing: 6) {
            ForEach(iconPickerNames(), id: \.self) { name in
                Button {
                    selection = name
                } label: {
                    Image(systemName: Symbols.name(for: name))
                        .frame(width: 28, height: 28)
                        .foregroundStyle(selection == name ? Color.white : color)
                        .background(selection == name ? color : Color.clear, in: RoundedRectangle(cornerRadius: 7))
                }
                .buttonStyle(.plain)
                .help(name)
            }
        }
        .frame(maxHeight: 180)
    }
}

// MARK: - Transaction

private struct TransactionForm: View {
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
    private var isTransfer: Bool { draft.kind == .transfer }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier la transaction" : "Nouvelle transaction",
            submitTitle: "Enregistrer",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            Section {
                KindPicker(kind: $draft.kind)
                TextField("Description", text: $draft.description, prompt: Text("Ex : Loyer"))
                AmountField(label: "Montant", text: $amount)
                DayField(label: "Date", day: $draft.date)
            }
            Section {
                AccountPicker(label: isTransfer ? "Compte source" : "Compte", selection: $draft.accountId)
                if isTransfer {
                    AccountPicker(label: "Compte destination", selection: Binding(
                        get: { draft.toAccountId ?? "" },
                        set: { draft.toAccountId = $0.isEmpty ? nil : $0 }
                    ), noneTitle: "Sélectionner un compte")
                } else {
                    CategoryPicker(label: "Catégorie", selection: $draft.categoryId)
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
            store.showToast(isEditing ? "Transaction mise à jour" : (isTransfer ? "Virement ajouté" : "Transaction ajoutée"))
            onClose()
        }
    }
}

// MARK: - Budget

private struct BudgetForm: View {
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
            submitDisabled: draft.name.trimmingCharacters(in: .whitespaces).isEmpty,
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            Section {
                TextField("Nom", text: $draft.name, prompt: Text("Ex : Courses"))
                LabeledContent("Montant mensuel") { AmountField(label: "Montant", text: $amount) }
            }
            Section {
                CategoryPicker(label: "Catégorie", selection: $draft.categoryId)
                AccountPicker(label: "Compte", selection: Binding(
                    get: { draft.accountId ?? "" },
                    set: { draft.accountId = $0.isEmpty ? nil : $0 }
                ), noneTitle: "Tous les comptes")
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

private struct ScheduledForm: View {
    @EnvironmentObject private var store: AppStore
    @State private var draft: ScheduledDraft
    @State private var amount: String
    @State private var hasEndDate: Bool
    @State private var error: String?
    private let onClose: () -> Void

    init(draft: ScheduledDraft, onClose: @escaping () -> Void) {
        _draft = State(initialValue: draft)
        _amount = State(initialValue: AmountInput.text(draft.amount))
        _hasEndDate = State(initialValue: draft.endDate != nil)
        self.onClose = onClose
    }

    private var isEditing: Bool { draft.id != nil }
    private var isTransfer: Bool { draft.kind == .transfer }
    private var budgets: [Budget] { store.peek { try $0.budgetsList() } ?? [] }
    private var linkedBudget: Budget? {
        guard draft.kind == .expense, let id = draft.budgetId else { return nil }
        return budgets.first { $0.id == id }
    }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier l'échéance" : "Nouvelle échéance",
            submitTitle: isEditing ? "Modifier" : "Ajouter",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            Section {
                KindPicker(kind: $draft.kind)
                TextField("Description", text: $draft.description, prompt: Text("Ex : Loyer"))
                AmountField(label: "Montant", text: $amount)
                Picker("Fréquence", selection: $draft.frequency) {
                    ForEach(allPeriodicities(), id: \.self) { frequency in
                        Text(periodicityFormLabel(frequency: frequency)).tag(frequency)
                    }
                }
            }
            Section {
                DayField(label: "Date de début", day: $draft.nextDate)
                Toggle("Date de fin", isOn: Binding(
                    get: { hasEndDate },
                    set: { enabled in
                        hasEndDate = enabled
                        draft.endDate = enabled ? (draft.endDate ?? draft.nextDate) : nil
                    }
                ))
                if hasEndDate {
                    DayField(label: "Jusqu'au", day: Binding(
                        get: { draft.endDate ?? draft.nextDate },
                        set: { draft.endDate = $0 }
                    ))
                }
            }
            Section {
                if draft.kind == .expense {
                    Picker("Budget lié", selection: Binding(get: { draft.budgetId ?? "" }, set: linkBudget)) {
                        Text("Hors budget").tag("")
                        ForEach(budgets, id: \.id) { budget in
                            Text(budget.name).tag(budget.id)
                        }
                    }
                }
                AccountPicker(
                    label: isTransfer ? "Compte source" : "Compte",
                    selection: $draft.accountId,
                    isEnabled: linkedBudget?.accountId == nil
                )
                if isTransfer {
                    AccountPicker(label: "Compte destination", selection: Binding(
                        get: { draft.toAccountId ?? "" },
                        set: { draft.toAccountId = $0.isEmpty ? nil : $0 }
                    ), noneTitle: "Sélectionner un compte")
                } else {
                    CategoryPicker(label: "Catégorie", selection: $draft.categoryId, isEnabled: linkedBudget == nil)
                }
            }
        }
    }

    /// Un budget lié impose sa catégorie et, s'il en a un, son compte (règle du noyau).
    private func linkBudget(_ id: String) {
        draft.budgetId = id.isEmpty ? nil : id
        if let budget = budgets.first(where: { $0.id == id }) {
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

// MARK: - Transaction fictive

private struct FakeTransactionForm: View {
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
    private var isTransfer: Bool { draft.kind == .transfer }

    var body: some View {
        FormSheet(
            title: isEditing ? "Modifier la transaction fictive" : "Nouvelle transaction fictive",
            submitTitle: isEditing ? "Enregistrer" : "Ajouter",
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            Section {
                KindPicker(kind: $draft.kind)
                TextField("Description", text: $draft.description, prompt: Text("Ex : Réparation voiture"))
                AmountField(label: "Montant", text: $amount)
                DayField(label: "Date", day: $draft.date)
            } footer: {
                Text("Elle modifie uniquement la projection et n'est pas ajoutée au journal.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Section {
                AccountPicker(label: isTransfer ? "Compte source" : "Compte", selection: $draft.accountId)
                if isTransfer {
                    AccountPicker(label: "Compte destination", selection: Binding(
                        get: { draft.toAccountId ?? "" },
                        set: { draft.toAccountId = $0.isEmpty ? nil : $0 }
                    ), noneTitle: "Sélectionner un compte")
                } else {
                    CategoryPicker(label: "Catégorie", selection: $draft.categoryId)
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
