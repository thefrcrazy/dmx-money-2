import SwiftUI

public enum AppInfo {
    public static var version: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? coreVersion()
    }

    public static var build: String {
        Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? ""
    }

    public static var systemVersion: String {
        let version = ProcessInfo.processInfo.operatingSystemVersion
        return "\(version.majorVersion).\(version.minorVersion).\(version.patchVersion)"
    }

    /// Comparaison numérique composant par composant (2.0.10 est plus récent que 2.0.9).
    public static func isVersion(_ candidate: String, newerThan current: String) -> Bool {
        let parse: (String) -> [Int] = { value in
            value.split(separator: ".").map { Int($0.filter(\.isNumber)) ?? 0 }
        }
        let left = parse(candidate)
        let right = parse(current)
        for index in 0..<max(left.count, right.count) {
            let a = index < left.count ? left[index] : 0
            let b = index < right.count ? right[index] : 0
            if a != b { return a > b }
        }
        return false
    }
}

extension AppStore {
    /// Propose de reprendre une base DmxMoney 1.x plus complète trouvée à côté du dossier de
    /// données. La base 1.x n'est pas modifiée, et l'actuelle est exportée en `.dmx` avant.
    public func proposeLegacyAdoptionIfNeeded() {
        // Désactivé : ne plus proposer d'adoption intempestive de base 1.x
    }

    /// Affiche les nouveautés après une mise à jour ; une installation neuve est marquée comme vue.
    public func presentWhatsNewIfNeeded() {
        let version = AppInfo.version
        guard settings.lastSeenVersion != version else { return }
        if settings.lastSeenVersion == nil && accounts.isEmpty {
            apply(.setLastSeenVersion(version: version))
        } else {
            present(.whatsNew)
        }
    }
}

// MARK: - Nouveautés

public enum ReleaseNotes {
    public static let current = [
        "DmxMoney devient une application native : AppKit et SwiftUI sur Mac, SwiftUI sur iPhone et iPad.",
        "Vos données DmxMoney 1.x sont reprises au premier lancement ; la base d'origine n'est jamais modifiée.",
        "Synchronisation iCloud entre vos appareils Apple, ou pont PWA sécurisé pour tous les mobiles : à vous de choisir.",
        "Budget, échéancier, analyses et prédictions sont calculés par le même noyau sur macOS, Windows et Linux.",
        "Deux applications Mac : interface SwiftUI sur Apple Silicon, interface compatible sur les Mac Intel jusqu'à macOS Catalina.",
    ]
}

struct WhatsNewView: View {
    @EnvironmentObject private var store: AppStore
    private let onClose: () -> Void

    init(onClose: @escaping () -> Void) {
        self.onClose = onClose
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                IconBadge(icon: "Sparkles", colorHex: "#6366f1", size: 44, filled: true)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Nouveautés").font(.system(size: 17, weight: .semibold))
                    Text("DmxMoney \(AppInfo.version)").font(.system(size: 12)).foregroundColor(.secondary)
                }
                Spacer()
            }
            .padding(20)
            Divider()
            VStack(alignment: .leading, spacing: 12) {
                ForEach(ReleaseNotes.current, id: \.self) { note in
                    HStack(alignment: .top, spacing: 10) {
                        DmxIcon("CheckCircle2", size: 14).foregroundColor(.accentColor).padding(.top, 1)
                        Text(note).font(.system(size: 13)).fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            .padding(20)
            Divider()
            HStack {
                Spacer()
                Button(action: close) { Text("Continuer") }.buttonStyle(DmxButtonStyle(.primary))
            }
            .padding(16)
        }
    }

    private func close() {
        store.apply(.setLastSeenVersion(version: AppInfo.version))
        onClose()
    }
}

// MARK: - Restauration .dmx

struct RestoreBackupForm: View {
    @EnvironmentObject private var store: AppStore
    private let content: String
    private let fileName: String
    private let onClose: () -> Void
    @State private var summary: BackupSummary?
    @State private var mode: RestoreMode = .replace
    @State private var error: String?
    @State private var isWorking = false

    init(content: String, fileName: String, onClose: @escaping () -> Void) {
        self.content = content
        self.fileName = fileName
        self.onClose = onClose
    }

    var body: some View {
        FormSheet(
            title: "Importer une sauvegarde",
            submitTitle: mode == .replace ? "Remplacer mes données" : "Fusionner",
            error: error,
            submitDisabled: summary == nil || isWorking,
            onCancel: onClose,
            onSubmit: submit
        ) {
            HStack(spacing: 12) {
                IconBadge(icon: "Database", colorHex: "#6366f1", size: 40)
                VStack(alignment: .leading, spacing: 2) {
                    Text(fileName).font(.system(size: 13, weight: .semibold)).lineLimit(1)
                    if let summary = summary {
                        Text("Sauvegarde du \(DayFormat.long(String(summary.timestamp.prefix(10))))")
                            .font(.system(size: 12)).foregroundColor(.secondary)
                    }
                }
            }
            if let summary = summary {
                HStack(spacing: 8) {
                    count(summary.accounts, "comptes")
                    count(summary.transactions, "transactions")
                    count(summary.categories, "catégories")
                    count(summary.scheduled, "échéances")
                    count(summary.budgets, "budgets")
                }
            }
            FormField("Mode d'import") {
                Picker("", selection: $mode) {
                    Text("Remplacer").tag(RestoreMode.replace)
                    Text("Fusionner").tag(RestoreMode.merge)
                }
                .labelsHidden()
                .pickerStyle(SegmentedPickerStyle())
            }
            Text(mode == .replace
                 ? "Toutes les données actuelles seront remplacées par celles de la sauvegarde."
                 : "Les éléments de la sauvegarde sont ajoutés ; les éléments déjà présents sont conservés.")
                .font(.system(size: 12))
                .foregroundColor(mode == .replace ? DmxColors.warning : .secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .onAppear(perform: inspect)
    }

    private func count(_ value: UInt32, _ label: String) -> some View {
        VStack(spacing: 2) {
            Text("\(value)").font(.system(size: 16, weight: .bold))
            Text(label).font(.system(size: 10)).foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color.primary.opacity(0.04)))
    }

    private func inspect() {
        do {
            summary = try store.engine.inspectBackup(content: content)
        } catch {
            self.error = "Le fichier de sauvegarde est corrompu. (\(AppStore.message(for: error)))"
        }
    }

    private func submit() {
        isWorking = true
        let content = self.content
        let mode = self.mode
        store.perform({ engine in try engine.restoreBackup(content: content, mode: mode) }, completion: { [store] _ in
            store.showToast("Import réussi")
            onClose()
        }, failure: { message in
            error = message
            isWorking = false
        })
    }
}

// MARK: - Import de relevés (CSV, QIF, OFX)

struct StatementImportWizard: View {
    private enum Step: Int, CaseIterable {
        case columns
        case account
        case categories
        case confirm

        var label: String {
            switch self {
            case .columns: return "Colonnes"
            case .account: return "Compte"
            case .categories: return "Catégories"
            case .confirm: return "Confirmation"
            }
        }
    }

    private enum ColumnRole: String, CaseIterable {
        case ignore, date, amount, description, category

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
    private let content: String
    private let fileName: String
    private let onClose: () -> Void

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
    @State private var accountChoice: String?
    @State private var newAccountName = ""
    @State private var newAccountType = "Courant"
    @State private var finalBalance = ""
    @State private var error: String?
    @State private var isImporting = false

    init(content: String, fileName: String, onClose: @escaping () -> Void) {
        self.content = content
        self.fileName = fileName
        self.onClose = onClose
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider()
            stepIndicator
            Divider()
            VStack(alignment: .leading, spacing: 14) {
                stepContent
                if let error = error {
                    HStack(spacing: 6) {
                        DmxIcon("AlertTriangle", size: 13)
                        Text(error).font(.system(size: 12, weight: .medium)).fixedSize(horizontal: false, vertical: true)
                    }
                    .foregroundColor(DmxColors.expense)
                }
            }
            .padding(20)
            Divider()
            footer
        }
        .onAppear(perform: load)
    }

    // MARK: Structure

    private var title: String {
        switch format {
        case .csv: return "Assistant d'import CSV"
        case .qif: return "Import QIF"
        case .ofx: return "Import OFX"
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            IconBadge(icon: "FileText", colorHex: "#6366f1", size: 36)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.system(size: 16, weight: .bold))
                Text(fileName).font(.system(size: 11)).foregroundColor(.secondary).lineLimit(1)
            }
            Spacer()
            IconButton("X", action: onClose)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
    }

    private var visibleSteps: [Step] {
        format == .csv ? Step.allCases : [.account, .categories, .confirm]
    }

    private var stepIndicator: some View {
        HStack {
            ForEach(Array(visibleSteps.enumerated()), id: \.element.rawValue) { item in
                let isActive = item.element == step
                let isPast = item.element.rawValue < step.rawValue
                HStack(spacing: 6) {
                    ZStack {
                        Circle()
                            .stroke(isActive ? Color.accentColor : (isPast ? DmxColors.income : Color.secondary.opacity(0.5)), lineWidth: 1)
                            .background(Circle().fill(isActive ? Color.accentColor.opacity(0.12) : (isPast ? DmxColors.income.opacity(0.12) : Color.clear)))
                        if isPast {
                            DmxIcon("Check", size: 11).foregroundColor(DmxColors.income)
                        } else {
                            Text("\(item.offset + 1)").font(.system(size: 11, weight: .bold))
                        }
                    }
                    .frame(width: 22, height: 22)
                    Text(item.element.label).font(.system(size: 12, weight: .medium))
                }
                .foregroundColor(isActive ? .accentColor : (isPast ? DmxColors.income : .secondary))
                if item.offset < visibleSteps.count - 1 {
                    Spacer()
                }
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
    }

    private var footer: some View {
        HStack {
            Button(action: back) { Text(step == visibleSteps.first ? "Annuler" : "Retour") }
                .buttonStyle(DmxButtonStyle(.secondary))
                .disabled(isImporting)
            Spacer()
            if step == .confirm {
                Button(action: importNow) { Text(isImporting ? "Import…" : "Importer maintenant") }
                    .buttonStyle(DmxButtonStyle(.primary))
                    .disabled(isImporting)
            } else {
                Button(action: next) { Text("Suivant") }
                    .buttonStyle(DmxButtonStyle(.primary))
                    .disabled(!canContinue)
            }
        }
        .padding(16)
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

    // MARK: Étape 1 : colonnes

    private var columnsStep: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .bottom, spacing: 16) {
                FormField("Séparateur") {
                    Picker("", selection: Binding(get: { separator }, set: { separator = $0; refreshPreview() })) {
                        Text("Point-virgule (;)").tag(";")
                        Text("Virgule (,)").tag(",")
                        Text("Tabulation").tag("\t")
                    }
                    .labelsHidden()
                    .pickerStyle(SegmentedPickerStyle())
                }
                Toggle(isOn: Binding(get: { hasHeader }, set: { hasHeader = $0; refreshPreview() })) {
                    Text("La première ligne est un en-tête").font(.system(size: 12))
                }
            }
            if let preview = preview {
                ScrollView(.horizontal) {
                    VStack(alignment: .leading, spacing: 0) {
                        HStack(spacing: 0) {
                            ForEach(0..<Int(preview.columnCount), id: \.self) { column in
                                ChoicePicker(
                                    options: ColumnRole.allCases.map { SelectOption(id: $0.rawValue, label: $0.label) },
                                    selection: Binding(get: { role(of: column).rawValue }, set: { setRole(ColumnRole(rawValue: $0) ?? .ignore, column: column) })
                                )
                                .frame(width: 150)
                                .padding(6)
                            }
                        }
                        .background(Color.primary.opacity(0.04))
                        ForEach(Array(preview.rows.prefix(8).enumerated()), id: \.offset) { row in
                            HStack(spacing: 0) {
                                ForEach(0..<Int(preview.columnCount), id: \.self) { column in
                                    Text(column < row.element.count ? row.element[column] : "")
                                        .font(.system(size: 12))
                                        .lineLimit(1)
                                        .frame(width: 150, alignment: .leading)
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 5)
                                        .background(role(of: column) == .ignore ? Color.clear : Color.accentColor.opacity(0.06))
                                }
                            }
                            Divider()
                        }
                    }
                }
                .frame(height: 290)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(DmxPalette.separator, lineWidth: 0.5))
                Text("Assignez les colonnes en utilisant les listes déroulantes ci-dessus.")
                    .font(.system(size: 11)).foregroundColor(.secondary)
            }
        }
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

    // MARK: Étape 2 : compte

    private var accountStep: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Vers quel compte importer ?").font(.system(size: 13, weight: .medium))
            ScrollView {
                VStack(spacing: 8) {
                    ForEach(store.accounts, id: \.id) { account in
                        choiceRow(id: account.id) {
                            IconBadge(icon: account.icon, colorHex: account.color, size: 30, filled: true)
                            Text(account.name).font(.system(size: 13, weight: .medium))
                        }
                    }
                    choiceRow(id: Self.newAccountId) {
                        IconCircle(icon: "Upload", color: .secondary, size: 30)
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Nouveau compte").font(.system(size: 13, weight: .medium))
                            if accountChoice == Self.newAccountId {
                                DmxTextField("Nom du compte", text: $newAccountName)
                                ChoicePicker(options: accountTypes().map { SelectOption(id: $0, label: $0) }, selection: $newAccountType)
                                FormField("Solde final (optionnel)") { AmountField(text: $finalBalance) }
                                Text("Saisissez le solde final du relevé pour calculer automatiquement le solde initial.")
                                    .font(.system(size: 11)).foregroundColor(.secondary)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
            }
            .frame(maxHeight: 360)
        }
    }

    private func choiceRow<Content: View>(id: String, @ViewBuilder content: () -> Content) -> some View {
        let selected = accountChoice == id
        return HStack(alignment: .top, spacing: 10) {
            DmxIcon(selected ? "CheckCircle2" : "Circle", size: 16)
                .foregroundColor(selected ? .accentColor : .secondary)
                .padding(.top, 7)
            content()
            Spacer(minLength: 0)
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(selected ? Color.accentColor.opacity(0.08) : Color.clear))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(selected ? Color.accentColor : DmxPalette.separator, lineWidth: selected ? 1.5 : 0.5))
        .contentShape(Rectangle())
        .onTapGesture { accountChoice = id }
    }

    // MARK: Étape 3 : catégories

    private var categoriesStep: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Associez les catégories du fichier à vos catégories existantes.")
                .font(.system(size: 13)).foregroundColor(.secondary)
            ScrollView {
                VStack(spacing: 8) {
                    ForEach(sources, id: \.self) { source in
                        HStack(spacing: 12) {
                            Text(source).font(.system(size: 13, weight: .medium)).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                            DmxIcon("ArrowRight", size: 14).foregroundColor(.secondary)
                            SearchableSelect(
                                "Sélectionner une catégorie",
                                options: [SelectOption(id: Self.newCategoryId, label: "+ Créer « \(source) »", icon: "Plus")] + store.categoryOptions,
                                required: Binding(get: { categoryMapping[source] ?? Self.newCategoryId }, set: { categoryMapping[source] = $0 })
                            )
                            .frame(maxWidth: .infinity)
                        }
                        .padding(10)
                        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color.primary.opacity(0.04)))
                    }
                }
            }
            .frame(maxHeight: 360)
        }
    }

    // MARK: Étape 4 : confirmation

    private var confirmStep: some View {
        VStack(spacing: 14) {
            ZStack {
                Circle().fill(DmxColors.income.opacity(0.14)).frame(width: 60, height: 60)
                DmxIcon("Check", size: 28).foregroundColor(DmxColors.income)
            }
            Text("Prêt à importer").font(.system(size: 18, weight: .bold))
            (Text("\(transactions.count)").bold() + Text(" transactions seront importées dans le compte ") + Text(targetAccountName).bold() + Text("."))
                .font(.system(size: 13))
                .multilineTextAlignment(.center)
            if accountChoice == Self.newAccountId, let final = AmountInput.parse(finalBalance) {
                Text("Solde initial calculé : \(Money.format(initialBalanceFromFinal(transactions: transactions, finalBalance: final)))")
                    .font(.system(size: 12)).foregroundColor(.secondary)
            }
            if format == .csv && mapping.category == nil {
                HStack(alignment: .top, spacing: 8) {
                    DmxIcon("AlertTriangle", size: 15).foregroundColor(DmxColors.intradayWarning)
                    Text("Aucune colonne catégorie n'a été sélectionnée. Toutes les transactions seront classées dans « Divers ».")
                        .font(.system(size: 12))
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(12)
                .background(RoundedRectangle(cornerRadius: 8).fill(DmxColors.intradayWarning.opacity(0.12)))
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var targetAccountName: String {
        if accountChoice == Self.newAccountId { return newAccountName }
        return store.account(id: accountChoice)?.name ?? ""
    }

    // MARK: Logique

    private var canContinue: Bool {
        switch step {
        case .columns: return mapping.date != nil && mapping.amount != nil
        case .account:
            guard let choice = accountChoice else { return false }
            return choice != Self.newAccountId || !newAccountName.trimmingCharacters(in: .whitespaces).isEmpty
        case .categories, .confirm: return true
        }
    }

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
                error = "Aucune transaction n'a été trouvée dans le fichier."
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
                error = "Saisissez un montant valide"
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
        case .account: if format == .csv { step = .columns } else { onClose() }
        case .categories: step = .account
        case .confirm: step = sources.isEmpty ? .account : .categories
        }
    }

    private func importNow() {
        guard let choice = accountChoice else { return }
        let target: ImportTarget = choice == Self.newAccountId
            ? .newAccount(name: newAccountName.trimmingCharacters(in: .whitespaces), accountType: newAccountType, finalBalance: AmountInput.parse(finalBalance))
            : .existingAccount(accountId: choice)
        let matches = sources.map { source -> CategoryMatch in
            let id = categoryMapping[source]
            return CategoryMatch(source: source, categoryId: id == Self.newCategoryId ? nil : id)
        }
        let request = StatementImportRequest(transactions: transactions, target: target, categoryMapping: matches)
        isImporting = true
        store.perform({ engine in try engine.importStatement(request: request) }, completion: { [store] result in
            let duplicates = result.duplicates > 0 ? " (\(result.duplicates) doublons ignorés)" : ""
            store.showToast("\(result.imported) transactions importées\(duplicates)")
            onClose()
        }, failure: { message in
            error = message
            isImporting = false
        })
    }
}
