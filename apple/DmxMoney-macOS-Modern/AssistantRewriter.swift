import DmxKit
import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

/// Normalise une demande en langage naturel avec le modèle sur l'appareil, avant que le noyau
/// ne l'analyse.
///
/// Le modèle ne calcule jamais un montant et ne touche pas aux données : il réécrit la phrase
/// dans la forme que `dmx-core` sait lire (« ajoute 12,50 € en alimentation »). Sans Apple
/// Intelligence — Mac Intel, modèle non téléchargé, fonction désactivée — la phrase d'origine est
/// analysée telle quelle, et la réponse reste la même.
enum AssistantRewriter {
    /// Formes attendues par le noyau, données au modèle comme cadre de réécriture.
    private static let instructions = """
    Tu traduis une demande d'argent en une phrase courte et canonique en français, sans rien \
    calculer et sans rien inventer. Réponds uniquement par la phrase, sans guillemets.
    Formes acceptées :
    - « ajoute <montant> € en <catégorie> sur <compte> » pour une dépense
    - « j'ai reçu <montant> € en <catégorie> sur <compte> » pour un revenu
    - « quel est mon solde » ou « solde <compte> »
    - « combien me reste-t-il en <catégorie> »
    - « prochaines échéances »
    - « résumé du mois »
    - « traiter les échéances dues »
    Garde le montant exactement tel qu'il est dit. Si la demande ne correspond à aucune forme, \
    renvoie la demande d'origine sans la modifier.
    """

    /// Fonction à installer sur le store, ou `nil` si l'appareil n'a pas de modèle utilisable.
    static func hook() -> (@Sendable (String) -> String?)? {
        #if canImport(FoundationModels)
        guard #available(macOS 26.0, *), SystemLanguageModel.default.isAvailable else { return nil }
        return { text in rewrite(text) }
        #else
        return nil
        #endif
    }

    /// Vrai quand le modèle sur l'appareil est prêt (pour l'afficher dans les réglages).
    static var isAvailable: Bool {
        #if canImport(FoundationModels)
        guard #available(macOS 26.0, *) else { return false }
        return SystemLanguageModel.default.isAvailable
        #else
        return false
        #endif
    }

    #if canImport(FoundationModels)
    /// Appel synchrone : le pont attend la réponse sur son thread, avec une limite de temps pour
    /// ne jamais bloquer une requête de la PWA.
    @available(macOS 26.0, *)
    private static func rewrite(_ text: String) -> String? {
        let semaphore = DispatchSemaphore(value: 0)
        let box = Box()
        Task.detached(priority: .userInitiated) {
            let session = LanguageModelSession(instructions: instructions)
            do {
                let response = try await session.respond(to: text)
                box.value = response.content.trimmingCharacters(in: .whitespacesAndNewlines)
            } catch {
                NSLog("DmxMoney : reformulation indisponible (%@)", error.localizedDescription)
            }
            semaphore.signal()
        }
        guard semaphore.wait(timeout: .now() + 6) == .success else { return nil }
        guard let value = box.value, !value.isEmpty, value.count < 300 else { return nil }
        return value
    }

    private final class Box: @unchecked Sendable {
        var value: String?
    }
    #endif
}
