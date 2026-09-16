import Foundation

/// Pages de l'application, identiques à la barre latérale de DmxMoney 1.x.
public enum AppRoute: String, CaseIterable, Identifiable {
    case dashboard
    case accounts
    case transactions
    case budget
    case scheduled
    case analytics
    case predictions
    case categories
    case settings

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .dashboard: return "Vue d'ensemble"
        case .accounts: return "Mes Comptes"
        case .transactions: return "Journal"
        case .budget: return "Budget"
        case .scheduled: return "Échéancier"
        case .analytics: return "Analyses"
        case .predictions: return "Prédictions"
        case .categories: return "Catégories"
        case .settings: return "Paramètres"
        }
    }

    /// Libellé court de la barre d'onglets mobile.
    public var shortTitle: String {
        switch self {
        case .dashboard: return "Accueil"
        case .accounts: return "Comptes"
        default: return title
        }
    }

    public var icon: String {
        switch self {
        case .dashboard: return "LayoutDashboard"
        case .accounts: return "Wallet"
        case .transactions: return "Receipt"
        case .budget: return "Calculator"
        case .scheduled: return "CalendarClock"
        case .analytics: return "PieChart"
        case .predictions: return "TrendingUp"
        case .categories: return "Tag"
        case .settings: return "Settings"
        }
    }

    /// Pages où le filtre global de comptes s'applique.
    public var usesAccountFilter: Bool {
        switch self {
        case .dashboard, .transactions, .budget, .analytics, .predictions, .scheduled: return true
        case .accounts, .categories, .settings: return false
        }
    }

    /// Pages qui affichent les soldes Pointé / Actuel dans la barre d'outils.
    public var showsBalances: Bool {
        true
    }

    public static let sidebarSections: [SidebarSection] = [
        SidebarSection(title: "Général", routes: [.dashboard, .accounts, .transactions]),
        SidebarSection(title: "Finances", routes: [.budget, .scheduled]),
        SidebarSection(title: "Analyses", routes: [.analytics, .predictions]),
    ]

    public static let footerRoutes: [AppRoute] = [.categories, .settings]
}

public struct SidebarSection: Identifiable {
    public let title: String
    public let routes: [AppRoute]
    public var id: String { title }
}

/// Formulaires présentés par l'hôte (feuille AppKit sur macOS, sheet SwiftUI sur iOS).
public enum FormRequest: Identifiable, Equatable {
    case account(id: String?)
    case accountGroups
    case transaction(id: String?)
    case category(id: String?)
    case budget(id: String?)
    case scheduled(id: String?)
    case fakeTransaction(id: String?)
    case restoreBackup(content: String, fileName: String)
    case statementImport(content: String, fileName: String)
    case whatsNew
    case newBudget(categoryId: String)
    case budgetSuggestions
    case scheduledSuggestions

    public var id: String {
        switch self {
        case let .account(id): return "account-\(id ?? "new")"
        case .accountGroups: return "account-groups"
        case let .transaction(id): return "transaction-\(id ?? "new")"
        case let .category(id): return "category-\(id ?? "new")"
        case let .budget(id): return "budget-\(id ?? "new")"
        case let .scheduled(id): return "scheduled-\(id ?? "new")"
        case let .fakeTransaction(id): return "fake-\(id ?? "new")"
        case let .restoreBackup(_, fileName): return "restore-\(fileName)"
        case let .statementImport(_, fileName): return "import-\(fileName)"
        case .whatsNew: return "whats-new"
        case let .newBudget(categoryId): return "budget-for-\(categoryId)"
        case .budgetSuggestions: return "budget-suggestions"
        case .scheduledSuggestions: return "scheduled-suggestions"
        }
    }

    /// Largeur préférée de la feuille sur macOS et iPad.
    public var preferredWidth: Double {
        switch self {
        case .statementImport, .budgetSuggestions, .scheduledSuggestions: return 640
        case .category, .account: return 520
        case .whatsNew: return 480
        default: return 460
        }
    }
}

/// Demande de confirmation (suppression, restauration…).
public struct ConfirmRequest: Identifiable {
    public let id = UUID()
    public let title: String
    public let message: String
    public let confirmTitle: String
    public let destructive: Bool
    public let action: () -> Void
}
