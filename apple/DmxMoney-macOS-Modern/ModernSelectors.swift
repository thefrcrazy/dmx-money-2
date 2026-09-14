import DmxKit
import SwiftUI

/// Sélecteur multi-choix en popover : cases à cocher système, **avec l'icône et la couleur** de
/// chaque entrée.
///
/// Un `Menu` SwiftUI ne rend pas les images de ses `Toggle` sur macOS : les icônes demandées
/// disparaissaient. Un popover est un contrôle système tout aussi natif, et on y maîtrise le
/// contenu de chaque ligne.
struct FilterSelector: View {
    let title: String
    var systemImage: String?
    let options: [SelectOption]
    @Binding var selection: [String]
    @State private var isPresented = false

    var body: some View {
        Button {
            isPresented = true
        } label: {
            HStack(spacing: 5) {
                if let systemImage {
                    Image(systemName: systemImage)
                }
                Text(label)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .help(title)
        .popover(isPresented: $isPresented, arrowEdge: .bottom) {
            content
        }
    }

    private var label: String {
        selection.isEmpty ? title : "\(title) (\(selection.count))"
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(title).font(.headline)
                Spacer(minLength: 12)
                Button("Tout afficher") { selection = [] }
                    .buttonStyle(.link)
                    .disabled(selection.isEmpty)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 1) {
                    ForEach(options) { option in
                        SelectorRow(isOn: selection.contains(option.id)) {
                            toggle(option.id)
                        } label: {
                            OptionLabel(option: option)
                        }
                    }
                }
                .padding(.vertical, 6)
            }
            .frame(maxHeight: 340)
            .scrollBounceBehavior(.basedOnSize)
        }
        .frame(minWidth: 250)
    }

    private func toggle(_ id: String) {
        if selection.contains(id) {
            selection.removeAll { $0 == id }
        } else {
            selection.append(id)
        }
    }
}

/// Ligne à cocher d'un sélecteur.
///
/// Un `Toggle` teinte tout son libellé avec la couleur d'accentuation : les icônes de comptes et
/// de catégories ressortaient toutes en bleu. La case est donc dessinée avec la ligne, ce qui
/// laisse chaque icône dans sa couleur.
struct SelectorRow<Label: View>: View {
    let isOn: Bool
    let action: () -> Void
    @ViewBuilder var label: Label
    @State private var isHovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: isOn ? "checkmark.square.fill" : "square")
                    .foregroundStyle(isOn ? AnyShapeStyle(.tint) : AnyShapeStyle(.tertiary))
                    .font(.system(size: 13))
                label
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(isHovering ? Color.primary.opacity(0.06) : .clear, in: RoundedRectangle(cornerRadius: 6))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 2)
        .onHover { isHovering = $0 }
    }
}

/// Ligne d'un sélecteur : icône colorée puis libellé.
struct OptionLabel: View {
    let option: SelectOption

    var body: some View {
        HStack(spacing: 7) {
            if let icon = option.icon {
                Image(systemName: Symbols.name(for: icon))
                    .foregroundStyle(Color(hex: option.color ?? "", fallback: .secondary))
                    .frame(width: 16)
            }
            Text(option.label)
        }
    }
}

/// Filtre de comptes global, dans la barre d'outils : un seul bouton, et le détail dans le
/// popover — avec l'icône et la couleur de chaque compte.
struct AccountSelector: View {
    @ObservedObject var store: AppStore
    @State private var isPresented = false

    var body: some View {
        Button {
            isPresented = true
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "line.3.horizontal.decrease.circle")
                // Largeur fixe : la barre d'outils ne doit pas bouger selon le nom du compte.
                Text(label)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .frame(width: 128, alignment: .leading)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .help("Filtrer par compte")
        .popover(isPresented: $isPresented, arrowEdge: .bottom) {
            content
        }
    }

    private var label: String {
        let selected = store.selectedAccountIds
        switch selected.count {
        case 0: return "Tous les comptes"
        case 1: return store.account(id: selected[0])?.name ?? "1 compte"
        default: return "\(selected.count) comptes"
        }
    }

    private var content: some View {
        // Soldes par compte fournis par le noyau (le même résumé que la barre des menus).
        let balances = Dictionary(
            (store.peek { try $0.traySummary() }?.accounts ?? []).map { ($0.accountId, $0.balance) },
            uniquingKeysWith: { first, _ in first }
        )
        return VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Comptes pris en compte").font(.headline)
                Spacer(minLength: 12)
                Button("Tous les comptes") { store.clearAccountFilter() }
                    .buttonStyle(.link)
                    .disabled(store.selectedAccountIds.isEmpty)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 1) {
                    ForEach(store.accounts, id: \.id) { account in
                        SelectorRow(isOn: isOn(account.id)) {
                            toggle(account.id)
                        } label: {
                            HStack(spacing: 7) {
                                Image(systemName: Symbols.name(for: account.icon))
                                    .foregroundStyle(Color(hex: account.color, fallback: .accentColor))
                                    .frame(width: 16)
                                Text(account.name).lineLimit(1)
                                Spacer(minLength: 10)
                                Text(Money.format(balances[account.id] ?? 0))
                                    .font(.caption.monospacedDigit())
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
                .padding(.vertical, 6)
            }
            .frame(maxHeight: 340)
            .scrollBounceBehavior(.basedOnSize)
        }
        .frame(minWidth: 300)
    }

    /// Sans filtre, tous les comptes comptent : les cases apparaissent donc cochées.
    private func isOn(_ id: String) -> Bool {
        store.selectedAccountIds.isEmpty || store.isSelected(accountId: id)
    }

    private func toggle(_ id: String) {
        if store.selectedAccountIds.isEmpty {
            store.selectedAccountIds = [id]
        } else {
            store.toggleAccountFilter(id)
        }
    }
}

/// Soldes pointé et actuel, dans la barre d'outils.
struct ToolbarBalances: View {
    @ObservedObject var store: AppStore

    var body: some View {
        let balances = store.balances
        HStack(spacing: 16) {
            value("Pointé", amount: balances.checkedBalance, color: .green)
            value("Actuel", amount: balances.currentBalance, color: balances.currentBalance < 0 ? .red : .primary)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 5)
        .help("Soldes des comptes pris en compte")
    }

    private func value(_ label: String, amount: Double, color: Color) -> some View {
        VStack(alignment: .trailing, spacing: -1) {
            Text(label)
                .font(.caption2)
                .textCase(.uppercase)
                .foregroundStyle(.secondary)
            Text(Money.format(amount))
                .font(.callout.weight(.semibold).monospacedDigit())
                .foregroundStyle(color)
                .lineLimit(1)
                .truncationMode(.tail)
        }
        // Largeur fixe pour que les soldes ne fassent pas respirer la barre d'outils.
        .frame(width: 104, alignment: .trailing)
    }
}

/// Recherche repliée en loupe, qui s'ouvre au clic et se referme avec Échap.
///
/// `searchToolbarBehavior(.minimize)` n'existe que sur iOS, et le champ système de la barre
/// d'outils ne se replie que lorsque la place manque — d'où cette version explicite, bâtie sur
/// les mêmes contrôles système.
struct ToolbarSearch: View {
    let prompt: String
    @Binding var text: String
    @State private var isOpen = false
    @FocusState private var isFocused: Bool

    var body: some View {
        Group {
            if isOpen {
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(.secondary)
                    TextField(prompt, text: $text)
                        .textFieldStyle(.plain)
                        .focused($isFocused)
                        .frame(width: 190)
                    Button {
                        text = ""
                        isFocused = true
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .help("Effacer la recherche")
                    .opacity(text.isEmpty ? 0 : 1)
                    .disabled(text.isEmpty)
                }
                .padding(.leading, 10)
                .padding(.trailing, 8)
                .padding(.vertical, 6)
                .background(.quaternary.opacity(0.5), in: Capsule())
                .overlay(Capsule().stroke(.separator, lineWidth: 0.5))
                .onExitCommand { close() }
                // Quitter le champ le referme, sauf s'il reste une recherche en cours.
                .onChange(of: isFocused) { _, focused in
                    if !focused, text.isEmpty { isOpen = false }
                }
            } else {
                Button {
                    isOpen = true
                    isFocused = true
                } label: {
                    Image(systemName: "magnifyingglass")
                }
                .keyboardShortcut("f")
                .help(prompt)
            }
        }
        .onChange(of: isOpen) { _, open in
            isFocused = open
        }
    }

    private func close() {
        text = ""
        isOpen = false
        isFocused = false
    }
}
