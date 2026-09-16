import AppKit
import Combine
import DmxKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private(set) var store: AppStore!
    private var windowController: MainWindowController?
    private var cloud: CloudSyncController?
    private var statusItemController: StatusItemController?
    private var cancellables = Set<AnyCancellable>()
    private var pendingOpenURLs: [URL] = []

    // MARK: - Cycle de vie

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = MainMenu.build(delegate: self)

        let store: AppStore
        do {
            store = try AppStore.openDefault()
        } catch {
            let alert = NSAlert()
            alert.alertStyle = .critical
            alert.messageText = "Impossible d'ouvrir la base DmxMoney"
            alert.informativeText = AppStore.message(for: error)
            alert.runModal()
            NSApp.terminate(nil)
            return
        }
        self.store = store
        cloud = CloudSyncController(store: store)

        store.$settings
            .map { $0.theme }
            .removeDuplicates()
            .sink { theme in AppDelegate.apply(theme: theme) }
            .store(in: &cancellables)

        let controller = MainWindowController(store: store, actions: settingsActions)
        windowController = controller

        if let directory = SnapshotRunner.directory, let window = controller.window {
            NSApp.setActivationPolicy(.accessory)
            window.alphaValue = 0
            window.orderFrontRegardless()
            SnapshotRunner.run(store: store, window: window, directory: directory)
            return
        }

        controller.showWindow(nil)
        statusItemController = StatusItemController(store: store, appDelegate: self)

        store.startScheduledRefresh()
        startBridge()
        // Après le premier cycle d'affichage : un panneau modal ouvert pendant la mise en page
        // de la fenêtre relancerait le rendu des pages SwiftUI hébergées.
        DispatchQueue.main.async { [weak self] in
            self?.reportLegacyImport()
            store.presentWhatsNewIfNeeded()
            UpdateChecker.shared.checkInBackgroundIfNeeded()
        }

        pendingOpenURLs.forEach { openImportFile($0) }
        pendingOpenURLs = []
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        runDueScheduled()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        // La fenêtre se ferme sans quitter : le pont PWA et la barre des menus restent actifs.
        false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showMainWindow(nil)
        return true
    }

    func applicationWillTerminate(_ notification: Notification) {
        store?.engine.stopBridge()
    }

    func application(_ sender: NSApplication, openFile filename: String) -> Bool {
        let url = URL(fileURLWithPath: filename)
        guard store != nil else {
            pendingOpenURLs.append(url)
            return true
        }
        showMainWindow(nil)
        return openImportFile(url)
    }

    private static func apply(theme: Theme) {
        switch theme {
        case .light: NSApp.appearance = NSAppearance(named: .aqua)
        case .dark: NSApp.appearance = NSAppearance(named: .darkAqua)
        case .system: NSApp.appearance = nil
        }
    }

    private func runDueScheduled() {
        guard let store = store else { return }
        store.processDueScheduled()
    }

    private func startBridge() {
        BridgeLauncher.start(store: store)
    }

    private func reportLegacyImport() {
        let report = store.engine.openReport()
        if let error = report.legacyImportError {
            store.errorMessage = "La base DmxMoney 1.x n'a pas pu être reprise : \(error)"
        } else if report.importedLegacyDatabase != nil {
            store.showToast("Vos données DmxMoney 1.x ont été reprises")
        }
    }

    // MARK: - Actions de menu

    @objc func showMainWindow(_ sender: Any?) {
        windowController?.showWindow(nil)
        windowController?.window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc func showSettings(_ sender: Any?) {
        navigate(to: .settings)
    }

    @objc func newTransaction(_ sender: Any?) {
        showMainWindow(nil)
        store.present(.transaction(id: nil))
    }

    @objc func newAccount(_ sender: Any?) {
        showMainWindow(nil)
        store.present(.account(id: nil))
    }

    @objc func navigateFromMenu(_ sender: NSMenuItem) {
        guard AppRoute.allCases.indices.contains(sender.tag) else { return }
        navigate(to: AppRoute.allCases[sender.tag])
    }

    func navigate(to route: AppRoute) {
        showMainWindow(nil)
        store.route = route
    }

    @objc func syncNow(_ sender: Any?) {
        cloud?.syncNow()
        store.reload()
        store.refreshBridgeStatus()
        store.processDueScheduled(force: true)
    }

    @objc func exportBackup(_ sender: Any?) {
        showMainWindow(nil)
        store.perform({ engine in try engine.exportBackup() }, completion: { [weak self] content in
            guard let self = self else { return }
            let panel = NSSavePanel()
            panel.title = "Exporter les données"
            panel.nameFieldStringValue = backupFileName(today: self.store.today)
            panel.allowedFileTypes = ["dmx"]
            panel.canCreateDirectories = true
            let handler: (NSApplication.ModalResponse) -> Void = { response in
                guard response == .OK, let url = panel.url else { return }
                do {
                    try content.write(to: url, atomically: true, encoding: .utf8)
                    self.store.showToast("Vos données ont été exportées avec succès.")
                } catch {
                    self.store.errorMessage = "Impossible de créer la sauvegarde. (\(error.localizedDescription))"
                }
            }
            if let window = self.windowController?.window {
                panel.beginSheetModal(for: window, completionHandler: handler)
            } else {
                handler(panel.runModal())
            }
        })
    }

    @objc func importFile(_ sender: Any?) {
        showMainWindow(nil)
        let panel = NSOpenPanel()
        panel.title = "Importer ou restaurer"
        panel.message = "Sauvegarde DmxMoney (.dmx) ou relevé bancaire (CSV, QIF, OFX)"
        panel.allowedFileTypes = ["dmx", "json", "csv", "qif", "ofx", "txt"]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        let handler: (NSApplication.ModalResponse) -> Void = { [weak self] response in
            guard response == .OK, let url = panel.url else { return }
            self?.openImportFile(url)
        }
        if let window = windowController?.window {
            panel.beginSheetModal(for: window, completionHandler: handler)
        } else {
            handler(panel.runModal())
        }
    }

    @discardableResult
    func openImportFile(_ url: URL) -> Bool {
        guard let content = FileText.read(url) else {
            store.errorMessage = "Le fichier n'a pas pu être lu."
            return false
        }
        let name = url.lastPathComponent
        switch url.pathExtension.lowercased() {
        case "dmx", "json":
            store.present(.restoreBackup(content: content, fileName: name))
        default:
            store.present(.statementImport(content: content, fileName: name))
        }
        return true
    }

    var settingsActions: SettingsActions {
        SettingsActions(
            exportBackup: { [weak self] in self?.exportBackup(nil) },
            importFile: { [weak self] in self?.importFile(nil) },
            copyToClipboard: { text in
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(text, forType: .string)
            },
            iCloud: cloud?.settings
        )
    }
}
