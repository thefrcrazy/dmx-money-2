import CoreImage.CIFilterBuiltins
import DmxKit
import SwiftUI

/// Compagnon mobile : accès Internet chiffré ou pont local, QR et appareils appairés.
struct ModernCompanion: View {
    @EnvironmentObject private var store: AppStore
    @State private var isWorking = false

    private var bridge: SecureBridgeInfo? { store.bridgeStatus?.secureBridge }
    private var isRemote: Bool { bridge?.apiUrl?.contains("/relay/") == true }
    private var hasLegacyBridge: Bool { !isRemote && bridge?.localHost != nil }
    private var needsHostedPwaUpdate: Bool {
        CompanionMigration.needsHostedPwaUpdate(apiUrl: bridge?.apiUrl, appUrl: bridge?.appUrl)
    }

    var body: some View {
        PageBody {
            VStack(alignment: .leading, spacing: 16) {
                if store.bridgeAvailable {
                    activation
                    if hasLegacyBridge || needsHostedPwaUpdate {
                        Card {
                            VStack(alignment: .leading, spacing: 8) {
                                Text(hasLegacyBridge
                                    ? "Votre ancien compagnon utilise le réseau local. L’accès en 4G ou 5G nécessite le compagnon Internet."
                                    : "Votre compagnon Internet utilise l’ancienne page. Mettez-le à jour pour ouvrir la PWA Cloudflare Pages.")
                                    .font(.callout)
                                Button(hasLegacyBridge ? "Passer à l’accès Internet" : "Mettre à jour le compagnon") { setEnabled(true) }
                                    .buttonStyle(.borderedProminent)
                                    .disabled(isWorking)
                            }
                        }
                    }
                    if bridge?.enabled == true {
                        HStack(alignment: .top, spacing: 16) {
                            steps
                            pairing
                        }
                        passkeys
                    }
                } else {
                    ContentUnavailableView {
                        Label("Compagnon indisponible", systemImage: "iphone.slash")
                    } description: {
                        Text("Le pont est désactivé sur un dossier de données de test (DMXMONEY_DATA_DIR).")
                    }
                    .frame(minHeight: 240)
                }
            }
        }
        .onAppear { store.refreshBridgeStatus() }
    }

    // MARK: Activation

    private var activation: some View {
        Card {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: (bridge?.active ?? false) ? "lock.shield.fill" : "lock.shield")
                    .font(.system(size: 30))
                    .foregroundStyle(stateColor)
                    .frame(width: 44, height: 44)
                    .background(stateColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        Text("Compagnon mobile").font(.headline)
                        Text(badge)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(stateColor)
                            .padding(.horizontal, 8).padding(.vertical, 2)
                            .background(stateColor.opacity(0.14), in: Capsule())
                    }
                    Text(detail).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    if bridge?.enabled == true {
                        ProgressView(value: Double(readyCount), total: Double(stepList.count)) {
                            Text("Connexion \(readyCount)/\(stepList.count)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        .progressViewStyle(.linear)
                        .frame(maxWidth: 320)
                    }
                }
                Spacer(minLength: 8)
                Toggle("", isOn: Binding(
                    get: { bridge?.enabled ?? false },
                    set: setEnabled
                ))
                .labelsHidden()
                .disabled(isWorking)
            }
        }
    }

    private var stateColor: Color {
        guard let bridge, bridge.enabled else { return .secondary }
        return bridge.active && isRemote ? .green : .orange
    }

    private var readyCount: Int {
        stepList.filter(\.ready).count
    }

    private var badge: String {
        guard let bridge else { return "Désactivé" }
        if !bridge.enabled { return "Désactivé" }
        if hasLegacyBridge { return "Accès local hérité" }
        if bridge.active { return "Prêt à appairer" }
        if isRemote { return "Connexion Internet" }
        if bridge.certificateReady { return "Démarrage local" }
        return "Préparation HTTPS"
    }

    private var detail: String {
        guard let bridge, bridge.enabled else {
            return "Activez l’accès Internet pour modifier les données de ce Mac depuis votre téléphone, en Wi-Fi, 4G ou 5G, puis scannez le QR."
        }
        if isRemote {
            return "Les modifications de votre téléphone passent par le relais chiffré jusqu’à ce Mac, en Wi-Fi, 4G ou 5G. DmxMoney doit rester ouvert sur ce Mac allumé et connecté à Internet."
        }
        return "Ancien accès local : le téléphone doit être sur le même réseau que ce Mac. Cet accès ne fonctionne pas en 4G ou 5G."
    }

    // MARK: Étapes

    private var steps: some View {
        Card("Connexion sécurisée", systemImage: "checklist") {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(stepList.enumerated()), id: \.element.title) { item in
                    if item.offset > 0 { Divider() }
                    HStack(spacing: 10) {
                        Image(systemName: item.element.ready ? "checkmark.circle.fill" : item.element.icon)
                            .font(.system(size: 15))
                            .foregroundStyle(item.element.ready ? .green : .orange)
                            .frame(width: 24, height: 24)
                            .background((item.element.ready ? Color.green : Color.orange).opacity(0.12), in: Circle())
                        VStack(alignment: .leading, spacing: 0) {
                            Text(item.element.title).fontWeight(.medium)
                            Text(item.element.value).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(.vertical, 2)
                }
                if let error = bridge?.lastError, !error.isEmpty {
                    Label(error, systemImage: "exclamationmark.triangle")
                        .font(.caption)
                        .foregroundStyle(.orange)
                }
                if !isRemote, let api = bridge?.apiUrl {
                    Divider()
                    LabeledContent("Adresse servie") {
                        Text(api).font(.caption.monospaced()).textSelection(.enabled)
                    }
                    // Les deux versions partagent le sous-domaine et le certificat (même entrée
                    // de trousseau) : si la 1.x tourne, elle garde le port habituel et le mobile
                    // continue de lui parler, avec son ancien client.
                    Label(
                        "DmxMoney 1.x partage ce sous-domaine : quittez-la, puis appairez de nouveau le mobile avec le QR ci-contre.",
                        systemImage: "info.circle"
                    )
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    private struct Step {
        let title: String
        let value: String
        let ready: Bool
        let icon: String
    }

    private var stepList: [Step] {
        let bridge = self.bridge
        let enabled = bridge?.enabled ?? false
        if isRemote {
            let connected = bridge?.active ?? false
            let encryptionReady = bridge?.managedCredentialReady ?? false
            return [
                Step(title: "Compagnon mobile", value: bridge?.appUrl == nil ? "En attente" : "Disponible", ready: bridge?.appUrl != nil, icon: "globe"),
                Step(title: "Chiffrement entre appareils", value: encryptionReady ? "Prêt" : "En préparation", ready: encryptionReady, icon: "lock.shield"),
                Step(title: "Connexion Internet", value: connected ? "Connectée" : "Reconnexion en cours", ready: connected, icon: "wifi"),
                Step(title: "Relais sécurisé", value: (bridge?.configured ?? false) ? "Prêt" : "En préparation", ready: bridge?.configured ?? false, icon: "server.rack"),
            ]
        }
        return [
            Step(
                title: "Client PWA",
                value: bridge?.appUrl == nil ? "En attente" : "Servi par ce Mac",
                ready: bridge?.appUrl != nil,
                icon: "globe"
            ),
            Step(
                title: "Provisionnement",
                value: (bridge?.configured ?? false) ? "Prêt" : (enabled ? "En cours" : "En attente d'activation"),
                ready: bridge?.configured ?? false,
                icon: "key"
            ),
            Step(
                title: "DNS local",
                value: bridge?.dnsRecordId != nil ? "Configuré" : (enabled ? "En attente" : "En attente d'activation"),
                ready: bridge?.dnsRecordId != nil,
                icon: "wifi"
            ),
            Step(
                title: "Certificat HTTPS",
                value: (bridge?.certificateReady ?? false) ? "Prêt" : (enabled ? "En génération" : "Absent"),
                ready: bridge?.certificateReady ?? false,
                icon: "lock"
            ),
            Step(
                title: "Serveur local",
                value: (bridge?.active ?? false) ? "Actif" : (enabled ? "Démarrage" : "Inactif"),
                ready: bridge?.active ?? false,
                icon: "server.rack"
            ),
        ]
    }

    // MARK: Appairage

    private var pairing: some View {
        Card("Appairer un mobile", systemImage: "qrcode") {
            VStack(spacing: 10) {
                if let url = bridge?.pairingUrl, let image = qrCode(url) {
                    Image(nsImage: image)
                        .interpolation(.none)
                        .resizable()
                        .frame(width: 196, height: 196)
                        .padding(10)
                        .background(.white, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                        .overlay {
                            RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(.separator, lineWidth: 0.5)
                        }
                    Text(isRemote
                        ? "Scannez ce QR, puis validez avec Face ID, Touch ID ou le verrouillage du téléphone. Vos modifications seront envoyées à DmxMoney sur ce Mac par Internet."
                        : "Ce QR utilise l’ancien accès local, disponible sur le même réseau que ce Mac. Passez à l’accès Internet pour utiliser la 4G ou la 5G.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                    HStack(spacing: 8) {
                        Button("Copier le lien", systemImage: "doc.on.doc") {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(url, forType: .string)
                            store.showToast("Lien d'appairage copié")
                        }
                        Button("Nouveau QR", systemImage: "arrow.trianglehead.clockwise") { regenerate() }
                            .buttonStyle(.borderedProminent)
                    }
                    if let app = bridge?.appUrl, let target = URL(string: app) {
                        Button("Ouvrir le client sur ce Mac", systemImage: "safari") {
                            NSWorkspace.shared.open(target)
                        }
                        .buttonStyle(.link)
                        .font(.caption)
                    }
                } else {
                    ContentUnavailableView {
                        Label("QR indisponible", systemImage: "qrcode")
                    } description: {
                        Text(pairingHint)
                    } actions: {
                        Button("Générer un QR") { regenerate() }
                            .disabled(!(bridge?.active ?? false))
                    }
                    .frame(width: 230, height: 230)
                }
            }
            .frame(width: 240)
        }
        .fixedSize(horizontal: true, vertical: false)
    }

    private var pairingHint: String {
        guard let bridge, bridge.enabled else { return "Active le compagnon pour obtenir un QR." }
        if isRemote {
            return bridge.active ? "Générez un QR pour appairer un mobile." : "Connexion Internet en cours."
        }
        if !bridge.certificateReady { return "Certificat HTTPS en cours de génération." }
        if !bridge.active { return "Serveur local en démarrage." }
        return "Génère un QR pour appairer un mobile."
    }

    private func regenerate() {
        store.perform({ engine in try engine.regeneratePairingToken() }, completion: { status in
            store.updateBridgeStatus(status)
            store.showToast("Nouveau QR d'appairage")
        }, failure: { message in
            store.errorMessage = "Appairage impossible : \(message)"
        })
    }

    private func setEnabled(_ enabled: Bool) {
        guard !isWorking else { return }
        if enabled && (hasLegacyBridge || needsHostedPwaUpdate) {
            store.confirm(
                title: hasLegacyBridge ? "Passer à l’accès Internet ?" : "Mettre à jour le compagnon ?",
                message: "Synchronisez d’abord les saisies en attente dans l’ancienne PWA. Vos mobiles devront ensuite être appairés avec un nouveau QR ; les anciennes sessions seront révoquées.",
                confirmTitle: hasLegacyBridge ? "Passer à Internet" : "Mettre à jour",
                destructive: false
            ) { applyEnabled(true) }
        } else {
            applyEnabled(enabled)
        }
    }

    private func applyEnabled(_ enabled: Bool) {
        isWorking = true
        store.perform({ engine in try engine.setSecureBridgeEnabled(enabled: enabled) }, completion: { status in
            store.updateBridgeStatus(status)
            isWorking = false
        }, failure: { message in
            store.errorMessage = "Compagnon Internet indisponible : \(message)"
            isWorking = false
        })
    }

    /// QR d'appairage rendu par CoreImage (aucune dépendance externe).
    private func qrCode(_ url: String) -> NSImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(url.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage?.transformed(by: CGAffineTransform(scaleX: 10, y: 10)) else { return nil }
        let context = CIContext()
        guard let cgImage = context.createCGImage(output, from: output.extent) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: output.extent.width, height: output.extent.height))
    }

    // MARK: Mobiles appairés

    private var passkeys: some View {
        let active = (bridge?.passkeys ?? []).filter { $0.revokedAt == nil }
        return Card("Mobiles appairés (\(active.count))", systemImage: "iphone") {
            if active.isEmpty {
                Text("Aucun mobile appairé pour l'instant.").foregroundStyle(.secondary)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(active.enumerated()), id: \.element.id) { item in
                        if item.offset > 0 { Divider() }
                        HStack(spacing: 10) {
                            Image(systemName: "iphone").foregroundStyle(.secondary)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(item.element.deviceLabel ?? "Mobile")
                                Text(meta(item.element)).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer(minLength: 8)
                            Button("Désappairer", role: .destructive) { revoke(item.element) }
                        }
                        .padding(.vertical, 8)
                    }
                }
            }
        }
    }

    private func meta(_ passkey: PasskeyInfo) -> String {
        var parts = ["Appairé le \(DayFormat.medium(String(passkey.createdAt.prefix(10))))"]
        if let used = passkey.lastUsedAt {
            parts.append("utilisé le \(DayFormat.medium(String(used.prefix(10))))")
        }
        return parts.joined(separator: " · ")
    }

    private func revoke(_ passkey: PasskeyInfo) {
        let label = passkey.deviceLabel ?? "Mobile"
        store.confirm(
            title: "Désappairer « \(label) » ?",
            message: "Ce mobile devra être appairé de nouveau pour accéder à vos données."
        ) {
            store.perform({ engine in try engine.revokeMobilePasskey(passkeyId: passkey.id) }, completion: { status in
                store.updateBridgeStatus(status)
                store.showToast("Mobile désappairé")
            }, failure: { message in
                store.errorMessage = "Le mobile n'a pas pu être désappairé : \(message)"
            })
        }
    }
}
