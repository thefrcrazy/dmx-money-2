import AppKit
import DmxKit
import UniformTypeIdentifiers

/// Export `.dmx` et import (sauvegarde ou relevé bancaire) par les panneaux macOS.
/// Partagé par les deux variantes de l'app.
enum FileActions {
    static func settingsActions(store: AppStore, cloud: CloudSyncController?) -> SettingsActions {
        SettingsActions(
            exportBackup: { exportBackup(store: store) },
            importFile: { importFile(store: store) },
            copyToClipboard: { text in
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(text, forType: .string)
            },
            checkForUpdates: UpdateChecker.shared.isAvailable ? { UpdateChecker.shared.checkForUpdates(nil) } : nil,
            updateAvailable: { UpdateChecker.shared.isAvailable },
            iCloud: cloud?.settings
        )
    }

    static func exportBackup(store: AppStore) {
        store.perform({ engine in try engine.exportBackup() }, completion: { content in
            let panel = NSSavePanel()
            panel.title = "Exporter les données"
            panel.nameFieldStringValue = backupFileName(today: store.today)
            // La variante legacy cible macOS 10.15, où `UTType` n'existe pas encore.
            if #available(macOS 11.0, *) {
                panel.allowedContentTypes = [UTType(exportedAs: "com.dmxmoney.backup")]
            } else {
                panel.allowedFileTypes = ["dmx"]
            }
            panel.canCreateDirectories = true
            present(panel) { response in
                guard response == .OK, let url = panel.url else { return }
                do {
                    try content.write(to: url, atomically: true, encoding: .utf8)
                    store.showToast("Vos données ont été exportées avec succès.")
                } catch {
                    store.errorMessage = "Impossible de créer la sauvegarde. (\(error.localizedDescription))"
                }
            }
        })
    }

    static func importFile(store: AppStore) {
        let panel = NSOpenPanel()
        panel.title = "Importer ou restaurer"
        panel.message = "Sauvegarde DmxMoney (.dmx) ou relevé bancaire (CSV, QIF, OFX)"
        if #available(macOS 11.0, *) {
            panel.allowedContentTypes = [UTType(exportedAs: "com.dmxmoney.backup"), .json, .commaSeparatedText, .plainText, .data]
        } else {
            panel.allowedFileTypes = ["dmx", "json", "csv", "qif", "ofx", "txt"]
        }
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        present(panel) { response in
            guard response == .OK, let url = panel.url else { return }
            open(url, store: store)
        }
    }

    @discardableResult
    static func open(_ url: URL, store: AppStore) -> Bool {
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

    /// Feuille attachée à la fenêtre principale quand elle existe.
    private static func present(_ panel: NSSavePanel, completion: @escaping (NSApplication.ModalResponse) -> Void) {
        if let window = NSApp.windows.first(where: { $0.isVisible && $0.canBecomeMain }) {
            panel.beginSheetModal(for: window, completionHandler: completion)
        } else {
            completion(panel.runModal())
        }
    }
}
