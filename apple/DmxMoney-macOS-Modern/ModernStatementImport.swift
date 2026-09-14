import DmxKit
import SwiftUI

/// Assistant d'import de relevés (CSV, QIF, OFX), en contrôles système.
///
/// Le fichier est analysé par le noyau : l'assistant ne fait que présenter l'aperçu, recueillir
/// l'affectation des colonnes, le compte de destination et la correspondance des catégories.
struct ModernStatementImport: View {
    private enum Step: Int, CaseIterable {
        case columns, account, categories, confirm

        var label: String {
            switch self {
            case .columns: return "Colonnes"
            case .account: return "Compte"
            case .categories: return "Catégories"
            case .confirm: return "Confirmation"
            }
        }
    }

    private enum ColumnRole: String, CaseIterable, Identifiable {
        case ignore, date, amount, description, category

        var id: String { rawValue }

        var label: String {
            switch self {
            case .ignore: return "Ignorer"
            case .date: return "Date"
            case .amount: return "Montant"
            case .description: return "Description"
            case .category: return "Catégorie"
            }
        }
    }

    private static let newAccountId = "__new__"
    private static let newCategoryId = "__new__"

    @EnvironmentObject private var store: AppStore
    let content: String
    let fileName: String
    let onClose: () -> Void

    @State private var format: StatementFormat = .csv
    @State private var step: Step = .columns
    @State private var loaded = false
    @State private var separator = ";"
    @State private var hasHeader = true
    @State private var mapping = CsvColumnMapping(date: 0, amount: 1, description: 3, category: nil)
    @State private var preview: CsvPreview?
    @State private var transactions: [ParsedStatementTransaction] = []
    @State private var sources: [String] = []
    @State private var categoryMapping: [String: String] = [:]
    @State private var accountChoice = ""
    @State private var newAccountName = ""
    @State private var newAccountType = "Courant"
    @State private var finalBalance = ""
    @State private var error: String?
    @State private var isImporting = false

    var body: some View {
        VStack(spacing: 0) {
            indicator
            Divider()
            stepContent
            Divider()
            footer
        }
        .navigationTitle(title)
        .frame(minWidth: 640, minHeight: 500)
        .onAppear(perform: load)
    }

    private var title: String {
        switch format {
        case .csv: return "Assistant d'import CSV"
        case .qif: return "Import QIF"
        case .ofx: return "Import OFX"
        }
    }

    private var visibleSteps: [Step] {
        format == .csv ? Step.allCases : [.account, .categories, .confirm]
    }

    // MARK: Étapes

    private var indicator: some View {
        HStack(spacing: 12) {
            ForEach(Array(visibleSteps.enumerated()), id: \.element) { item in
                let done = item.element.rawValue < step.rawValue
                Label {
                    Text(item.element.label)
                } icon: {
                    Image(systemName: done ? "checkmark.circle.fill" : "\(item.offset + 1).circle\(item.element == step ? ".fill" : "")")
                }
                .foregroundStyle(item.element == step ? AnyShapeStyle(.tint) : (done ? AnyShapeStyle(Color.green) : AnyShapeStyle(.secondary)))
                .fontWeight(item.element == step ? .semibold : .regular)
                if item.offset < visibleSteps.count - 1 {
                    Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.tertiary)
                }
            }
            Spacer(minLength: 0)
            Text(fileName).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
    }

    @ViewBuilder
    private var stepContent: some View {
        switch step {
        case .columns: columnsStep
        case .account: accountStep
        case .categories: categoriesStep
        case .confirm: confirmStep
        }
    }

    // MARK: 1 — colonnes

    private var columnsStep: some View {
        Form {
            Section {
                Picker("Séparateur", selection: Binding(get: { separator }, set: { separator = $0; refreshPreview() })) {
                    Text("Point-virgule ;").tag(";")
                    Text("Virgule ,").tag(",")
                    Text("Tabulation").tag("\t")
                }
                .pickerStyle(.segmented)
                Toggle("La première ligne est un en-tête", isOn: Binding(
                    get: { hasHeader },
                    set: { hasHeader = $0; refreshPreview() }
                ))
            }
            if let preview {
                Section {
                    ScrollView([.horizontal, .vertical]) {
                        Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 6) {
                            GridRow {
                                ForEach(0..<Int(preview.columnCount), id: \.self) { column in
                                    Picker("Colonne \(column + 1)", selection: Binding(
                                        get: { role(of: column) },
                                        set: { setRole($0, column: column) }
                                    )) {
                                        ForEach(ColumnRole.allCases) { role in
                                            Text(role.label).tag(role)
                                        }
                                    }
                                    .labelsHidden()
                                    .pickerStyle(.menu)
                                    .frame(width: 132)
                                }
                            }
                            Divider().gridCellUnsizedAxes(.horizontal)
                            ForEach(Array(preview.rows.prefix(8).enumerated()), id: \.offset) { row in
                                GridRow {
                                    ForEach(0..<Int(preview.columnCount), id: \.self) { column in
                                        Text(column < row.element.count ? row.element[column] : "")
                                            .font(.caption)
                                            .lineLimit(1)
                                            .frame(width: 132, alignment: .leading)
                                            .foregroundStyle(role(of: column) == .ignore ? .secondary : .primary)
                                    }
                                }
                            }
                        }
                        .padding(.vertical, 4)
                        .frame(maxHeight: .infinity, alignment: .top)
                    }
                    .frame(height: 240, alignment: .top)
                } header: {
                    Text("Affectation des colonnes")
                } footer: {
                    Text("La date et le montant sont obligatoires ; les colonnes « Ignorer » ne sont pas importées.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            errorSection
        }
        .formStyle(.grouped)
    }

    private func role(of column: Int) -> ColumnRole {
        let index = UInt32(column)
        if mapping.date == index { return .date }
        if mapping.amount == index { return .amount }
        if mapping.description == index { return .description }
        if mapping.category == index { return .category }
        return .ignore
    }

    private func setRole(_ role: ColumnRole, column: Int) {
        let index = UInt32(column)
        var updated = mapping
        if updated.date == index { updated.date = nil }
        if updated.amount == index { updated.amount = nil }
        if updated.description == index { updated.description = nil }
        if updated.category == index { updated.category = nil }
        switch role {
        case .ignore: break
        case .date: updated.date = index
        case .amount: updated.amount = index
        case .description: updated.description = index
        case .category: updated.category = index
        }
        mapping = updated
    }

    // MARK: 2 — compte

    private var accountStep: some View {
        Form {
            Section {
                Picker("Importer vers", selection: $accountChoice) {
                    Text("Sélectionner un compte").tag("")
                    ForEach(store.accounts, id: \.id) { account in
                        Label(account.name, systemImage: Symbols.name(for: account.icon)).tag(account.id)
                    }
                    Divider()
                    Label("Nouveau compte…", systemImage: "plus.circle").tag(Self.newAccountId)
                }
            } header: {
                Text("Compte de destination")
            }
            if accountChoice == Self.newAccountId {
                Section("Nouveau compte") {
                    TextField("Nom du compte", text: $newAccountName, prompt: Text("Ex : Compte courant"))
                    Picker("Type", selection: $newAccountType) {
                        ForEach(accountTypes(), id: \.self) { type in
                            Text(type).tag(type)
                        }
                    }
                    LabeledContent("Solde final du relevé") {
                        AmountField(label: "Solde final", text: $finalBalance)
                    }
                }
            }
            if accountChoice == Self.newAccountId {
                Section {
                    Text("Le solde initial est calculé à partir du solde final et des opérations importées.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            errorSection
        }
        .formStyle(.grouped)
    }

    // MARK: 3 — catégories

    private var categoriesStep: some View {
        Form {
            Section {
                ForEach(sources, id: \.self) { source in
                    Picker(source, selection: Binding(
                        get: { categoryMapping[source] ?? Self.newCategoryId },
                        set: { categoryMapping[source] = $0 }
                    )) {
                        Text("Créer « \(source) »").tag(Self.newCategoryId)
                        Divider()
                        ForEach(store.categories.filter { $0.id != "transfer" }, id: \.id) { category in
                            Label(category.name, systemImage: Symbols.name(for: category.icon)).tag(category.id)
                        }
                    }
                }
            } header: {
                Text("Catégories du fichier")
            } footer: {
                Text("Chaque catégorie du relevé est associée à une de vos catégories, ou créée.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            errorSection
        }
        .formStyle(.grouped)
    }

    // MARK: 4 — confirmation

    private var confirmStep: some View {
        Form {
            Section {
                LabeledContent("Opérations à importer") {
                    Text("\(transactions.count)").monospacedDigit().fontWeight(.semibold)
                }
                LabeledContent("Compte de destination", value: targetAccountName)
                if accountChoice == Self.newAccountId, let final = AmountInput.parse(finalBalance) {
                    LabeledContent("Solde initial calculé") {
                        Text(Money.format(initialBalanceFromFinal(transactions: transactions, finalBalance: final)))
                            .monospacedDigit()
                    }
                }
                if !sources.isEmpty {
                    LabeledContent("Catégories associées") {
                        Text("\(sources.count)").monospacedDigit()
                    }
                }
            } header: {
                Text("Prêt à importer")
            }
            if format == .csv, mapping.category == nil {
                Section {
                    Label(
                        "Aucune colonne catégorie : toutes les opérations seront classées dans « Divers ».",
                        systemImage: "exclamationmark.triangle"
                    )
                    .foregroundStyle(.orange)
                    .fixedSize(horizontal: false, vertical: true)
                }
            }
            Section {
                Text("Les doublons déjà présents dans le journal sont détectés et ignorés.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            errorSection
        }
        .formStyle(.grouped)
    }

    @ViewBuilder
    private var errorSection: some View {
        if let error {
            Section {
                Label(error, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    // MARK: Pied

    private var footer: some View {
        HStack {
            Button(step == visibleSteps.first ? "Annuler" : "Retour", action: back)
                .keyboardShortcut(.cancelAction)
                .disabled(isImporting)
            Spacer()
            if step == .confirm {
                Button(isImporting ? "Import…" : "Importer maintenant", action: importNow)
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .disabled(isImporting)
            } else {
                Button("Suivant", action: next)
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .disabled(!canContinue)
            }
        }
        .padding(16)
    }

    private var targetAccountName: String {
        if accountChoice == Self.newAccountId {
            let name = newAccountName.trimmingCharacters(in: .whitespaces)
            return name.isEmpty ? "Nouveau compte" : name
        }
        return store.account(id: accountChoice)?.name ?? "—"
    }

    private var canContinue: Bool {
        switch step {
        case .columns:
            return mapping.date != nil && mapping.amount != nil
        case .account:
            guard !accountChoice.isEmpty else { return false }
            return accountChoice != Self.newAccountId || !newAccountName.trimmingCharacters(in: .whitespaces).isEmpty
        case .categories, .confirm:
            return true
        }
    }

    // MARK: Logique (identique à la 1.x)

    private func load() {
        guard !loaded else { return }
        loaded = true
        format = statementFormatForFile(fileName: fileName) ?? .csv
        if format == .csv {
            separator = detectCsvSeparator(content: content)
            refreshPreview()
            step = .columns
        } else {
            parse()
            step = .account
        }
    }

    private func refreshPreview() {
        do {
            preview = try store.engine.previewCsv(content: content, options: CsvOptions(separator: separator, hasHeader: hasHeader))
            error = nil
        } catch {
            self.error = AppStore.message(for: error)
        }
    }

    @discardableResult
    private func parse() -> Bool {
        do {
            let isCsv = format == .csv
            transactions = try store.engine.parseStatement(
                format: format,
                content: content,
                csvOptions: isCsv ? CsvOptions(separator: separator, hasHeader: hasHeader) : nil,
                csvMapping: isCsv ? mapping : nil,
                today: store.today
            )
            guard !transactions.isEmpty else {
                error = "Aucune opération n'a été trouvée dans le fichier."
                return false
            }
            sources = sourceCategories(transactions: transactions)
            var suggested: [String: String] = [:]
            for match in (try? store.engine.suggestCategoryMapping(sources: sources)) ?? [] {
                suggested[match.source] = match.categoryId ?? Self.newCategoryId
            }
            categoryMapping = suggested
            error = nil
            return true
        } catch {
            self.error = AppStore.message(for: error)
            return false
        }
    }

    private func next() {
        switch step {
        case .columns:
            if parse() { step = .account }
        case .account:
            if accountChoice == Self.newAccountId,
               !finalBalance.trimmingCharacters(in: .whitespaces).isEmpty,
               AmountInput.parse(finalBalance) == nil {
                error = "Saisissez un montant valide."
                return
            }
            error = nil
            step = sources.isEmpty ? .confirm : .categories
        case .categories:
            step = .confirm
        case .confirm:
            break
        }
    }

    private func back() {
        error = nil
        switch step {
        case .columns: onClose()
        case .account: format == .csv ? step = .columns : onClose()
        case .categories: step = .account
        case .confirm: step = sources.isEmpty ? .account : .categories
        }
    }

    private func importNow() {
        let target: ImportTarget = accountChoice == Self.newAccountId
            ? .newAccount(
                name: newAccountName.trimmingCharacters(in: .whitespaces),
                accountType: newAccountType,
                finalBalance: AmountInput.parse(finalBalance)
            )
            : .existingAccount(accountId: accountChoice)
        let matches = sources.map { source -> CategoryMatch in
            let id = categoryMapping[source]
            return CategoryMatch(source: source, categoryId: id == Self.newCategoryId ? nil : id)
        }
        let request = StatementImportRequest(transactions: transactions, target: target, categoryMapping: matches)
        isImporting = true
        store.perform({ engine in try engine.importStatement(request: request) }, completion: { [store] result in
            let duplicates = result.duplicates > 0 ? " (\(result.duplicates) doublons ignorés)" : ""
            store.showToast("\(result.imported) opérations importées\(duplicates)")
            onClose()
        }, failure: { message in
            error = message
            isImporting = false
        })
    }
}
