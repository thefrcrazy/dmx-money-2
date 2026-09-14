import DmxKit
import SwiftUI

// MARK: - Nouveautés

/// Notes de version, en feuille native.
struct ModernWhatsNew: View {
    @EnvironmentObject private var store: AppStore
    let onClose: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            Form {
                Section {
                    ForEach(ReleaseNotes.current, id: \.self) { note in
                        Label {
                            Text(note).fixedSize(horizontal: false, vertical: true)
                        } icon: {
                            Image(systemName: "checkmark.circle.fill").foregroundStyle(.tint)
                        }
                    }
                } header: {
                    Text("DmxMoney \(AppInfo.version)")
                }
            }
            .formStyle(.grouped)
            Divider()
            HStack {
                Spacer()
                Button("Continuer") {
                    store.apply(.setLastSeenVersion(version: AppInfo.version))
                    onClose()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
            .padding(16)
        }
        .navigationTitle("Nouveautés")
        .frame(minWidth: 480, minHeight: 360)
    }
}

// MARK: - Restauration .dmx

/// Import d'une sauvegarde `.dmx` : inventaire du fichier, puis remplacement ou fusion.
struct ModernRestoreBackup: View {
    @EnvironmentObject private var store: AppStore
    let content: String
    let fileName: String
    let onClose: () -> Void

    @State private var summary: BackupSummary?
    @State private var mode: RestoreMode = .replace
    @State private var error: String?
    @State private var isWorking = false

    var body: some View {
        FormSheet(
            title: "Importer une sauvegarde",
            submitTitle: mode == .replace ? "Remplacer mes données" : "Fusionner",
            submitDisabled: summary == nil || isWorking,
            error: error,
            onCancel: onClose,
            onSubmit: submit
        ) {
            Section {
                LabeledContent("Fichier") {
                    Text(fileName).lineLimit(1).truncationMode(.middle)
                }
                if let summary {
                    LabeledContent("Sauvegarde du", value: DayFormat.long(String(summary.timestamp.prefix(10))))
                }
            }
            if let summary {
                Section("Contenu") {
                    LabeledContent("Comptes") { Text("\(summary.accounts)").monospacedDigit() }
                    LabeledContent("Opérations") { Text("\(summary.transactions)").monospacedDigit() }
                    LabeledContent("Catégories") { Text("\(summary.categories)").monospacedDigit() }
                    LabeledContent("Échéances") { Text("\(summary.scheduled)").monospacedDigit() }
                    LabeledContent("Budgets") { Text("\(summary.budgets)").monospacedDigit() }
                }
            }
            Section {
                Picker("Mode d'import", selection: $mode) {
                    Text("Remplacer").tag(RestoreMode.replace)
                    Text("Fusionner").tag(RestoreMode.merge)
                }
                .pickerStyle(.segmented)
            } footer: {
                Label(
                    mode == .replace
                        ? "Toutes les données actuelles seront remplacées par celles de la sauvegarde."
                        : "Les éléments de la sauvegarde sont ajoutés ; ceux déjà présents sont conservés.",
                    systemImage: mode == .replace ? "exclamationmark.triangle" : "info.circle"
                )
                .font(.caption)
                .foregroundStyle(mode == .replace ? .orange : .secondary)
                .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onAppear(perform: inspect)
    }

    private func inspect() {
        do {
            summary = try store.engine.inspectBackup(content: content)
        } catch {
            self.error = "Le fichier de sauvegarde est illisible. (\(AppStore.message(for: error)))"
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

// MARK: - Suggestions

/// Coquille commune aux deux listes de suggestions.
private struct SuggestionSheet<Content: View>: View {
    let title: String
    let isEmpty: Bool
    let onClose: () -> Void
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) {
            if isEmpty {
                ContentUnavailableView {
                    Label("Aucune suggestion", systemImage: "sparkles")
                } description: {
                    Text("DmxMoney propose des budgets et des échéances dès que vos opérations se répètent.")
                }
                .frame(maxHeight: .infinity)
            } else {
                List { content }
                    .listStyle(.inset)
            }
            Divider()
            HStack {
                Spacer()
                Button("Fermer", action: onClose)
                    .keyboardShortcut(.defaultAction)
            }
            .padding(16)
        }
        .navigationTitle(title)
        .frame(minWidth: 680, minHeight: 420)
    }
}

/// Budgets proposés à partir des dépenses répétées du journal.
struct ModernBudgetSuggestions: View {
    @EnvironmentObject private var store: AppStore
    let onClose: () -> Void

    private var suggestions: [BudgetSuggestion] {
        let query = BudgetQuery(accounts: store.selectedAccountIds, search: "", categories: [])
        let today = store.today
        return store.peek { engine in try engine.budget(query: query, today: today).suggestions } ?? []
    }

    var body: some View {
        let items = suggestions
        return SuggestionSheet(title: "Suggestions de budgets", isEmpty: items.isEmpty, onClose: onClose) {
            ForEach(items, id: \.key) { suggestion in
                HStack(spacing: 12) {
                    CategoryBadge(icon: suggestion.category.icon, colorHex: suggestion.category.color, size: 30)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(suggestion.name).fontWeight(.medium).lineLimit(1)
                        Text(meta(suggestion)).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    MoneyText(amount: suggestion.amount)
                    Button("Ajouter", systemImage: "plus") { accept(suggestion) }
                    Button("Ignorer", systemImage: "xmark") { dismiss(suggestion) }
                        .buttonStyle(.link)
                }
                .padding(.vertical, 4)
            }
        }
    }

    private func meta(_ suggestion: BudgetSuggestion) -> String {
        var parts = [suggestion.accountName, "\(suggestion.monthCount) mois observé\(suggestion.monthCount > 1 ? "s" : "")"]
        if suggestion.currentMonthSpent > 0 {
            parts.append("\(Money.format(suggestion.currentMonthSpent)) ce mois-ci")
        }
        return parts.joined(separator: " · ")
    }

    private func accept(_ suggestion: BudgetSuggestion) {
        let accounts = store.selectedAccountIds
        let today = store.today
        store.run("Budget ajouté") { engine in
            _ = try engine.acceptBudgetSuggestion(key: suggestion.key, accounts: accounts, today: today)
        }
    }

    private func dismiss(_ suggestion: BudgetSuggestion) {
        store.run("Suggestion ignorée") { engine in try engine.dismissBudgetSuggestion(key: suggestion.key) }
    }
}

/// Échéances proposées à partir des opérations qui reviennent chaque mois.
struct ModernScheduledSuggestions: View {
    @EnvironmentObject private var store: AppStore
    let onClose: () -> Void

    private var suggestions: [ScheduledSuggestion] {
        let query = ScheduledQuery(accounts: store.selectedAccountIds, dueRange: .all, search: "", categories: [], frequencies: [])
        let today = store.today
        return store.peek { engine in try engine.scheduled(query: query, today: today).suggestions } ?? []
    }

    var body: some View {
        let items = suggestions
        return SuggestionSheet(title: "Suggestions d'échéances", isEmpty: items.isEmpty, onClose: onClose) {
            ForEach(items, id: \.key) { suggestion in
                HStack(spacing: 12) {
                    CategoryBadge(
                        icon: suggestion.transactionType == .transfer ? "ArrowRightLeft" : suggestion.category.icon,
                        colorHex: suggestion.transactionType == .transfer ? "#6366f1" : suggestion.category.color,
                        size: 30
                    )
                    VStack(alignment: .leading, spacing: 1) {
                        Text(suggestion.description).fontWeight(.medium).lineLimit(1)
                        Text(meta(suggestion)).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    MoneyText(amount: suggestion.amount, signed: suggestion.transactionType)
                    Button("Ajouter", systemImage: "plus") { accept(suggestion) }
                    Button("Ignorer", systemImage: "xmark") { dismiss(suggestion) }
                        .buttonStyle(.link)
                }
                .padding(.vertical, 4)
            }
        }
    }

    private func meta(_ suggestion: ScheduledSuggestion) -> String {
        [
            suggestion.accountName,
            periodicityLabel(frequency: suggestion.frequency),
            "\(suggestion.occurrenceCount) occurrence\(suggestion.occurrenceCount > 1 ? "s" : "")",
            "prochaine le \(DayFormat.short(suggestion.nextDate))",
        ].joined(separator: " · ")
    }

    private func accept(_ suggestion: ScheduledSuggestion) {
        let accounts = store.selectedAccountIds
        let today = store.today
        store.run("Échéance ajoutée") { engine in
            _ = try engine.acceptScheduledSuggestion(key: suggestion.key, accounts: accounts, today: today)
        }
    }

    private func dismiss(_ suggestion: ScheduledSuggestion) {
        store.run("Suggestion ignorée") { engine in try engine.dismissScheduledSuggestion(key: suggestion.key) }
    }
}
