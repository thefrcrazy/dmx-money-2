import SwiftUI

#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Icône d'un nom stocké en base (catalogue partagé avec les autres plateformes), rendue avec les
/// SF Symbols du système et teintée par la couleur du texte. Seul macOS 10.15, qui n'a pas de
/// SF Symbols, retombe sur les dessins embarqués.
public struct DmxIcon: View {
    private let name: String
    private let size: CGFloat

    public init(_ name: String, size: CGFloat = 16) {
        self.name = name
        self.size = size
    }

    public var body: some View {
        DmxIcon.swiftUIImage(name)
            .resizable()
            .aspectRatio(contentMode: .fit)
            .frame(width: size, height: size)
    }

    /// SF Symbol correspondant au nom stocké en base (« tag » par défaut).
    public static func symbolName(for name: String) -> String {
        SymbolNames.map[name] ?? "tag"
    }

    /// Image SwiftUI en mode modèle (barres d'onglets, listes système).
    public static func swiftUIImage(_ name: String) -> Image {
        if #available(macOS 11, iOS 14, *) {
            return Image(systemName: availableSymbol(for: name))
        }
        return Image(resolvedName(name), bundle: .module).renderingMode(.template)
    }

    /// Symbole présent sur le système en cours : certains n'existent qu'à partir d'une version
    /// récente, « tag » les remplace alors.
    @available(macOS 11, iOS 14, *)
    static func availableSymbol(for name: String) -> String {
        let symbol = symbolName(for: name)
        lock.lock()
        defer { lock.unlock() }
        if let known = symbolCache[symbol] { return known }
        #if os(macOS)
        let exists = NSImage(systemSymbolName: symbol, accessibilityDescription: nil) != nil
        #else
        let exists = UIImage(systemName: symbol) != nil
        #endif
        let resolved = exists ? symbol : "tag"
        symbolCache[symbol] = resolved
        return resolved
    }

    /// Dessin embarqué (macOS 10.15) : nom disponible dans le catalogue, « Tag » sinon.
    public static func resolvedName(_ name: String) -> String {
        exists(name) ? name : "Tag"
    }

    private static var cache: [String: Bool] = [:]
    private static var symbolCache: [String: String] = [:]
    private static let lock = NSLock()

    public static func exists(_ name: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if let known = cache[name] { return known }
        #if os(macOS)
        var found = Bundle.module.image(forResource: NSImage.Name(name)) != nil
        #else
        var found = UIImage(named: name, in: .module, compatibleWith: nil) != nil
        #endif
        if !found {
            // Construit par SwiftPM en ligne de commande, le catalogue n'est pas compilé :
            // on regarde alors le fichier vectoriel dans le bundle.
            found = Bundle.module.url(forResource: "Icons.xcassets/\(name).imageset/\(name)", withExtension: "svg") != nil
        }
        cache[name] = found
        return found
    }

    #if os(macOS)
    /// Image AppKit pour les tableaux et menus.
    public static func image(_ name: String, size: CGFloat = 16) -> NSImage? {
        if #available(macOS 11, *),
           let symbol = NSImage(systemSymbolName: availableSymbol(for: name), accessibilityDescription: nil)?
               .withSymbolConfiguration(NSImage.SymbolConfiguration(pointSize: size * 0.8, weight: .regular)) {
            symbol.isTemplate = true
            return symbol
        }
        guard let image = Bundle.module.image(forResource: NSImage.Name(resolvedName(name)))?.copy() as? NSImage else {
            return nil
        }
        image.size = NSSize(width: size, height: size)
        image.isTemplate = true
        return image
    }
    #endif
}

/// Pastille arrondie teintée de la couleur d'un compte ou d'une catégorie.
public struct IconBadge: View {
    @Environment(\.colorScheme) private var colorScheme
    private let icon: String
    private let colorHex: String
    private let size: CGFloat
    private let filled: Bool

    public init(icon: String, colorHex: String, size: CGFloat = 32, filled: Bool = false) {
        self.icon = icon
        self.colorHex = colorHex
        self.size = size
        self.filled = filled
    }

    public var body: some View {
        let color = Color(hex: colorHex, fallback: DmxColors.muted)
        return ZStack {
            RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
                .fill(filled ? color : color.opacity(0.14))
            DmxIcon(icon, size: size * 0.5)
                .foregroundColor(IconContrast.color(hex: colorHex, filled: filled, dark: colorScheme == .dark))
        }
        .frame(width: size, height: size)
    }
}
