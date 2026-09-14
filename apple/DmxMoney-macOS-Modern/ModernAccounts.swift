import DmxKit
import SwiftUI
import UniformTypeIdentifiers

/// Mes Comptes : sections par groupe, cartes natives, glisser-déposer pour réordonner.
struct ModernAccounts: View {
    @ObservedObject var model: AccountsModel
    @EnvironmentObject private var store: AppStore
    @State private var dragged: String?

    var body: some View {
        PageBody {
            VStack(alignment: .leading, spacing: 18) {
                header
                if let view = model.view {
                    if view.groups.isEmpty {
                        ContentUnavailableView {
                            Label("Aucun compte", systemImage: "creditcard")
                        } description: {
                            Text(model.search.isEmpty ? "Créez votre premier compte." : "Aucun compte ne correspond à la recherche.")
                        } actions: {
                            Button("Nouveau compte") { store.present(.account(id: nil)) }
                        }
                        .frame(minHeight: 260)
                    }
                    ForEach(view.groups, id: \.name) { group in
                        section(group)
                    }
                }
            }
        }
    }

    private var header: some View {
        HStack(spacing: 10) {
            Button("Nouveau compte", systemImage: "plus") { store.present(.account(id: nil)) }
                .buttonStyle(.borderedProminent)
            Button("Gérer les groupes", systemImage: "folder") { store.present(.accountGroups) }
            Spacer(minLength: 12)
            HStack(spacing: 5) {
                Image(systemName: "creditcard")
                Picker("Type", selection: typeBinding) {
                    Text("Tous les types").tag("")
                    ForEach(accountTypes(), id: \.self) { type in
                        Text(type).tag(type)
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .fixedSize()
            }
            if let view = model.view, view.visibleCount != view.totalCount {
                Text("\(view.visibleCount) / \(view.totalCount)").foregroundStyle(.secondary).monospacedDigit()
            }
        }
    }

    private var typeBinding: Binding<String> {
        Binding(
            get: { model.types.first ?? "" },
            set: { model.types = $0.isEmpty ? [] : [$0] }
        )
    }

    private func section(_ group: AccountGroupSection) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                Image(systemName: group.isUngrouped ? "tray" : "folder")
                    .foregroundStyle(.secondary)
                Text(group.name).font(.headline)
                Text("\(group.accounts.count)")
                    .font(.caption.monospacedDigit())
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(.quaternary, in: Capsule())
            }
            .contentShape(Rectangle())
                .draggable(GroupTransfer(name: group.isUngrouped ? "" : group.name)) {
                dragPreview(group.name, icon: "Folder", color: "#6366f1")
            }
            .dropDestination(for: GroupTransfer.self) { items, _ in
                // Un groupe depose sur un autre prend sa place dans l'ordre.
                guard let moved = items.first, !moved.name.isEmpty, !group.isUngrouped, moved.name != group.name else {
                    return false
                }
                store.run { engine in try engine.moveGroup(group: moved.name, overGroup: group.name) }
                return true
            }
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 280), spacing: 14)], spacing: 14) {
                ForEach(Array(group.accounts.enumerated()), id: \.element.account.id) { item in
                    accountCard(item.element, group: group, at: item.offset)
                }
            }
        }
        .dropDestination(for: String.self) { items, _ in
            guard let moved = items.first else { return false }
            store.run { engine in
                try engine.moveAccount(accountId: moved, target: .onGroup(group: group.isUngrouped ? nil : group.name))
            }
            return true
        }
    }

    private func accountCard(_ card: AccountCard, group: AccountGroupSection, at index: Int) -> some View {
        Card {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 10) {
                    CategoryBadge(icon: card.account.icon, colorHex: card.account.color, size: 34)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(card.account.name).font(.headline).lineLimit(1)
                        Text(card.account.accountType).font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 6)
                    // Le glisser-déposer marche toujours, mais il n'est plus le seul moyen :
                    // l'ordre et le groupe se changent aussi depuis ce menu.
                    Menu {
                        Button("Déplacer avant", systemImage: "arrow.left") {
                            move(card, to: group.accounts[index - 1].account.id)
                        }
                        .disabled(index == 0)
                        Button("Déplacer après", systemImage: "arrow.right") {
                            move(card, to: group.accounts[index + 1].account.id)
                        }
                        .disabled(index >= group.accounts.count - 1)
                        Divider()
                        Menu("Déplacer vers le groupe") {
                            ForEach(groupNames, id: \.self) { name in
                                Button(name.isEmpty ? "Non groupés" : name) { move(card, toGroup: name) }
                                    .disabled(name == (group.isUngrouped ? "" : group.name))
                            }
                        }
                    } label: {
                        Image(systemName: "line.3.horizontal")
                    }
                    .menuStyle(.borderlessButton)
                    .menuIndicator(.hidden)
                    .fixedSize()
                    .help("Réordonner ou changer de groupe")
                    Button { store.present(.account(id: card.account.id)) } label: {
                        Image(systemName: "pencil")
                    }
                    .buttonStyle(.plain)
                    .help("Modifier")
                    Button { delete(card) } label: {
                        Image(systemName: "trash")
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.red)
                    .help("Supprimer")
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text("Solde actuel").font(.caption).textCase(.uppercase).foregroundStyle(.secondary)
                    MoneyText(amount: card.currentBalance, weight: .bold, size: .title2)
                }
                Divider()
                LabeledContent("Solde pointé") {
                    Text(Money.format(card.checkedBalance)).monospacedDigit().foregroundStyle(.secondary)
                }
                .font(.caption)
            }
        }
        .contextMenu {
            Button("Modifier") { store.present(.account(id: card.account.id)) }
            Button("Supprimer", role: .destructive) { delete(card) }
        }
        .onTapGesture(count: 2) { store.present(.account(id: card.account.id)) }
        .draggable(card.account.id) { dragPreview(card.account.name, icon: card.account.icon, color: card.account.color) }
        .dropDestination(for: String.self) { items, _ in
            guard let moved = items.first, moved != card.account.id else { return false }
            store.run { engine in
                try engine.moveAccount(accountId: moved, target: .onAccount(accountId: card.account.id))
            }
            return true
        }
    }

    /// Vignette portée par le curseur pendant un glisser-déposer : opaque et bordée, sinon
    /// elle est invisible sur un fond clair.
    private func dragPreview(_ name: String, icon: String, color: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: Symbols.name(for: icon))
                .foregroundStyle(Color(hex: color, fallback: .accentColor))
            Text(name).fontWeight(.medium)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(Color(nsColor: .controlBackgroundColor), in: Capsule())
        .overlay(Capsule().stroke(Color(hex: color, fallback: .accentColor), lineWidth: 1))
    }

    /// Groupes existants, « Non groupés » compris (chaîne vide).
    private var groupNames: [String] {
        [""] + store.settings.effectiveGroupOrder.filter { $0 != "Non groupés" }
    }

    private func move(_ card: AccountCard, to accountId: String) {
        store.run { engine in
            try engine.moveAccount(accountId: card.account.id, target: .onAccount(accountId: accountId))
        }
    }

    private func move(_ card: AccountCard, toGroup group: String) {
        store.run(group.isEmpty ? "Compte retiré de son groupe" : "Compte déplacé dans « \(group) »") { engine in
            try engine.moveAccount(accountId: card.account.id, target: .onGroup(group: group.isEmpty ? nil : group))
        }
    }

    private func delete(_ card: AccountCard) {
        store.confirm(
            title: "Supprimer « \(card.account.name) » ?",
            message: "Les opérations du compte seront supprimées et les virements liés déliés."
        ) {
            store.run("Compte supprimé") { engine in try engine.deleteAccount(id: card.account.id) }
        }
    }
}


/// Glisser-deposer d'un groupe de comptes (distinct du glisser-deposer d'un compte).
struct GroupTransfer: Codable, Transferable {
    let name: String

    static var transferRepresentation: some TransferRepresentation {
        CodableRepresentation(contentType: .dmxAccountGroup)
    }
}

extension UTType {
    static let dmxAccountGroup = UTType(exportedAs: "com.dmxmoney.account-group")
}
