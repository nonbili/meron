import SwiftUI

func iosPreferredColorScheme(_ mode: String) -> ColorScheme? {
    guard let dark = iosThemeOption(mode).dark else { return nil }
    return dark ? .dark : .light
}

func iosThemeTint(_ mode: String) -> Color {
    iosThemeOption(mode).accent
}
