import AppKit
import DmxKit

/// Propose de reprendre une base DmxMoney 1.x plus complète trouvée à côté du dossier de
/// données (cas d'un dossier `com.dmxmoney.app` laissé par un ancien build : la 2.x l'adopte
/// sans rien importer, et l'utilisateur croit avoir perdu budgets et échéances).
///
/// La base 1.x n'est jamais modifiée, et la base courante est exportée en `.dmx` avant reprise.
///
/// Ce panneau AppKit sert la variante legacy ; la variante SwiftUI passe par une alerte portée
/// par la fenêtre (`ModernLegacyAdoption`) et réutilise `adopt`, `ignore` et `summary`.
enum LegacyAdoptionPrompt {
    /// Une fenêtre visible sert de garde-fou : une alerte présentée sans fenêtre à l'écran peut
    /// se résoudre seule, et `runModal` relance la boucle d'affichage.
    static var hasVisibleWindow: Bool {
        NSApp.windows.contains { !($0 is NSPanel) && $0.isVisible && $0.frame.width > 600 }
    }

    /// À n'appeler qu'une fois la fenêtre affichée.
    static func presentIfNeeded(store: AppStore) {
        // Désactivé : ne plus proposer d'adoption intempestive de base 1.x
    }

    static func adopt(_ candidate: DatabaseInventory, store: AppStore) {
        let today = store.today
        store.perform({ engine in
            try engine.adoptLegacyDatabase(path: candidate.path, today: today)
        }, completion: { adoption in
            store.reload()
            let counts = "\(adoption.summary.transactions) opérations, \(adoption.summary.budgets) budgets, \(adoption.summary.scheduled) échéances"
            store.showToast("Données DmxMoney 1.x reprises : \(counts)")
        }, failure: { message in
            store.errorMessage = "La reprise a échoué : \(message)"
        })
    }

    /// « Ne plus demander » : le noyau retient l'empreinte de cette base 1.x.
    static func ignore(store: AppStore) {
        _ = store.attempt { engine in try engine.ignoreLegacyCandidate() }
    }

    static func current(_ store: AppStore) -> DatabaseInventory {
        // Inventaire de la base ouverte, pour la comparaison affichée.
        let inventory = store.read { engine in try engine.currentInventory() }
        return inventory ?? DatabaseInventory(
            path: store.engine.openReport().databasePath,
            accounts: 0, transactions: 0, categories: 0, budgets: 0, scheduled: 0,
            lastTransactionDate: nil
        )
    }

    static func summary(_ inventory: DatabaseInventory) -> String {
        var parts = [
            "\(inventory.accounts) comptes",
            "\(inventory.transactions) opérations",
            "\(inventory.categories) catégories",
            "\(inventory.budgets) budgets",
            "\(inventory.scheduled) échéances",
        ]
        if let date = inventory.lastTransactionDate {
            parts.append("dernière opération le \(DayFormat.medium(date))")
        }
        return parts.joined(separator: ", ")
    }
}
