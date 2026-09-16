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
///     "darwin-arm64":  { "url": "https://…-apple-silicon.dmg",  "minimumSystemVersion": "26.0" },
///     "darwin-x86_64": { "url": "https://…-intel-catalina.dmg", "minimumSystemVersion": "10.15" }
///   }
/// }
/// ```
///
/// Aucune dépendance externe : Sparkle 2 n'est distribué qu'en binaire macOS 11+, ce qui
/// empêcherait l'application de se lancer sous Catalina. Le téléchargement utilise URLSession
/// et le remplacement conserve une copie de secours jusqu’à la fin de l’installation.
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

    private var periodicTimer: Timer?
    private var isDownloading = false

    /// Vérification silencieuse au lancement, puis toutes les 24 heures en tâche de fond.
    func checkInBackgroundIfNeeded() {
        guard isAvailable else { return }

        if periodicTimer == nil {
            // Vérification périodique toutes les 6 heures si l'application reste ouverte
            periodicTimer = Timer.scheduledTimer(withTimeInterval: 6 * 3600, repeats: true) { [weak self] _ in
                self?.checkInBackgroundIfNeeded()
            }
        }

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
        guard !isDownloading else { return }
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
        progress.style = .bar
        progress.isIndeterminate = false
        progress.minValue = 0
        progress.maxValue = 1
        progress.doubleValue = 0
        alert.accessoryView = progress
        alert.addButton(withTitle: "Annuler")

        // A sheet keeps the main queue running throughout the download.
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow else {
            NSWorkspace.shared.open(build.url)
            return
        }
        guard !isDownloading else { return }
        isDownloading = true
        let targetDMG = FileManager.default.temporaryDirectory
            .appendingPathComponent("DmxMoney-\(UUID().uuidString).dmg")
        var finished = false
        var result: Result<URL, Error>?
        let configuration = URLSessionConfiguration.default
        configuration.timeoutIntervalForRequest = 60
        configuration.timeoutIntervalForResource = 600
        let session = URLSession(configuration: configuration)
        let task = session.downloadTask(with: build.url) { tempURL, response, error in
            let downloaded: Result<URL, Error> = Result {
                if let error { throw error }
                guard let response = response as? HTTPURLResponse,
                      (200...299).contains(response.statusCode), let tempURL else {
                    throw URLError(.badServerResponse)
                }
                // URLSession deletes tempURL when this callback returns.
                try FileManager.default.moveItem(at: tempURL, to: targetDMG)
                return targetDMG
            }
            DispatchQueue.main.async {
                guard !finished else {
                    try? FileManager.default.removeItem(at: targetDMG)
                    return
                }
                result = downloaded
                window.endSheet(alert.window, returnCode: .alertSecondButtonReturn)
            }
        }
        let observation = task.progress.observe(\.fractionCompleted) { taskProgress, _ in
            DispatchQueue.main.async {
                guard !finished else { return }
                progress.isIndeterminate = taskProgress.totalUnitCount <= 0
                progress.doubleValue = taskProgress.fractionCompleted
            }
        }
        alert.beginSheetModal(for: window) { [weak self] response in
            finished = true
            self?.isDownloading = false
            observation.invalidate()
            if response == .alertFirstButtonReturn || result == nil {
                session.invalidateAndCancel()
                try? FileManager.default.removeItem(at: targetDMG)
                return
            }
            session.finishTasksAndInvalidate()
            switch result! {
            case .success(let url):
                self?.applyDmgAndRestart(dmgURL: url)
            case .failure(let error):
                let fail = NSAlert()
                fail.messageText = "Échec du téléchargement"
                fail.informativeText = error.localizedDescription
                fail.addButton(withTitle: "Télécharger manuellement")
                fail.addButton(withTitle: "Fermer")
                fail.beginSheetModal(for: window) { response in
                    if response == .alertFirstButtonReturn { NSWorkspace.shared.open(build.url) }
                }
            }
        }
        task.resume()
    }

    private func applyDmgAndRestart(dmgURL: URL) {
        let bundlePath = Bundle.main.bundlePath
        let pid = ProcessInfo.processInfo.processIdentifier

        // Pass paths as arguments: application paths can contain quotes or shell syntax.
        // Stage a complete replacement before moving the installed application aside.
        let script = """
        #!/bin/sh
        set -eu
        PID="$1"
        DMG="$2"
        APP="$3"
        WORK=""
        MOUNT_DIR=""
        BACKUP=""
        cleanup() {
            STATUS=$?
            if [ -n "$MOUNT_DIR" ]; then /usr/bin/hdiutil detach "$MOUNT_DIR" -quiet 2>/dev/null || true; fi
            if [ -d "$BACKUP" ] && [ ! -e "$APP" ]; then
                mv "$BACKUP" "$APP"
            fi
            if [ -n "$WORK" ]; then rm -rf "$WORK"; fi
            rm -f "$DMG" "$0"
            if [ "$STATUS" -ne 0 ]; then
                /usr/bin/open -n "$APP" || true
                /usr/bin/osascript -e 'display alert "DmxMoney" message "La mise à jour a échoué. La version précédente a été conservée. Relancez le téléchargement ou installez le DMG manuellement."' || true
            fi
        }
        trap cleanup EXIT
        WORK=$(/usr/bin/mktemp -d "${APP}.update.XXXXXX")
        MOUNT_DIR="$WORK/mount"
        BACKUP="$WORK/previous.app"
        mkdir "$MOUNT_DIR"
        /usr/bin/hdiutil attach "$DMG" -nobrowse -mountpoint "$MOUNT_DIR" -quiet
        SRC_APP="$MOUNT_DIR/DmxMoney.app"
        test -d "$SRC_APP"
        test "$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$SRC_APP/Contents/Info.plist")" = "com.dmxmoney.app"
        /usr/bin/codesign --verify --deep --strict "$SRC_APP"
        /usr/bin/ditto "$SRC_APP" "$WORK/replacement.app"
        while kill -0 "$PID" 2>/dev/null; do sleep 0.1; done
        mv "$APP" "$BACKUP"
        if ! mv "$WORK/replacement.app" "$APP"; then
            mv "$BACKUP" "$APP"
            exit 1
        fi
        /usr/bin/open -n "$APP"
        """

        let scriptURL = FileManager.default.temporaryDirectory.appendingPathComponent("dmx_restart_\(pid).sh")
        do {
            try script.write(to: scriptURL, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: scriptURL.path)

            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/bin/sh")
            proc.arguments = [scriptURL.path, String(pid), dmgURL.path, bundlePath]
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
