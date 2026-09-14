import SwiftUI

// Contrôles de formulaire communs à macOS (10.15+) et iOS.

/// Cadre d'un formulaire : titre, contenu, erreur, boutons Annuler / Valider.
public struct FormSheet<Content: View>: View {
    private let title: String
    private let submitTitle: String
    private let error: String?
    private let submitDisabled: Bool
    private let onCancel: () -> Void
    private let onSubmit: () -> Void
    private let content: Content

    public init(
        title: String,
        submitTitle: String,
        error: String?,
        submitDisabled: Bool = false,
        onCancel: @escaping () -> Void,
        onSubmit: @escaping () -> Void,
        @ViewBuilder content: () -> Content
    ) {
        self.title = title
        self.submitTitle = submitTitle
        self.error = error
        self.submitDisabled = submitDisabled
        self.onCancel = onCancel
        self.onSubmit = onSubmit
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(title).font(.system(size: 17, weight: .semibold))
                Spacer()
                IconButton("X", action: onCancel)
            }
            .padding(.horizontal, 20)
            .padding(.top, 16)
            .padding(.bottom, 12)
            Divider()
            body(content)
            if let error = error {
                HStack(spacing: 6) {
                    DmxIcon("AlertCircle", size: 13)
                    Text(error).font(.system(size: 12, weight: .medium))
                }
                .foregroundColor(DmxColors.expense)
                .padding(.horizontal, 20)
                .padding(.bottom, 10)
            }
            Spacer(minLength: 0)
            Divider()
            HStack(spacing: 10) {
                Spacer()
                Button(action: onCancel) { Text("Annuler") }
                    .buttonStyle(DmxButtonStyle(.secondary))
                Button(action: onSubmit) { Text(submitTitle) }
                    .buttonStyle(DmxButtonStyle(.primary))
                    .disabled(submitDisabled)
            }
            .padding(16)
        }
    }

    #if os(iOS)
    private func body(_ content: Content) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) { content }
                .padding(20)
        }
    }
    #else
    private func body(_ content: Content) -> some View {
        VStack(alignment: .leading, spacing: 14) { content }
            .padding(20)
    }
    #endif
}

public struct FormField<Content: View>: View {
    private let label: String
    private let content: Content

    public init(_ label: String, @ViewBuilder content: () -> Content) {
        self.label = label
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(label).font(.system(size: 12, weight: .medium)).foregroundColor(.secondary)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct FieldBackground: ViewModifier {
    func body(content: Content) -> some View {
        content
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(DmxPalette.fieldBackground))
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(DmxPalette.separator.opacity(0.6), lineWidth: 0.5))
    }
}

public struct DmxTextField: View {
    private let placeholder: String
    @Binding private var text: String

    public init(_ placeholder: String, text: Binding<String>) {
        self.placeholder = placeholder
        self._text = text
    }

    public var body: some View {
        TextField(placeholder, text: $text)
            .textFieldStyle(PlainTextFieldStyle())
            .font(.system(size: 13))
            .modifier(FieldBackground())
    }
}

/// Montant saisi en texte (virgule acceptée), suffixé de « € ».
public struct AmountField: View {
    @Binding private var text: String

    public init(text: Binding<String>) {
        self._text = text
    }

    public var body: some View {
        HStack(spacing: 4) {
            amountInput
            Text("€").font(.system(size: 13)).foregroundColor(.secondary)
        }
        .modifier(FieldBackground())
    }

    #if os(iOS)
    private var amountInput: some View {
        TextField("0,00", text: $text)
            .textFieldStyle(PlainTextFieldStyle())
            .font(.system(size: 13))
            .keyboardType(.decimalPad)
    }
    #else
    private var amountInput: some View {
        TextField("0,00", text: $text)
            .textFieldStyle(PlainTextFieldStyle())
            .font(.system(size: 13))
    }
    #endif
}

/// Date `YYYY-MM-DD` éditée avec le sélecteur natif.
public struct DayPicker: View {
    @Binding private var day: String

    public init(day: Binding<String>) {
        self._day = day
    }

    public var body: some View {
        DatePicker("", selection: Binding(get: { DayString.date(day) }, set: { day = DayString.string($0) }), displayedComponents: .date)
            .labelsHidden()
            .environment(\.locale, Locale(identifier: "fr_FR"))
    }
}

/// Choix du type d'opération (Dépense / Revenu / Virement).
public struct TransactionKindPicker: View {
    @Binding private var kind: TransactionType
    private let allowsTransfer: Bool

    public init(kind: Binding<TransactionType>, allowsTransfer: Bool = true) {
        self._kind = kind
        self.allowsTransfer = allowsTransfer
    }

    public var body: some View {
        Picker("", selection: $kind) {
            Text("Dépense").tag(TransactionType.expense)
            Text("Revenu").tag(TransactionType.income)
            if allowsTransfer {
                Text("Virement").tag(TransactionType.transfer)
            }
        }
        .labelsHidden()
        .pickerStyle(SegmentedPickerStyle())
    }
}

/// Liste déroulante simple (valeurs fixes).
public struct ChoicePicker: View {
    private let options: [SelectOption]
    @Binding private var selection: String

    public init(options: [SelectOption], selection: Binding<String>) {
        self.options = options
        self._selection = selection
    }

    public var body: some View {
        picker.labelsHidden()
    }

    #if os(macOS)
    private var picker: some View {
        Picker("", selection: $selection) {
            ForEach(options) { option in Text(option.label).tag(option.id) }
        }
        .pickerStyle(PopUpButtonPickerStyle())
    }
    #else
    private var picker: some View {
        Picker("", selection: $selection) {
            ForEach(options) { option in Text(option.label).tag(option.id) }
        }
        .pickerStyle(.menu)
    }
    #endif
}

/// Sélection d'un élément avec recherche (catégorie, compte, budget), dans un popover.
public struct SearchableSelect: View {
    private let placeholder: String
    private let options: [SelectOption]
    private let noneLabel: String?
    @Binding private var selection: String?
    @State private var isOpen = false
    @State private var search = ""

    public init(_ placeholder: String, options: [SelectOption], selection: Binding<String?>, noneLabel: String? = nil) {
        self.placeholder = placeholder
        self.options = options
        self._selection = selection
        self.noneLabel = noneLabel
    }

    /// Variante pour un identifiant obligatoire (chaîne vide = rien de choisi).
    public init(_ placeholder: String, options: [SelectOption], required selection: Binding<String>) {
        self.init(placeholder, options: options, selection: Binding(
            get: { selection.wrappedValue.isEmpty ? nil : selection.wrappedValue },
            set: { selection.wrappedValue = $0 ?? "" }
        ))
    }

    public var body: some View {
        Button(action: { isOpen.toggle() }) {
            HStack(spacing: 8) {
                if let option = selected {
                    optionMarker(option)
                    Text(option.label).font(.system(size: 13)).foregroundColor(.primary).lineLimit(1)
                } else {
                    Text(noneLabel ?? placeholder).font(.system(size: 13)).foregroundColor(noneLabel == nil ? .secondary : .primary).lineLimit(1)
                }
                Spacer(minLength: 4)
                DmxIcon("ChevronDown", size: 11).foregroundColor(.secondary)
            }
            .modifier(FieldBackground())
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
        .popover(isPresented: $isOpen, arrowEdge: .bottom) { popover }
    }

    private var selected: SelectOption? {
        guard let selection = selection else { return nil }
        return options.first { $0.id == selection }
    }

    private var filtered: [SelectOption] {
        let query = search.trimmingCharacters(in: .whitespaces)
        guard !query.isEmpty else { return options }
        return options.filter { $0.label.range(of: query, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
    }

    @ViewBuilder
    private func optionMarker(_ option: SelectOption) -> some View {
        if let icon = option.icon {
            DmxIcon(icon, size: 14).foregroundColor(Color(hex: option.color ?? "", fallback: .secondary))
        } else if let color = option.color {
            Circle().fill(Color(hex: color)).frame(width: 9, height: 9)
        }
    }

    private var list: some View {
        VStack(alignment: .leading, spacing: 0) {
            if options.count > 8 {
                SearchField("Rechercher...", text: $search).padding(8)
                Divider()
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    if let noneLabel = noneLabel {
                        row(label: noneLabel, marker: AnyView(EmptyView()), isSelected: selection == nil) { choose(nil) }
                    }
                    ForEach(filtered) { option in
                        row(label: option.label, marker: AnyView(optionMarker(option)), isSelected: selection == option.id) { choose(option.id) }
                    }
                }
                .padding(.vertical, 6)
            }
            .frame(height: min(CGFloat(filtered.count + (noneLabel == nil ? 0 : 1)) * 31 + 12, 320))
        }
        .frame(width: 280)
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

    private func row(label: String, marker: AnyView, isSelected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 8) {
                marker
                Text(label).font(.system(size: 13)).lineLimit(1)
                Spacer(minLength: 0)
                if isSelected {
                    DmxIcon("Check", size: 13).foregroundColor(.accentColor)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
    }

    private func choose(_ id: String?) {
        selection = id
        isOpen = false
        search = ""
    }
}

extension AppStore {
    public var accountOptions: [SelectOption] {
        accounts.map { SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color) }
    }

    /// Catégories choisissables (sans « Virement », réservée aux virements).
    public var categoryOptions: [SelectOption] {
        categories.filter { $0.id != "transfer" }.map { SelectOption(id: $0.id, label: $0.name, icon: $0.icon, color: $0.color) }
    }
}
