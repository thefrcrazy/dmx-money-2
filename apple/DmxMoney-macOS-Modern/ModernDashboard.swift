import Charts
import DmxKit
import SwiftUI

/// Vue d'ensemble : six cartes natives, valeurs calculées par le noyau.
struct ModernDashboard: View {
    @ObservedObject var model: DashboardModel
    @EnvironmentObject private var store: AppStore

    var body: some View {
        PageBody {
            if let view = model.view {
                // Hauteur plancher commune : les six cartes gardent la même taille, comme en 1.x.
                Grid(horizontalSpacing: 16, verticalSpacing: 16) {
                    GridRow {
                        upcoming(view)
                        month(view)
                        categories(view)
                    }
                    .frame(minHeight: 268)
                    GridRow {
                        accounts(view)
                        recent(view)
                        budget(view)
                    }
                    .frame(minHeight: 268)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, minHeight: 200)
            }
        }
    }

    // MARK: Échéances

    private func upcoming(_ view: DashboardView) -> some View {
        Card("Échéances", systemImage: "calendar.badge.clock", trailing: {
            Button("Tout voir") { store.route = .scheduled }
                .buttonStyle(.link)
        }) {
            if view.upcoming.isEmpty {
                ContentUnavailableView("Aucune échéance à venir", systemImage: "checkmark.circle")
                    .frame(height: 150)
            } else {
                VStack(spacing: 10) {
                    ForEach(view.upcoming, id: \.scheduledId) { item in
                        HStack(spacing: 10) {
                            Image(systemName: item.daysUntil < 0 ? "exclamationmark.circle" : "clock")
                                .foregroundStyle(item.daysUntil < 0 ? .red : .green)
                                .frame(width: 26, height: 26)
                                .background((item.daysUntil < 0 ? Color.red : Color.green).opacity(0.14), in: Circle())
                            VStack(alignment: .leading, spacing: 1) {
                                Text(item.description).lineLimit(1)
                                Text(DayFormat.relative(days: item.daysUntil))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer(minLength: 8)
                            MoneyText(amount: item.amount)
                        }
                    }
                }
            }
        }
    }

    // MARK: Opérations du mois

    private func month(_ view: DashboardView) -> some View {
        Card("Opérations", systemImage: "arrow.left.arrow.right", trailing: {
            Text("Ce mois-ci").font(.caption).foregroundStyle(.secondary)
        }) {
            VStack(spacing: 12) {
                VStack(spacing: 2) {
                    Text("Revenus − dépenses")
                        .font(.caption).textCase(.uppercase).foregroundStyle(.secondary)
                    MoneyText(amount: view.month.saved, weight: .bold, size: .largeTitle)
                }
                .frame(maxWidth: .infinity)
                Divider()
                HStack {
                    figure("Revenus", systemImage: "arrow.up.right", value: view.month.income, color: .green)
                    figure("Dépenses", systemImage: "arrow.down.right", value: view.month.expenses, color: .red)
                    figure("Épargné", systemImage: "banknote", value: view.month.saved, color: view.month.saved >= 0 ? .green : .red)
                }
            }
        }
    }

    private func figure(_ label: String, systemImage: String, value: Double, color: Color) -> some View {
        VStack(spacing: 3) {
            Label(label, systemImage: systemImage)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(Money.format(value))
                .font(.callout.weight(.semibold).monospacedDigit())
                .foregroundStyle(color)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: Top catégories

    private func categories(_ view: DashboardView) -> some View {
        Card("Catégories", systemImage: "tag", trailing: {
            Text("Ce mois-ci").font(.caption).foregroundStyle(.secondary)
        }) {
            if view.topCategories.isEmpty {
                Text("Aucune dépense ce mois-ci").foregroundStyle(.secondary)
            } else {
                VStack(spacing: 10) {
                    ForEach(view.topCategories, id: \.category.id) { share in
                        HStack(spacing: 10) {
                            Circle()
                                .fill(Color(hex: share.category.color, fallback: .secondary))
                                .frame(width: 9, height: 9)
                            Text(share.category.name).lineLimit(1)
                            Spacer(minLength: 8)
                            Text("\(Int(share.percentage.rounded()))%")
                                .font(.callout.weight(.semibold).monospacedDigit())
                        }
                    }
                }
            }
        }
    }

    // MARK: Comptes

    private func accounts(_ view: DashboardView) -> some View {
        Card("Mes Comptes", systemImage: "creditcard", trailing: {
            Button { store.present(.account(id: nil)) } label: {
                Image(systemName: "plus")
            }
            .buttonStyle(.plain)
            .help("Nouveau compte")
        }) {
            if view.accounts.isEmpty {
                ContentUnavailableView("Aucun compte configuré", systemImage: "creditcard")
                    .frame(height: 150)
            } else {
                VStack(spacing: 10) {
                    ForEach(view.accounts, id: \.account.id) { item in
                        Button {
                            store.route = .accounts
                        } label: {
                            HStack(spacing: 10) {
                                CategoryBadge(icon: item.account.icon, colorHex: item.account.color)
                                Text(item.account.name).lineLimit(1)
                                Spacer(minLength: 8)
                                MoneyText(amount: item.balance)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                    Divider()
                    HStack {
                        Text("Total").font(.caption).textCase(.uppercase).foregroundStyle(.secondary)
                        Spacer()
                        MoneyText(amount: view.accountsTotal, weight: .bold)
                    }
                }
            }
        }
    }

    // MARK: Dernières opérations

    private func recent(_ view: DashboardView) -> some View {
        Card("Dernières", systemImage: "list.bullet", trailing: {
            Button("Tout voir") { store.route = .transactions }
                .buttonStyle(.link)
        }) {
            if view.recentTransactions.isEmpty {
                ContentUnavailableView("Aucune opération", systemImage: "list.bullet")
                    .frame(height: 150)
            } else {
                VStack(spacing: 10) {
                    ForEach(view.recentTransactions, id: \.transaction.id) { item in
                        Button {
                            store.present(.transaction(id: item.transaction.id))
                        } label: {
                            HStack(spacing: 10) {
                                CategoryBadge(icon: item.category.icon, colorHex: item.category.color)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(item.transaction.description.isEmpty ? item.category.name : item.transaction.description)
                                        .lineLimit(1)
                                    Text(DayFormat.short(item.transaction.date))
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer(minLength: 8)
                                MoneyText(
                                    amount: item.transaction.amount,
                                    signed: item.category.id == "transfer" ? .transfer : item.transaction.transactionType,
                                    stored: item.transaction.transactionType
                                )
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    // MARK: Budget

    private func budget(_ view: DashboardView) -> some View {
        Card("Budget", systemImage: "chart.pie.fill", trailing: {
            Text("Ce mois-ci").font(.caption).foregroundStyle(.secondary)
        }) {
            VStack(spacing: 12) {
                Chart {
                    SectorMark(
                        angle: .value("Dépensé", max(view.budget.spent, 0)),
                        innerRadius: .ratio(0.72),
                        angularInset: 1.5
                    )
                    .foregroundStyle(view.budget.progress > 100 ? Color.red : Color.green)
                    SectorMark(
                        angle: .value("Restant", max(view.budget.remaining, 0)),
                        innerRadius: .ratio(0.72),
                        angularInset: 1.5
                    )
                    .foregroundStyle(Color.secondary.opacity(0.22))
                }
                .chartLegend(.hidden)
                .frame(height: 150)
                .overlay {
                    VStack(spacing: 0) {
                        Text("Dépenses").font(.caption2).textCase(.uppercase).foregroundStyle(.secondary)
                        Text("\(Int(view.budget.progress.rounded()))%").font(.title3.weight(.bold))
                    }
                }
                HStack {
                    figure("Restant", systemImage: "wallet.bifold", value: view.budget.remaining, color: view.budget.remaining >= 0 ? .green : .red)
                    figure("Dépensé", systemImage: "arrow.down.right", value: view.budget.spent, color: .primary)
                    figure("Prévu", systemImage: "calendar", value: view.budget.totalBudgeted, color: .primary)
                }
            }
        }
    }
}
