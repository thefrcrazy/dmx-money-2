import CoreImage
import SwiftUI

#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Fonctions propres à la plateforme, fournies par l'application hôte.
public struct SettingsActions {
    public var exportBackup: () -> Void
    public var importFile: () -> Void
    public var copyToClipboard: (String) -> Void
    /// Action du téléchargeur macOS : `nil` masque la ligne.
    public var checkForUpdates: (() -> Void)?
    public var updateAvailable: () -> Bool
    public var iCloud: ICloudSettings?

    public init(
        exportBackup: @escaping () -> Void,
        importFile: @escaping () -> Void,
        copyToClipboard: @escaping (String) -> Void,
        checkForUpdates: (() -> Void)? = nil,
        updateAvailable: @escaping () -> Bool = { false },
        iCloud: ICloudSettings? = nil
    ) {
        self.exportBackup = exportBackup
        self.importFile = importFile
        self.copyToClipboard = copyToClipboard
        self.checkForUpdates = checkForUpdates
        self.updateAvailable = updateAvailable
        self.iCloud = iCloud
    }
}

/// Synchronisation iCloud (macOS 14+ / iOS 17+, avec les droits CloudKit).
public struct ICloudSettings {
    public var isAvailable: () -> Bool
    public var unavailableReason: () -> String
    public var isEnabled: () -> Bool
    public var setEnabled: (Bool) -> Void
    public var status: () -> String
    public var syncNow: () -> Void

    public init(
        isAvailable: @escaping () -> Bool,
        unavailableReason: @escaping () -> String,
        isEnabled: @escaping () -> Bool,
        setEnabled: @escaping (Bool) -> Void,
        status: @escaping () -> String,
        syncNow: @escaping () -> Void
    ) {
        self.isAvailable = isAvailable
        self.unavailableReason = unavailableReason
        self.isEnabled = isEnabled
        self.setEnabled = setEnabled
        self.status = status
        self.syncNow = syncNow
    }
}

/// Paramètres : apparence, synchronisation (iCloud et pont PWA), données, à propos.
public struct SettingsPage: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact
    private let actions: SettingsActions
    @State private var selectedTab: SettingsTab = .general
    @State private var bridgeBusy = false
    @State private var refreshTick = 0
    @State private var includePrereleases: Bool = (UserDefaults.standard.object(forKey: "DmxIncludePrereleases") as? Bool) ?? true

    public init(actions: SettingsActions) {
        self.actions = actions
    }

    private enum SettingsTab: String, CaseIterable, Identifiable {
        case general, companion, data, about
        var id: String { rawValue }
        var title: String {
            switch self {
            case .general: return "Général"
            case .companion: return "Compagnon mobile"
            case .data: return "Données"
            case .about: return "À propos"
            }
        }
    }

    public var body: some View {
        VStack(spacing: 0) {
            if !compact {
                HStack {
                    Picker("Section", selection: $selectedTab) {
                        ForEach(SettingsTab.allCases) { tab in
                            Text(tab.title).tag(tab)
                        }
                    }
                    .pickerStyle(SegmentedPickerStyle())
                    .labelsHidden()
                    .frame(maxWidth: 580)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                Divider()
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if compact || selectedTab == .general {
                        appearanceSection
                        if let iCloud = actions.iCloud { iCloudSection(iCloud) }
                    }
                    if compact || selectedTab == .companion {
                        if store.bridgeAvailable {
                            bridgeSection
                        } else if !compact {
                            Text("Le compagnon mobile n’est pas disponible sur cette plateforme.")
                                .foregroundColor(.secondary)
                        }
                    }
                    if compact || selectedTab == .data { dataSection }
                    if compact || selectedTab == .about { aboutSection }
                }
                .padding(compact ? 16 : 24)
                .frame(maxWidth: 760, alignment: .topLeading)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .background(DmxPalette.pageBackground)
        .onAppear { store.refreshBridgeStatus() }
    }

    // MARK: Structure

    private func section<Content: View>(_ title: String, icon: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                DmxIcon(icon, size: 14).foregroundColor(.secondary)
                SectionLabel(title)
            }
            .padding(.horizontal, 4)
            VStack(spacing: 0) {
                content()
            }
            .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(DmxPalette.cardBackground))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(DmxPalette.separator.opacity(0.5), lineWidth: 0.5))
        }
    }

    private func row<Trailing: View>(_ title: String, subtitle: String? = nil, @ViewBuilder trailing: () -> Trailing) -> some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 14, weight: .medium))
                if let subtitle = subtitle {
                    Text(subtitle).font(.system(size: 12)).foregroundColor(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 8)
            trailing()
        }
        .padding(16)
    }

    // MARK: Apparence

    private var appearanceSection: some View {
        section("Apparence", icon: "Palette") {
            row("Thème de l'interface", subtitle: "Choisissez l'aspect visuel de l'application") {
                HStack(spacing: 2) {
                    ForEach([(Theme.light, "Sun"), (Theme.dark, "Moon"), (Theme.system, "Monitor")], id: \.1) { item in
                        let isSelected = store.settings.theme == item.0
                        Button(action: { store.apply(.setTheme(theme: item.0)) }) {
                            DmxIcon(item.1, size: 15)
                                .foregroundColor(isSelected ? .primary : .secondary)
                                .frame(width: 40, height: 28)
                                .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(isSelected ? DmxPalette.cardBackground : Color.clear).shadow(color: Color.black.opacity(isSelected ? 0.12 : 0), radius: 2, y: 1))
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(PlainButtonStyle())
                    }
                }
                .padding(3)
                .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.07)))
            }
            Divider()
            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Couleur d'accentuation").font(.system(size: 14, weight: .medium))
                    Text("Personnalisez la couleur principale").font(.system(size: 12)).foregroundColor(.secondary)
                }
                HStack(spacing: 8) {
                    let isDefault = store.settings.accentColor == nil
                    Button(action: { store.apply(.setPrimaryColor(color: "default")) }) {
                        Text("Def")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundColor(isDefault ? .primary : .secondary)
                            .frame(width: 28, height: 28)
                            .overlay(Circle().stroke(isDefault ? Color.primary : DmxPalette.separator, lineWidth: 2))
                            .contentShape(Circle())
                    }
                    .buttonStyle(PlainButtonStyle())
                    Rectangle().fill(DmxPalette.separator).frame(width: 1, height: 20)
                    ForEach(accentColors(), id: \.self) { hex in
                        let isSelected = store.settings.accentColor?.lowercased() == hex.lowercased()
                        Button(action: { store.apply(.setPrimaryColor(color: hex)) }) {
                            Circle()
                                .fill(Color(hex: hex))
                                .frame(width: 26, height: 26)
                                .overlay(Circle().stroke(Color.primary, lineWidth: isSelected ? 2 : 0).padding(-3))
                                .contentShape(Circle())
                        }
                        .buttonStyle(PlainButtonStyle())
                    }
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    // MARK: iCloud

    private func iCloudSection(_ iCloud: ICloudSettings) -> some View {
        let _ = refreshTick
        let available = iCloud.isAvailable()
        let enabled = available && iCloud.isEnabled()
        return section("Synchronisation iCloud", icon: "Cloud") {
            row(
                "Synchroniser avec iCloud",
                subtitle: available
                    ? "Vos comptes, transactions, budgets et réglages sont partagés entre vos appareils Apple connectés au même compte iCloud."
                    : iCloud.unavailableReason()
            ) {
                Toggle("", isOn: Binding(get: { enabled }, set: { value in
                    iCloud.setEnabled(value)
                    refreshTick += 1
                }))
                .labelsHidden()
                .disabled(!available)
            }
            if enabled {
                Divider()
                row("État", subtitle: iCloud.status()) {
                    Button(action: {
                        iCloud.syncNow()
                        refreshTick += 1
                    }) {
                        HStack(spacing: 6) {
                            DmxIcon("RefreshCw", size: 13)
                            Text("Synchroniser")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(.secondary))
                }
            }
        }
    }

    // MARK: Pont PWA

    private var bridgeSection: some View {
        let status = store.bridgeStatus
        let bridge = status?.secureBridge
        let enabled = bridge?.enabled ?? false
        let active = bridge?.active ?? false
        let certificateReady = bridge?.certificateReady ?? false
        let passkeys = bridge?.passkeys.filter { $0.revokedAt == nil } ?? []
        let localLabel = (status?.active ?? false) ? "Actif" : (enabled ? "Démarrage" : "Inactif")

        let state: (label: String, detail: String, color: Color) = {
            if !enabled { return ("Désactivé", "Active le mode pour préparer le pont HTTPS et le QR mobile.", .secondary) }
            if active { return ("Prêt à appairer", "La PWA peut se connecter à l’API locale sécurisée.", DmxColors.income) }
            if certificateReady { return ("Démarrage local", "Le certificat est prêt, le serveur local termine son démarrage.", Color(hex: "#3b82f6")) }
            return ("Préparation HTTPS", "DNS et certificat sont préparés automatiquement en arrière-plan.", DmxColors.warning)
        }()

        let provisioningReady = bridge?.configured ?? false
        let provisioningLabel = provisioningReady ? "Prêt" : (!enabled ? "En attente d’activation" : "En cours ou indisponible")
        let dnsLabel = bridge?.dnsRecordId != nil ? "Configuré" : ((bridge?.managedCredentialReady ?? false) ? "Prêt" : (enabled ? "En attente" : "En attente d’activation"))
        let steps: [(label: String, value: String, ready: Bool, icon: String)] = [
            ("PWA publique", bridge?.appUrl != nil ? "Disponible" : "En attente", bridge?.appUrl != nil, "Globe2"),
            ("Provisionnement", provisioningLabel, provisioningReady, "KeyRound"),
            ("DNS local", dnsLabel, bridge?.dnsRecordId != nil, "Wifi"),
            ("Certificat HTTPS", certificateReady ? "Prêt" : (enabled ? "En génération" : "Absent"), certificateReady, "ShieldCheck"),
            ("API locale", bridge?.apiUrl != nil ? localLabel : "Non active", active, "Server"),
        ]
        let pairingLabel = active ? "Nouveau QR" : (enabled ? "Préparation HTTPS" : "Activer d’abord")
        let qrEmpty = !enabled ? "Le QR sera disponible après activation."
            : (!certificateReady ? "Certificat HTTPS en cours de génération."
               : (!active ? "Serveur local en démarrage." : "Génère un QR pour appairer un mobile."))

        return section("Mode compagnon mobile", icon: "Smartphone") {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .top, spacing: 12) {
                    IconCircle(icon: "Lock", color: DmxColors.income, size: 34)
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 8) {
                            Text("Accès mobile local + PWA").font(.system(size: 14, weight: .semibold))
                            Pill(state.label, color: state.color)
                        }
                        Text(state.detail).font(.system(size: 12)).foregroundColor(.secondary)
                    }
                    Spacer()
                    Toggle(isOn: Binding(get: { enabled }, set: setBridgeEnabled)) {
                        Text("Activer").font(.system(size: 13, weight: .semibold))
                    }
                    .disabled(bridgeBusy || status == nil)
                }
                HStack(spacing: 8) {
                    infoTile(icon: "Globe2", label: "PWA mobile", value: bridge?.appUrl ?? "Provisionnement automatique en attente")
                    infoTile(icon: "Server", label: "API locale sécurisée", value: bridge?.apiUrl ?? "Non active")
                }
            }
            .padding(16)
            Divider()
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(steps.chunked(compact ? 1 : 2).enumerated()), id: \.offset) { pair in
                        HStack(spacing: 8) {
                            ForEach(pair.element, id: \.label) { step in
                                HStack(spacing: 8) {
                                    ZStack {
                                        RoundedRectangle(cornerRadius: 6).fill((step.ready ? DmxColors.income : (enabled ? DmxColors.warning : Color.secondary)).opacity(0.14))
                                        DmxIcon(step.ready ? "CheckCircle2" : step.icon, size: 13)
                                            .foregroundColor(step.ready ? DmxColors.income : (enabled ? DmxColors.warning : .secondary))
                                    }
                                    .frame(width: 24, height: 24)
                                    VStack(alignment: .leading, spacing: 0) {
                                        Text(step.label).font(.system(size: 12, weight: .medium)).lineLimit(1)
                                        Text(step.value).font(.system(size: 11)).foregroundColor(.secondary).lineLimit(1)
                                    }
                                    Spacer(minLength: 0)
                                }
                                .padding(.horizontal, 10)
                                .padding(.vertical, 7)
                                .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.04)))
                            }
                            if pair.element.count == 1 && !compact {
                                Color.clear.frame(maxWidth: .infinity, maxHeight: 1)
                            }
                        }
                    }
                    HStack(spacing: 6) {
                        badge("Local", localLabel)
                        badge("Certificat", certificateReady ? "Prêt" : "Absent")
                        badge("Mobiles", "\(passkeys.count)")
                    }
                    if enabled, let error = bridge?.lastError {
                        HStack(alignment: .top, spacing: 8) {
                            DmxIcon("AlertTriangle", size: 14)
                            VStack(alignment: .leading, spacing: 2) {
                                if bridge?.degraded ?? false {
                                    Text("Appairage toujours possible").fontWeight(.semibold)
                                }
                                Text(error).fixedSize(horizontal: false, vertical: true)
                            }
                        }
                        .font(.system(size: 12))
                        .foregroundColor((bridge?.degraded ?? false) ? DmxColors.warning : DmxColors.expense)
                        .padding(10)
                        .background(RoundedRectangle(cornerRadius: 10).fill(((bridge?.degraded ?? false) ? DmxColors.warning : DmxColors.expense).opacity(0.1)))
                    }
                }
                if !compact {
                    qrPanel(url: bridge?.pairingUrl, emptyMessage: qrEmpty, pairingLabel: pairingLabel, active: active)
                        .frame(width: 190)
                }
            }
            .padding(16)
            if compact {
                qrPanel(url: bridge?.pairingUrl, emptyMessage: qrEmpty, pairingLabel: pairingLabel, active: active)
                    .padding([.horizontal, .bottom], 16)
            }
            if bridge != nil {
                Divider()
                VStack(alignment: .leading, spacing: 8) {
                    SectionLabel("Mobiles appairés (\(passkeys.count))")
                    if passkeys.isEmpty {
                        Text("Aucun mobile appairé pour l’instant.")
                            .font(.system(size: 12)).foregroundColor(.secondary)
                            .padding(10)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(RoundedRectangle(cornerRadius: 10).fill(Color.primary.opacity(0.04)))
                    }
                    ForEach(passkeys, id: \.id) { passkey in
                        HStack(spacing: 10) {
                            IconCircle(icon: "Smartphone", color: .secondary, size: 30)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(passkey.deviceLabel ?? "Mobile").font(.system(size: 12, weight: .medium)).lineLimit(1)
                                Text(passkeyMeta(passkey)).font(.system(size: 11)).foregroundColor(.secondary).lineLimit(1)
                            }
                            Spacer()
                            Button(action: { revoke(passkey) }) {
                                Text("Désappairer").font(.system(size: 11, weight: .medium)).foregroundColor(DmxColors.expense)
                                    .padding(.horizontal, 10).padding(.vertical, 5)
                                    .background(RoundedRectangle(cornerRadius: 7).fill(DmxColors.expense.opacity(0.1)))
                            }
                            .buttonStyle(PlainButtonStyle())
                            .disabled(bridgeBusy)
                        }
                        .padding(10)
                        .background(RoundedRectangle(cornerRadius: 10).fill(Color.primary.opacity(0.04)))
                    }
                    if passkeys.count == 1 {
                        Text("Pour ajouter un second appareil, génère un nouveau QR et scanne-le depuis celui-ci : le premier reste appairé.")
                            .font(.system(size: 11)).foregroundColor(.secondary).fixedSize(horizontal: false, vertical: true)
                    }
                }
                .padding(16)
            }
        }
    }

    private func infoTile(icon: String, label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 5) {
                DmxIcon(icon, size: 11)
                SectionLabel(label)
            }
            .foregroundColor(.secondary)
            Text(value).font(.system(size: 12)).lineLimit(1).truncationMode(.middle)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.04)))
    }

    private func badge(_ label: String, _ value: String) -> some View {
        (Text(label + " ").foregroundColor(.secondary) + Text(value).fontWeight(.semibold))
            .font(.system(size: 11))
            .padding(.horizontal, 9)
            .padding(.vertical, 4)
            .background(Capsule().fill(Color.primary.opacity(0.05)))
    }

    private func qrPanel(url: String?, emptyMessage: String, pairingLabel: String, active: Bool) -> some View {
        VStack(spacing: 8) {
            VStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 12).fill(Color.white)
                    if let url = url {
                        QRCodeView(text: url).padding(8)
                    } else {
                        VStack(spacing: 6) {
                            DmxIcon("KeyRound", size: 22).foregroundColor(Color.gray.opacity(0.6))
                            Text(emptyMessage).font(.system(size: 11)).foregroundColor(.gray).multilineTextAlignment(.center)
                        }
                        .padding(10)
                    }
                }
                .frame(width: 128, height: 128)
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.black.opacity(0.1), lineWidth: 0.5))
                Button(action: regeneratePairing) {
                    HStack(spacing: 6) {
                        DmxIcon("KeyRound", size: 13)
                        Text(pairingLabel)
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(DmxButtonStyle(.primary))
                .disabled(bridgeBusy || !active)
                if let url = url {
                    Button(action: {
                        actions.copyToClipboard(url)
                        store.showToast("URL copiée")
                    }) {
                        HStack(spacing: 6) {
                            DmxIcon("Copy", size: 13)
                            Text("Copier")
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(DmxButtonStyle(.secondary))
                }
            }
            .padding(12)
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(DmxPalette.separator, lineWidth: 0.5))
            Text("Ouvre la PWA sur mobile, puis appaire le téléphone avec ce QR. Les données apparaissent après cette étape. Génère un QR par appareil : plusieurs mobiles peuvent rester appairés en même temps.")
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func passkeyMeta(_ passkey: PasskeyInfo) -> String {
        var parts = ["Appairé le \(DayFormat.medium(String(passkey.createdAt.prefix(10))))"]
        if let used = passkey.lastUsedAt {
            parts.append("utilisé le \(DayFormat.medium(String(used.prefix(10))))")
        }
        return parts.joined(separator: " · ")
    }

    private func setBridgeEnabled(_ enabled: Bool) {
        bridgeBusy = true
        store.perform({ engine in try engine.setSecureBridgeEnabled(enabled: enabled) }, completion: { [store] status in
            store.updateBridgeStatus(status)
            bridgeBusy = false
        }, failure: { [store] message in
            store.errorMessage = "Pont sécurisé indisponible : \(message)"
            bridgeBusy = false
        })
    }

    private func regeneratePairing() {
        bridgeBusy = true
        store.perform({ engine in try engine.regeneratePairingToken() }, completion: { [store] status in
            store.updateBridgeStatus(status)
            bridgeBusy = false
        }, failure: { [store] message in
            store.errorMessage = "Pairing impossible : \(message)"
            bridgeBusy = false
        })
    }

    private func revoke(_ passkey: PasskeyInfo) {
        store.confirm(title: "Désappairer ce mobile ?", message: "« \(passkey.deviceLabel ?? "Mobile") » devra être appairé de nouveau pour accéder à vos données.", confirmTitle: "Désappairer") { [store] in
            store.perform({ engine in try engine.revokeMobilePasskey(passkeyId: passkey.id) }, completion: { status in
                store.updateBridgeStatus(status)
                store.showToast("Mobile désappairé")
            }, failure: { _ in
                store.errorMessage = "Le mobile n’a pas pu être désappairé."
            })
        }
    }

    // MARK: Données

    private var dataSection: some View {
        section("Données & Stockage", icon: "HardDrive") {
            navigationRow(icon: "Download", title: "Exporter les données", subtitle: "Créer une sauvegarde locale (.dmx)", action: actions.exportBackup)
            Divider()
            navigationRow(icon: "Upload", title: "Importer ou Restaurer", subtitle: "Depuis un backup ou un fichier CSV/OFX/QIF", action: actions.importFile)
        }
    }

    private func navigationRow(icon: String, title: String, subtitle: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 14) {
                ZStack {
                    RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.06))
                    DmxIcon(icon, size: 17).foregroundColor(.secondary)
                }
                .frame(width: 36, height: 36)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.system(size: 14, weight: .medium)).foregroundColor(.primary)
                    Text(subtitle).font(.system(size: 12)).foregroundColor(.secondary)
                }
                Spacer()
                DmxIcon("ChevronRight", size: 16).foregroundColor(.secondary)
            }
            .padding(16)
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
    }

    // MARK: À propos

    private var aboutSection: some View {
        section("À propos", icon: "Info") {
            row("DmxMoney", subtitle: "Version \(AppInfo.version)") {
                Button(action: { store.present(.whatsNew) }) { Text("Nouveautés") }
                    .buttonStyle(DmxButtonStyle(.subtle))
            }
            if let checkForUpdates = actions.checkForUpdates {
                Divider()
                let available = actions.updateAvailable()
                row("Mise à jour logicielle", subtitle: available ? "Nouvelle version disponible" : "Rechercher une nouvelle version") {
                    Button(action: checkForUpdates) {
                        HStack(spacing: 6) {
                            DmxIcon("RefreshCw", size: 13)
                            Text(available ? "Installer" : "Vérifier les mises à jour…")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(available ? .primary : .secondary))
                }
                Divider()
                let prereleaseBinding = Binding<Bool>(
                    get: { includePrereleases },
                    set: { newValue in
                        includePrereleases = newValue
                        UserDefaults.standard.set(newValue, forKey: "DmxIncludePrereleases")
                    }
                )
                Toggle("Autoriser les versions pré-release (bêta / RC)", isOn: prereleaseBinding)
                    .font(.system(size: 13))
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
            }
        }
    }
}

/// QR code généré avec Core Image (lien d'appairage du pont PWA).
public struct QRCodeView: View {
    private let text: String

    public init(text: String) {
        self.text = text
    }

    public var body: some View {
        Group {
            if let image = QRCodeView.makeImage(text) {
                #if os(macOS)
                Image(nsImage: image).interpolation(.none).resizable().aspectRatio(contentMode: .fit)
                #else
                Image(uiImage: image).interpolation(.none).resizable().aspectRatio(contentMode: .fit)
                #endif
            } else {
                DmxIcon("QrCode", size: 40).foregroundColor(.gray)
            }
        }
    }

    #if os(macOS)
    static func makeImage(_ text: String) -> NSImage? {
        guard let cgImage = cgImage(text) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: cgImage.width, height: cgImage.height))
    }
    #else
    static func makeImage(_ text: String) -> UIImage? {
        guard let cgImage = cgImage(text) else { return nil }
        return UIImage(cgImage: cgImage)
    }
    #endif

    private static func cgImage(_ text: String) -> CGImage? {
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(Data(text.utf8), forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let output = filter.outputImage?.transformed(by: CGAffineTransform(scaleX: 8, y: 8)) else { return nil }
        return CIContext().createCGImage(output, from: output.extent)
    }
}
