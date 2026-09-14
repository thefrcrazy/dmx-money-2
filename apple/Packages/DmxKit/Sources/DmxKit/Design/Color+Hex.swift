import SwiftUI

#if os(macOS)
import AppKit
public typealias PlatformColor = NSColor
#else
import UIKit
public typealias PlatformColor = UIColor
#endif

extension PlatformColor {
    /// Couleur `#RRGGBB`, `#RGB` ou `#RRGGBBAA` telle que stockée en base.
    public static func dmx(hex: String) -> PlatformColor? {
        var value = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("#") { value.removeFirst() }
        if value.count == 3 {
            value = value.map { "\($0)\($0)" }.joined()
        }
        guard value.count == 6 || value.count == 8, let number = UInt64(value, radix: 16) else {
            return nil
        }
        let hasAlpha = value.count == 8
        let red = CGFloat((number >> (hasAlpha ? 24 : 16)) & 0xff) / 255
        let green = CGFloat((number >> (hasAlpha ? 16 : 8)) & 0xff) / 255
        let blue = CGFloat((number >> (hasAlpha ? 8 : 0)) & 0xff) / 255
        let alpha = hasAlpha ? CGFloat(number & 0xff) / 255 : 1
        #if os(macOS)
        return NSColor(srgbRed: red, green: green, blue: blue, alpha: alpha)
        #else
        return UIColor(red: red, green: green, blue: blue, alpha: alpha)
        #endif
    }
}

extension Color {
    public init(hex: String, fallback: Color = Color.gray) {
        if let color = PlatformColor.dmx(hex: hex) {
            #if os(macOS)
            self.init(color)
            #else
            self.init(uiColor: color)
            #endif
        } else {
            self = fallback
        }
    }
}

/// Couleurs sémantiques de DmxMoney, identiques sur toutes les plateformes.
public enum DmxColors {
    public static let income = Color(hex: "#10b981")
    public static let expense = Color(hex: "#ef4444")
    public static let transfer = Color(hex: "#6366f1")
    public static let checked = Color(hex: "#059669")
    public static let warning = Color(hex: "#f97316")
    public static let intradayDanger = Color(hex: "#a855f7")
    public static let intradayWarning = Color(hex: "#eab308")
    public static let muted = Color(hex: "#9ca3af")
    public static let defaultAccent = Color(hex: "#6366f1")

    public static func accent(_ hex: String?) -> Color {
        guard let hex = hex else { return defaultAccent }
        return Color(hex: hex, fallback: defaultAccent)
    }
}

extension Color {
    /// Couleur au format `#rrggbb`, pour la stocker en base comme en 1.x.
    /// `NSColor(Color)` demande macOS 11 : la variante legacy n'utilise pas de `ColorPicker`.
    @available(macOS 11.0, iOS 14.0, *)
    public var dmxHex: String? {
        #if os(macOS)
        guard let color = NSColor(self).usingColorSpace(.sRGB) else { return nil }
        let components = (color.redComponent, color.greenComponent, color.blueComponent)
        #else
        var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
        UIColor(self).getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        let components = (red, green, blue)
        #endif
        return String(
            format: "#%02x%02x%02x",
            Int((components.0 * 255).rounded()),
            Int((components.1 * 255).rounded()),
            Int((components.2 * 255).rounded())
        )
    }
}
