import DmxKit
import SwiftUI

/// SF Symbols des noms d'icônes stockés en base, identiques sur toutes les plateformes. La table
/// est commune aux apps Apple (DmxKit, générée depuis shared/icons/native.json) : seul
/// l'affichage diffère d'un système à l'autre.
enum Symbols {
    static func name(for lucide: String) -> String {
        DmxIcon.symbolName(for: lucide)
    }

    static func image(_ lucide: String, size: CGFloat? = nil) -> Image {
        Image(systemName: name(for: lucide))
    }

    /// Icône d'une page de la barre latérale.
    static func route(_ route: AppRoute) -> String {
        switch route {
        case .dashboard: return "square.grid.2x2"
        case .accounts: return "creditcard"
        case .transactions: return "list.bullet.rectangle.portrait"
        case .budget: return "chart.bar.horizontal.page"
        case .scheduled: return "calendar.badge.clock"
        case .analytics: return "chart.pie"
        case .predictions: return "chart.line.uptrend.xyaxis"
        case .categories: return "tag"
        case .settings: return "gearshape"
        }
    }
}
