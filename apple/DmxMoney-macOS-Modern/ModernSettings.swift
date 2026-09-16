import DmxKit
import SwiftUI

/// Paramètres dans la fenêtre, comme en 1.x : quatre onglets natifs plutôt qu'une fenêtre
/// séparée, pour que ⌘, et la barre latérale mènent au même endroit.
struct ModernSettings: View {
    @ObservedObject var store: AppStore
    let actions: SettingsActions
    @State private var tab: Tab = .general

    enum Tab: String, CaseIterable, Identifiable {
        case general, companion, data, about

        var id: String { rawValue }

        var label: String {
            switch self {
            case .general: return "Général"
            case .companion: return "Compagnon mobile"
            case .data: return "Données"
            case .about: return "À propos"
            }
        }

        var symbol: String {
            switch self {
            case .general: return "paintbrush"
            case .companion: return "iphone.and.arrow.forward"
            case .data: return "externaldrive"
            case .about: return "info.circle"
            }
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Picker("Section", selection: $tab) {
                    ForEach(Tab.allCases) { tab in
                        Label(tab.label, systemImage: tab.symbol).tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            Divider()
            switch tab {
            case .general: column { GeneralSettings(store: store, actions: actions) }
            case .companion: ModernCompanion()
            case .data: column { DataSettings(store: store, actions: actions) }
            case .about: column { AboutSettings(store: store, actions: actions) }
            }
        }
    }

    /// Colonne alignée à gauche : un `Form` groupé se centre tout seul au milieu de la fenêtre,
    /// ce qui laisse les réglages perdus au centre d'un grand écran.
    private func column<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        HStack(alignment: .top, spacing: 0) {
            content().frame(maxWidth: 620)
            Spacer(minLength: 0)
        }
    }
}

private struct GeneralSettings: View {
    @ObservedObject var store: AppStore
    let actions: SettingsActions

    var body: some View {
        Form {
            Section("Apparence") {
                Picker(selection: themeBinding) {
                    Text("Clair").tag(Theme.light)
                    Text("Sombre").tag(Theme.dark)
                    Text("Système").tag(Theme.system)
                } label: {
                    Label("Thème", systemImage: "circle.lefthalf.filled")
                }
                .pickerStyle(.segmented)

                LabeledContent {
                    HStack(spacing: 6) {
                        Button("Déf.") { store.apply(.setPrimaryColor(color: "default")) }
                            .buttonStyle(.bordered)
                        ForEach(accentColors(), id: \.self) { hex in
                            Button {
                                store.apply(.setPrimaryColor(color: hex))
                            } label: {
                                Circle()
                                    .fill(Color(hex: hex, fallback: .accentColor))
                                    .frame(width: 18, height: 18)
                                    .overlay {
                                        if store.settings.accentColor?.caseInsensitiveCompare(hex) == .orderedSame {
                                            Image(systemName: "checkmark")
                                                .font(.system(size: 9, weight: .bold))
                                                .foregroundStyle(.white)
                                        }
                                    }
                            }
                            .buttonStyle(.plain)
                            .help(hex)
                        }
                    }
                } label: {
                    Label("Couleur d'accentuation", systemImage: "paintpalette")
                }
            }
            if let iCloud = actions.iCloud {
                Section("Synchronisation") {
                    Toggle(isOn: Binding(get: { iCloud.isEnabled() }, set: { iCloud.setEnabled($0) })) {
                        Label("Synchroniser avec iCloud", systemImage: "icloud")
                    }
                    .disabled(!iCloud.isAvailable())
                    LabeledContent {
                        Text(iCloud.isAvailable() ? iCloud.status() : iCloud.unavailableReason())
                    } label: {
                        Label("État", systemImage: "arrow.triangle.2.circlepath")
                    }
                    if iCloud.isAvailable() {
                        Button {
                            iCloud.syncNow()
                        } label: {
                            Label("Synchroniser maintenant", systemImage: "arrow.clockwise")
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
    }

    private var themeBinding: Binding<Theme> {
        Binding(get: { store.settings.theme }, set: { store.apply(.setTheme(theme: $0)) })
    }
}

private struct DataSettings: View {
    @ObservedObject var store: AppStore
    let actions: SettingsActions

    var body: some View {
        Form {
            Section("Sauvegardes") {
                LabeledContent {
                    Button("Exporter…") { actions.exportBackup() }
                } label: {
                    Label("Exporter les données", systemImage: "square.and.arrow.up")
                }
                LabeledContent {
                    Button("Choisir un fichier…") { actions.importFile() }
                } label: {
                    Label("Importer ou restaurer", systemImage: "square.and.arrow.down")
                }
            }
            Section {
                LabeledContent {
                    Text((try? AppStore.dataDirectory().path) ?? "—")
                        .textSelection(.enabled)
                        .font(.caption)
                } label: {
                    Label("Emplacement", systemImage: "folder")
                }
            } header: {
                Text("Base locale")
            } footer: {
                Text("Sauvegardes .dmx compatibles DmxMoney 1.x ; imports CSV, QIF et OFX.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
}

private struct AboutSettings: View {
    @ObservedObject var store: AppStore
    let actions: SettingsActions
    @AppStorage("DmxIncludePrereleases") private var includePrereleases = true

    var body: some View {
        Form {
            Section {
                LabeledContent {
                    Text("\(AppInfo.version) (\(AppInfo.build))")
                } label: {
                    Label("Version", systemImage: "number")
                }
                LabeledContent {
                    Text(coreVersion())
                } label: {
                    Label("Noyau", systemImage: "cpu")
                }
                if let check = actions.checkForUpdates {
                    Button {
                        check()
                    } label: {
                        Label("Vérifier les mises à jour…", systemImage: "arrow.down.circle")
                    }
                    Toggle(isOn: $includePrereleases) {
                        Label("Autoriser les versions pré-release", systemImage: "flask")
                    }
                }
                Button {
                    store.present(.whatsNew)
                } label: {
                    Label("Nouveautés", systemImage: "sparkles")
                }
            }
            Section {
                LabeledContent {
                    Text(AssistantRewriter.isAvailable ? "Modèle sur l'appareil" : "Analyse du noyau")
                } label: {
                    Label("Assistant", systemImage: "text.bubble")
                }
            } footer: {
                Text("Siri et le compagnon mobile passent par les intentions de DmxMoney. Les montants viennent toujours du noyau ; le modèle sur l'appareil ne sert qu'à reformuler la demande, et l'assistant fonctionne sans lui.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
}
