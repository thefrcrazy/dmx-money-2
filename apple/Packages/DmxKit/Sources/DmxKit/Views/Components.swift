import SwiftUI

#if os(macOS)
import AppKit
#else
import UIKit
#endif

// Composants partagés, compatibles macOS 10.15 (pas de LazyVGrid, Label, Menu ni ProgressView).

public enum DmxPalette {
    #if os(macOS)
    public static var cardBackground: Color { Color(NSColor.controlBackgroundColor) }
    public static var pageBackground: Color { Color(NSColor.windowBackgroundColor) }
    public static var separator: Color { Color(NSColor.separatorColor) }
    #else
    public static var cardBackground: Color { Color(uiColor: .secondarySystemGroupedBackground) }
    public static var pageBackground: Color { Color(uiColor: .systemGroupedBackground) }
    public static var separator: Color { Color(uiColor: .separator) }
    #endif
    public static var fieldBackground: Color { Color.primary.opacity(0.05) }
}

private struct CompactLayoutKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// Mise en page étroite (iPhone) : une colonne, titres portés par la barre de navigation.
    public var dmxCompact: Bool {
        get { self[CompactLayoutKey.self] }
        set { self[CompactLayoutKey.self] = newValue }
    }
}

// MARK: - Mise en page

public struct PageScroll<Content: View>: View {
    @Environment(\.dmxCompact) private var compact
    private let content: Content

    public init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: compact ? 16 : 20) {
                content
            }
            .padding(compact ? 16 : 24)
            .frame(maxWidth: 1280, alignment: .topLeading)
            .frame(maxWidth: .infinity)
        }
        .background(DmxPalette.pageBackground)
    }
}

public struct PageHeader<Trailing: View>: View {
    @Environment(\.dmxCompact) private var compact
    private let title: String
    private let subtitle: String?
    private let trailing: Trailing

    public init(_ title: String, subtitle: String? = nil, @ViewBuilder trailing: () -> Trailing) {
        self.title = title
        self.subtitle = subtitle
        self.trailing = trailing()
    }

    @ViewBuilder
    public var body: some View {
        if compact {
            // Largeur d'iPhone : les boutons d'action seraient tronqués à côté du titre.
            VStack(alignment: .leading, spacing: 10) {
                if let subtitle = subtitle {
                    Text(subtitle).font(.system(size: 13)).foregroundColor(.secondary)
                }
                trailing.frame(maxWidth: .infinity, alignment: .leading)
            }
        } else {
            HStack(alignment: .center, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.system(size: 24, weight: .bold))
                    if let subtitle = subtitle {
                        Text(subtitle).font(.system(size: 13)).foregroundColor(.secondary)
                    }
                }
                Spacer(minLength: 8)
                trailing
            }
        }
    }
}

extension PageHeader where Trailing == EmptyView {
    public init(_ title: String, subtitle: String? = nil) {
        self.init(title, subtitle: subtitle, trailing: { EmptyView() })
    }
}

/// Rangée de cartes : colonnes de même hauteur sur grand écran, empilées sur iPhone.
public struct CardRow<A: View, B: View, C: View>: View {
    @Environment(\.dmxCompact) private var compact
    private let a: A
    private let b: B
    private let c: C

    public init(@ViewBuilder _ a: () -> A, @ViewBuilder _ b: () -> B, @ViewBuilder _ c: () -> C) {
        self.a = a()
        self.b = b()
        self.c = c()
    }

    public var body: some View {
        if compact {
            VStack(spacing: 16) { a; b; c }
        } else {
            HStack(alignment: .top, spacing: 20) {
                a.frame(maxHeight: .infinity, alignment: .top)
                b.frame(maxHeight: .infinity, alignment: .top)
                c.frame(maxHeight: .infinity, alignment: .top)
            }
            .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: - Cartes

public struct DmxCard<Accessory: View, Content: View>: View {
    private let title: String?
    private let icon: String?
    private let padded: Bool
    private let accessory: Accessory
    private let content: Content

    public init(title: String? = nil, icon: String? = nil, padded: Bool = true, @ViewBuilder accessory: () -> Accessory, @ViewBuilder content: () -> Content) {
        self.title = title
        self.icon = icon
        self.padded = padded
        self.accessory = accessory()
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if title != nil || icon != nil {
                HStack(spacing: 8) {
                    if let icon = icon {
                        DmxIcon(icon, size: 15).foregroundColor(.accentColor)
                    }
                    if let title = title {
                        Text(title).font(.system(size: 14, weight: .semibold))
                    }
                    Spacer(minLength: 8)
                    accessory
                }
                .padding(.horizontal, 16)
                .padding(.top, 14)
                .padding(.bottom, 10)
            }
            content
                .padding(.horizontal, padded ? 16 : 0)
                .padding(.top, (title == nil && icon == nil && padded) ? 16 : 0)
                .padding(.bottom, padded ? 16 : 0)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(DmxPalette.cardBackground))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(DmxPalette.separator.opacity(0.5), lineWidth: 0.5))
    }
}

extension DmxCard where Accessory == EmptyView {
    public init(title: String? = nil, icon: String? = nil, padded: Bool = true, @ViewBuilder content: () -> Content) {
        self.init(title: title, icon: icon, padded: padded, accessory: { EmptyView() }, content: content)
    }
}

public struct StatTile: View {
    private let label: String
    private let value: String
    private let valueColor: Color
    private let icon: String?
    private let caption: String?

    public init(_ label: String, value: String, valueColor: Color = .primary, icon: String? = nil, caption: String? = nil) {
        self.label = label
        self.value = value
        self.valueColor = valueColor
        self.icon = icon
        self.caption = caption
    }

    public var body: some View {
        DmxCard {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    if let icon = icon {
                        DmxIcon(icon, size: 13).foregroundColor(.secondary)
                    }
                    SectionLabel(label)
                }
                Text(value)
                    .font(.system(size: 20, weight: .bold))
                    .foregroundColor(valueColor)
                    .lineLimit(1)
                    .minimumScaleFactor(0.6)
                if let caption = caption {
                    Text(caption).font(.system(size: 11)).foregroundColor(.secondary).lineLimit(2)
                }
            }
        }
    }
}

// MARK: - Textes

public struct SectionLabel: View {
    private let text: String

    public init(_ text: String) {
        self.text = text
    }

    public var body: some View {
        Text(text.uppercased())
            .font(.system(size: 10, weight: .bold))
            .kerning(0.8)
            .foregroundColor(.secondary)
            .lineLimit(1)
    }
}

public struct Pill: View {
    private let text: String
    private let color: Color?

    public init(_ text: String, color: Color? = nil) {
        self.text = text
        self.color = color
    }

    public var body: some View {
        Text(text)
            .font(.system(size: 10, weight: .semibold))
            .foregroundColor(color ?? .secondary)
            .padding(.horizontal, 8)
            .padding(.vertical, 2)
            .background(Capsule().fill((color ?? Color.primary).opacity(color == nil ? 0.07 : 0.14)))
            .lineLimit(1)
    }
}

public struct AmountText: View {
    private let text: String
    private let color: Color
    private let font: Font

    public init(_ amount: Double, kind: TransactionType? = nil, signed: Bool = false, font: Font = .system(size: 13, weight: .semibold)) {
        if let kind = kind, signed {
            text = Money.signed(amount, kind: kind)
        } else {
            text = Money.format(amount)
        }
        if kind == .income {
            color = DmxColors.income
        } else if kind == .transfer {
            color = DmxColors.transfer
        } else if amount < 0 {
            color = DmxColors.expense
        } else {
            color = .primary
        }
        self.font = font
    }

    public var body: some View {
        Text(text).font(font).foregroundColor(color).lineLimit(1)
    }
}

public struct EmptyStateView: View {
    private let icon: String
    private let title: String
    private let message: String?

    public init(icon: String, title: String, message: String? = nil) {
        self.icon = icon
        self.title = title
        self.message = message
    }

    public var body: some View {
        VStack(spacing: 8) {
            ZStack {
                Circle().fill(Color.primary.opacity(0.05)).frame(width: 48, height: 48)
                DmxIcon(icon, size: 22).foregroundColor(.secondary)
            }
            Text(title).font(.system(size: 13, weight: .semibold)).multilineTextAlignment(.center)
            if let message = message {
                Text(message).font(.system(size: 12)).foregroundColor(.secondary).multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
    }
}

// MARK: - Icônes

public struct IconCircle: View {
    private let icon: String
    private let color: Color
    private let size: CGFloat

    public init(icon: String, color: Color, size: CGFloat = 32) {
        self.icon = icon
        self.color = color
        self.size = size
    }

    public var body: some View {
        ZStack {
            Circle().fill(color.opacity(0.12))
            DmxIcon(icon, size: size * 0.5).foregroundColor(color)
        }
        .frame(width: size, height: size)
    }
}

// MARK: - Boutons

public struct DmxButtonStyle: ButtonStyle {
    public enum Kind {
        case primary
        case secondary
        case destructive
        case subtle
    }

    private let kind: Kind

    public init(_ kind: Kind = .primary) {
        self.kind = kind
    }

    public func makeBody(configuration: Configuration) -> some View {
        StyledButtonLabel(configuration: configuration, kind: kind)
    }

    private struct StyledButtonLabel: View {
        @Environment(\.isEnabled) private var isEnabled
        let configuration: ButtonStyle.Configuration
        let kind: Kind

        var body: some View {
            configuration.label
                .font(.system(size: 13, weight: .semibold))
                .lineLimit(1)
                .padding(.horizontal, 14)
                .padding(.vertical, 7)
                .foregroundColor(foreground)
                .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(background))
                .opacity(isEnabled ? (configuration.isPressed ? 0.75 : 1) : 0.45)
                .contentShape(Rectangle())
        }

        private var foreground: Color {
            switch kind {
            case .primary, .destructive: return .white
            case .secondary: return .primary
            case .subtle: return .accentColor
            }
        }

        private var background: Color {
            switch kind {
            case .primary: return .accentColor
            case .secondary: return Color.primary.opacity(0.08)
            case .destructive: return DmxColors.expense
            case .subtle: return Color.accentColor.opacity(0.12)
            }
        }
    }
}

public struct LinkButton: View {
    private let title: String
    private let action: () -> Void

    public init(_ title: String, action: @escaping () -> Void) {
        self.title = title
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            Text(title).font(.system(size: 12, weight: .medium)).foregroundColor(.accentColor)
        }
        .buttonStyle(PlainButtonStyle())
    }
}

public struct IconButton: View {
    private let icon: String
    private let color: Color
    private let size: CGFloat
    private let action: () -> Void

    public init(_ icon: String, color: Color = .secondary, size: CGFloat = 14, action: @escaping () -> Void) {
        self.icon = icon
        self.color = color
        self.size = size
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            DmxIcon(icon, size: size)
                .foregroundColor(color)
                .frame(width: size + 14, height: size + 14)
                .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
    }
}

// MARK: - Recherche et filtres

public struct SearchField: View {
    private let placeholder: String
    @Binding private var text: String

    public init(_ placeholder: String, text: Binding<String>) {
        self.placeholder = placeholder
        self._text = text
    }

    public var body: some View {
        HStack(spacing: 6) {
            DmxIcon("Search", size: 13).foregroundColor(.secondary)
            TextField(placeholder, text: $text)
                .textFieldStyle(PlainTextFieldStyle())
                .font(.system(size: 13))
            if !text.isEmpty {
                Button(action: { text = "" }) {
                    DmxIcon("X", size: 12).foregroundColor(.secondary)
                }
                .buttonStyle(PlainButtonStyle())
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(DmxPalette.fieldBackground))
    }
}

public struct SelectOption: Identifiable, Hashable {
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

/// Sélection multiple dans un popover (catégories, types, états, comptes…).
public struct MultiSelectButton: View {
    private let title: String
    private let options: [SelectOption]
    @Binding private var selection: [String]
    @State private var isOpen = false

    public init(_ title: String, options: [SelectOption], selection: Binding<[String]>) {
        self.title = title
        self.options = options
        self._selection = selection
    }

    public var body: some View {
        Button(action: { isOpen.toggle() }) {
            HStack(spacing: 6) {
                Text(label).font(.system(size: 12, weight: .medium)).lineLimit(1)
                DmxIcon("ChevronDown", size: 11).foregroundColor(.secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(selection.isEmpty ? DmxPalette.fieldBackground : Color.accentColor.opacity(0.15)))
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
        .popover(isPresented: $isOpen, arrowEdge: .bottom) { popover }
    }

    private var label: String {
        if selection.isEmpty { return title }
        if selection.count == 1, let option = options.first(where: { $0.id == selection[0] }) {
            return option.label
        }
        return "\(title) (\(selection.count))"
    }

    private var list: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(options) { option in
                        Button(action: { toggle(option.id) }) {
                            HStack(spacing: 8) {
                                DmxIcon(selection.contains(option.id) ? "CheckCircle2" : "Circle", size: 14)
                                    .foregroundColor(selection.contains(option.id) ? .accentColor : .secondary)
                                if let icon = option.icon {
                                    DmxIcon(icon, size: 14).foregroundColor(Color(hex: option.color ?? "", fallback: .secondary))
                                } else if let color = option.color {
                                    Circle().fill(Color(hex: color)).frame(width: 8, height: 8)
                                }
                                Text(option.label).font(.system(size: 13)).lineLimit(1)
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 7)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(PlainButtonStyle())
                    }
                }
                .padding(.vertical, 6)
            }
            .frame(height: min(CGFloat(options.count) * 31 + 12, 340))
            if !selection.isEmpty {
                Divider()
                Button(action: { selection = [] }) {
                    Text("Tout désélectionner").font(.system(size: 12, weight: .medium)).foregroundColor(.accentColor)
                }
                .buttonStyle(PlainButtonStyle())
                .padding(10)
            }
        }
        .frame(width: 260)
    }

    #if os(iOS)
    private var popover: some View {
        list.presentationCompactAdaptation(.popover)
    }
    #else
    private var popover: some View {
        list
    }
    #endif

    private func toggle(_ id: String) {
        if let index = selection.firstIndex(of: id) {
            selection.remove(at: index)
        } else {
            selection.append(id)
        }
    }
}

/// Filtre global de comptes (barre d'outils).
public struct AccountFilterButton: View {
    @EnvironmentObject private var store: AppStore

    public init() {}

    public var body: some View {
        MultiSelectButton(
            "Tous les comptes",
            options: store.accounts.map { SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color) },
            selection: Binding(get: { store.selectedAccountIds }, set: { store.selectedAccountIds = $0 })
        )
    }
}

// MARK: - Sélecteurs de couleur et d'icône

public struct ColorGridPicker: View {
    private let colors: [String]
    @Binding private var selection: String
    private let columns: Int

    public init(colors: [String] = categoryColors(), selection: Binding<String>, columns: Int = 12) {
        self.colors = colors
        self._selection = selection
        self.columns = columns
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            ForEach(Array(colors.chunked(columns).enumerated()), id: \.offset) { row in
                HStack(spacing: 7) {
                    ForEach(row.element, id: \.self) { hex in
                        Circle()
                            .fill(Color(hex: hex))
                            .frame(width: 20, height: 20)
                            .overlay(
                                Circle()
                                    .stroke(Color.primary.opacity(0.85), lineWidth: selection.lowercased() == hex.lowercased() ? 2 : 0)
                                    .padding(-3)
                            )
                            .contentShape(Circle())
                            .onTapGesture { selection = hex }
                    }
                }
            }
        }
        .padding(3)
    }
}

public struct IconGridPicker: View {
    private let icons: [String]
    @Binding private var selection: String
    private let color: Color
    private let columns: Int
    @State private var search = ""

    public init(icons: [String] = iconPickerNames(), selection: Binding<String>, color: Color, columns: Int = 10) {
        self.icons = icons
        self._selection = selection
        self.color = color
        self.columns = columns
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SearchField("Rechercher une icône...", text: $search)
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(filtered.chunked(columns).enumerated()), id: \.offset) { row in
                        HStack(spacing: 6) {
                            ForEach(row.element, id: \.self) { name in
                                ZStack {
                                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                                        .fill(selection == name ? color.opacity(0.2) : Color.primary.opacity(0.04))
                                    DmxIcon(name, size: 16).foregroundColor(selection == name ? color : .primary)
                                }
                                .frame(width: 32, height: 32)
                                .contentShape(Rectangle())
                                .onTapGesture { selection = name }
                            }
                        }
                    }
                }
                .padding(2)
            }
            .frame(height: 170)
        }
    }

    private var filtered: [String] {
        let query = search.trimmingCharacters(in: .whitespaces).lowercased()
        return query.isEmpty ? icons : icons.filter { $0.lowercased().contains(query) }
    }
}

// MARK: - Messages

public struct ToastView: View {
    private let message: String

    public init(_ message: String) {
        self.message = message
    }

    public var body: some View {
        HStack(spacing: 8) {
            DmxIcon("CheckCircle2", size: 14).foregroundColor(DmxColors.income)
            Text(message).font(.system(size: 13, weight: .medium))
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        .background(Capsule().fill(DmxPalette.cardBackground).shadow(color: Color.black.opacity(0.18), radius: 12, y: 4))
        .overlay(Capsule().stroke(DmxPalette.separator.opacity(0.5), lineWidth: 0.5))
    }
}
