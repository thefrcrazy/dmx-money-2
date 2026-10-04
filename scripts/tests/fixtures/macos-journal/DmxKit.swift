import AppKit
import Foundation
import SwiftUI

public enum TransactionType: Sendable { case expense, income, transfer }
public struct Transaction: Sendable {
    public let id: String
    public let date: String
    public let description: String
    public let amount: Double
    public let transactionType: TransactionType
    public let checked: Bool
}
public struct Category: Sendable {
    public let name: String
    public let icon: String
    public let color: String
    public let id: String
}
public struct Budget: Sendable {
    public let remaining: Double
    public let budgetName: String
}
public struct JournalRow: Identifiable, Sendable {
    public var id: String { transaction.id }
    public let transaction: Transaction
    public let accountName: String
    public let accountColor: String
    public let category: Category
    public let budget: Budget?
    public let balance: Double
    public let displayType: TransactionType
}
public struct JournalView { public let rows: [JournalRow] }
public struct SelectOption {
    public let id: String
    public let label: String
    public let icon: String?
    public let color: String?
    public init(id: String, label: String, icon: String? = nil, color: String? = nil) {
        self.id = id
        self.label = label
        self.icon = icon
        self.color = color
    }
}
public final class AppStore: ObservableObject {
    @Published public var errorMessage: String?
    public var categories: [Category] = []
    public init() {}
    public enum Form { case transaction(id: String?) }
    public func present(_ form: Form) {}
}
public final class JournalModel: ObservableObject {
    @Published public var selection: Set<String> = []
    @Published public var rowsRevision: UInt64 = 1
    @Published public var isLoading = false
    @Published public var categories: [String] = []
    @Published public var types: [String] = []
    @Published public var statuses: [String] = []
    @Published public var budgetStatuses: [String] = []
    public var actions: [String] = []
    public var isActive = true
    public var onChange: (() -> Void)?
    public var hasFilters = false
    public var view: JournalView?
    public var rows: [JournalRow] { view?.rows ?? [] }
    public static let typeOptions: [SelectOption] = []
    public static let statusOptions: [SelectOption] = []
    public static let budgetOptions: [SelectOption] = []
    public init(count: Int) {
        let category = Category(name: "Catégorie fictive", icon: "Wallet", color: "#6366f1", id: "fiction")
        var rows: [JournalRow] = []
        rows.reserveCapacity(count)
        for i in 0..<count {
            let kind: TransactionType = i % 2 == 0 ? .expense : .income
            let transaction = Transaction(
                id: "fiction-\(i)", date: "2026-10-03", description: "Opération fictive \(i)",
                amount: Double(i % 500) + 0.37, transactionType: kind, checked: i % 3 == 0)
            let budget = Budget(remaining: Double(i % 100), budgetName: "Budget fictif")
            let row = JournalRow(
                transaction: transaction, accountName: "Compte fictif", accountColor: "#6366f1",
                category: category, budget: budget, balance: Double(i) * 0.27, displayType: kind)
            rows.append(row)
        }
        view = JournalView(rows: rows)
    }
    public func clearFilters() {}
    public func toggleCheckedSelection() {}
    public func deleteSelection() {}
    public func toggleChecked(_ id: String) { actions.append("check|" + id) }
    public func delete(_ id: String) {}
    public func updateDescription(_ id: String, _ text: String, baseDescription: String?) {
        actions.append("description|" + id + "|" + text)
    }
    public func updateAmount(_ id: String, text: String, baseAmount: Double?) -> Bool {
        actions.append("amount|" + id + "|" + text)
        return true
    }
}
public enum Money {
    public static func format(_ value: Double) -> String { String(format: "%.2f €", value) }
    public static func signed(_ value: Double, display: TransactionType, stored: TransactionType) -> String {
        (stored == .income ? "+" : "-") + format(value)
    }
    public static func signed(_ value: Double, kind: TransactionType) -> String {
        signed(value, display: kind, stored: kind)
    }
}
public enum DayFormat { public static func short(_ value: String) -> String { "03/10/2026" } }
public enum AmountInput {
    public static func parse(_ text: String) -> Double? { Double(text) }
    public static func text(_ value: Double, emptyWhenZero: Bool = false) -> String { String(value) }
}

public typealias PlatformColor = NSColor
extension NSColor { public static func dmx(hex: String) -> NSColor? { .systemIndigo } }
public enum DmxIcon {
    public static func image(_ name: String, size: CGFloat) -> NSImage? {
        NSImage(systemSymbolName: name == "Circle" ? "circle" : "checkmark.circle", accessibilityDescription: name)
    }
}
public struct StoreRoot<Content: View>: View {
    let store: AppStore
    let content: Content
    public init(store: AppStore, @ViewBuilder content: () -> Content) {
        self.store = store
        self.content = content()
    }
    public var body: some View { content.environmentObject(store) }
}
