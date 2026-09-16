import XCTest
@testable import DmxKit

final class IconContrastTests: XCTestCase {
    func testPaleAndDarkBadgeGlyphsKeepContrastInBothThemes() {
        let colors = [
            IconContrast.RGB(red: 1, green: 1, blue: 1),
            IconContrast.RGB(red: 0, green: 0, blue: 0),
            IconContrast.RGB(red: 0.65, green: 0.95, blue: 0.2),
            IconContrast.RGB(red: 0.1, green: 0.2, blue: 0.8),
        ]
        for dark in [false, true] {
            let base = dark ? 0.12 : 1.0
            for source in colors {
                let background = IconContrast.RGB(red: base, green: base, blue: base).mixed(with: source, fraction: 0.14)
                let foreground = IconContrast.foreground(source, background: background, filled: false, dark: dark)
                XCTAssertGreaterThanOrEqual(foreground.contrast(with: background), 3)
                let filled = IconContrast.foreground(source, background: source, filled: true, dark: dark)
                XCTAssertGreaterThanOrEqual(filled.contrast(with: source), 4.5)
            }
        }
    }
}
