import SwiftUI

public final class DashboardModel: PageModel {
    public private(set) var view: DashboardView? = nil {
        willSet { objectWillChange.send() }
    }

    public override init(store: AppStore) {
        super.init(store: store)
        refresh()
    }

    public override func refresh() {
        let accounts = store.selectedAccountIds
        let today = store.today
        view = store.read { engine in try engine.dashboard(accounts: accounts, today: today) }
    }
}

/// Vue d'ensemble : six cartes, comme le tableau de bord de 1.x.
public struct DashboardPage: View {
    @ObservedObject private var model: DashboardModel
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact

    public init(model: DashboardModel) {
        self.model = model
    }

    public var body: some View {
        PageScroll {
            if !compact {
                Text("Tableau de Bord").font(.system(size: 24, weight: .bold))
            }
            if let view = model.view {
                CardRow({ upcomingCard(view) }, { operationsCard(view) }, { categoriesCard(view) })
                CardRow({ accountsCard(view) }, { recentCard(view) }, { budgetCard(view) })
            }
        }
    }

    // MARK: Échéances

    private func upcomingCard(_ view: DashboardView) -> some View {
        DmxCard(title: "Échéances", icon: "CalendarClock", accessory: {
            LinkButton("Tout voir") { store.route = .scheduled }
        }) {
            VStack(spacing: 14) {
                if view.upcoming.isEmpty {
                    EmptyStateView(icon: "CheckCircle2", title: "Aucune échéance à venir")
                } else {
                    ForEach(view.upcoming, id: \.scheduledId) { item in
                        HStack(spacing: 12) {
                            IconCircle(
                                icon: item.daysUntil < 0 ? "AlertCircle" : "Clock",
                                color: item.daysUntil < 0 ? DmxColors.expense : DmxColors.income
                            )
                            VStack(alignment: .leading, spacing: 1) {
                                Text(item.description).font(.system(size: 13, weight: .medium)).lineLimit(1)
                                Text(DayFormat.relative(days: item.daysUntil)).font(.system(size: 10)).foregroundColor(.secondary)
                            }
                            Spacer(minLength: 8)
                            Text(Money.format(item.amount)).font(.system(size: 13, weight: .bold))
                        }
                    }
                }
            }
            .frame(minHeight: 140)
        }
    }

    // MARK: Opérations

    private func operationsCard(_ view: DashboardView) -> some View {
        DmxCard(title: "Opérations", icon: "ArrowRightLeft", accessory: { Pill("Ce mois-ci") }) {
            VStack(spacing: 0) {
                SectionLabel("Revenus - Dépenses").padding(.bottom, 4)
                Text((view.month.saved > 0 ? "+" : "") + Money.rounded(view.month.saved))
                    .font(.system(size: 34, weight: .bold))
                    .foregroundColor(view.month.saved >= 0 ? DmxColors.income : DmxColors.expense)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                    .padding(.bottom, 18)
                Divider()
                HStack(spacing: 8) {
                    miniStat(icon: "TrendingUp", iconColor: DmxColors.income, label: "Revenus", value: Money.rounded(view.month.income))
                    miniStat(icon: "TrendingDown", iconColor: DmxColors.expense, label: "Dépenses", value: Money.rounded(view.month.expenses))
                    miniStat(icon: "Wallet", iconColor: .accentColor, label: "Épargné", value: Money.rounded(view.month.saved),
                             valueColor: view.month.saved >= 0 ? DmxColors.income : DmxColors.expense)
                }
                .padding(.top, 14)
            }
            .frame(maxWidth: .infinity, minHeight: 140)
        }
    }

    private func miniStat(icon: String, iconColor: Color, label: String, value: String, valueColor: Color = .primary) -> some View {
        VStack(spacing: 2) {
            HStack(spacing: 4) {
                DmxIcon(icon, size: 11).foregroundColor(iconColor)
                Text(label).font(.system(size: 10)).foregroundColor(.secondary)
            }
            Text(value).font(.system(size: 12, weight: .bold)).foregroundColor(valueColor).lineLimit(1).minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: Catégories

    private func categoriesCard(_ view: DashboardView) -> some View {
        DmxCard(title: "Catégories", icon: "Tag", accessory: { Pill("Ce mois-ci") }) {
            VStack(spacing: 12) {
                if view.topCategories.isEmpty {
                    Text("Aucune dépense ce mois-ci")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .frame(maxWidth: .infinity, minHeight: 140)
                } else {
                    ForEach(view.topCategories.prefix(3), id: \.category.id) { share in
                        HStack(spacing: 8) {
                            Circle().fill(Color(hex: share.category.color)).frame(width: 8, height: 8)
                            Text(share.category.name).font(.system(size: 13)).lineLimit(1)
                            Spacer(minLength: 8)
                            Text("\(Int(share.percentage.rounded()))%").font(.system(size: 13, weight: .semibold))
                        }
                    }
                    Spacer(minLength: 0)
                }
            }
            .frame(minHeight: 140)
        }
    }

    // MARK: Mes Comptes

    private func accountsCard(_ view: DashboardView) -> some View {
        DmxCard(title: "Mes Comptes", icon: "Wallet", padded: false, accessory: {
            IconButton("Plus", color: .accentColor) { store.present(.account(id: nil)) }
        }) {
            VStack(spacing: 0) {
                ForEach(Array(view.accounts.enumerated()), id: \.element.account.id) { item in
                    if item.offset > 0 { Divider().padding(.leading, 16) }
                    Button(action: { openJournal(accountId: item.element.account.id) }) {
                        HStack(spacing: 12) {
                            IconBadge(icon: item.element.account.icon, colorHex: item.element.account.color, size: 32)
                            Text(item.element.account.name).font(.system(size: 13, weight: .medium)).lineLimit(1)
                            Spacer(minLength: 8)
                            Text(Money.format(item.element.balance))
                                .font(.system(size: 13, weight: .bold))
                                .foregroundColor(item.element.balance < 0 ? DmxColors.expense : .primary)
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 10)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(PlainButtonStyle())
                }
                if view.accounts.isEmpty {
                    EmptyStateView(icon: "Wallet", title: "Aucun compte configuré")
                }
                Spacer(minLength: 0)
                Divider()
                HStack {
                    SectionLabel("Total")
                    Spacer()
                    Text(Money.format(view.accountsTotal)).font(.system(size: 17, weight: .bold)).foregroundColor(.accentColor)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
            }
            .frame(minHeight: 200)
        }
    }

    private func openJournal(accountId: String) {
        store.selectedAccountIds = [accountId]
        store.route = .transactions
    }

    // MARK: Dernières

    private func recentCard(_ view: DashboardView) -> some View {
        DmxCard(title: "Dernières", icon: "Receipt", padded: false, accessory: {
            LinkButton("Tout voir") { store.route = .transactions }
        }) {
            VStack(spacing: 0) {
                if view.recentTransactions.isEmpty {
                    EmptyStateView(icon: "Receipt", title: "Aucune transaction")
                }
                ForEach(Array(view.recentTransactions.enumerated()), id: \.element.transaction.id) { item in
                    if item.offset > 0 { Divider().padding(.leading, 16) }
                    HStack(spacing: 12) {
                        IconCircle(icon: item.element.category.icon, color: Color(hex: item.element.category.color, fallback: DmxColors.muted))
                        VStack(alignment: .leading, spacing: 1) {
                            Text(item.element.transaction.description).font(.system(size: 13, weight: .medium)).lineLimit(1)
                            Text(DayFormat.short(item.element.transaction.date)).font(.system(size: 10)).foregroundColor(.secondary)
                        }
                        Spacer(minLength: 8)
                        Text(Money.signed(item.element.transaction.amount, kind: item.element.transaction.transactionType, showMinus: false))
                            .font(.system(size: 13, weight: .bold))
                            .foregroundColor(item.element.transaction.transactionType == .income ? DmxColors.income : .primary)
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                }
                Spacer(minLength: 0)
            }
            .frame(minHeight: 200)
        }
    }

    // MARK: Budget

    private func budgetCard(_ view: DashboardView) -> some View {
        let gauge = view.budget
        return DmxCard(title: "Budget", icon: "DollarSign", accessory: { Pill("Ce mois-ci") }) {
            VStack(spacing: 16) {
                DonutChart(slices: [
                    DonutSlice(id: "spent", label: "Dépenses", value: gauge.spent, color: gauge.remaining >= 0 ? DmxColors.income : DmxColors.expense),
                    DonutSlice(id: "remaining", label: "Restant", value: max(gauge.remaining, 0), color: Color.primary.opacity(0.1)),
                ], thickness: 10) {
                    VStack(spacing: 0) {
                        SectionLabel("Dépenses")
                        Text("\(Int(gauge.progress.rounded()))%").font(.system(size: 18, weight: .bold))
                    }
                }
                .frame(width: 112, height: 112)

                HStack(spacing: 4) {
                    budgetFigure("Restant", Money.rounded(gauge.remaining), gauge.remaining < 0 ? DmxColors.expense : .primary)
                    budgetFigure("Dépenses", Money.rounded(gauge.spent), DmxColors.income)
                    budgetFigure("Prévu", Money.rounded(gauge.totalBudgeted), .primary)
                }
            }
            .frame(maxWidth: .infinity, minHeight: 200)
        }
    }

    private func budgetFigure(_ label: String, _ value: String, _ color: Color) -> some View {
        VStack(spacing: 2) {
            SectionLabel(label)
            Text(value).font(.system(size: 12, weight: .bold)).foregroundColor(color).lineLimit(1).minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity)
    }
}
