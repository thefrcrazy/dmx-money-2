import DmxKit
import SwiftUI

/// Variante « modern » : application SwiftUI native macOS 26, Apple Silicon.
/// La variante « legacy » (AppKit, Intel, macOS 10.15) vit dans `DmxMoney-macOS`.
@main
struct DmxMoneyModernApp: App {
    @StateObject private var launcher = Launcher.shared
    @NSApplicationDelegateAdaptor(ModernAppDelegate.self) private var delegate
    @Environment(\.openWindow) private var openWindow

    var body: some Scene {
        WindowGroup(id: "main") {
            Group {
                if let store = launcher.store {
                    ModernShell(store: store, actions: launcher.actions)
                } else {
                    LaunchFailureView(message: launcher.error)
                }
            }
            .frame(minWidth: 980, minHeight: 620)
            .task { launcher.startPostLaunchTasks() }
        }
        .defaultSize(width: 1340, height: 860)
        .windowToolbarStyle(.unified)
        .commands {
            DmxCommands(store: launcher.store, actions: launcher.actions)
        }
        .windowResizability(.contentMinSize)

        // Résumé des comptes dans la barre des menus, avec le logo officiel monochrome.
        MenuBarExtra {
            if let store = launcher.store {
                MenuBarSummary(store: store) { openWindow(id: "main") }
            }
        } label: {
            Image("MenuBarIcon")
                .renderingMode(.template)
        }
    }
}

/// Ouverture de la base et services de l'application.
@MainActor
final class Launcher: ObservableObject {
    static let shared = Launcher()

    @Published private(set) var store: AppStore?
    @Published private(set) var error: String?
    private(set) var cloud: CloudSyncController?
    private(set) var actions = SettingsActions(exportBackup: {}, importFile: {}, copyToClipboard: { _ in })

    private var didStartPostLaunch = false

    init() {
        do {
            let store = try AppStore.openDefault()
            self.store = store
            cloud = CloudSyncController(store: store)
            actions = FileActions.settingsActions(store: store, cloud: cloud)
            // Demandes de la PWA : le modèle sur l'appareil les normalise avant le noyau.
            store.assistantRewriter = AssistantRewriter.hook()
            BridgeLauncher.start(store: store)
        } catch {
            self.error = AppStore.message(for: error)
        }
    }

    /// Tâches lancées quand la fenêtre principale existe.
    ///
    /// Rien de tout cela ne doit partir d'`init()` : celui-ci s'exécute pendant la construction
    /// des scènes (`applicationWillFinishLaunching`), où ouvrir un panneau modal ou publier un
    /// changement relance le rendu SwiftUI au milieu d'une transaction — AttributeGraph échoue
    /// alors sur une précondition et l'application s'arrête net.
    func startPostLaunchTasks() {
        guard let store, !didStartPostLaunch else { return }
        didStartPostLaunch = true
        // Publie les raccourcis Siri auprès du système à chaque lancement : sans cet appel,
        // l'index des intentions peut rester vide pour une app installée hors de /Applications.
        DmxShortcuts.updateAppShortcutParameters()
        store.processDueScheduled()
        guard SnapshotRunner.directory == nil else { return }
        UpdateChecker.shared.checkInBackgroundIfNeeded()
        // La reprise d'une base 1.x est proposée par `ModernLegacyAdoption`, en alerte SwiftUI :
        // les nouveautés attendent la réponse pour ne pas empiler deux présentations.
        if store.engine.openReport().legacyCandidate == nil {
            store.presentWhatsNewIfNeeded()
        }
    }
}

private struct LaunchFailureView: View {
    let message: String?

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 38))
                .foregroundStyle(.red)
            Text("Impossible d'ouvrir la base DmxMoney").font(.headline)
            if let message {
                Text(message).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
        }
        .padding(40)
    }
}

/// Menus de l'application (l'équivalent SwiftUI de l'ancien `MainMenu` AppKit).
struct DmxCommands: Commands {
    let store: AppStore?
    let actions: SettingsActions

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Nouvelle transaction") { store?.present(.transaction(id: nil)) }
                .keyboardShortcut("n")
            Button("Nouveau compte") { store?.present(.account(id: nil)) }
                .keyboardShortcut("n", modifiers: [.command, .shift])
            Divider()
            Button("Importer ou restaurer…") { actions.importFile() }
                .keyboardShortcut("o")
            Button("Exporter les données…") { actions.exportBackup() }
                .keyboardShortcut("e", modifiers: [.command, .shift])
        }
        CommandGroup(after: .newItem) {
            Button("Synchroniser") {
                store?.reload()
                store?.refreshBridgeStatus()
            }
            .keyboardShortcut("r")
        }
        CommandGroup(replacing: .appSettings) {
            // Les réglages sont une page de la fenêtre, pas une fenêtre séparée.
            Button("Paramètres…") { store?.route = .settings }
                .keyboardShortcut(",")
        }
        CommandGroup(after: .appInfo) {
            if UpdateChecker.shared.isAvailable {
                Button("Rechercher les mises à jour…") { UpdateChecker.shared.checkForUpdates(nil) }
            }
            Button("Nouveautés") { store?.present(.whatsNew) }
        }
    }
}

/// Contenu de l'icône de la barre des menus : soldes, navigation, synchronisation.
struct MenuBarSummary: View {
    @ObservedObject var store: AppStore
    let openMain: () -> Void

    var body: some View {
        let summary = store.peek { try $0.traySummary() }
        let accounts = Array((summary?.accounts ?? []).prefix(8))
        Text("Total : \(Money.format(summary?.total ?? 0))")
        Divider()
        ForEach(accounts, id: \.accountId) { account in
            Button("\(account.name) — \(Money.format(account.balance))") {
                store.route = .accounts
                openMain()
            }
        }
        Divider()
        Button("Ouvrir DmxMoney") { openMain() }
        Button("Nouvelle transaction") {
            store.present(.transaction(id: nil))
            openMain()
        }
        Button("Synchroniser") {
            store.reload()
            store.refreshBridgeStatus()
        }
        Button("Rechercher les mises à jour…") { UpdateChecker.shared.checkForUpdates(nil) }
        Divider()
        Button("Quitter DmxMoney") { NSApp.terminate(nil) }
    }
}


/// Démarrage côté AppKit : activation de l'app et mode capture.
final class ModernAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.activate(ignoringOtherApps: true)
        guard let directory = SnapshotRunner.directory, let store = Launcher.shared.store else { return }
        // La fenêtre du `WindowGroup` apparaît après le lancement : on l'attend avant de capturer.
        // (à lancer via `open`, LaunchServices étant ce qui crée la fenêtre d'une app SwiftUI)
        waitForMainWindow { window in
            SnapshotRunner.run(store: store, window: window, directory: directory)
        }
    }

    private func waitForMainWindow(attempt: Int = 0, _ body: @escaping (NSWindow) -> Void) {
        let window = NSApp.windows.first {
            !($0 is NSPanel) && $0.contentView != nil && $0.frame.width > 600 && $0.isVisible
        }
        if let window {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { body(window) }
            return
        }
        guard attempt < 40 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { [weak self] in
            self?.waitForMainWindow(attempt: attempt + 1, body)
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        // La fenêtre se ferme sans quitter : le pont PWA et la barre des menus restent actifs.
        false
    }
}
