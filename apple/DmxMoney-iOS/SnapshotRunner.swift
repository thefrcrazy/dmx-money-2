import DmxKit
import SwiftUI
import UIKit

/// Mode capture (`DMXMONEY_SNAPSHOT_DIR`) : rend chaque page et quelques formulaires en PNG,
/// puis quitte. Pendant du `SnapshotRunner` de macOS et du mode capture de l'app GTK : vérifier
/// l'interface sans piloter l'écran.
enum SnapshotRunner {
    typealias Step = (name: String, action: () -> Void)

    static var directory: URL? {
        guard let path = ProcessInfo.processInfo.environment["DMXMONEY_SNAPSHOT_DIR"], !path.isEmpty else { return nil }
        return URL(fileURLWithPath: path, isDirectory: true)
    }

    static func run(store: AppStore, directory: URL) {
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var steps: [Step] = AppRoute.allCases.map { route in
            (route.rawValue, { store.route = route })
        }
        steps.append(("form-transaction", { store.present(.transaction(id: nil)) }))
        steps.append(("form-scheduled", { store.present(.scheduled(id: nil)) }))
        steps.append(("form-category", { store.present(.category(id: nil)) }))
        steps.append(("form-whats-new", { store.present(.whatsNew) }))
        steps.append(("dark-dashboard", {
            store.apply(.setTheme(theme: .dark))
            store.route = .dashboard
        }))
        perform(steps[...], store: store, directory: directory)
    }

    private static func perform(_ steps: ArraySlice<Step>, store: AppStore, directory: URL) {
        guard let step = steps.first else {
            store.apply(.setTheme(theme: .system))
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { exit(0) }
            return
        }
        step.action()
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
            capture(to: directory.appendingPathComponent("\(step.name).png"))
            if store.form != nil {
                store.form = nil
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.7) {
                perform(steps.dropFirst(), store: store, directory: directory)
            }
        }
    }

    /// Rend la fenêtre clé, feuilles présentées comprises.
    private static func capture(to url: URL) {
        let window = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first { $0.isKeyWindow }
        guard let window = window else { return }
        let renderer = UIGraphicsImageRenderer(bounds: window.bounds)
        let image = renderer.image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        try? image.pngData()?.write(to: url)
    }
}
