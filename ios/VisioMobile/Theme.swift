import SwiftUI

// MARK: - Meet Dark Theme Palette

enum VisioColors {
    static let primaryDark50 = Color(hex: 0x161622)
    static let primaryDark75 = Color(hex: 0x222234)
    static let primaryDark100 = Color(hex: 0x2D2D46)
    static let primaryDark300 = Color(hex: 0x5A5A8F)
    static let primary500 = Color(hex: 0x6A6AF4)
    static let greyscale400 = Color(hex: 0x929292)
    static let error200 = Color(hex: 0x6C302E)
    static let error500 = Color(hex: 0xEF413D)
    static let handRaise = Color(hex: 0xFDE047)
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
