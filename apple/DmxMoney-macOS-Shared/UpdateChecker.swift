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

    private struct GitHubRelease: Decodable {
        struct Asset: Decodable {
            let name: String
            let browser_download_url: URL
        }
        let tag_name: String
        let name: String?
        let body: String?
        let draft: Bool?
        let prerelease: Bool?
        let assets: [Asset]
    }

    private static let defaultFeedURL = URL(string: "https://api.github.com/repos/thefrcrazy/dmx-money-2/releases")!
    private let feedURL: URL
    private let lastCheckKey = "DmxLastUpdateCheck"
    private let skippedVersionKey = "DmxSkippedUpdateVersion"
    private let includePrereleasesKey = "DmxIncludePrereleases"

    var isAvailable: Bool { true }

    var includePrereleases: Bool {
        get {
            if UserDefaults.standard.object(forKey: includePrereleasesKey) != nil {
                return UserDefaults.standard.bool(forKey: includePrereleasesKey)
            }
            return true
        }
        set {
            UserDefaults.standard.set(newValue, forKey: includePrereleasesKey)
        }
    }

    override private init() {
        let configured = (Bundle.main.object(forInfoDictionaryKey: "DmxUpdateFeedURL") as? String ?? "")
            .trimmingCharacters(in: .whitespaces)
        feedURL = configured.isEmpty ? Self.defaultFeedURL : (URL(string: configured) ?? Self.defaultFeedURL)
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
        var request = URLRequest(url: feedURL)
        request.cachePolicy = .reloadIgnoringLocalCacheData
        request.timeoutInterval = 15
        request.setValue("DmxMoney-macOS", forHTTPHeaderField: "User-Agent")
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")

        URLSession.shared.dataTask(with: request) { [weak self] data, _, error in
            guard let self = self else { return }
            DispatchQueue.main.async {
                guard let data = data else {
                    if !silent { self.presentFailure(error) }
                    return
                }

                if let feed = try? JSONDecoder().decode(Feed.self, from: data) {
                    self.handle(feed, silent: silent)
                    return
                }

                // Essai de décodage sous forme de releases GitHub
                if let releases = try? JSONDecoder().decode([GitHubRelease].self, from: data) {
                    let current = AppInfo.version
                    let isCurrentPrerelease = current.contains("-")
                    let allowPrerelease = self.includePrereleases || isCurrentPrerelease

                    // Trouver la release candidate la plus récente admissible
                    let matching = releases.first { rel in
                        guard !(rel.draft ?? false) else { return false }
                        if !allowPrerelease {
                            return !(rel.prerelease ?? false)
                        }
                        return true
                    }

                    if let latest = matching {
                        let version = latest.tag_name.trimmingCharacters(in: CharacterSet(charactersIn: "vV "))
                        var platforms: [String: Feed.Build] = [:]
                        for asset in latest.assets {
                            if asset.name.hasSuffix("apple-silicon.dmg") {
                                platforms["darwin-arm64"] = Feed.Build(url: asset.browser_download_url, minimumSystemVersion: "26.0")
                            } else if asset.name.hasSuffix("intel-catalina.dmg") {
                                platforms["darwin-x86_64"] = Feed.Build(url: asset.browser_download_url, minimumSystemVersion: "10.15")
                            }
                        }
                        let feed = Feed(
                            version: version,
                            notes: latest.body,
                            platforms: platforms,
                            url: nil,
                            minimumSystemVersion: nil
                        )
                        self.handle(feed, silent: silent)
                        return
                    }
                }

                if !silent {
                    self.presentFailure(error)
                }
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

    // MARK: - Alertes et installation

    private func presentUpdate(_ feed: Feed, build: Feed.Build, current: String, silent: Bool) {
        let alert = NSAlert()
        alert.messageText = "DmxMoney \(feed.version) est disponible"
        var text = "Vous utilisez actuellement la version \(current)."
        if let notes = feed.notes, !notes.isEmpty {
            text += "\n\nNouveautés :\n\(notes)"
        }
        alert.informativeText = text

        let isWritable = FileManager.default.isWritableFile(atPath: Bundle.main.bundlePath)
        if isWritable {
            alert.addButton(withTitle: "Mettre à jour et redémarrer")
            alert.addButton(withTitle: "Télécharger manuellement")
            alert.addButton(withTitle: "Plus tard")
        } else {
            alert.addButton(withTitle: "Télécharger")
            alert.addButton(withTitle: "Plus tard")
        }

        if silent {
            alert.addButton(withTitle: "Ignorer cette version")
        }

        let response = alert.runModal()
        if isWritable {
            switch response {
            case .alertFirstButtonReturn:
                performDownloadAndRestart(build: build, version: feed.version)
            case .alertSecondButtonReturn:
                NSWorkspace.shared.open(build.url)
            case .alertThirdButtonReturn:
                break
            default:
                if silent {
                    UserDefaults.standard.set(feed.version, forKey: skippedVersionKey)
                }
            }
        } else {
            switch response {
            case .alertFirstButtonReturn:
                NSWorkspace.shared.open(build.url)
            case .alertSecondButtonReturn:
                break
            default:
                if silent {
                    UserDefaults.standard.set(feed.version, forKey: skippedVersionKey)
                }
            }
        }
    }

    private func performDownloadAndRestart(build: Feed.Build, version: String) {
        let alert = NSAlert()
        alert.messageText = "Téléchargement de DmxMoney \(version)"
        alert.informativeText = "La mise à jour est en cours de téléchargement… L'application redémarrera automatiquement une fois prête."
        let progress = NSProgressIndicator(frame: NSRect(x: 0, y: 0, width: 300, height: 20))
        progress.isIndeterminate = true
        progress.startAnimation(nil)
        alert.accessoryView = progress
        alert.addButton(withTitle: "Annuler")

        var isCancelled = false
        var downloadTask: URLSessionDownloadTask?
        downloadTask = URLSession.shared.downloadTask(with: build.url) { [weak self] tempURL, _, error in
            DispatchQueue.main.async {
                guard !isCancelled else { return }
                NSApp.stopModal(withCode: .alertSecondButtonReturn)

                guard let self = self, let tempURL = tempURL, error == nil else {
                    let fail = NSAlert()
                    fail.messageText = "Échec du téléchargement"
                    fail.informativeText = error?.localizedDescription ?? "Impossible de télécharger la mise à jour."
                    fail.addButton(withTitle: "Télécharger manuellement")
                    fail.addButton(withTitle: "Fermer")
                    if fail.runModal() == .alertFirstButtonReturn {
                        NSWorkspace.shared.open(build.url)
                    }
                    return
                }

                let targetDMG = FileManager.default.temporaryDirectory
                    .appendingPathComponent("DmxMoney-\(version)-\(UUID().uuidString).dmg")
                try? FileManager.default.moveItem(at: tempURL, to: targetDMG)
                self.applyDmgAndRestart(dmgURL: targetDMG)
            }
        }
        downloadTask?.resume()

        if alert.runModal() == .alertFirstButtonReturn {
            isCancelled = true
            downloadTask?.cancel()
        }
    }

    private func applyDmgAndRestart(dmgURL: URL) {
        let bundlePath = Bundle.main.bundlePath
        let pid = ProcessInfo.processInfo.processIdentifier

        let script = """
        #!/bin/sh
        set -e
        while kill -0 \(pid) 2>/dev/null; do
            sleep 0.1
        done

        MOUNT_DIR=$(mktemp -d /tmp/dmx_mount_XXXXXX)
        if /usr/bin/hdiutil attach "\(dmgURL.path)" -nobrowse -noverify -mountpoint "$MOUNT_DIR" -quiet; then
            SRC_APP=$(find "$MOUNT_DIR" -maxdepth 1 -name "DmxMoney*.app" -o -name "*.app" | head -n 1)
            if [ -n "$SRC_APP" ] && [ -d "$SRC_APP" ]; then
                rm -rf "\(bundlePath)"
                cp -pR "$SRC_APP" "\(bundlePath)"
                xattr -dr com.apple.quarantine "\(bundlePath)" 2>/dev/null || true
            fi
            /usr/bin/hdiutil detach "$MOUNT_DIR" -quiet -force 2>/dev/null || true
            rm -rf "$MOUNT_DIR"
        fi

        rm -f "\(dmgURL.path)"
        /usr/bin/open -n "\(bundlePath)"
        """

        let scriptURL = FileManager.default.temporaryDirectory.appendingPathComponent("dmx_restart_\(pid).sh")
        do {
            try script.write(to: scriptURL, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: scriptURL.path)

            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/bin/sh")
            proc.arguments = [scriptURL.path]
            try proc.run()

            NSApp.terminate(nil)
        } catch {
            let failAlert = NSAlert()
            failAlert.messageText = "Erreur lors de l'application de la mise à jour"
            failAlert.informativeText = error.localizedDescription
            failAlert.addButton(withTitle: "OK")
            failAlert.runModal()
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
