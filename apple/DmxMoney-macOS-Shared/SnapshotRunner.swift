import AppKit
import DmxKit

/// Mode capture (`DMXMONEY_SNAPSHOT_DIR`) : rend chaque page et quelques formulaires en PNG
/// dans une fenêtre invisible, puis quitte. Sert à vérifier l'interface sans piloter l'écran.
enum SnapshotRunner {
    typealias Step = (name: String, action: () -> Void)

    static var directory: URL? {
        guard let path = ProcessInfo.processInfo.environment["DMXMONEY_SNAPSHOT_DIR"], !path.isEmpty else { return nil }
        return URL(fileURLWithPath: path, isDirectory: true)
    }

    static func run(store: AppStore, window: NSWindow, directory: URL) {
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var steps: [Step] = [("light-initial", {
            store.apply(.setTheme(theme: .light))
            NSApp.appearance = NSAppearance(named: .aqua)
        })]
        steps += AppRoute.allCases.map { route in
            (route.rawValue, { store.route = route })
        }
        if ProcessInfo.processInfo.environment["DMXMONEY_VERIFY_SETTINGS"] == "1" {
            steps.append(("settings-general", { store.route = .settings }))
            for (index, name) in [(1, "companion"), (2, "data"), (3, "about")] {
                steps.append(("settings-\(name)", { selectSettingsTab(index, in: window) }))
            }
        }
        steps.append(("form-transaction", { store.present(.transaction(id: nil)) }))
        steps.append(("form-scheduled", { store.present(.scheduled(id: nil)) }))
        steps.append(("form-category", { store.present(.category(id: nil)) }))
        steps.append(("form-budget", { store.present(.budget(id: nil)) }))
        steps.append(("form-whats-new", { store.present(.whatsNew) }))
        steps.append(("form-budget-suggestions", { store.present(.budgetSuggestions) }))
        steps.append(("form-scheduled-suggestions", { store.present(.scheduledSuggestions) }))
        steps.append(("form-statement-import", {
            store.present(.statementImport(content: sampleCsv, fileName: "releve-demo.csv"))
        }))
        steps.append(("form-restore-backup", {
            guard let content = store.peek({ engine in try engine.exportBackup() }) else { return }
            store.present(.restoreBackup(content: content, fileName: "sauvegarde-demo.dmx"))
        }))
        steps.append(("dark-dashboard", {
            store.apply(.setTheme(theme: .dark))
            NSApp.appearance = NSAppearance(named: .darkAqua)
            store.route = .dashboard
        }))
        steps.append(("light-after-dark-dashboard", {
            store.apply(.setTheme(theme: .light))
            NSApp.appearance = NSAppearance(named: .aqua)
            store.route = .dashboard
        }))
        steps.append(("light-after-dark-categories", { store.route = .categories }))
        steps.append(("light-after-dark-settings", { store.route = .settings }))
        perform(steps[...], store: store, window: window, directory: directory)
    }

    private static func selectSettingsTab(_ index: Int, in window: NSWindow) {
        func find(_ view: NSView) -> NSSegmentedControl? {
            if let control = view as? NSSegmentedControl,
               control.segmentCount == 4, control.label(forSegment: 0) == "Général" {
                return control
            }
            for child in view.subviews {
                if let control = find(child) { return control }
            }
            return nil
        }
        guard let root = window.contentView, let control = find(root) else {
            fputs("Settings verification failed: four-section navigation missing.\n", stderr)
            exit(1)
        }
        control.selectedSegment = index
        control.sendAction(control.action, to: control.target)
    }

    private static func perform(_ steps: ArraySlice<Step>, store: AppStore, window: NSWindow, directory: URL) {
        guard let step = steps.first else {
            NSApp.terminate(nil)
            return
        }
        step.action()
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            capture(window, to: directory.appendingPathComponent("\(step.name).png"))
            // Seconde image avec la barre d'outils et la barre latérale : elles vivent dans la
            // vue de cadre, hors du contenu.
            captureChrome(window, to: directory.appendingPathComponent("\(step.name)-chrome.png"))
            if let sheet = window.attachedSheet {
                capture(sheet, to: directory.appendingPathComponent("\(step.name)-sheet.png"))
            }
            if store.form != nil {
                store.form = nil
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) {
                perform(steps.dropFirst(), store: store, window: window, directory: directory)
            }
        }
    }

    /// Relevé factice, pour vérifier l'assistant d'import sans fichier sur le disque.
    private static let sampleCsv = """
    Date;Montant;Catégorie;Libellé
    2026-09-01;-42,50;Alimentation;Supermarché
    2026-09-03;1800,00;Salaire;Virement employeur
    2026-09-05;-12,99;Abonnements;Musique
    """

    private static func captureChrome(_ window: NSWindow, to url: URL) {
        guard let frame = window.contentView?.superview else { return }
        render(frame, to: url)
    }

    private static func capture(_ window: NSWindow, to url: URL) {
        guard let view = window.contentView else { return }
        render(view, to: url)
    }

    private static func render(_ view: NSView, to url: URL) {
        view.layoutSubtreeIfNeeded()
        let bounds = view.bounds
        guard let rep = view.bitmapImageRepForCachingDisplay(in: bounds) else { return }
        view.cacheDisplay(in: bounds, to: rep)
        try? rep.representation(using: .png, properties: [:])?.write(to: url)
    }
}
