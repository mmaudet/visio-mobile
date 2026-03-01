import SwiftUI

// MARK: - Meet Theme Palette (Dynamic Light/Dark)

enum VisioColors {
    // Dark palette
    static let primaryDark50 = Color(hex: 0x161622)
    static let primaryDark75 = Color(hex: 0x222234)
    static let primaryDark100 = Color(hex: 0x2D2D46)
    static let primaryDark300 = Color(hex: 0x5A5A8F)
    static let primary500 = Color(hex: 0x6A6AF4)
    static let greyscale400 = Color(hex: 0x929292)
    static let error200 = Color(hex: 0x6C302E)
    static let error500 = Color(hex: 0xEF413D)
    static let handRaise = Color(hex: 0xFDE047)

    // Light palette
    static let lightBackground = Color.white
    static let lightSurface = Color(hex: 0xF5F5F7)
    static let lightSurfaceVariant = Color(hex: 0xE8E8ED)
    static let lightOnBackground = Color(hex: 0x1D1D1F)
    static let lightBorder = Color(hex: 0xD1D1D6)
    static let lightTextSecondary = Color(hex: 0x6E6E73)
    static let lightErrorBg = Color(hex: 0xFDE8E7)

    // Dynamic resolution based on theme
    static func background(dark: Bool) -> Color { dark ? primaryDark50 : lightBackground }
    static func surface(dark: Bool) -> Color { dark ? primaryDark75 : lightSurface }
    static func surfaceVariant(dark: Bool) -> Color { dark ? primaryDark100 : lightSurfaceVariant }
    static func onBackground(dark: Bool) -> Color { dark ? .white : lightOnBackground }
    static func onSurface(dark: Bool) -> Color { dark ? .white : lightOnBackground }
    static func secondaryText(dark: Bool) -> Color { dark ? greyscale400 : lightTextSecondary }
    static func border(dark: Bool) -> Color { dark ? greyscale400 : lightBorder }
}

extension Color {
    init(hex: UInt, alpha: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: alpha
        )
    }
}
