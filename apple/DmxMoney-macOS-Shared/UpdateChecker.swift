import AppKit
import DmxKit

/// Recherche de mise à jour pour la distribution directe (hors App Store).
///
/// Le flux est un petit JSON publié à côté des DMG (`DmxUpdateFeedURL`, réglé par
/// `Config/Signing.local.xcconfig`). Il donne une entrée par architecture, comme le
/// `latest.json` de DmxMoney 1.x : un Mac Intel sous Catalina ne doit jamais recevoir le
/// DMG Apple Silicon.
///
/// ```json
/// {
///   "version": "2.0.1",
///   "notes": "…",
///   "platforms": {
///     "darwin-arm64":  { "url": "https://…-apple-silicon.dmg",  "minimumSystemVersion": "11.0" },
///     "darwin-x86_64": { "url": "https://…-intel-catalina.dmg", "minimumSystemVersion": "10.15" }
///   }
/// }
/// ```
///
/// Aucune dépendance externe : Sparkle 2 n'est distribué qu'en binaire macOS 11+, ce qui
/// empêcherait l'application de se lancer sous Catalina. On se limite donc à prévenir et à
/// ouvrir la page de téléchargement ; l'installation reste un glisser-déposer.
final class UpdateChecker: NSObject {
    static let shared = UpdateChecker()

    /// Clé de plateforme de ce build.
    #if arch(arm64)
    static let platform = "darwin-arm64"
    #else
    static let platform = "darwin-x86_64"
    #endif

    private struct Feed: Decodable {
        struct Build: Decodable {
            let url: URL
            var minimumSystemVersion: String?
        }

        let version: String
        var notes: String?
        var platforms: [String: Build]?
        /// Flux à une seule entrée (ancien format) : conservé pour compatibilité.
        var url: URL?
        var minimumSystemVersion: String?

        /// Build correspondant à cette architecture, ou `nil` si le flux n'en propose pas.
        var build: Build? {
            if let platforms = platforms {
                return platforms[UpdateChecker.platform]
            }
            return url.map { Build(url: $0, minimumSystemVersion: minimumSystemVersion) }
        }
    }

    private let feedURL: URL?
    private let lastCheckKey = "DmxLastUpdateCheck"
    private let skippedVersionKey = "DmxSkippedUpdateVersion"

    var isAvailable: Bool { feedURL != nil }

    override private init() {
        let configured = (Bundle.main.object(forInfoDictionaryKey: "DmxUpdateFeedURL") as? String ?? "")
            .trimmingCharacters(in: .whitespaces)
        feedURL = configured.isEmpty ? nil : URL(string: configured)
        super.init()
    }

    // MARK: - Points d'entrée

    /// Vérification silencieuse au lancement, au plus une fois par jour.
    func checkInBackgroundIfNeeded() {
        guard isAvailable else { return }
        let defaults = UserDefaults.standard
        let last = defaults.double(forKey: lastCheckKey)
        let now = Date().timeIntervalSince1970
        guard now - last > 24 * 3600 else { return }
        defaults.set(now, forKey: lastCheckKey)
        check(silent: true)
    }

    /// Recherche demandée par le menu : indique aussi que l'app est à jour.
    @objc func checkForUpdates(_ sender: Any?) {
        UserDefaults.standard.removeObject(forKey: skippedVersionKey)
        check(silent: false)
    }

    // MARK: - Vérification

    private func check(silent: Bool) {
        guard let feedURL = feedURL else { return }
        var request = URLRequest(url: feedURL)
        request.cachePolicy = .reloadIgnoringLocalCacheData
        request.timeoutInterval = 15
        URLSession.shared.dataTask(with: request) { [weak self] data, _, error in
            guard let self = self else { return }
            DispatchQueue.main.async {
                guard let data = data, let feed = try? JSONDecoder().decode(Feed.self, from: data) else {
                    if !silent {
                        self.presentFailure(error)
                    }
                    return
                }
                self.handle(feed, silent: silent)
            }
        }
        .resume()
    }

    private func handle(_ feed: Feed, silent: Bool) {
        let current = AppInfo.version
        guard AppInfo.isVersion(feed.version, newerThan: current) else {
            if !silent {
                presentUpToDate(current)
            }
            return
        }
        // Pas de build pour cette architecture : rien à proposer.
        guard let build = feed.build else { return }
        if silent, UserDefaults.standard.string(forKey: skippedVersionKey) == feed.version {
            return
        }
        if let minimum = build.minimumSystemVersion, AppInfo.isVersion(minimum, newerThan: AppInfo.systemVersion) {
            // La mise à jour demande un macOS plus récent : inutile de la proposer.
            return
        }
        presentUpdate(feed, build: build, current: current, silent: silent)
    }

    // MARK: - Alertes

    private func presentUpdate(_ feed: Feed, build: Feed.Build, current: String, silent: Bool) {
        let alert = NSAlert()
        alert.messageText = "DmxMoney \(feed.version) est disponible"
        var text = "Vous utilisez la version \(current)."
        if let notes = feed.notes, !notes.isEmpty {
            text += "\n\n\(notes)"
        }
        alert.informativeText = text
        alert.addButton(withTitle: "Télécharger")
        alert.addButton(withTitle: "Plus tard")
        if silent {
            alert.addButton(withTitle: "Ignorer cette version")
        }
        switch alert.runModal() {
        case .alertFirstButtonReturn:
            NSWorkspace.shared.open(build.url)
        case .alertThirdButtonReturn:
            UserDefaults.standard.set(feed.version, forKey: skippedVersionKey)
        default:
            break
        }
    }

    private func presentUpToDate(_ current: String) {
        let alert = NSAlert()
        alert.messageText = "DmxMoney est à jour"
        alert.informativeText = "La version \(current) est la plus récente."
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    private func presentFailure(_ error: Error?) {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Recherche de mise à jour impossible"
        alert.informativeText = error?.localizedDescription ?? "Le flux de mise à jour est injoignable."
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}
