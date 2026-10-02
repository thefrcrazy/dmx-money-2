import DmxKit
import Foundation

/// Connexion sortante au compagnon Internet, commune aux deux variantes macOS.
enum BridgeLauncher {
    static func start(store: AppStore) {
        guard store.bridgeAvailable else { return }
        store.perform({ engine in try engine.startBridge(assetsDir: nil) }, completion: { _ in
            store.refreshBridgeStatus()
        }, failure: { message in
            NSLog("DmxMoney : compagnon Internet indisponible (%@)", message)
        })
    }
}
