import SwiftUI

/// Preserve the chosen hue while keeping badge glyphs visible on very pale/dark colors.
enum IconContrast {
    struct RGB {
        var red: Double
        var green: Double
        var blue: Double

        var luminance: Double {
            func linear(_ value: Double) -> Double {
                value <= 0.04045 ? value / 12.92 : pow((value + 0.055) / 1.055, 2.4)
            }
            return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
        }
        func mixed(with other: RGB, fraction: Double) -> RGB {
            RGB(red: red + (other.red - red) * fraction,
                green: green + (other.green - green) * fraction,
                blue: blue + (other.blue - blue) * fraction)
        }
        func contrast(with other: RGB) -> Double {
            (max(luminance, other.luminance) + 0.05) / (min(luminance, other.luminance) + 0.05)
        }
        var color: Color { Color(.sRGB, red: red, green: green, blue: blue, opacity: 1) }
    }

    static func foreground(_ source: RGB, background: RGB, filled: Bool, dark: Bool) -> RGB {
        let black = RGB(red: 0, green: 0, blue: 0)
        let white = RGB(red: 1, green: 1, blue: 1)
        if filled { return black.contrast(with: background) >= white.contrast(with: background) ? black : white }
        for step in 0...20 {
            let candidate = source.mixed(with: dark ? white : black, fraction: Double(step) / 20)
            if candidate.contrast(with: background) >= 3 { return candidate }
        }
        return dark ? white : black
    }

    static func color(hex: String, filled: Bool, dark: Bool) -> Color {
        let color = PlatformColor.dmx(hex: hex)
        var r: CGFloat = 0.5, g: CGFloat = 0.5, b: CGFloat = 0.5, a: CGFloat = 1
        #if os(macOS)
        if let rgb = color?.usingColorSpace(.sRGB) {
            r = rgb.redComponent; g = rgb.greenComponent; b = rgb.blueComponent; a = rgb.alphaComponent
        }
        #else
        color?.getRed(&r, green: &g, blue: &b, alpha: &a)
        #endif
        let source = RGB(red: Double(r), green: Double(g), blue: Double(b))
        let base = dark ? 0.12 : 1.0
        let background = RGB(red: base, green: base, blue: base).mixed(with: source, fraction: Double(a) * (filled ? 1 : 0.14))
        return foreground(source, background: background, filled: filled, dark: dark).color
    }
}
