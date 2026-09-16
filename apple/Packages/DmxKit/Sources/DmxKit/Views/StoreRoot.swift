import SwiftUI

/// Racine des vues hébergées : injecte le store et applique la couleur d'accentuation choisie.
public struct StoreRoot<Content: View>: View {
    @Environment(\.colorScheme) private var inheritedColorScheme
    @ObservedObject private var store: AppStore
    private let content: Content

    public init(store: AppStore, @ViewBuilder content: () -> Content) {
        self.store = store
        self.content = content()
    }

    public var body: some View {
        content
            .environmentObject(store)
            .environment(\.colorScheme, resolvedColorScheme)
            .accentColor(DmxColors.accent(store.settings.accentColor))
    }
    // AppKit changes its appearance separately. Explicitly pass forced themes to every
    // hosted SwiftUI page, including views retained while they are off screen on Catalina.
    private var resolvedColorScheme: ColorScheme {
        switch store.settings.theme {
        case .light: return .light
        case .dark: return .dark
        case .system: return inheritedColorScheme
        }
    }
}

/// Lecture des fichiers importés (relevés bancaires souvent en Windows-1252).
public enum FileText {
    public static func read(_ url: URL) -> String? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return decode(data)
    }

    public static func decode(_ data: Data) -> String? {
        for encoding in [String.Encoding.utf8, .windowsCP1252, .isoLatin1] {
            if let text = String(data: data, encoding: encoding) {
                return text.hasPrefix("\u{FEFF}") ? String(text.dropFirst()) : text
            }
        }
        return nil
    }
}
