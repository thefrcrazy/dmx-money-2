import AppKit
import Combine
import DmxKit
import SwiftUI

/// Journal en NSTableView : sélection multiple, édition de la description et du montant, pointage.
final class JournalViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSTextFieldDelegate,
    NSMenuDelegate
{
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
    private let includesHeader: Bool
    private var mounted = true
    private var preparedRevision: UInt64?
    private var requestedRevision: UInt64?
    private var loadToken: TablePreparationToken?
    private let preparationQueue = DispatchQueue(label: "com.dmxmoney.journal-table", qos: .userInitiated)
    private let tableView = JournalTableView()
    private var emptyView: NSView?
    private var rows: [JournalRow] = []
    private var rowIndexes: [String: Int] = [:]
    private var cancellables = Set<AnyCancellable>()
    private var isApplyingSelection = false
    private var editing: (field: NSTextField, transactionID: String, rowsRevision: UInt64)?

    init(store: AppStore, model: JournalModel, includesHeader: Bool = true) {
        self.store = store
        self.model = model
        self.includesHeader = includesHeader
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
            if [.account, .date, .category, .description, .amount, .balance].contains(column) {
                tableColumn.sortDescriptorPrototype = NSSortDescriptor(key: column.rawValue, ascending: true)
            }
            tableColumn.resizingMask =
                column == .description ? [.autoresizingMask, .userResizingMask] : .userResizingMask
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
        tableView.rowHeight = includesHeader ? 34 : 30
        tableView.usesAutomaticRowHeights = false
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
        tableView.onClear = { [weak self] in self?.model.selection = [] }
        if #available(macOS 11.0, *) {
            tableView.style = includesHeader ? .fullWidth : .inset
        }

        let scrollView = NSScrollView()
        scrollView.documentView = tableView
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.translatesAutoresizingMaskIntoConstraints = false

        let empty = NSHostingView(rootView: StoreRoot(store: store) { JournalEmptyState(model: self.model) })
        empty.translatesAutoresizingMaskIntoConstraints = false

        if includesHeader { root.addSubview(header) }
        root.addSubview(scrollView)
        if includesHeader { root.addSubview(empty) }
        if includesHeader {
            NSLayoutConstraint.activate([
                header.topAnchor.constraint(equalTo: root.topAnchor),
                header.leadingAnchor.constraint(equalTo: root.leadingAnchor),
                header.trailingAnchor.constraint(equalTo: root.trailingAnchor),
                empty.centerXAnchor.constraint(equalTo: scrollView.centerXAnchor),
                empty.centerYAnchor.constraint(equalTo: scrollView.centerYAnchor),
                empty.widthAnchor.constraint(equalToConstant: 420),
            ])
        }
        NSLayoutConstraint.activate([
            scrollView.topAnchor.constraint(equalTo: includesHeader ? header.bottomAnchor : root.topAnchor),
            scrollView.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: root.bottomAnchor),
        ])
        emptyView = includesHeader ? empty : nil
        view = root
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        model.objectWillChange
            .receive(on: DispatchQueue.main)
            .sink { [weak self] in
                guard let self, self.mounted else { return }
                if self.requestedRevision != self.model.rowsRevision { self.reload() } else { self.applySelection() }
            }
            .store(in: &cancellables)
        reload()
    }

    override func viewWillAppear() {
        super.viewWillAppear()
        mounted = true
        if preparedRevision != model.rowsRevision { reload() } else { applySelection() }
    }

    override func viewWillDisappear() {
        super.viewWillDisappear()
        if editing != nil { tableView.window?.makeFirstResponder(nil) }
        mounted = false
        cancelPreparation()
        editing = nil
        tableView.abortEditing()
    }

    /// Called when SwiftUI removes the journal: no queued result or cell work survives it.
    func stopUpdates() {
        if editing != nil { tableView.window?.makeFirstResponder(nil) }
        mounted = false
        cancelPreparation()
        editing = nil
        tableView.abortEditing()
        cancellables.removeAll()
        tableView.delegate = nil
        tableView.dataSource = nil
    }

    private func cancelPreparation() {
        loadToken?.cancel()
        loadToken = nil
        requestedRevision = preparedRevision
    }

    private func reload() {
        guard isViewLoaded, mounted else { return }
        cancelPreparation()
        let token = TablePreparationToken()
        loadToken = token
        let revision = model.rowsRevision
        requestedRevision = revision
        let source = model.rows
        let order = tableView.sortDescriptors.map { JournalSort(key: $0.key ?? "", ascending: $0.ascending) }
        preparationQueue.async { [weak self] in
            guard !token.isCancelled else { return }
            let prepared = JournalTablePreparation.prepare(source, order: order)
            guard !token.isCancelled else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self, self.mounted, !token.isCancelled,
                    self.model.rowsRevision == revision
                else { return }
                self.editing = nil
                self.tableView.abortEditing()
                self.rows = prepared.rows
                self.rowIndexes = prepared.indexes
                self.preparedRevision = revision
                self.loadToken = nil
                self.tableView.reloadData()
                self.applySelection()
                self.emptyView?.isHidden = !self.rows.isEmpty
            }
        }
    }

    func tableView(_ tableView: NSTableView, sortDescriptorsDidChange oldDescriptors: [NSSortDescriptor]) {
        // Finish the editor against its captured ID before moving any rows.
        tableView.window?.makeFirstResponder(tableView)
        reload()
    }

    private func applySelection() {
        let indexes = IndexSet(model.selection.compactMap { rowIndexes[$0] })
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
        guard let identifier = tableColumn?.identifier, let column = Column(rawValue: identifier.rawValue),
            rows.indices.contains(index)
        else {
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
            cell.configure(
                title: row.category.name, icon: row.category.icon,
                color: PlatformColor.dmx(hex: row.category.color) ?? .systemGray)
            return cell
        case .description:
            let cell = reuse(EditableCell.self, column)
            cell.configure(
                row.transaction.description, font: .systemFont(ofSize: 13, weight: .medium), color: .labelColor,
                alignment: .left)
            cell.field.identifier = Self.descriptionField
            cell.field.delegate = self
            return cell
        case .amount:
            let cell = reuse(EditableCell.self, column)
            // Chaque côté d'un virement est un revenu ou une dépense : signe et couleur suivent ce sens.
            let color: NSColor =
                row.transaction.transactionType == .income ? PlatformColor.dmx(hex: "#059669")! : .systemRed
            cell.configure(
                Money.signed(row.transaction.amount, kind: row.transaction.transactionType),
                font: .monospacedDigitSystemFont(ofSize: 13, weight: .bold), color: color, alignment: .right)
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
        let ids = Set(
            tableView.selectedRowIndexes.compactMap { rows.indices.contains($0) ? rows[$0].transaction.id : nil })
        if ids != model.selection {
            model.selection = ids
        }
    }

    // MARK: - Édition en ligne

    func control(_ control: NSControl, textShouldBeginEditing fieldEditor: NSText) -> Bool {
        let index = tableView.row(for: control)
        guard let field = control as? NSTextField, rows.indices.contains(index),
            preparedRevision == model.rowsRevision
        else { return false }
        editing = (field, rows[index].transaction.id, model.rowsRevision)
        if control.identifier == Self.amountField {
            fieldEditor.string = AmountInput.text(rows[index].transaction.amount, emptyWhenZero: false)
        }
        return true
    }

    func controlTextDidEndEditing(_ notification: Notification) {
        guard let field = notification.object as? NSTextField else { return }
        guard let edit = editing, edit.field === field else { return }
        editing = nil
        guard edit.rowsRevision == model.rowsRevision,
            let index = rowIndexes[edit.transactionID], rows.indices.contains(index)
        else { return }
        let row = rows[index]
        let text = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)

        if field.identifier == Self.descriptionField {
            if text.isEmpty || text == row.transaction.description {
                field.stringValue = row.transaction.description
                return
            }
            DispatchQueue.main.async {
                self.model.updateDescription(row.transaction.id, text, baseDescription: row.transaction.description)
            }
        } else if field.identifier == Self.amountField {
            let display = Money.signed(row.transaction.amount, kind: row.transaction.transactionType)
            guard let amount = AmountInput.parse(text), amount != row.transaction.amount else {
                field.stringValue = display
                return
            }
            DispatchQueue.main.async {
                if !self.model.updateAmount(row.transaction.id, text: text, baseAmount: row.transaction.amount),
                    self.transactionId(for: field) == row.transaction.id
                {
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
        let delete = NSMenuItem(
            title: count > 1 ? "Supprimer \(count) transactions" : "Supprimer", action: #selector(deleteSelection),
            keyEquivalent: "")
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
                    Text(
                        "\(view.rows.count) / \(view.totalTransactionCount) lignes · Net \(view.visibleNet >= 0 ? "+" : "")\(Money.format(view.visibleNet))"
                    )
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
                    options: store.categories.map {
                        SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color)
                    },
                    selection: $model.categories
                )
                MultiSelectButton("Tous les types", options: JournalModel.typeOptions, selection: $model.types)
                MultiSelectButton("Tous les états", options: JournalModel.statusOptions, selection: $model.statuses)
                MultiSelectButton(
                    "Tous les budgets", options: JournalModel.budgetOptions, selection: $model.budgetStatuses)
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
                title: model.isLoading ? "Chargement du journal…" : "Aucune transaction",
                message: model.isLoading
                    ? nil
                    : model.hasFilters
                        ? "Aucun résultat pour vos filtres actuels."
                        : "Commencez par ajouter une transaction ou importez un relevé bancaire."
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
    var onClear: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 51, 117:
            onDelete?()
        case 49:
            onToggle?()
        case 53:
            onClear?()
        default:
            super.keyDown(with: event)
        }
    }
}

// Fixed-height table cells use frames: no constraint graph is rebuilt while scrolling.
final class LabelCell: NSTableCellView {
    private let label = NSTextField(labelWithString: "")
    override init(frame: NSRect) {
        super.init(frame: frame)
        label.lineBreakMode = .byTruncatingTail
        addSubview(label)
        textField = label
    }
    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas utilisé") }
    override func layout() {
        super.layout()
        label.frame = NSRect(x: 2, y: (bounds.height - 18) / 2, width: max(0, bounds.width - 4), height: 18)
    }
    func configure(_ text: String, color: NSColor, alignment: NSTextAlignment = .left, monospaced: Bool = false) {
        if label.stringValue != text { label.stringValue = text }
        label.textColor = color
        label.alignment = alignment
        label.font = monospaced ? .monospacedDigitSystemFont(ofSize: 12, weight: .regular) : .systemFont(ofSize: 12)
    }
}
final class AccountCell: NSTableCellView {
    private let bar = NSView()
    private let label = NSTextField(labelWithString: "")
    override init(frame: NSRect) {
        super.init(frame: frame)
        bar.wantsLayer = true
        bar.layer?.cornerRadius = 2
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.lineBreakMode = .byTruncatingTail
        addSubview(bar)
        addSubview(label)
        textField = label
    }
    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas utilisé") }
    override func layout() {
        super.layout()
        bar.frame = NSRect(x: 2, y: (bounds.height - 16) / 2, width: 4, height: 16)
        label.frame = NSRect(x: 14, y: (bounds.height - 18) / 2, width: max(0, bounds.width - 16), height: 18)
    }
    func configure(name: String, color: NSColor) {
        if label.stringValue != name { label.stringValue = name }
        bar.layer?.backgroundColor = color.cgColor
    }
}
final class ChipCell: NSTableCellView {
    private let chip = NSView(), icon = NSImageView(), label = NSTextField(labelWithString: "")
    private var preferredWidth: CGFloat = 0
    private var iconName: String?
    override init(frame: NSRect) {
        super.init(frame: frame)
        chip.wantsLayer = true
        chip.layer?.cornerRadius = 9
        label.font = .systemFont(ofSize: 10, weight: .bold)
        label.lineBreakMode = .byTruncatingTail
        addSubview(chip)
        chip.addSubview(icon)
        chip.addSubview(label)
        setAccessibilityElement(true)
        setAccessibilityRole(.staticText)
    }
    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas utilisé") }
    override func layout() {
        super.layout()
        let width = min(max(0, bounds.width - 4), preferredWidth)
        chip.frame = NSRect(x: 2, y: (bounds.height - 18) / 2, width: width, height: 18)
        icon.frame = NSRect(x: 7, y: 3.5, width: 11, height: 11)
        label.frame = NSRect(x: 22, y: 2, width: max(0, width - 30), height: 15)
    }
    func configure(title: String, icon name: String, color: NSColor) {
        let text = title.uppercased()
        if label.stringValue != text {
            label.stringValue = text
            preferredWidth = label.intrinsicContentSize.width + 30
            needsLayout = true
        }
        if iconName != name {
            iconName = name
            icon.image = DmxIcon.image(name, size: 11)
        }
        label.textColor = color
        icon.contentTintColor = color
        chip.layer?.backgroundColor = color.withAlphaComponent(0.13).cgColor
        setAccessibilityLabel(title)
    }
}
final class EditableCell: NSTableCellView {
    let field = NSTextField()
    override init(frame: NSRect) {
        super.init(frame: frame)
        field.isBordered = false
        field.drawsBackground = false
        field.isEditable = true
        field.focusRingType = .none
        field.lineBreakMode = .byTruncatingTail
        field.cell?.usesSingleLineMode = true
        addSubview(field)
        textField = field
    }
    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas utilisé") }
    override func layout() {
        super.layout()
        field.frame = NSRect(x: 2, y: (bounds.height - 20) / 2, width: max(0, bounds.width - 4), height: 20)
    }
    func configure(_ text: String, font: NSFont, color: NSColor, alignment: NSTextAlignment) {
        if field.stringValue != text { field.stringValue = text }
        field.font = font
        field.textColor = color
        field.alignment = alignment
    }
}
final class BudgetCell: NSTableCellView {
    private let label = NSTextField(labelWithString: "")
    private var preferredWidth: CGFloat = 0
    override init(frame: NSRect) {
        super.init(frame: frame)
        label.font = .monospacedDigitSystemFont(ofSize: 10, weight: .bold)
        label.textColor = NSColor.systemIndigo.withAlphaComponent(0.8)
        label.wantsLayer = true
        label.layer?.cornerRadius = 4
        label.layer?.borderWidth = 1
        label.layer?.borderColor = NSColor.systemIndigo.withAlphaComponent(0.25).cgColor
        addSubview(label)
        textField = label
    }
    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas utilisé") }
    override func layout() {
        super.layout()
        let width = min(preferredWidth, max(0, bounds.width - 4))
        label.frame = NSRect(x: bounds.width - width - 2, y: (bounds.height - 16) / 2, width: width, height: 16)
    }
    func configure(_ text: String?, tooltip: String?) {
        label.isHidden = text == nil
        let value = text.map { " \($0) " } ?? ""
        if label.stringValue != value {
            label.stringValue = value
            preferredWidth = label.intrinsicContentSize.width
            needsLayout = true
        }
        toolTip = tooltip
    }
}
final class StatusCell: NSTableCellView {
    private let button = NSButton()
    private var checkedValue: Bool?
    override init(frame: NSRect) {
        super.init(frame: frame)
        button.isBordered = false
        button.imagePosition = .imageOnly
        addSubview(button)
    }
    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas utilisé") }
    override func layout() {
        super.layout()
        button.frame = NSRect(x: (bounds.width - 22) / 2, y: (bounds.height - 22) / 2, width: 22, height: 22)
    }
    func configure(checked: Bool, target: AnyObject, action: Selector) {
        if checkedValue != checked {
            checkedValue = checked
            button.image = DmxIcon.image(checked ? "CheckCircle2" : "Circle", size: 18)
        }
        button.contentTintColor = checked ? PlatformColor.dmx(hex: "#10b981") : .tertiaryLabelColor
        let label = checked ? "Dépointer" : "Pointer"
        button.toolTip = label
        button.setAccessibilityLabel(label)
        button.target = target
        button.action = action
    }
}
final class ActionsCell: NSTableCellView {
    private let editButton = NSButton(), deleteButton = NSButton()
    override init(frame: NSRect) {
        super.init(frame: frame)
        for (button, icon, label) in [(editButton, "Edit2", "Modifier"), (deleteButton, "Trash2", "Supprimer")] {
            button.isBordered = false
            button.imagePosition = .imageOnly
            button.image = DmxIcon.image(icon, size: 14)
            button.toolTip = label
            button.setAccessibilityLabel(label)
            addSubview(button)
        }
        editButton.contentTintColor = .secondaryLabelColor
        deleteButton.contentTintColor = .systemRed
    }
    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas utilisé") }
    override func layout() {
        super.layout()
        deleteButton.frame = NSRect(x: bounds.width - 26, y: (bounds.height - 24) / 2, width: 24, height: 24)
        editButton.frame = NSRect(x: bounds.width - 54, y: (bounds.height - 24) / 2, width: 24, height: 24)
    }
    func configure(target: AnyObject, edit: Selector, delete: Selector) {
        editButton.target = target
        editButton.action = edit
        deleteButton.target = target
        deleteButton.action = delete
    }
}

final class TablePreparationToken: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false
    func cancel() {
        lock.lock()
        cancelled = true
        lock.unlock()
    }
    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }
}

struct JournalSort: Sendable {
    let key: String
    let ascending: Bool
}

/// Pure preparation off the UI thread; tie-breaks retain the kernel's existing row order.
enum JournalTablePreparation {
    static func prepare(_ source: [JournalRow], order: [JournalSort]) -> (rows: [JournalRow], indexes: [String: Int]) {
        let rows: [JournalRow]
        if order.isEmpty {
            rows = source
        } else {
            rows = source.enumerated().sorted { left, right in
                for sort in order {
                    let result: ComparisonResult
                    switch sort.key {
                    case "account":
                        result = left.element.accountName.localizedStandardCompare(right.element.accountName)
                    case "date": result = left.element.transaction.date.compare(right.element.transaction.date)
                    case "category":
                        result = left.element.category.name.localizedStandardCompare(right.element.category.name)
                    case "description":
                        result = left.element.transaction.description.localizedStandardCompare(
                            right.element.transaction.description)
                    case "amount": result = compare(left.element.transaction.amount, right.element.transaction.amount)
                    case "balance": result = compare(left.element.balance, right.element.balance)
                    default: continue
                    }
                    if result != .orderedSame {
                        return sort.ascending ? result == .orderedAscending : result == .orderedDescending
                    }
                }
                return left.offset < right.offset
            }.map(\.element)
        }
        let indexes = Dictionary(
            rows.enumerated().map { ($0.element.transaction.id, $0.offset) }, uniquingKeysWith: { first, _ in first })
        return (rows, indexes)
    }

    private static func compare(_ left: Double, _ right: Double) -> ComparisonResult {
        if left < right { return .orderedAscending }
        if left > right { return .orderedDescending }
        return .orderedSame
    }
}
