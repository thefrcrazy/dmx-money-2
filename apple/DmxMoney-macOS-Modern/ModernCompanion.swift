import CoreImage.CIFilterBuiltins
import DmxKit
import SwiftUI

/// Compagnon mobile : accès Internet chiffré, QR et appareils appairés.
struct ModernCompanion: View {
    @EnvironmentObject private var store: AppStore
    @State private var isWorking = false

    private var bridge: SecureBridgeInfo? { store.bridgeStatus?.secureBridge }

    var body: some View {
        PageBody {
            VStack(alignment: .leading, spacing: 16) {
                if store.bridgeAvailable {
                    activation
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
                Toggle("Activer le compagnon Internet", isOn: Binding(
                    get: { bridge?.enabled ?? false },
                    set: setEnabled
                ))
                .labelsHidden()
                .disabled(isWorking || store.bridgeStatus == nil)
            }
        }
    }

    private var stateColor: Color {
        guard let bridge, bridge.enabled else { return .secondary }
        return bridge.active ? .green : .orange
    }

    private var readyCount: Int {
        stepList.filter(\.ready).count
    }

    private var badge: String {
        guard let bridge else { return "Désactivé" }
        if !bridge.enabled { return "Désactivé" }
        if bridge.active { return "Prêt à appairer" }
        return "Connexion Internet"
    }

    private var detail: String {
        guard let bridge, bridge.enabled else {
            return "Activez l’accès Internet pour modifier les données de ce Mac depuis votre téléphone, en Wi-Fi, 4G ou 5G, puis scannez le QR."
        }
        return "Les modifications de votre téléphone passent par le relais chiffré jusqu’à ce Mac, en Wi-Fi, 4G ou 5G. DmxMoney doit rester ouvert sur ce Mac allumé et connecté à Internet."
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
        let connected = bridge?.active ?? false
        let encryptionReady = bridge?.managedCredentialReady ?? false
        return [
            Step(title: "Compagnon mobile", value: bridge?.appUrl == nil ? "En attente" : "Disponible", ready: bridge?.appUrl != nil, icon: "globe"),
            Step(title: "Chiffrement entre appareils", value: encryptionReady ? "Prêt" : "En préparation", ready: encryptionReady, icon: "lock.shield"),
            Step(title: "Connexion Internet", value: connected ? "Connectée" : "Reconnexion en cours", ready: connected, icon: "wifi"),
            Step(title: "Relais sécurisé", value: (bridge?.configured ?? false) ? "Prêt" : "En préparation", ready: bridge?.configured ?? false, icon: "server.rack"),
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
                    Text("Scannez ce QR, puis validez avec Face ID, Touch ID ou le verrouillage du téléphone. Vos modifications seront envoyées à DmxMoney sur ce Mac par Internet.")
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
                            .disabled(isWorking || !(bridge?.active ?? false))
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
                            .disabled(isWorking || !(bridge?.active ?? false))
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
        return bridge.active ? "Générez un QR pour appairer un mobile." : "Connexion Internet en cours."
    }

    private func regenerate() {
        guard !isWorking else { return }
        isWorking = true
        store.perform({ engine in try engine.regeneratePairingToken() }, completion: { status in
            store.updateBridgeStatus(status)
            store.showToast("Nouveau QR d'appairage")
            isWorking = false
        }, failure: { message in
            store.errorMessage = "Appairage impossible : \(message)"
            isWorking = false
        })
    }

    private func setEnabled(_ enabled: Bool) {
        guard !isWorking else { return }
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
