import DmxKit
import SwiftUI

/// En-tête mobile : solde actuel et pointé, puis filtre de comptes en pastilles (comme la PWA).
struct MobileHeader: View {
    @EnvironmentObject private var store: AppStore
    let route: AppRoute

    var body: some View {
        VStack(spacing: 10) {
            if route.showsBalances {
                HStack {
                    VStack(alignment: .leading, spacing: 1) {
                        SectionLabel("Solde Actuel")
                        Text(Money.format(store.balances.currentBalance))
                            .font(.system(size: 21, weight: .heavy))
                            .lineLimit(1)
                            .minimumScaleFactor(0.6)
                    }
                    Spacer()
                    HStack(spacing: 5) {
                        Text("POINTÉ").font(.system(size: 8, weight: .heavy))
                        Text(Money.format(store.balances.checkedBalance)).font(.system(size: 12, weight: .bold).monospacedDigit())
                    }
                    .foregroundColor(DmxColors.checked)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(DmxColors.income.opacity(0.12)))
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
                .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(DmxPalette.cardBackground))
                .padding(.horizontal, 16)
            }
            if route.usesAccountFilter && !store.accounts.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        chip(label: "Tous", icon: nil, color: .accentColor, isSelected: store.selectedAccountIds.isEmpty) {
                            store.clearAccountFilter()
                        }
                        ForEach(store.accounts, id: \.id) { account in
                            chip(
                                label: account.name,
                                icon: account.icon,
                                color: Color(hex: account.color, fallback: .accentColor),
                                isSelected: store.selectedAccountIds.contains(account.id)
                            ) {
                                store.toggleAccountFilter(account.id)
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                }
            }
        }
        .padding(.vertical, 8)
    }

    private func chip(label: String, icon: String?, color: Color, isSelected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 5) {
                if let icon {
                    DmxIcon(icon, size: 13)
                }
                Text(label.uppercased()).font(.system(size: 10, weight: .heavy)).lineLimit(1)
            }
            .foregroundColor(isSelected ? .white : .secondary)
            .padding(.horizontal, 11)
            .padding(.vertical, 7)
            .background(Capsule().fill(isSelected ? color : Color.primary.opacity(0.06)))
        }
        .buttonStyle(.plain)
    }
}

/// Journal mobile : opérations groupées par jour, balayage pour pointer, modifier ou supprimer.
struct JournalListPage: View {
    @EnvironmentObject private var store: AppStore
    @ObservedObject var model: JournalModel
    @State private var editMode: EditMode = .inactive

    var body: some View {
        List(selection: $model.selection) {
            Section {
                summary
                filters
            }
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
            .selectionDisabled()

            if let view = model.view, !view.rows.isEmpty {
                ForEach(view.dayGroups, id: \.date) { group in
                    Section {
                        ForEach(rows(view, group), id: \.transaction.id) { row in
                            rowView(row).tag(row.transaction.id)
                        }
                    } header: {
                        HStack {
                            Text(group.label)
                            Spacer()
                            Text("\(group.net >= 0 ? "+" : "")\(Money.format(group.net))")
                                .foregroundColor(group.net >= 0 ? DmxColors.income : DmxColors.expense)
                        }
                        .font(.caption.weight(.bold))
                    }
                }
            } else {
                Section {
                    EmptyStateView(
                        icon: "Search",
                        title: "Aucune transaction",
                        message: model.hasFilters ? "Aucun résultat pour les filtres actuels." : "Ajoute une transaction pour commencer."
                    )
                }
                .selectionDisabled()
            }
        }
        .listStyle(.insetGrouped)
        .searchable(text: $model.search, prompt: "Rechercher...")
        .environment(\.editMode, $editMode)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button(editMode.isEditing ? "OK" : "Sélectionner") {
                    withAnimation {
                        editMode = editMode.isEditing ? .inactive : .active
                        if !editMode.isEditing { model.selection = [] }
                    }
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button { store.present(.transaction(id: nil)) } label: { Image(systemName: "plus") }
            }
            if editMode.isEditing {
                ToolbarItemGroup(placement: .bottomBar) {
                    Button("Pointer/Dépointer", action: model.toggleCheckedSelection)
                        .disabled(model.selection.isEmpty)
                    Spacer()
                    Text("\(model.selection.count) \(model.selection.count > 1 ? "sélectionnées" : "sélectionnée")").font(.footnote)
                    Spacer()
                    Button("Supprimer", role: .destructive, action: model.deleteSelection)
                        .disabled(model.selection.isEmpty)
                }
            }
        }
    }

    private func rows(_ view: JournalView, _ group: JournalDayGroup) -> ArraySlice<JournalRow> {
        let start = Int(group.startIndex)
        let end = min(start + Int(group.count), view.rows.count)
        return view.rows[start..<end]
    }

    private var summary: some View {
        HStack(spacing: 8) {
            tile("Lignes", "\(model.view?.rows.count ?? 0)", color: .primary)
            tile("Filtres", "\(model.view?.activeFilterCount ?? 0)", color: .primary)
            let net = model.view?.visibleNet ?? 0
            tile("Net", "\(net >= 0 ? "+" : "")\(Money.format(net))", color: net >= 0 ? DmxColors.income : DmxColors.expense)
        }
    }

    private func tile(_ label: String, _ value: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            SectionLabel(label)
            Text(value).font(.system(size: 17, weight: .bold)).foregroundColor(color).lineLimit(1).minimumScaleFactor(0.6)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(DmxPalette.cardBackground))
    }

    private var filters: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                MultiSelectButton(
                    "Catégories",
                    options: store.categories.map { SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color) },
                    selection: $model.categories
                )
                MultiSelectButton("Types", options: JournalModel.typeOptions, selection: $model.types)
                MultiSelectButton("États", options: JournalModel.statusOptions, selection: $model.statuses)
                MultiSelectButton("Budgets", options: JournalModel.budgetOptions, selection: $model.budgetStatuses)
            }
        }
    }

    private func rowView(_ row: JournalRow) -> some View {
        let transaction = row.transaction
        let isIncome = transaction.transactionType == .income
        let categoryColor = Color(hex: row.category.color, fallback: DmxColors.muted)
        return HStack(alignment: .top, spacing: 12) {
            IconBadge(icon: row.category.icon, colorHex: row.category.color, size: 40)
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(transaction.description.isEmpty ? row.category.name : transaction.description)
                        .font(.system(size: 15, weight: .semibold))
                        .lineLimit(1)
                    Spacer(minLength: 4)
                    Text(Money.signed(transaction.amount, kind: transaction.transactionType))
                        .font(.system(size: 15, weight: .bold).monospacedDigit())
                        .foregroundColor(isIncome ? DmxColors.income : DmxColors.expense)
                }
                HStack(spacing: 6) {
                    Text(row.accountName).lineLimit(1)
                    Circle().fill(Color.secondary.opacity(0.4)).frame(width: 3, height: 3)
                    Text(row.category.name.uppercased())
                        .font(.system(size: 9, weight: .bold))
                        .foregroundColor(categoryColor)
                        .lineLimit(1)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Capsule().fill(categoryColor.opacity(0.12)))
                    if let budget = row.budget {
                        Text("\(Money.format(budget.remaining)) restant")
                            .foregroundColor(.indigo)
                            .fontWeight(.semibold)
                            .lineLimit(1)
                    }
                }
                .font(.system(size: 12))
                .foregroundColor(.secondary)
            }
            if !editMode.isEditing {
                Button { model.toggleChecked(transaction.id) } label: {
                    DmxIcon(transaction.checked ? "CheckCircle2" : "Circle", size: 20)
                        .foregroundColor(transaction.checked ? DmxColors.income : .secondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
        .onTapGesture {
            if !editMode.isEditing { store.present(.transaction(id: transaction.id)) }
        }
        .swipeActions(edge: .leading) {
            Button { model.toggleChecked(transaction.id) } label: {
                Label(transaction.checked ? "Dépointer" : "Pointer", systemImage: transaction.checked ? "circle" : "checkmark.circle")
            }
            .tint(DmxColors.income)
        }
        .swipeActions(edge: .trailing) {
            Button { model.delete(transaction.id) } label: { Label("Supprimer", systemImage: "trash") }
                .tint(DmxColors.expense)
            Button { store.present(.transaction(id: transaction.id)) } label: { Label("Modifier", systemImage: "pencil") }
                .tint(.indigo)
        }
    }
}
