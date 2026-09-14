import DmxKit
import Foundation

/// Démarrage du pont compagnon mobile, commun aux deux variantes macOS.
///
/// L'app embarque le client PWA : le pont le sert lui-même, et le QR d'appairage pointe vers
/// le pont local plutôt que vers la PWA publique déployée sur le Worker.
enum BridgeLauncher {
    /// Dossier du client PWA dans le bundle. XcodeGen embarque le dossier sous son nom
    /// d'origine (`dist`) : les deux noms sont acceptés.
    static var assetsPath: String? {
        guard let resources = Bundle.main.resourceURL else { return nil }
        for name in ["pwa", "dist"] {
            let directory = resources.appendingPathComponent(name, isDirectory: true)
            if FileManager.default.fileExists(atPath: directory.appendingPathComponent("index.html").path) {
                return directory.path
            }
        }
        return nil
    }

    static func start(store: AppStore) {
        guard store.bridgeAvailable else { return }
        let assets = assetsPath
        store.perform({ engine in try engine.startBridge(assetsDir: assets) }, completion: { _ in
            store.refreshBridgeStatus()
        }, failure: { message in
            NSLog("DmxMoney : pont PWA indisponible (%@)", message)
        })
    }
}
