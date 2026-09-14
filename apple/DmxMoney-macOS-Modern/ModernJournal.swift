import DmxKit
import SwiftUI

/// Journal en `Table` native : colonnes triables, sélection multiple, édition en ligne,
/// menus contextuels et actions groupées.
struct ModernJournal: View {
    @ObservedObject var model: JournalModel
    @EnvironmentObject private var store: AppStore
    @State private var sort: [KeyPathComparator<JournalRow>] = []

    var body: some View {
        VStack(spacing: 0) {
            filters
            Divider()
            table
        }
    }

    // MARK: Filtres

    /// Actions de la sélection à gauche, filtres alignés à droite comme sur les autres pages.
    private var filters: some View {
        FilterBar {
            selectionActions
        } trailing: {
            FilterSelector(
                title: "Catégories",
                systemImage: "tag",
                options: store.categories.map { SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color) },
                selection: $model.categories
            )
            FilterSelector(
                title: "Types",
                systemImage: "arrow.left.arrow.right",
                options: JournalModel.typeOptions,
                selection: $model.types
            )
            FilterSelector(
                title: "États",
                systemImage: "checkmark.circle",
                options: JournalModel.statusOptions,
                selection: $model.statuses
            )
            FilterSelector(
                title: "Budgets",
                systemImage: "wallet.bifold",
                options: Self.budgetOptions,
                selection: $model.budgetStatuses
            )
            if model.hasFilters {
                Button("Réinitialiser", systemImage: "xmark.circle") { model.clearFilters() }
                    .buttonStyle(.link)
            }
        }
    }

    /// Pointer, modifier et supprimer la sélection, visibles dès qu'une ligne est choisie.
    @ViewBuilder
    private var selectionActions: some View {
        if model.selection.isEmpty {
            Text("Cliquez une ligne pour la pointer, la modifier ou la supprimer.")
                .font(.callout)
                .foregroundStyle(.secondary)
        } else {
            HStack(spacing: 8) {
                Text(model.selection.count == 1 ? "1 opération" : "\(model.selection.count) opérations")
                    .font(.callout.weight(.medium))
                Button("Pointer", systemImage: "checkmark.circle") { model.toggleCheckedSelection() }
                if model.selection.count == 1, let id = model.selection.first {
                    Button("Modifier", systemImage: "pencil") { store.present(.transaction(id: id)) }
                }
                Button("Supprimer", systemImage: "trash", role: .destructive) { model.deleteSelection() }
                Button("Désélectionner", systemImage: "xmark") { model.selection = [] }
                    .buttonStyle(.link)
            }
            .font(.callout)
        }
    }

    /// Les options de budget arrivent sans icône du noyau : on les habille ici.
    private static let budgetOptions = JournalModel.budgetOptions.map { option in
        SelectOption(
            id: option.id,
            label: option.label,
            icon: option.id == "budgeted" ? "Wallet" : "CircleOff",
            color: option.id == "budgeted" ? "#6366f1" : "#9ca3af"
        )
    }

    // MARK: Tableau

    private var table: some View {
        Table(rows, selection: $model.selection, sortOrder: $sort) {
            TableColumn("Compte", value: \.accountName) { row in
                HStack(spacing: 6) {
                    Circle().fill(Color(hex: row.accountColor, fallback: .secondary)).frame(width: 7, height: 7)
                    Text(row.accountName).lineLimit(1)
                }
            }
            .width(min: 110, ideal: 150)

            TableColumn("Date", value: \.transaction.date) { row in
                Text(DayFormat.short(row.transaction.date)).foregroundStyle(.secondary).monospacedDigit()
            }
            .width(min: 80, ideal: 92)

            TableColumn("Catégorie", value: \.category.name) { row in
                HStack(spacing: 6) {
                    Image(systemName: Symbols.name(for: row.category.icon))
                        .foregroundStyle(Color(hex: row.category.color, fallback: .secondary))
                        .frame(width: 18, alignment: .center)
                    Text(row.category.name).lineLimit(1)
                }
            }
            .width(min: 120, ideal: 170)

            TableColumn("Description", value: \.transaction.description) { row in
                InlineTextCell(value: row.transaction.description, prompt: "Description") { text in
                    model.updateDescription(row.transaction.id, text)
                }
            }
            .width(min: 140, ideal: 260)

            TableColumn("Montant", value: \.transaction.amount) { row in
                InlineTextCell(
                    value: Money.signed(row.transaction.amount, display: row.displayType, stored: row.transaction.transactionType),
                    prompt: "Montant",
                    alignment: .trailing,
                    color: color(row.displayType)
                ) { text in
                    // Le signe vient du type d'opération : on n'envoie que la valeur absolue.
                    let value = AmountInput.parse(text).map { AmountInput.text(abs($0)) } ?? text
                    model.updateAmount(row.transaction.id, text: value)
                }
            }
            .width(min: 90, ideal: 110)
            .alignment(.trailing)

            TableColumn("Budget restant") { row in
                if let budget = row.budget {
                    Text(Money.format(budget.remaining))
                        .monospacedDigit()
                        .foregroundStyle(budget.remaining < 0 ? .red : .indigo)
                } else {
                    Text("—").foregroundStyle(.tertiary)
                }
            }
            .width(min: 90, ideal: 120)
            .alignment(.trailing)

            TableColumn("État") { row in
                Button {
                    model.toggleChecked(row.transaction.id)
                } label: {
                    Image(systemName: row.transaction.checked ? "checkmark.circle.fill" : "circle")
                        .foregroundStyle(row.transaction.checked ? AnyShapeStyle(.green) : AnyShapeStyle(.tertiary))
                }
                .buttonStyle(.plain)
                .help(row.transaction.checked ? "Dépointer" : "Pointer")
            }
            .width(46)
            .alignment(.center)

            TableColumn("Solde", value: \.balance) { row in
                Text(Money.format(row.balance)).monospacedDigit().foregroundStyle(.secondary)
            }
            .width(min: 90, ideal: 110)
            .alignment(.trailing)

            TableColumn("Actions") { row in
                HStack(spacing: 8) {
                    Button { store.present(.transaction(id: row.transaction.id)) } label: {
                        Image(systemName: "pencil")
                    }
                    .buttonStyle(.plain)
                    .help("Modifier")
                    Button { model.delete(row.transaction.id) } label: {
                        Image(systemName: "trash")
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.red)
                    .help("Supprimer")
                }
            }
            .width(62)
        }
        .tableStyle(.inset)
        .contextMenu(forSelectionType: String.self) { ids in
            if ids.count == 1, let id = ids.first {
                Button("Modifier") { store.present(.transaction(id: id)) }
                Button(isChecked(id) ? "Dépointer" : "Pointer") { model.toggleChecked(id) }
                Divider()
                Button("Supprimer", role: .destructive) { model.delete(id) }
            } else if !ids.isEmpty {
                Button("Pointer ou dépointer") {
                    model.selection = ids
                    model.toggleCheckedSelection()
                }
                Button("Supprimer \(ids.count) opérations", role: .destructive) {
                    model.selection = ids
                    model.deleteSelection()
                }
            }
        } primaryAction: { ids in
            if let id = ids.first, ids.count == 1 { store.present(.transaction(id: id)) }
        }
        // Échap vide la sélection : un clic dans le vide d'une `Table` ne la désélectionne pas.
        .onExitCommand { model.selection = [] }
        .overlay {
            if rows.isEmpty {
                ContentUnavailableView {
                    Label("Aucune opération", systemImage: "list.bullet.rectangle.portrait")
                } description: {
                    Text(model.hasFilters ? "Aucune opération ne correspond aux filtres." : "Ajoutez votre première opération.")
                } actions: {
                    if model.hasFilters {
                        Button("Réinitialiser les filtres") { model.clearFilters() }
                    } else {
                        Button("Nouvelle transaction") { store.present(.transaction(id: nil)) }
                    }
                }
            }
        }
    }

    private var rows: [JournalRow] {
        let rows = model.rows
        return sort.isEmpty ? rows : rows.sorted(using: sort)
    }

    private func color(_ kind: TransactionType) -> Color {
        switch kind {
        case .income: return .green
        case .expense: return .red
        case .transfer: return .indigo
        }
    }

    private func isChecked(_ id: String) -> Bool {
        model.rows.first { $0.transaction.id == id }?.transaction.checked ?? false
    }
}

/// Cellule de tableau éditable : la saisie reste locale et n'est envoyée au noyau qu'à la
/// validation ou en quittant le champ.
///
/// Écrire à chaque frappe rechargeait le journal sous le curseur : le champ perdait le focus et
/// la ligne changeait de place en cours de frappe.
struct InlineTextCell: View {
    let value: String
    let prompt: String
    var alignment: TextAlignment = .leading
    var color: Color = .primary
    let commit: (String) -> Void

    @State private var draft: String
    @FocusState private var isFocused: Bool

    init(
        value: String,
        prompt: String,
        alignment: TextAlignment = .leading,
        color: Color = .primary,
        commit: @escaping (String) -> Void
    ) {
        self.value = value
        self.prompt = prompt
        self.alignment = alignment
        self.color = color
        self.commit = commit
        _draft = State(initialValue: value)
    }

    var body: some View {
        TextField(prompt, text: $draft)
            .textFieldStyle(.plain)
            .multilineTextAlignment(alignment)
            .monospacedDigit()
            .foregroundStyle(color)
            .focused($isFocused)
            .onSubmit(send)
            .onChange(of: isFocused) { _, focused in
                if !focused { send() }
            }
            // La valeur du noyau reprend la main quand elle change ailleurs.
            .onChange(of: value) { _, new in
                if !isFocused { draft = new }
            }
    }

    private func send() {
        let trimmed = draft.trimmingCharacters(in: .whitespaces)
        guard trimmed != value.trimmingCharacters(in: .whitespaces) else { return }
        commit(draft)
    }
}
