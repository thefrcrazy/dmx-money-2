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

    func body(content: Content) -> some View {
        content
    }
}
