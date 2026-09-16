import AppKit
import DmxKit

/// Barre latérale : Général / Finances / Analyses, puis Catégories, Paramètres et Quitter en pied.
final class SidebarViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate {
    private final class Node: NSObject {
        let title: String
        let route: AppRoute?
        let children: [Node]

        init(title: String, route: AppRoute?, children: [Node] = []) {
            self.title = title
            self.route = route
            self.children = children
        }
    }

    private let store: AppStore
    private let outlineView = NSOutlineView()
    private let nodes: [Node]
    private var footerButtons: [AppRoute: SidebarButton] = [:]
    private var isSyncingSelection = false

    init(store: AppStore) {
        self.store = store
        nodes = AppRoute.sidebarSections.map { section in
            Node(title: section.title, route: nil, children: section.routes.map { Node(title: $0.title, route: $0) })
        }
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    override func loadView() {
        let root = NSVisualEffectView()
        root.material = .sidebar
        root.blendingMode = .behindWindow
        root.state = .followsWindowActiveState

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("main"))
        column.isEditable = false
        outlineView.addTableColumn(column)
        outlineView.outlineTableColumn = column
        outlineView.headerView = nil
        outlineView.selectionHighlightStyle = .regular
        outlineView.floatsGroupRows = false
        outlineView.indentationPerLevel = 0
        outlineView.rowSizeStyle = .medium
        outlineView.backgroundColor = .clear
        outlineView.focusRingType = .none
        outlineView.dataSource = self
        outlineView.delegate = self
        if #available(macOS 11.0, *) {
            outlineView.style = .plain
        }

        let scrollView = NSScrollView()
        scrollView.documentView = outlineView
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = false
        scrollView.translatesAutoresizingMaskIntoConstraints = false

        let separator = NSBox()
        separator.boxType = .separator
        separator.translatesAutoresizingMaskIntoConstraints = false

        var footerViews: [NSView] = []
        for route in AppRoute.footerRoutes {
            let button = SidebarButton(title: route.title, icon: route.icon, target: self, action: #selector(footerSelected(_:)))
            button.tag = AppRoute.allCases.firstIndex(of: route) ?? 0
            footerButtons[route] = button
            footerViews.append(button)
        }
        let quit = SidebarButton(title: "Quitter", icon: "Power", tint: .systemRed, target: NSApp, action: #selector(NSApplication.terminate(_:)))
        footerViews.append(quit)

        let version = NSTextField(labelWithString: "DMXMONEY • V\(AppInfo.version)")
        version.font = .systemFont(ofSize: 9, weight: .bold)
        version.textColor = .tertiaryLabelColor
        version.alignment = .center

        let footer = NSStackView(views: footerViews + [version])
        footer.orientation = .vertical
        footer.alignment = .leading
        footer.spacing = 2
        footer.setCustomSpacing(12, after: quit)
        footer.translatesAutoresizingMaskIntoConstraints = false
        footerViews.forEach { $0.widthAnchor.constraint(equalTo: footer.widthAnchor).isActive = true }
        version.widthAnchor.constraint(equalTo: footer.widthAnchor).isActive = true

        root.addSubview(scrollView)
        root.addSubview(separator)
        root.addSubview(footer)
        NSLayoutConstraint.activate([
            scrollView.topAnchor.constraint(equalTo: root.topAnchor, constant: 8),
            scrollView.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            separator.topAnchor.constraint(equalTo: scrollView.bottomAnchor),
            separator.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            footer.topAnchor.constraint(equalTo: separator.bottomAnchor, constant: 10),
            footer.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 10),
            footer.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -10),
            footer.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -12),
        ])
        view = root
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        outlineView.reloadData()
        outlineView.expandItem(nil, expandChildren: true)
        select(store.route)
    }

    func select(_ route: AppRoute) {
        guard isViewLoaded else { return }
        isSyncingSelection = true
        defer { isSyncingSelection = false }
        if let node = nodes.flatMap({ $0.children }).first(where: { $0.route == route }) {
            let row = outlineView.row(forItem: node)
            if row >= 0 {
                outlineView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
            }
        } else {
            outlineView.deselectAll(nil)
        }
        footerButtons.forEach { $0.value.isActive = $0.key == route }
    }

    @objc private func footerSelected(_ sender: NSButton) {
        guard AppRoute.allCases.indices.contains(sender.tag) else { return }
        store.route = AppRoute.allCases[sender.tag]
    }

    // MARK: - NSOutlineViewDataSource

    func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        (item as? Node)?.children.count ?? nodes.count
    }

    func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        (item as? Node)?.children[index] ?? nodes[index]
    }

    func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
        !((item as? Node)?.children.isEmpty ?? true)
    }

    // MARK: - NSOutlineViewDelegate

    func outlineView(_ outlineView: NSOutlineView, isGroupItem item: Any) -> Bool {
        (item as? Node)?.route == nil
    }

    func outlineView(_ outlineView: NSOutlineView, shouldSelectItem item: Any) -> Bool {
        (item as? Node)?.route != nil
    }

    func outlineView(_ outlineView: NSOutlineView, shouldShowOutlineCellForItem item: Any) -> Bool {
        false
    }

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? Node else { return nil }
        let cell = node.route == nil ? NSTableCellView() : SidebarRouteCell()
        let label = NSTextField(labelWithString: node.route == nil ? node.title.uppercased() : node.title)
        label.translatesAutoresizingMaskIntoConstraints = false
        label.lineBreakMode = .byTruncatingTail
        cell.addSubview(label)
        cell.textField = label

        if let route = node.route {
            label.font = .systemFont(ofSize: 13)
            let imageView = NSImageView()
            imageView.image = DmxIcon.image(route.icon, size: 16)
            imageView.contentTintColor = .secondaryLabelColor
            imageView.translatesAutoresizingMaskIntoConstraints = false
            cell.addSubview(imageView)
            cell.imageView = imageView
            NSLayoutConstraint.activate([
                imageView.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6),
                imageView.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
                imageView.widthAnchor.constraint(equalToConstant: 16),
                imageView.heightAnchor.constraint(equalToConstant: 16),
                label.leadingAnchor.constraint(equalTo: imageView.trailingAnchor, constant: 8),
                label.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -4),
                label.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
            ])
        } else {
            label.font = .systemFont(ofSize: 10, weight: .bold)
            label.textColor = .tertiaryLabelColor
            NSLayoutConstraint.activate([
                label.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6),
                label.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -4),
                label.bottomAnchor.constraint(equalTo: cell.bottomAnchor, constant: -3),
            ])
        }
        return cell
    }

    func outlineView(_ outlineView: NSOutlineView, rowViewForItem item: Any) -> NSTableRowView? {
        SidebarRowView()
    }

    func outlineView(_ outlineView: NSOutlineView, heightOfRowByItem item: Any) -> CGFloat {
        (item as? Node)?.route == nil ? 30 : 28
    }

    func outlineViewSelectionDidChange(_ notification: Notification) {
        guard !isSyncingSelection,
              let node = outlineView.item(atRow: outlineView.selectedRow) as? Node,
              let route = node.route,
              store.route != route
        else { return }
        store.route = route
    }
}

/// Bouton du pied de barre latérale (Catégories, Paramètres, Quitter).
final class SidebarButton: NSButton {
    private let tint: NSColor

    var isActive = false {
        didSet { updateAppearance() }
    }

    init(title: String, icon: String, tint: NSColor = .labelColor, target: AnyObject?, action: Selector) {
        self.tint = tint
        super.init(frame: .zero)
        self.title = "  " + title
        self.target = target
        self.action = action
        image = DmxIcon.image(icon, size: 16)
        imagePosition = .imageLeading
        alignment = .left
        isBordered = false
        font = .systemFont(ofSize: 13)
        wantsLayer = true
        layer?.cornerRadius = 6
        translatesAutoresizingMaskIntoConstraints = false
        heightAnchor.constraint(equalToConstant: 28).isActive = true
        updateAppearance()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateAppearance()
    }

    private func updateAppearance() {
        contentTintColor = isActive ? .controlAccentColor : (tint == .labelColor ? .secondaryLabelColor : tint)
        attributedTitle = NSAttributedString(string: title, attributes: [
            .foregroundColor: isActive ? NSColor.controlAccentColor : tint,
            .font: NSFont.systemFont(ofSize: 13, weight: isActive ? .semibold : .regular),
        ])
        layer?.backgroundColor = isActive ? NSColor.controlAccentColor.withAlphaComponent(0.15).cgColor : nil
    }
}

/// Match template icons and text to AppKit's selection background in both appearances.
final class SidebarRouteCell: NSTableCellView {
    override var allowsVibrancy: Bool { false }
    override var backgroundStyle: NSView.BackgroundStyle {
        didSet { updateColors() }
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateColors()
    }

    private func updateColors() {
        let selected = backgroundStyle == .emphasized
        textField?.textColor = selected ? .alternateSelectedControlTextColor : .labelColor
        imageView?.contentTintColor = selected ? .alternateSelectedControlTextColor : .secondaryLabelColor
    }
}

/// A translucent accent keeps the source-list selection readable in inactive windows too.
final class SidebarRowView: NSTableRowView {
    override var allowsVibrancy: Bool { false }
    override var interiorBackgroundStyle: NSView.BackgroundStyle { .normal }

    override func drawSelection(in dirtyRect: NSRect) {
        guard selectionHighlightStyle != .none else { return }
        NSColor.controlAccentColor.withAlphaComponent(isEmphasized ? 0.18 : 0.10).setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: 6, yRadius: 6).fill()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        needsDisplay = true
    }
}
