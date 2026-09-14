import SwiftUI

public final class AccountsModel: PageModel {
    public var search = "" {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public var types: [String] = [] {
        willSet { objectWillChange.send() }
        didSet { refresh() }
    }

    public private(set) var view: AccountsView? = nil {
        willSet { objectWillChange.send() }
    }

    public override init(store: AppStore) {
        super.init(store: store)
        refresh()
    }

    public override func refresh() {
        let query = AccountsQuery(search: search, types: types)
        view = store.read { engine in try engine.accounts(query: query) }
    }
}

/// Élément glissé : un compte ou un groupe.
enum AccountDragPayload {
    case account(String)
    case group(String)

    static let typeIdentifier = "public.text"

    init?(_ string: String) {
        if string.hasPrefix("account:") {
            self = .account(String(string.dropFirst("account:".count)))
        } else if string.hasPrefix("group:") {
            self = .group(String(string.dropFirst("group:".count)))
        } else {
            return nil
        }
    }

    var string: String {
        switch self {
        case let .account(id): return "account:\(id)"
        case let .group(name): return "group:\(name)"
        }
    }

    var itemProvider: NSItemProvider {
        NSItemProvider(object: string as NSString)
    }

    static func load(_ providers: [NSItemProvider], perform: @escaping (AccountDragPayload) -> Void) -> Bool {
        guard let provider = providers.first else { return false }
        provider.loadObject(ofClass: NSString.self) { object, _ in
            guard let text = object as? String, let payload = AccountDragPayload(text) else { return }
            DispatchQueue.main.async { perform(payload) }
        }
        return true
    }
}

/// Mes Comptes : groupes personnalisés, cartes de soldes, glisser-déposer des comptes et des groupes.
public struct AccountsPage: View {
    @ObservedObject private var model: AccountsModel
    @EnvironmentObject private var store: AppStore
    @Environment(\.dmxCompact) private var compact
    @State private var targetedGroup: String?

    public init(model: AccountsModel) {
        self.model = model
    }

    public var body: some View {
        PageScroll {
            PageHeader("Mes Comptes") {
                HStack(spacing: 8) {
                    Button(action: { store.present(.accountGroups) }) {
                        HStack(spacing: 6) {
                            DmxIcon("Settings", size: 13)
                            Text("Gérer les groupes")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(.secondary))
                    Button(action: { store.present(.account(id: nil)) }) {
                        HStack(spacing: 6) {
                            DmxIcon("Plus", size: 13)
                            Text("Nouveau compte")
                        }
                    }
                    .buttonStyle(DmxButtonStyle(.primary))
                }
            }

            HStack(spacing: 10) {
                SearchField("Rechercher un compte...", text: $model.search)
                    .frame(maxWidth: compact ? .infinity : 360)
                MultiSelectButton(
                    "Tous les types",
                    options: accountTypes().map { type in
                        let defaults = accountTypeDefaults(accountType: type)
                        return SelectOption(id: type, label: type, icon: defaults.icon, color: defaults.color)
                    },
                    selection: $model.types
                )
                if !compact { Spacer() }
            }

            if let view = model.view {
                if view.totalCount == 0 {
                    DmxCard {
                        VStack(spacing: 4) {
                            EmptyStateView(icon: "Settings", title: "Aucun compte configuré")
                            Button(action: { store.present(.account(id: nil)) }) { Text("Créer votre premier compte") }
                                .buttonStyle(DmxButtonStyle(.subtle))
                        }
                    }
                } else if view.visibleCount == 0 {
                    DmxCard {
                        EmptyStateView(icon: "Search", title: "Aucun compte ne correspond aux filtres.")
                    }
                } else {
                    VStack(alignment: .leading, spacing: 24) {
                        ForEach(view.groups, id: \.name) { section in
                            groupSection(section)
                        }
                    }
                }
            }
        }
    }

    private var columns: Int { compact ? 1 : 3 }

    private func groupSection(_ section: AccountGroupSection) -> some View {
        let isTargeted = targetedGroup == section.name
        return VStack(alignment: .leading, spacing: 14) {
            groupHeader(section)
            if section.accounts.isEmpty {
                Text("Glissez un compte ici")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 64)
                    .overlay(RoundedRectangle(cornerRadius: 12).stroke(DmxPalette.separator, style: StrokeStyle(lineWidth: 1, dash: [5, 4])))
            } else {
                VStack(spacing: 16) {
                    ForEach(Array(section.accounts.chunked(columns).enumerated()), id: \.offset) { row in
                        HStack(alignment: .top, spacing: 16) {
                            ForEach(row.element, id: \.account.id) { card in
                                AccountCardView(card: card, groups: store.settings.customGroups, onDrop: { payload in
                                    drop(payload, onAccount: card)
                                })
                            }
                            ForEach(0..<(columns - row.element.count), id: \.self) { _ in
                                Color.clear.frame(maxWidth: .infinity, maxHeight: 1)
                            }
                        }
                    }
                }
            }
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(isTargeted ? Color.accentColor.opacity(0.07) : Color.clear))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(isTargeted ? Color.accentColor : Color.clear, lineWidth: 2))
        .onDrop(of: [AccountDragPayload.typeIdentifier], isTargeted: Binding(
            get: { targetedGroup == section.name },
            set: { targeted in
                if targeted {
                    targetedGroup = section.name
                } else if targetedGroup == section.name {
                    targetedGroup = nil
                }
            }
        )) { providers in
            AccountDragPayload.load(providers) { payload in drop(payload, onGroup: section) }
        }
    }

    @ViewBuilder
    private func groupHeader(_ section: AccountGroupSection) -> some View {
        let header = HStack(spacing: 8) {
            if !section.isUngrouped {
                DmxIcon("GripVertical", size: 14).foregroundColor(.secondary)
            }
            Text(section.name).font(.system(size: 17, weight: .semibold))
            Text("\(section.accounts.count)")
                .font(.system(size: 12))
                .foregroundColor(.secondary)
                .padding(.horizontal, 8)
                .padding(.vertical, 2)
                .background(Capsule().fill(Color.primary.opacity(0.07)))
            Spacer()
        }
        .contentShape(Rectangle())

        if section.isUngrouped {
            header
        } else {
            header.onDrag { AccountDragPayload.group(section.name).itemProvider }
        }
    }

    private func drop(_ payload: AccountDragPayload, onGroup section: AccountGroupSection) {
        targetedGroup = nil
        switch payload {
        case let .account(id):
            store.run { engine in
                try engine.moveAccount(accountId: id, target: .onGroup(group: section.isUngrouped ? nil : section.name))
            }
        case let .group(name):
            guard !section.isUngrouped, name != section.name else { return }
            store.run { engine in try engine.moveGroup(group: name, overGroup: section.name) }
        }
    }

    private func drop(_ payload: AccountDragPayload, onAccount card: AccountCard) {
        targetedGroup = nil
        switch payload {
        case let .account(id):
            guard id != card.account.id else { return }
            store.run { engine in try engine.moveAccount(accountId: id, target: .onAccount(accountId: card.account.id)) }
        case let .group(name):
            guard let group = card.group, group != name else { return }
            store.run { engine in try engine.moveGroup(group: name, overGroup: group) }
        }
    }
}

struct AccountCardView: View {
    @EnvironmentObject private var store: AppStore
    let card: AccountCard
    let groups: [String]
    let onDrop: (AccountDragPayload) -> Void
    @State private var isTargeted = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 8) {
                HStack(spacing: 12) {
                    IconBadge(icon: card.account.icon, colorHex: card.account.color, size: 46)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(card.account.name).font(.system(size: 15, weight: .semibold)).lineLimit(1)
                        Text(card.account.accountType).font(.system(size: 13)).foregroundColor(.secondary)
                    }
                }
                Spacer(minLength: 4)
                IconButton("Edit2") { store.present(.account(id: card.account.id)) }
                IconButton("Trash2", color: DmxColors.expense, action: delete)
            }
            VStack(alignment: .leading, spacing: 4) {
                Text("Solde actuel").font(.system(size: 13)).foregroundColor(.secondary)
                Text(Money.format(card.currentBalance))
                    .font(.system(size: 24, weight: .bold))
                    .foregroundColor(card.currentBalance >= 0 ? .primary : DmxColors.expense)
                    .lineLimit(1)
                    .minimumScaleFactor(0.6)
                Divider().padding(.vertical, 4)
                HStack {
                    Text("Solde pointé").font(.system(size: 12)).foregroundColor(.secondary)
                    Spacer()
                    Text(Money.format(card.checkedBalance)).font(.system(size: 13, weight: .medium)).foregroundColor(.secondary)
                }
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(DmxPalette.cardBackground))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous)
            .stroke(isTargeted ? Color.accentColor : DmxPalette.separator.opacity(0.5), lineWidth: isTargeted ? 2 : 0.5))
        .onDrag { AccountDragPayload.account(card.account.id).itemProvider }
        .onDrop(of: [AccountDragPayload.typeIdentifier], isTargeted: $isTargeted) { providers in
            AccountDragPayload.load(providers, perform: onDrop)
        }
        .contextMenu {
            Button(action: { store.present(.account(id: card.account.id)) }) { Text("Modifier") }
            Button(action: openJournal) { Text("Voir le journal") }
            ForEach(groups.filter { $0 != card.group }, id: \.self) { group in
                Button(action: { move(to: group) }) { Text("Déplacer vers « \(group) »") }
            }
            if card.group != nil {
                Button(action: { move(to: nil) }) { Text("Retirer du groupe") }
            }
            Button(action: delete) { Text("Supprimer") }
        }
    }

    private func openJournal() {
        store.selectedAccountIds = [card.account.id]
        store.route = .transactions
    }

    private func move(to group: String?) {
        let id = card.account.id
        store.run { engine in try engine.moveAccount(accountId: id, target: .onGroup(group: group)) }
    }

    private func delete() {
        let id = card.account.id
        store.confirm(
            title: "Supprimer le compte",
            message: "Êtes-vous sûr de vouloir supprimer ce compte ? Cette action est irréversible et supprimera toutes les transactions associées."
        ) { [store] in
            store.run("Compte supprimé") { engine in try engine.deleteAccount(id: id) }
        }
    }
}
