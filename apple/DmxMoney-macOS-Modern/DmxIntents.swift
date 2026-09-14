import AppIntents
import DmxKit
import SwiftUI
import UserNotifications

/// Intentions Siri et Raccourcis.
///
/// Elles ne calculent rien : chaque intention transmet la demande à `dmx-core` et lit la phrase
/// qu'il renvoie. Les noms de compte et de catégorie sont résolus par le noyau, pour que Siri
/// puisse dire « en alimentation » sans connaître d'identifiant.
enum DmxAssistant {
    /// Exécute un appel sur le moteur de l'application et rafraîchit l'interface si besoin.
    @MainActor
    static func run(_ body: (DmxEngine, String) throws -> AssistantResult) throws -> AssistantResult {
        let launcher = Launcher.shared
        guard let store = launcher.store else {
            throw DmxIntentError.unavailable(launcher.error ?? "La base DmxMoney n'a pas pu être ouverte.")
        }
        let result = try body(store.engine, store.today)
        if result.changed {
            store.reload()
        }
        notify(result)
        return result
    }

    /// Notification système avec la réponse : lancée depuis Raccourcis, une intention sans
    /// interface ne montrait rien, même quand l'opération était bien enregistrée.
    static func notify(_ result: AssistantResult) {
        let center = UNUserNotificationCenter.current()
        let summary = result.summary
        let details = result.details.prefix(3).joined(separator: "\n")
        Task {
            guard (try? await center.requestAuthorization(options: [.alert, .sound])) == true else { return }
            let content = UNMutableNotificationContent()
            content.title = "DmxMoney"
            content.body = details.isEmpty ? summary : summary + "\n" + details
            try? await center.add(UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil))
        }
    }
}

/// Vue affichée par Raccourcis et Siri sous la réponse.
struct AssistantSnippet: View {
    let result: AssistantResult

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Label(result.summary, systemImage: result.changed ? "checkmark.circle.fill" : "eurosign.circle")
                .font(.headline)
            ForEach(Array(result.details.prefix(4).enumerated()), id: \.offset) { item in
                Text(item.element).font(.callout).foregroundStyle(.secondary)
            }
        }
        .padding(12)
    }
}

// MARK: - Comptes et catégories proposés dans les listes

struct AccountEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = TypeDisplayRepresentation(name: LocalizedStringResource("Compte", table: "AppIntents"))
    static let defaultQuery = AccountQuery()

    let id: String
    let name: String
    let icon: String

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", image: .init(systemName: Symbols.name(for: icon)))
    }
}

struct AccountQuery: EntityQuery {
    func entities(for identifiers: [String]) async throws -> [AccountEntity] {
        try await suggestedEntities().filter { identifiers.contains($0.id) }
    }

    /// Les comptes de la base, pour que Raccourcis et Siri proposent la liste.
    func suggestedEntities() async throws -> [AccountEntity] {
        await MainActor.run {
            (Launcher.shared.store?.accounts ?? []).map { AccountEntity(id: $0.id, name: $0.name, icon: $0.icon) }
        }
    }
}

struct CategoryEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = TypeDisplayRepresentation(name: LocalizedStringResource("Catégorie", table: "AppIntents"))
    static let defaultQuery = CategoryQuery()

    let id: String
    let name: String
    let icon: String

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", image: .init(systemName: Symbols.name(for: icon)))
    }
}

struct CategoryQuery: EntityQuery {
    func entities(for identifiers: [String]) async throws -> [CategoryEntity] {
        try await suggestedEntities().filter { identifiers.contains($0.id) }
    }

    /// Catégories de la base, sans « Virement », réservée aux virements.
    func suggestedEntities() async throws -> [CategoryEntity] {
        await MainActor.run {
            (Launcher.shared.store?.categories ?? [])
                .filter { $0.id != "transfer" }
                .map { CategoryEntity(id: $0.id, name: $0.name, icon: $0.icon) }
        }
    }
}

enum DmxIntentError: Error, CustomLocalizedStringResourceConvertible {
    case unavailable(String)

    var localizedStringResource: LocalizedStringResource {
        switch self {
        case let .unavailable(message): return "\(message)"
        }
    }
}

// MARK: - Type d'opération

/// Dépense ou revenu. Les synonymes portent l'article, pour que « Ajouter un revenu dans
/// DmxMoney » renseigne directement le type.
enum TransactionKindChoice: String, AppEnum {
    case expense
    case income

    static let typeDisplayRepresentation = TypeDisplayRepresentation(
        name: LocalizedStringResource("Type d'opération", table: "AppIntents")
    )

    static let caseDisplayRepresentations: [TransactionKindChoice: DisplayRepresentation] = [
        .expense: DisplayRepresentation(
            title: LocalizedStringResource("Dépense", table: "AppIntents"),
            image: .init(systemName: "arrow.down.right"),
            synonyms: [
                LocalizedStringResource("une dépense", table: "AppIntents"),
                LocalizedStringResource("dépense", table: "AppIntents"),
            ]
        ),
        .income: DisplayRepresentation(
            title: LocalizedStringResource("Revenu", table: "AppIntents"),
            image: .init(systemName: "arrow.up.right"),
            synonyms: [
                LocalizedStringResource("un revenu", table: "AppIntents"),
                LocalizedStringResource("revenu", table: "AppIntents"),
            ]
        ),
    ]

    var coreType: TransactionType { self == .income ? .income : .expense }
}

// MARK: - Ajouter une opération

struct AddTransactionIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource("Ajouter une opération", table: "AppIntents")
    static let description = IntentDescription(LocalizedStringResource("Enregistre une dépense ou un revenu dans DmxMoney.", table: "AppIntents"))
    static let openAppWhenRun = false

    @Parameter(title: LocalizedStringResource("Montant", table: "AppIntents"), controlStyle: .field)
    var amount: Double

    @Parameter(title: LocalizedStringResource("Catégorie", table: "AppIntents"), requestValueDialog: IntentDialog(LocalizedStringResource("Dans quelle catégorie ?", table: "AppIntents")))
    var category: CategoryEntity?

    @Parameter(title: LocalizedStringResource("Compte", table: "AppIntents"))
    var account: AccountEntity?

    @Parameter(title: LocalizedStringResource("Type", table: "AppIntents"), default: .expense)
    var kind: TransactionKindChoice

    @Parameter(title: LocalizedStringResource("Libellé", table: "AppIntents"))
    var note: String?

    static var parameterSummary: some ParameterSummary {
        Summary("\(\.$kind) de \(\.$amount) € en \(\.$category)") {
            \.$account
            \.$note
        }
    }

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog & ShowsSnippetView {
        let result = try DmxAssistant.run { engine, today in
            try engine.assistantAddTransaction(
                amount: amount,
                kind: kind.coreType,
                category: category?.name,
                account: account?.name,
                description: note,
                today: today
            )
        }
        return .result(value: result.summary, dialog: IntentDialog(stringLiteral: result.summary), view: AssistantSnippet(result: result))
    }
}

// MARK: - Solde

struct AccountBalanceIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource("Consulter un solde", table: "AppIntents")
    static let description = IntentDescription(LocalizedStringResource("Donne le solde d'un compte, ou le total de vos comptes.", table: "AppIntents"))
    static let openAppWhenRun = false

    @Parameter(title: LocalizedStringResource("Compte", table: "AppIntents"))
    var account: AccountEntity?

    static var parameterSummary: some ParameterSummary {
        Summary("Solde de \(\.$account)")
    }

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog & ShowsSnippetView {
        let result = try DmxAssistant.run { engine, today in
            try engine.assistantBalance(account: account?.name, today: today)
        }
        return .result(value: speech(result), dialog: IntentDialog(stringLiteral: speech(result)), view: AssistantSnippet(result: result))
    }
}

// MARK: - Budget restant

struct BudgetRemainingIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource("Budget restant", table: "AppIntents")
    static let description = IntentDescription(LocalizedStringResource("Dit ce qu'il reste à dépenser, pour une catégorie ou pour le mois.", table: "AppIntents"))
    static let openAppWhenRun = false

    @Parameter(title: LocalizedStringResource("Catégorie", table: "AppIntents"))
    var category: CategoryEntity?

    static var parameterSummary: some ParameterSummary {
        Summary("Reste à dépenser en \(\.$category)")
    }

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog & ShowsSnippetView {
        let result = try DmxAssistant.run { engine, today in
            try engine.assistantBudget(category: category?.name, today: today)
        }
        return .result(value: speech(result), dialog: IntentDialog(stringLiteral: speech(result)), view: AssistantSnippet(result: result))
    }
}

// MARK: - Prochaines échéances

struct UpcomingScheduledIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource("Prochaines échéances", table: "AppIntents")
    static let description = IntentDescription(LocalizedStringResource("Liste les échéances à venir.", table: "AppIntents"))
    static let openAppWhenRun = false

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog & ShowsSnippetView {
        let result = try DmxAssistant.run { engine, today in
            try engine.assistantUpcoming(today: today)
        }
        return .result(value: speech(result), dialog: IntentDialog(stringLiteral: speech(result)), view: AssistantSnippet(result: result))
    }
}

// MARK: - Résumé du mois

struct MonthSummaryIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource("Résumé du mois", table: "AppIntents")
    static let description = IntentDescription(LocalizedStringResource("Revenus, dépenses et écart du mois en cours.", table: "AppIntents"))
    static let openAppWhenRun = false

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog & ShowsSnippetView {
        let result = try DmxAssistant.run { engine, today in
            try engine.assistantMonth(today: today)
        }
        return .result(value: speech(result), dialog: IntentDialog(stringLiteral: speech(result)), view: AssistantSnippet(result: result))
    }
}

// MARK: - Traiter les échéances dues

struct ProcessDueIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource("Traiter les échéances dues", table: "AppIntents")
    static let description = IntentDescription(LocalizedStringResource("Enregistre les échéances arrivées à terme.", table: "AppIntents"))
    static let openAppWhenRun = false

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog & ShowsSnippetView {
        let result = try DmxAssistant.run { engine, today in
            try engine.assistant(text: "traiter les échéances dues", today: today, apply: true)
        }
        return .result(value: result.summary, dialog: IntentDialog(stringLiteral: result.summary), view: AssistantSnippet(result: result))
    }
}

// MARK: - Demande libre

struct AskDmxMoneyIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource("Demander à DmxMoney", table: "AppIntents")
    static let description = IntentDescription(LocalizedStringResource("Pose une question ou dicte une opération en une phrase.", table: "AppIntents"))
    static let openAppWhenRun = false

    @Parameter(title: LocalizedStringResource("Demande", table: "AppIntents"), requestValueDialog: IntentDialog(LocalizedStringResource("Que voulez-vous savoir ?", table: "AppIntents")))
    var text: String

    static var parameterSummary: some ParameterSummary {
        Summary("Demander « \(\.$text) » à DmxMoney")
    }

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog & ShowsSnippetView {
        let result = try DmxAssistant.run { engine, today in
            try engine.assistant(text: text, today: today, apply: true)
        }
        return .result(value: speech(result), dialog: IntentDialog(stringLiteral: speech(result)), view: AssistantSnippet(result: result))
    }
}

/// Phrase lue par Siri : le résumé, complété des deux premiers détails quand il y en a.
private func speech(_ result: AssistantResult) -> String {
    guard !result.details.isEmpty else { return result.summary }
    return ([result.summary] + result.details.prefix(2)).joined(separator: " ")
}

// MARK: - Phrases Siri

struct DmxShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: AccountBalanceIntent(),
            phrases: [
                "Quel est mon solde dans \(.applicationName)",
                "Quel est mon solde sur \(.applicationName)",
                "Quel est mon solde avec \(.applicationName)",
                "Mon solde \(.applicationName)",
                "Solde \(.applicationName)",
                "Combien j'ai sur \(.applicationName)",
            ],
            shortTitle: "Solde",
            systemImageName: "eurosign.circle"
        )
        AppShortcut(
            intent: BudgetRemainingIntent(),
            phrases: [
                "Combien il me reste dans \(.applicationName)",
                "Combien il me reste sur \(.applicationName)",
                "Mon budget \(.applicationName)",
                "Budget \(.applicationName)",
            ],
            shortTitle: "Budget restant",
            systemImageName: "chart.bar.horizontal.page"
        )
        AppShortcut(
            intent: UpcomingScheduledIntent(),
            phrases: [
                "Mes prochaines échéances dans \(.applicationName)",
                "Prochaines échéances \(.applicationName)",
            ],
            shortTitle: "Échéances",
            systemImageName: "calendar.badge.clock"
        )
        AppShortcut(
            intent: MonthSummaryIntent(),
            phrases: [
                "Mon résumé du mois dans \(.applicationName)",
                "Résumé du mois \(.applicationName)",
            ],
            shortTitle: "Résumé du mois",
            systemImageName: "chart.pie"
        )
        AppShortcut(
            intent: AskDmxMoneyIntent(),
            phrases: [
                "Demander à \(.applicationName)",
                "Poser une question à \(.applicationName)",
            ],
            shortTitle: "Demander",
            systemImageName: "text.bubble"
        )
        AppShortcut(
            intent: AddTransactionIntent(),
            phrases: [
                "Ajouter \(\.$kind) dans \(.applicationName)",
                "Noter \(\.$kind) dans \(.applicationName)",
                "Ajouter une opération dans \(.applicationName)",
                "Nouvelle opération \(.applicationName)",
            ],
            shortTitle: "Ajouter une opération",
            systemImageName: "plus.circle"
        )
    }
}
