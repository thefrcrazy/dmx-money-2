import DmxKit
import SwiftUI

@main
struct DmxMoneyApp: App {
    @StateObject private var launcher = Launcher()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            Group {
                if let store = launcher.store {
                    RootView(store: store, cloud: launcher.cloud)
                } else {
                    LaunchErrorView(message: launcher.error)
                }
            }
            .task { launcher.startPostLaunchTasks() }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active {
                launcher.store?.processDueScheduled()
            }
        }
    }
}

@MainActor
final class Launcher: ObservableObject {
    @Published private(set) var store: AppStore?
    @Published private(set) var error: String?
    private(set) var cloud: CloudSyncController?

    private var didStartPostLaunch = false

    init() {
        do {
            let store = try AppStore.openDefault()
            self.store = store
            cloud = CloudSyncController(store: store)
        } catch {
            self.error = AppStore.message(for: error)
        }
    }

    /// Présentations d'après-lancement, déclenchées par la vue racine.
    ///
    /// Publier un changement depuis `init()` reviendrait à modifier l'état pendant la
    /// construction des scènes SwiftUI, ce qui n'est pas permis.
    func startPostLaunchTasks() {
        guard let store, !didStartPostLaunch else { return }
        didStartPostLaunch = true
        // En mode capture, la modale « Nouveautés » est une étape parmi d'autres.
        guard SnapshotRunner.directory == nil else { return }
        store.presentWhatsNewIfNeeded()
    }
}

private struct LaunchErrorView: View {
    let message: String?

    var body: some View {
        VStack(spacing: 14) {
            DmxIcon("AlertTriangle", size: 40).foregroundColor(DmxColors.expense)
            Text("Impossible d'ouvrir la base DmxMoney").font(.headline)
            if let message {
                Text(message).font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
        }
        .padding(32)
    }
}
