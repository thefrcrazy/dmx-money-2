import AppKit
import DmxKit

/// Barre de menus construite en code (pas de storyboard).
enum MainMenu {
    static func build(delegate: AppDelegate) -> NSMenu {
        let main = NSMenu()
        main.addItem(submenu("DmxMoney", appMenu(delegate)))
        main.addItem(submenu("Fichier", fileMenu(delegate)))
        main.addItem(submenu("Édition", editMenu()))
        main.addItem(submenu("Présentation", viewMenu(delegate)))
        let window = windowMenu(delegate)
        main.addItem(submenu("Fenêtre", window))
        NSApp.windowsMenu = window
        let help = NSMenu(title: "Aide")
        main.addItem(submenu("Aide", help))
        NSApp.helpMenu = help
        return main
    }

    private static func submenu(_ title: String, _ menu: NSMenu) -> NSMenuItem {
        menu.title = title
        let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        item.submenu = menu
        return item
    }

    private static func item(_ title: String, _ action: Selector?, _ key: String = "", modifiers: NSEvent.ModifierFlags = .command, target: AnyObject? = nil) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.keyEquivalentModifierMask = modifiers
        item.target = target
        return item
    }

    private static func appMenu(_ delegate: AppDelegate) -> NSMenu {
        let menu = NSMenu()
        menu.addItem(item("À propos de DmxMoney", #selector(NSApplication.orderFrontStandardAboutPanel(_:))))
        let updates = item(
            "Vérifier les mises à jour…",
            #selector(UpdateChecker.checkForUpdates(_:)),
            target: UpdateChecker.shared
        )
        updates.isHidden = !UpdateChecker.shared.isAvailable
        menu.addItem(updates)
        menu.addItem(.separator())
        menu.addItem(item("Paramètres…", #selector(AppDelegate.showSettings(_:)), ",", target: delegate))
        menu.addItem(.separator())
        let services = NSMenu(title: "Services")
        menu.addItem(submenu("Services", services))
        NSApp.servicesMenu = services
        menu.addItem(.separator())
        menu.addItem(item("Masquer DmxMoney", #selector(NSApplication.hide(_:)), "h"))
        menu.addItem(item("Masquer les autres", #selector(NSApplication.hideOtherApplications(_:)), "h", modifiers: [.command, .option]))
        menu.addItem(item("Tout afficher", #selector(NSApplication.unhideAllApplications(_:))))
        menu.addItem(.separator())
        menu.addItem(item("Quitter DmxMoney", #selector(NSApplication.terminate(_:)), "q"))
        return menu
    }

    private static func fileMenu(_ delegate: AppDelegate) -> NSMenu {
        let menu = NSMenu()
        menu.addItem(item("Nouvelle transaction", #selector(AppDelegate.newTransaction(_:)), "n", target: delegate))
        menu.addItem(item("Nouveau compte", #selector(AppDelegate.newAccount(_:)), "n", modifiers: [.command, .shift], target: delegate))
        menu.addItem(.separator())
        menu.addItem(item("Importer ou restaurer…", #selector(AppDelegate.importFile(_:)), "o", target: delegate))
        menu.addItem(item("Exporter les données…", #selector(AppDelegate.exportBackup(_:)), "e", modifiers: [.command, .shift], target: delegate))
        menu.addItem(.separator())
        menu.addItem(item("Synchroniser", #selector(AppDelegate.syncNow(_:)), "r", target: delegate))
        menu.addItem(.separator())
        menu.addItem(item("Fermer la fenêtre", #selector(NSWindow.performClose(_:)), "w"))
        return menu
    }

    private static func editMenu() -> NSMenu {
        let menu = NSMenu()
        menu.addItem(item("Annuler", Selector(("undo:")), "z"))
        menu.addItem(item("Rétablir", Selector(("redo:")), "z", modifiers: [.command, .shift]))
        menu.addItem(.separator())
        menu.addItem(item("Couper", #selector(NSText.cut(_:)), "x"))
        menu.addItem(item("Copier", #selector(NSText.copy(_:)), "c"))
        menu.addItem(item("Coller", #selector(NSText.paste(_:)), "v"))
        menu.addItem(item("Tout sélectionner", #selector(NSText.selectAll(_:)), "a"))
        return menu
    }

    private static func viewMenu(_ delegate: AppDelegate) -> NSMenu {
        let menu = NSMenu()
        for (index, route) in AppRoute.allCases.enumerated() {
            let key = index < 9 ? String(index + 1) : ""
            let entry = item(route.title, #selector(AppDelegate.navigateFromMenu(_:)), key, target: delegate)
            entry.tag = index
            entry.image = DmxIcon.image(route.icon, size: 14)
            menu.addItem(entry)
            if route == .transactions || route == .scheduled || route == .predictions {
                menu.addItem(.separator())
            }
        }
        menu.addItem(.separator())
        menu.addItem(item("Afficher/Masquer la barre latérale", #selector(NSSplitViewController.toggleSidebar(_:)), "s", modifiers: [.command, .control]))
        menu.addItem(item("Plein écran", #selector(NSWindow.toggleFullScreen(_:)), "f", modifiers: [.command, .control]))
        return menu
    }

    private static func windowMenu(_ delegate: AppDelegate) -> NSMenu {
        let menu = NSMenu()
        menu.addItem(item("Réduire", #selector(NSWindow.performMiniaturize(_:)), "m"))
        menu.addItem(item("Zoom", #selector(NSWindow.performZoom(_:))))
        menu.addItem(.separator())
        menu.addItem(item("Fenêtre principale", #selector(AppDelegate.showMainWindow(_:)), "0", target: delegate))
        menu.addItem(item("Tout ramener au premier plan", #selector(NSApplication.arrangeInFront(_:))))
        return menu
    }
}
