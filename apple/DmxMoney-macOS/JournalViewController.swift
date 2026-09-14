import AppKit
import Combine
import DmxKit
import SwiftUI

/// Journal en NSTableView : sélection multiple, édition de la description et du montant, pointage.
final class JournalViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSTextFieldDelegate, NSMenuDelegate {
    private enum Column: String, CaseIterable {
        case account, date, category, description, amount, budget, status, balance, actions

        var title: String {
            switch self {
            case .account: return "Compte"
            case .date: return "Date"
            case .category: return "Catégorie"
            case .description: return "Description"
            case .amount: return "Montant"
            case .budget: return "Budget restant"
            case .status: return "État"
            case .balance: return "Solde"
            case .actions: return ""
            }
        }

        var width: CGFloat {
            switch self {
            case .account: return 140
            case .date: return 74
            case .category: return 150
            case .description: return 260
            case .amount: return 118
            case .budget: return 118
            case .status: return 44
            case .balance: return 118
            case .actions: return 64
            }
        }

        var identifier: NSUserInterfaceItemIdentifier { NSUserInterfaceItemIdentifier(rawValue) }
    }

    private static let descriptionField = NSUserInterfaceItemIdentifier("descriptionField")
    private static let amountField = NSUserInterfaceItemIdentifier("amountField")

    private let store: AppStore
    private let model: JournalModel
    private let tableView = JournalTableView()
    private var emptyView: NSView?
    private var rows: [JournalRow] = []
    private var cancellables = Set<AnyCancellable>()
    private var isApplyingSelection = false

    init(store: AppStore, model: JournalModel) {
        self.store = store
        self.model = model
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    override func loadView() {
        let root = NSView()
        let header = NSHostingView(rootView: StoreRoot(store: store) { JournalHeader(model: self.model) })
        header.translatesAutoresizingMaskIntoConstraints = false

        for column in Column.allCases {
            let tableColumn = NSTableColumn(identifier: column.identifier)
            tableColumn.title = column.title
            tableColumn.width = column.width
            tableColumn.minWidth = column == .description ? 160 : column.width * 0.7
            tableColumn.resizingMask = column == .description ? [.autoresizingMask, .userResizingMask] : .userResizingMask
            switch column {
            case .amount, .budget, .balance: tableColumn.headerCell.alignment = .right
            case .status: tableColumn.headerCell.alignment = .center
            default: break
            }
            tableView.addTableColumn(tableColumn)
        }
        tableView.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        tableView.allowsMultipleSelection = true
        tableView.usesAlternatingRowBackgroundColors = true
        tableView.rowHeight = 34
        tableView.intercellSpacing = NSSize(width: 10, height: 0)
        tableView.dataSource = self
        tableView.delegate = self
        tableView.target = self
        tableView.doubleAction = #selector(openClickedRow(_:))
        let menu = NSMenu()
        menu.delegate = self
        tableView.menu = menu
        tableView.onDelete = { [weak self] in self?.model.deleteSelection() }
        tableView.onToggle = { [weak self] in self?.model.toggleCheckedSelection() }
        if #available(macOS 11.0, *) {
            tableView.style = .fullWidth
        }

        let scrollView = NSScrollView()
        scrollView.documentView = tableView
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.translatesAutoresizingMaskIntoConstraints = false

        let empty = NSHostingView(rootView: StoreRoot(store: store) { JournalEmptyState(model: self.model) })
        empty.translatesAutoresizingMaskIntoConstraints = false

        root.addSubview(header)
        root.addSubview(scrollView)
        root.addSubview(empty)
        NSLayoutConstraint.activate([
            header.topAnchor.constraint(equalTo: root.topAnchor),
            header.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            scrollView.topAnchor.constraint(equalTo: header.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            empty.centerXAnchor.constraint(equalTo: scrollView.centerXAnchor),
            empty.centerYAnchor.constraint(equalTo: scrollView.centerYAnchor),
            empty.widthAnchor.constraint(equalToConstant: 420),
        ])
        emptyView = empty
        view = root
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        model.onChange = { [weak self] in self?.reload() }
        model.objectWillChange
            .receive(on: DispatchQueue.main)
            .sink { [weak self] in self?.applySelection() }
            .store(in: &cancellables)
        reload()
    }

    private func reload() {
        guard isViewLoaded else { return }
        rows = model.rows
        tableView.reloadData()
        applySelection()
        emptyView?.isHidden = !rows.isEmpty
    }

    private func applySelection() {
        var indexes = IndexSet()
        for (index, row) in rows.enumerated() where model.selection.contains(row.transaction.id) {
            indexes.insert(index)
        }
        guard indexes != tableView.selectedRowIndexes else { return }
        isApplyingSelection = true
        tableView.selectRowIndexes(indexes, byExtendingSelection: false)
        isApplyingSelection = false
    }

    // MARK: - NSTableViewDataSource / Delegate

    func numberOfRows(in tableView: NSTableView) -> Int {
        rows.count
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row index: Int) -> NSView? {
        guard let identifier = tableColumn?.identifier, let column = Column(rawValue: identifier.rawValue), rows.indices.contains(index) else {
            return nil
        }
        let row = rows[index]
        switch column {
        case .account:
            let cell = reuse(AccountCell.self, column)
            cell.configure(name: row.accountName, color: PlatformColor.dmx(hex: row.accountColor) ?? .systemBlue)
            return cell
        case .date:
            let cell = reuse(LabelCell.self, column)
            cell.configure(DayFormat.short(row.transaction.date), color: .secondaryLabelColor)
            return cell
        case .category:
            let cell = reuse(ChipCell.self, column)
            cell.configure(title: row.category.name, icon: row.category.icon, color: PlatformColor.dmx(hex: row.category.color) ?? .systemGray)
            return cell
        case .description:
            let cell = reuse(EditableCell.self, column)
            cell.configure(row.transaction.description, font: .systemFont(ofSize: 13, weight: .medium), color: .labelColor, alignment: .left)
            cell.field.identifier = Self.descriptionField
            cell.field.delegate = self
            return cell
        case .amount:
            let cell = reuse(EditableCell.self, column)
            // Chaque côté d'un virement est un revenu ou une dépense : signe et couleur suivent ce sens.
            let color: NSColor = row.transaction.transactionType == .income ? PlatformColor.dmx(hex: "#059669")! : .systemRed
            cell.configure(Money.signed(row.transaction.amount, kind: row.transaction.transactionType), font: .monospacedDigitSystemFont(ofSize: 13, weight: .bold), color: color, alignment: .right)
            cell.field.identifier = Self.amountField
            cell.field.delegate = self
            return cell
        case .budget:
            let cell = reuse(BudgetCell.self, column)
            cell.configure(row.budget.map { Money.format($0.remaining) }, tooltip: row.budget?.budgetName)
            return cell
        case .status:
            let cell = reuse(StatusCell.self, column)
            cell.configure(checked: row.transaction.checked, target: self, action: #selector(toggleStatus(_:)))
            return cell
        case .balance:
            let cell = reuse(LabelCell.self, column)
            cell.configure(Money.format(row.balance), color: .tertiaryLabelColor, alignment: .right, monospaced: true)
            return cell
        case .actions:
            let cell = reuse(ActionsCell.self, column)
            cell.configure(target: self, edit: #selector(editRow(_:)), delete: #selector(deleteRow(_:)))
            return cell
        }
    }

    private func reuse<Cell: NSView>(_ type: Cell.Type, _ column: Column) -> Cell {
        let identifier = NSUserInterfaceItemIdentifier("cell-\(column.rawValue)")
        if let cell = tableView.makeView(withIdentifier: identifier, owner: nil) as? Cell {
            return cell
        }
        let cell = Cell(frame: .zero)
        cell.identifier = identifier
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        guard !isApplyingSelection else { return }
        let ids = Set(tableView.selectedRowIndexes.compactMap { rows.indices.contains($0) ? rows[$0].transaction.id : nil })
        if ids != model.selection {
            model.selection = ids
        }
    }

    // MARK: - Édition en ligne

    func control(_ control: NSControl, textShouldBeginEditing fieldEditor: NSText) -> Bool {
        let index = tableView.row(for: control)
        if control.identifier == Self.amountField, rows.indices.contains(index) {
            fieldEditor.string = AmountInput.text(rows[index].transaction.amount, emptyWhenZero: false)
        }
        return true
    }

    func controlTextDidEndEditing(_ notification: Notification) {
        guard let field = notification.object as? NSTextField else { return }
        let index = tableView.row(for: field)
        guard rows.indices.contains(index) else { return }
        let row = rows[index]
        let text = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)

        if field.identifier == Self.descriptionField {
            if text.isEmpty || text == row.transaction.description {
                field.stringValue = row.transaction.description
                return
            }
            DispatchQueue.main.async { self.model.updateDescription(row.transaction.id, text) }
        } else if field.identifier == Self.amountField {
            let display = Money.signed(row.transaction.amount, kind: row.transaction.transactionType)
            guard let amount = AmountInput.parse(text), amount != row.transaction.amount else {
                field.stringValue = display
                return
            }
            DispatchQueue.main.async {
                if !self.model.updateAmount(row.transaction.id, text: text) {
                    field.stringValue = display
                }
            }
        }
    }

    // MARK: - Actions

    private func transactionId(for sender: NSView) -> String? {
        let index = tableView.row(for: sender)
        return rows.indices.contains(index) ? rows[index].transaction.id : nil
    }

    @objc private func toggleStatus(_ sender: NSButton) {
        guard let id = transactionId(for: sender) else { return }
        model.toggleChecked(id)
    }

    @objc private func editRow(_ sender: NSButton) {
        guard let id = transactionId(for: sender) else { return }
        store.present(.transaction(id: id))
    }

    @objc private func deleteRow(_ sender: NSButton) {
        guard let id = transactionId(for: sender) else { return }
        model.delete(id)
    }

    @objc private func openClickedRow(_ sender: Any?) {
        let index = tableView.clickedRow
        guard rows.indices.contains(index) else { return }
        store.present(.transaction(id: rows[index].transaction.id))
    }

    // MARK: - Menu contextuel

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = tableView.clickedRow
        guard rows.indices.contains(clicked) else { return }
        if !tableView.selectedRowIndexes.contains(clicked) {
            tableView.selectRowIndexes(IndexSet(integer: clicked), byExtendingSelection: false)
        }
        let count = tableView.selectedRowIndexes.count
        if count == 1 {
            let edit = NSMenuItem(title: "Modifier…", action: #selector(openClickedRow(_:)), keyEquivalent: "")
            edit.target = self
            menu.addItem(edit)
        }
        let toggle = NSMenuItem(title: "Pointer / Dépointer", action: #selector(toggleSelection), keyEquivalent: "")
        toggle.target = self
        menu.addItem(toggle)
        menu.addItem(.separator())
        let delete = NSMenuItem(title: count > 1 ? "Supprimer \(count) transactions" : "Supprimer", action: #selector(deleteSelection), keyEquivalent: "")
        delete.target = self
        menu.addItem(delete)
    }

    @objc private func toggleSelection() {
        model.toggleCheckedSelection()
    }

    @objc private func deleteSelection() {
        model.deleteSelection()
    }
}

// MARK: - En-tête et état vide (SwiftUI)

struct JournalHeader: View {
    @EnvironmentObject private var store: AppStore
    @ObservedObject var model: JournalModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Journal").font(.system(size: 24, weight: .bold))
                Spacer()
                if let view = model.view {
                    Text("\(view.rows.count) / \(view.totalTransactionCount) lignes · Net \(view.visibleNet >= 0 ? "+" : "")\(Money.format(view.visibleNet))")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                }
                Button(action: { store.present(.transaction(id: nil)) }) {
                    HStack(spacing: 6) {
                        DmxIcon("Plus", size: 13)
                        Text("Nouvelle transaction")
                    }
                }
                .buttonStyle(DmxButtonStyle(.primary))
            }
            HStack(spacing: 10) {
                SearchField("Rechercher dans toutes les colonnes...", text: $model.search)
                    .frame(maxWidth: 320)
                MultiSelectButton(
                    "Toutes les catégories",
                    options: store.categories.map { SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color) },
                    selection: $model.categories
                )
                MultiSelectButton("Tous les types", options: JournalModel.typeOptions, selection: $model.types)
                MultiSelectButton("Tous les états", options: JournalModel.statusOptions, selection: $model.statuses)
                MultiSelectButton("Tous les budgets", options: JournalModel.budgetOptions, selection: $model.budgetStatuses)
                Spacer()
            }
            if !model.selection.isEmpty {
                HStack(spacing: 14) {
                    Text("\(model.selection.count) \(model.selection.count > 1 ? "sélectionnées" : "sélectionnée")")
                        .font(.system(size: 13, weight: .semibold))
                    Rectangle().fill(Color.accentColor.opacity(0.3)).frame(width: 1, height: 16)
                    Button(action: model.toggleCheckedSelection) {
                        HStack(spacing: 5) {
                            DmxIcon("CheckCircle2", size: 14)
                            Text("Pointer/Dépointer")
                        }
                        .font(.system(size: 13, weight: .medium))
                    }
                    .buttonStyle(PlainButtonStyle())
                    Spacer()
                    Button(action: { model.selection = [] }) { Text("Annuler") }
                        .buttonStyle(DmxButtonStyle(.subtle))
                    Button(action: model.deleteSelection) {
                        HStack(spacing: 5) {
                            DmxIcon("Trash2", size: 13)
                            Text("Supprimer")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(.destructive))
                }
                .foregroundColor(.accentColor)
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.accentColor.opacity(0.1)))
            }
        }
        .padding(.horizontal, 24)
        .padding(.top, 18)
        .padding(.bottom, 12)
        .background(DmxPalette.pageBackground)
    }
}

struct JournalEmptyState: View {
    @EnvironmentObject private var store: AppStore
    @ObservedObject var model: JournalModel

    var body: some View {
        VStack(spacing: 10) {
            EmptyStateView(
                icon: "Search",
                title: "Aucune transaction",
                message: model.hasFilters ? "Aucun résultat pour vos filtres actuels." : "Commencez par ajouter une transaction ou importez un relevé bancaire."
            )
            if !model.hasFilters {
                Button(action: { store.present(.transaction(id: nil)) }) {
                    HStack(spacing: 6) {
                        DmxIcon("Plus", size: 13)
                        Text("Ajouter une transaction")
                    }
                }
                .buttonStyle(DmxButtonStyle(.primary))
            }
        }
    }
}

// MARK: - Tableau et cellules

final class JournalTableView: NSTableView {
    var onDelete: (() -> Void)?
    var onToggle: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 51, 117:
            onDelete?()
        case 49:
            onToggle?()
        default:
            super.keyDown(with: event)
        }
    }
}

final class LabelCell: NSTableCellView {
    private let label = NSTextField(labelWithString: "")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        label.translatesAutoresizingMaskIntoConstraints = false
        label.lineBreakMode = .byTruncatingTail
        addSubview(label)
        textField = label
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    func configure(_ text: String, color: NSColor, alignment: NSTextAlignment = .left, monospaced: Bool = false) {
        label.stringValue = text
        label.textColor = color
        label.alignment = alignment
        label.font = monospaced ? .monospacedDigitSystemFont(ofSize: 12, weight: .regular) : .systemFont(ofSize: 12)
    }
}

final class AccountCell: NSTableCellView {
    private let bar = NSView()
    private let label = NSTextField(labelWithString: "")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        bar.wantsLayer = true
        bar.layer?.cornerRadius = 2
        bar.translatesAutoresizingMaskIntoConstraints = false
        label.translatesAutoresizingMaskIntoConstraints = false
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.lineBreakMode = .byTruncatingTail
        addSubview(bar)
        addSubview(label)
        NSLayoutConstraint.activate([
            bar.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            bar.centerYAnchor.constraint(equalTo: centerYAnchor),
            bar.widthAnchor.constraint(equalToConstant: 4),
            bar.heightAnchor.constraint(equalToConstant: 16),
            label.leadingAnchor.constraint(equalTo: bar.trailingAnchor, constant: 8),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    func configure(name: String, color: NSColor) {
        label.stringValue = name
        bar.layer?.backgroundColor = color.cgColor
    }
}

final class ChipCell: NSTableCellView {
    private let chip = NSView()
    private let icon = NSImageView()
    private let label = NSTextField(labelWithString: "")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        chip.wantsLayer = true
        chip.layer?.cornerRadius = 9
        chip.translatesAutoresizingMaskIntoConstraints = false
        icon.translatesAutoresizingMaskIntoConstraints = false
        label.translatesAutoresizingMaskIntoConstraints = false
        label.font = .systemFont(ofSize: 10, weight: .bold)
        label.lineBreakMode = .byTruncatingTail
        addSubview(chip)
        chip.addSubview(icon)
        chip.addSubview(label)
        NSLayoutConstraint.activate([
            chip.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            chip.centerYAnchor.constraint(equalTo: centerYAnchor),
            chip.heightAnchor.constraint(equalToConstant: 18),
            chip.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -2),
            icon.leadingAnchor.constraint(equalTo: chip.leadingAnchor, constant: 7),
            icon.centerYAnchor.constraint(equalTo: chip.centerYAnchor),
            icon.widthAnchor.constraint(equalToConstant: 11),
            icon.heightAnchor.constraint(equalToConstant: 11),
            label.leadingAnchor.constraint(equalTo: icon.trailingAnchor, constant: 4),
            label.trailingAnchor.constraint(equalTo: chip.trailingAnchor, constant: -8),
            label.centerYAnchor.constraint(equalTo: chip.centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    func configure(title: String, icon name: String, color: NSColor) {
        label.stringValue = title.uppercased()
        label.textColor = color
        icon.image = DmxIcon.image(name, size: 11)
        icon.contentTintColor = color
        chip.layer?.backgroundColor = color.withAlphaComponent(0.13).cgColor
    }
}

final class EditableCell: NSTableCellView {
    let field = NSTextField()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        field.translatesAutoresizingMaskIntoConstraints = false
        field.isBordered = false
        field.drawsBackground = false
        field.isEditable = true
        field.focusRingType = .none
        field.lineBreakMode = .byTruncatingTail
        field.cell?.usesSingleLineMode = true
        addSubview(field)
        textField = field
        NSLayoutConstraint.activate([
            field.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            field.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
            field.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    func configure(_ text: String, font: NSFont, color: NSColor, alignment: NSTextAlignment) {
        field.stringValue = text
        field.font = font
        field.textColor = color
        field.alignment = alignment
    }
}

final class BudgetCell: NSTableCellView {
    private let label = NSTextField(labelWithString: "")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        label.translatesAutoresizingMaskIntoConstraints = false
        label.font = .monospacedDigitSystemFont(ofSize: 10, weight: .bold)
        label.textColor = NSColor.systemIndigo.withAlphaComponent(0.8)
        label.wantsLayer = true
        label.layer?.cornerRadius = 4
        label.layer?.borderWidth = 1
        label.layer?.borderColor = NSColor.systemIndigo.withAlphaComponent(0.25).cgColor
        addSubview(label)
        NSLayoutConstraint.activate([
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    func configure(_ text: String?, tooltip: String?) {
        label.isHidden = text == nil
        label.stringValue = text.map { " \($0) " } ?? ""
        toolTip = tooltip
    }
}

final class StatusCell: NSTableCellView {
    private let button = NSButton()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.imagePosition = .imageOnly
        addSubview(button)
        NSLayoutConstraint.activate([
            button.centerXAnchor.constraint(equalTo: centerXAnchor),
            button.centerYAnchor.constraint(equalTo: centerYAnchor),
            button.widthAnchor.constraint(equalToConstant: 22),
            button.heightAnchor.constraint(equalToConstant: 22),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    func configure(checked: Bool, target: AnyObject, action: Selector) {
        button.image = DmxIcon.image(checked ? "CheckCircle2" : "Circle", size: 18)
        button.contentTintColor = checked ? PlatformColor.dmx(hex: "#10b981") : .tertiaryLabelColor
        button.toolTip = checked ? "Dépointer" : "Pointer"
        button.target = target
        button.action = action
    }
}

final class ActionsCell: NSTableCellView {
    private let editButton = NSButton()
    private let deleteButton = NSButton()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        for (button, icon) in [(editButton, "Edit2"), (deleteButton, "Trash2")] {
            button.translatesAutoresizingMaskIntoConstraints = false
            button.isBordered = false
            button.imagePosition = .imageOnly
            button.image = DmxIcon.image(icon, size: 14)
            addSubview(button)
        }
        editButton.contentTintColor = .secondaryLabelColor
        editButton.toolTip = "Modifier"
        deleteButton.contentTintColor = .systemRed
        deleteButton.toolTip = "Supprimer"
        NSLayoutConstraint.activate([
            deleteButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
            deleteButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            deleteButton.widthAnchor.constraint(equalToConstant: 24),
            editButton.trailingAnchor.constraint(equalTo: deleteButton.leadingAnchor, constant: -4),
            editButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            editButton.widthAnchor.constraint(equalToConstant: 24),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    func configure(target: AnyObject, edit: Selector, delete: Selector) {
        editButton.target = target
        editButton.action = edit
        deleteButton.target = target
        deleteButton.action = delete
    }
}
