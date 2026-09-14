import SwiftUI

#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Icône Lucide (nom stocké en base), rendue en mode modèle pour prendre la couleur du texte.
public struct DmxIcon: View {
    private let name: String
    private let size: CGFloat

    public init(_ name: String, size: CGFloat = 16) {
        self.name = name
        self.size = size
    }

    public var body: some View {
        Image(DmxIcon.resolvedName(name), bundle: .module)
            .renderingMode(.template)
            .resizable()
            .aspectRatio(contentMode: .fit)
            .frame(width: size, height: size)
    }

    /// Nom disponible dans le catalogue, « Tag » sinon (comme le repli de 1.x).
    public static func resolvedName(_ name: String) -> String {
        exists(name) ? name : "Tag"
    }

    /// Image SwiftUI en mode modèle (barres d'onglets, listes système).
    public static func swiftUIImage(_ name: String) -> Image {
        Image(resolvedName(name), bundle: .module).renderingMode(.template)
    }

    private static var cache: [String: Bool] = [:]
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
                .foregroundColor(filled ? .white : color)
        }
        .frame(width: size, height: size)
    }
}
