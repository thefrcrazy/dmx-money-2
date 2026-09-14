import DmxKit
import SwiftUI

/// Proposition de reprendre une base DmxMoney 1.x plus complète, en alerte système portée par
/// la fenêtre principale.
///
/// Deux précautions, parce que la reprise remplace la base ouverte :
/// * l'alerte part de la vue, jamais du démarrage — un panneau modal ouvert pendant la
///   construction des scènes SwiftUI relance le rendu au milieu d'une transaction et fait
///   échouer une précondition d'AttributeGraph (l'application s'arrête aussitôt) ;
/// * on attend une fenêtre réellement à l'écran — une alerte présentée alors que l'application
///   est masquée peut se résoudre seule, et « Plus tard » garde le bouton par défaut pour que
///   ni la touche Entrée ni une fermeture automatique ne déclenchent la reprise.
struct ModernLegacyAdoption: ViewModifier {
    @ObservedObject var store: AppStore
    @State private var candidate: DatabaseInventory?
    @State private var comparison = ""
    @State private var asked = false

    func body(content: Content) -> some View {
        content
            .task { await propose() }
            .alert(
                "Reprendre vos données DmxMoney 1.x ?",
                isPresented: Binding(get: { candidate != nil }, set: { if !$0 { candidate = nil } }),
                presenting: candidate
            ) { candidate in
                Button("Plus tard", role: .cancel) { resume() }
                    .keyboardShortcut(.defaultAction)
                Button("Reprendre les données 1.x") { adopt(candidate) }
                Button("Ne plus demander", role: .destructive) { ignore() }
            } message: { candidate in
                Text(message(for: candidate))
            }
    }

    private func propose() async {
        guard !asked, SnapshotRunner.directory == nil else { return }
        guard let found = store.engine.openReport().legacyCandidate else {
            asked = true
            return
        }
        // La fenêtre du `WindowGroup` peut encore être masquée (lancement au login, app cachée) :
        // on laisse jusqu'à 30 s, sinon la proposition attend le prochain lancement.
        for _ in 0..<120 where !LegacyAdoptionPrompt.hasVisibleWindow {
            try? await Task.sleep(nanoseconds: 250_000_000)
        }
        guard LegacyAdoptionPrompt.hasVisibleWindow else { return }
        asked = true
        comparison = LegacyAdoptionPrompt.summary(LegacyAdoptionPrompt.current(store))
        candidate = found
    }

    private func message(for candidate: DatabaseInventory) -> String {
        """
        Une base DmxMoney 1.x plus complète a été trouvée :
        \(LegacyAdoptionPrompt.summary(candidate)).

        Base actuellement ouverte : \(comparison).

        Vos données actuelles seront d'abord exportées en .dmx dans le dossier de l'application, \
        et la base 1.x ne sera pas modifiée.
        """
    }

    private func adopt(_ candidate: DatabaseInventory) {
        self.candidate = nil
        LegacyAdoptionPrompt.adopt(candidate, store: store)
    }

    private func ignore() {
        candidate = nil
        LegacyAdoptionPrompt.ignore(store: store)
        resume()
    }

    /// Les nouveautés attendaient la réponse pour ne pas se superposer à l'alerte.
    private func resume() {
        store.presentWhatsNewIfNeeded()
    }
}
