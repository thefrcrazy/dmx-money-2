import DmxKit
import SwiftUI

/// Budget du mois : indicateurs, consommation et enveloppes par catégorie.
struct ModernBudget: View {
    @ObservedObject var model: BudgetModel
    @EnvironmentObject private var store: AppStore

    var body: some View {
        PageBody {
            if let view = model.view {
                VStack(alignment: .leading, spacing: 16) {
                    header(view)
                    kpis(view)
                    consumption(view)
                    categories(view)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, minHeight: 200)
            }
        }
    }

    private func header(_ view: BudgetOverview) -> some View {
        HStack(spacing: 10) {
            if !view.suggestions.isEmpty {
                Button("Suggestions (\(view.suggestions.count))", systemImage: "sparkles") {
                    store.present(.budgetSuggestions)
                }
            }
            Text("\(view.monthLabel) · budgets configurés et dépenses du journal")
                .foregroundStyle(.secondary)
            Spacer(minLength: 0)
        }
    }

    private func kpis(_ view: BudgetOverview) -> some View {
        Grid(horizontalSpacing: 14, verticalSpacing: 14) {
            GridRow {
                kpi("Prévu", systemImage: "calendar", value: Money.format(view.totalBudgeted),
                    caption: "\(view.budgetCount) budget\(view.budgetCount > 1 ? "s" : "") configuré\(view.budgetCount > 1 ? "s" : "")", tint: .accentColor)
                kpi("Dépensé", systemImage: "arrow.down.right", value: Money.format(view.totalSpent),
                    caption: "\(view.expenseCount) dépense\(view.expenseCount > 1 ? "s" : "") ce mois-ci", tint: .red)
                kpi("Restant", systemImage: "wallet.bifold", value: Money.format(view.remaining),
                    caption: "\(Money.format(view.remainingPerDay)) / jour restant",
                    tint: view.remaining >= 0 ? .green : .red, valueColor: view.remaining >= 0 ? .green : .red)
                kpi("État", systemImage: stateIcon(view.state), value: budgetStateLabel(state: view.state),
                    caption: paceLabel(view), tint: stateColor(view.state), valueColor: stateColor(view.state))
            }
        }
    }

    private func kpi(_ label: String, systemImage: String, value: String, caption: String, tint: Color, valueColor: Color = .primary) -> some View {
        Card {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(label).font(.caption).textCase(.uppercase).foregroundStyle(.secondary)
                    Spacer()
                    Image(systemName: systemImage).foregroundStyle(tint)
                }
                Text(value).font(.title3.weight(.bold)).foregroundStyle(valueColor).lineLimit(1)
                Text(caption).font(.caption).foregroundStyle(.secondary).lineLimit(2)
            }
        }
    }

    private func consumption(_ view: BudgetOverview) -> some View {
        Card("Consommation du mois", systemImage: "gauge.with.needle", trailing: {
            Text("\(Int(view.progress.rounded())) % utilisé").foregroundStyle(.secondary)
        }) {
            VStack(alignment: .leading, spacing: 8) {
                ProgressView(value: min(view.progress, 100), total: 100)
                    .tint(view.progress > 100 ? .red : .green)
                HStack(spacing: 16) {
                    Text("\(Money.format(view.totalSpent)) dépensés")
                    Text("\(Money.format(view.totalBudgeted)) prévus")
                    Text("\(Money.format(view.expectedSpend)) théoriques au \(view.todayLabel)")
                    if view.unbudgetedCount > 0 {
                        Text("\(view.unbudgetedCount) catégorie\(view.unbudgetedCount > 1 ? "s" : "") non budgétée\(view.unbudgetedCount > 1 ? "s" : "")")
                            .foregroundStyle(.tint)
                    }
                    Spacer(minLength: 0)
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
        }
    }

    private func categories(_ view: BudgetOverview) -> some View {
        Card("Budget par catégorie", systemImage: "square.stack.3d.up", trailing: {
            HStack(spacing: 10) {
                CategoryFilterMenu(selection: $model.categories)
                Text("\(view.budgetedCategoryCount) catégorie\(view.budgetedCategoryCount > 1 ? "s" : "") budgétée\(view.budgetedCategoryCount > 1 ? "s" : "")")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }) {
            if view.categories.isEmpty {
                ContentUnavailableView {
                    Label("Aucun budget", systemImage: "chart.pie")
                } description: {
                    Text("Créez une enveloppe pour suivre une catégorie.")
                } actions: {
                    Button("Nouveau budget") { store.present(.budget(id: nil)) }
                }
                .frame(minHeight: 200)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(view.categories.enumerated()), id: \.element.category.id) { item in
                        if item.offset > 0 { Divider() }
                        categoryRow(item.element)
                    }
                }
            }
        }
    }

    private func categoryRow(_ row: BudgetCategoryRow) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                CategoryBadge(icon: row.category.icon, colorHex: row.category.color, size: 30)
                VStack(alignment: .leading, spacing: 1) {
                    Text(row.category.name).font(.headline)
                    Text(detail(row)).font(.caption).foregroundStyle(.secondary)
                }
                Spacer(minLength: 10)
                if row.isUnbudgeted {
                    Button("Créer un budget") { store.present(.newBudget(categoryId: row.category.id)) }
                        .buttonStyle(.bordered)
                } else {
                    Label(row.isOverBudget ? "Dépassé" : "OK", systemImage: row.isOverBudget ? "exclamationmark.triangle" : "checkmark.circle")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(row.isOverBudget ? .red : .green)
                }
            }
            ForEach(row.envelopes, id: \.budget.id) { envelope in
                envelopeRow(envelope, over: row.isOverBudget)
            }
        }
        .padding(.vertical, 10)
    }

    private func detail(_ row: BudgetCategoryRow) -> String {
        if row.isUnbudgeted {
            return "\(Money.format(row.spent)) dépensés · aucune enveloppe"
        }
        var parts = ["\(row.envelopes.count) enveloppe\(row.envelopes.count > 1 ? "s" : "")"]
        if row.linkedScheduledCount > 0 {
            parts.append("\(row.linkedScheduledCount) échéance\(row.linkedScheduledCount > 1 ? "s" : "") liée\(row.linkedScheduledCount > 1 ? "s" : "")")
        }
        return parts.joined(separator: " · ")
    }

    private func envelopeRow(_ envelope: BudgetEnvelope, over: Bool) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(envelope.budget.name).font(.callout.weight(.medium))
                    Text(envelope.accountName).font(.caption).foregroundStyle(.secondary)
                }
                Spacer(minLength: 8)
                LabeledContent("Prévu") { Text(Money.format(envelope.budget.amount)).monospacedDigit() }
                LabeledContent("Dépensé") { Text(Money.format(envelope.spent)).monospacedDigit() }
                LabeledContent("Restant") {
                    Text(Money.format(envelope.remaining))
                        .monospacedDigit()
                        .foregroundStyle(envelope.remaining < 0 ? .red : .green)
                }
                Button { store.present(.budget(id: envelope.budget.id)) } label: {
                    Image(systemName: "pencil")
                }
                .buttonStyle(.plain)
                .help("Modifier")
                Button { delete(envelope) } label: {
                    Image(systemName: "trash")
                }
                .buttonStyle(.plain)
                .foregroundStyle(.red)
                .help("Supprimer")
            }
            .font(.caption)
            ProgressView(value: min(envelope.progress, 100), total: 100)
                .tint(envelope.remaining < 0 ? .red : .green)
            if !envelope.linkedScheduled.isEmpty {
                Divider()
                ForEach(envelope.linkedScheduled, id: \.scheduledId) { linked in
                    HStack(spacing: 6) {
                        Image(systemName: "calendar.badge.clock").foregroundStyle(.secondary)
                        Text(linked.description).lineLimit(1)
                        Spacer(minLength: 6)
                        Text(DayFormat.short(linked.nextDate)).foregroundStyle(.secondary)
                        MoneyText(amount: linked.amount, signed: linked.transactionType, weight: .medium, size: .caption)
                    }
                    .font(.caption)
                }
            }
        }
        .padding(10)
        .background(Color(nsColor: .textBackgroundColor).opacity(0.6), in: RoundedRectangle(cornerRadius: 10))
    }

    private func delete(_ envelope: BudgetEnvelope) {
        store.confirm(
            title: "Supprimer « \(envelope.budget.name) » ?",
            message: "Les échéances liées à ce budget seront déliées."
        ) {
            store.run("Budget supprimé") { engine in try engine.deleteBudget(id: envelope.budget.id) }
        }
    }

    private func stateIcon(_ state: BudgetState) -> String {
        switch state {
        case .toConfigure: return "calendar.badge.clock"
        case .underControl: return "checkmark.circle"
        case .overrun: return "exclamationmark.triangle"
        }
    }

    private func stateColor(_ state: BudgetState) -> Color {
        switch state {
        case .toConfigure: return .secondary
        case .underControl: return .green
        case .overrun: return .red
        }
    }

    private func paceLabel(_ view: BudgetOverview) -> String {
        view.paceDelta > 0
            ? "\(Money.format(view.paceDelta)) au-dessus du rythme"
            : "\(Money.format(abs(view.paceDelta))) sous le rythme"
    }
}
