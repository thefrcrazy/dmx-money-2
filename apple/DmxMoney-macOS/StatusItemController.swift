import AppKit
import DmxKit

/// Icône de la barre des menus : 8 premiers comptes, total, navigation, synchronisation.
final class StatusItemController: NSObject, NSMenuDelegate {
    private let store: AppStore
    private weak var appDelegate: AppDelegate?
    private let statusItem: NSStatusItem

    init(store: AppStore, appDelegate: AppDelegate) {
        self.store = store
        self.appDelegate = appDelegate
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        super.init()
        if let button = statusItem.button {
            let icon = NSImage(named: "MenuBarIcon") ?? DmxIcon.image("Wallet", size: 18)
            icon?.isTemplate = true
            button.image = icon
            button.toolTip = "DmxMoney"
        }
        let menu = NSMenu()
        menu.delegate = self
        menu.autoenablesItems = false
        statusItem.menu = menu
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let header = NSMenuItem(title: "DmxMoney", action: nil, keyEquivalent: "")
        header.isEnabled = false
        menu.addItem(header)

        if let summary = try? store.engine.traySummary() {
            for account in summary.accounts.prefix(8) {
                let item = NSMenuItem(title: account.name, action: #selector(openAccount(_:)), keyEquivalent: "")
                item.attributedTitle = row(account.name, Money.format(account.balance), negative: account.balance < 0)
                item.representedObject = account.accountId
                item.target = self
                menu.addItem(item)
            }
            if summary.accounts.count > 8 {
                let more = NSMenuItem(title: "… et \(summary.accounts.count - 8) autres comptes", action: nil, keyEquivalent: "")
                more.isEnabled = false
                menu.addItem(more)
            }
            let total = NSMenuItem(title: "Total", action: nil, keyEquivalent: "")
            total.attributedTitle = row("Total", Money.format(summary.total), negative: summary.total < 0, bold: true)
            total.isEnabled = false
            menu.addItem(total)
            menu.addItem(.separator())
        }

        menu.addItem(action("Ouvrir DmxMoney", #selector(openApp)))
        menu.addItem(action("Nouvelle transaction…", #selector(newTransaction)))
        menu.addItem(.separator())
        for route in [AppRoute.dashboard, .transactions, .budget, .scheduled, .predictions] {
            let item = action(route.title, #selector(openPage(_:)))
            item.image = DmxIcon.image(route.icon, size: 14)
            item.representedObject = route.rawValue
            menu.addItem(item)
        }
        menu.addItem(.separator())
        menu.addItem(action("Synchroniser", #selector(sync)))
        menu.addItem(action("Vérifier les mises à jour…", #selector(checkForUpdates)))
        menu.addItem(action("Quitter DmxMoney", #selector(quit)))
    }

    @objc private func checkForUpdates() {
        UpdateChecker.shared.checkForUpdates(nil)
    }

    private func action(_ title: String, _ selector: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: selector, keyEquivalent: "")
        item.target = self
        return item
    }

    private func row(_ name: String, _ amount: String, negative: Bool, bold: Bool = false) -> NSAttributedString {
        let style = NSMutableParagraphStyle()
        style.tabStops = [NSTextTab(textAlignment: .right, location: 300)]
        let font = bold ? NSFont.boldSystemFont(ofSize: NSFont.systemFontSize) : NSFont.menuFont(ofSize: 0)
        let text = NSMutableAttributedString(string: "\(name)\t", attributes: [.font: font, .paragraphStyle: style])
        text.append(NSAttributedString(string: amount, attributes: [
            .font: NSFont.monospacedDigitSystemFont(ofSize: NSFont.systemFontSize, weight: bold ? .bold : .regular),
            .paragraphStyle: style,
            .foregroundColor: negative ? NSColor.systemRed : NSColor.labelColor,
        ]))
        return text
    }

    @objc private func openAccount(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        store.selectedAccountIds = [id]
        appDelegate?.navigate(to: .transactions)
    }

    @objc private func openPage(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let route = AppRoute(rawValue: raw) else { return }
        appDelegate?.navigate(to: route)
    }

    @objc private func openApp() {
        appDelegate?.showMainWindow(nil)
    }

    @objc private func newTransaction() {
        appDelegate?.newTransaction(nil)
    }

    @objc private func sync() {
        appDelegate?.syncNow(nil)
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}
