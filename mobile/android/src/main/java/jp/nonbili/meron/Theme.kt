package jp.nonbili.meron

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

enum class AppAppearanceMode(val storageValue: String, val label: String) {
    System("system", "System"),
    Indigo("indigo", "Indigo"),
    IndigoDark("indigo-dark", "Indigo Dark"),
    Light("light", "Meron Light"),
    Dark("dark", "Meron Dark"),
    Mist("mist", "Mist"),
    Paper("paper", "Paper"),
    Dawn("dawn", "Dawn"),
    Honey("honey", "Honey"),
    Lilac("lilac", "Lilac"),
    Graphite("graphite", "Graphite"),
    Midnight("midnight", "Midnight"),
    Forest("forest", "Forest"),
    Plum("plum", "Plum"),
    Ember("ember", "Ember"),
}

/** Colors that have no Material slot: the chat bubbles and dark sidebar. */
data class ChatColors(
    val sidebar: Color,
    val onSidebar: Color,
    val onSidebarMuted: Color,
    val bubbleIn: Color,
    val bubbleInText: Color,
    val bubbleOut: Color,
    val bubbleOutText: Color,
    val star: Color,
)

private data class MobileThemeSpec(
    val dark: Boolean,
    val bgApp: Color,
    val bgChats: Color,
    val bgRaised: Color,
    val bgActive: Color,
    val border: Color,
    val textPrimary: Color,
    val textSecondary: Color,
    val accent: Color,
    val accentContainer: Color,
    val onAccentContainer: Color,
    val sidebar: Color,
    val bubbleIn: Color,
    val bubbleInText: Color,
    val bubbleOut: Color,
    val bubbleOutText: Color,
)

// Built-in palettes mirror frontend/src/lib/themes.ts names and primary roles.
private val IndigoLight = MobileThemeSpec(false, Color(0xFFF1F5F9), Color.White, Color(0xFFF8FAFC), Color(0xFFE2E8F0), Color(0xFFE2E8F0), Color(0xFF0F172A), Color(0xFF64748B), Color(0xFF4F46E5), Color(0xFFE0E7FF), Color(0xFF312E81), Color(0xFF0F172A), Color.White, Color(0xFF0F172A), Color(0xFFE0E7FF), Color(0xFF312E81))
private val IndigoDark = MobileThemeSpec(true, Color(0xFF090D16), Color(0xFF0F172A), Color(0xFF111B2E), Color(0xFF1E293B), Color(0xFF1E293B), Color(0xFFF8FAFC), Color(0xFF94A3B8), Color(0xFF6366F1), Color(0xFF312E81), Color(0xFFE0E7FF), Color(0xFF05070C), Color(0xFF1E293B), Color(0xFFF8FAFC), Color(0xFF312E81), Color(0xFFE0E7FF))
private val MeronLight = MobileThemeSpec(false, Color(0xFFFFFFFF), Color.White, Color(0xFFF8FAFC), Color(0xFFE2E8F0), Color(0xFFE2E8F0), Color(0xFF111827), Color(0xFF6B7280), Color(0xFF2563EB), Color(0xFFDBEAFE), Color(0xFF1E3A8A), Color(0xFF111827), Color.White, Color(0xFF111827), Color(0xFFDBEAFE), Color(0xFF1E3A8A))
private val MeronDark = MobileThemeSpec(true, Color(0xFF111827), Color(0xFF1F2937), Color(0xFF172033), Color(0xFF374151), Color(0xFF374151), Color(0xFFF9FAFB), Color(0xFFD1D5DB), Color(0xFF60A5FA), Color(0xFF1E3A8A), Color(0xFFDBEAFE), Color(0xFF0B1020), Color(0xFF374151), Color(0xFFF9FAFB), Color(0xFF1E3A8A), Color(0xFFDBEAFE))
private val Mist = MobileThemeSpec(false, Color(0xFFEDF4F7), Color.White, Color(0xFFF4FAFB), Color(0xFFD7EAEF), Color(0xFFCFE0E5), Color(0xFF14323C), Color(0xFF6F8790), Color(0xFF0EA5B7), Color(0xFFD5F0F4), Color(0xFF0E5663), Color(0xFF123947), Color.White, Color(0xFF14323C), Color(0xFFD5F0F4), Color(0xFF0E5663))
private val Paper = MobileThemeSpec(false, Color(0xFFF4F1EA), Color(0xFFFFFDF8), Color(0xFFFAF6EE), Color(0xFFE6DED1), Color(0xFFDED4C4), Color(0xFF2F3A3D), Color(0xFF7B817D), Color(0xFF64748B), Color(0xFFE5EDF2), Color(0xFF334155), Color(0xFF263238), Color(0xFFFFFDF8), Color(0xFF2F3A3D), Color(0xFFE5EDF2), Color(0xFF334155))
private val Dawn = MobileThemeSpec(false, Color(0xFFF7EDE8), Color(0xFFFFFAF7), Color(0xFFFFF6F2), Color(0xFFEAD8D1), Color(0xFFE1CFC7), Color(0xFF4A3F4D), Color(0xFF897C83), Color(0xFFC06C84), Color(0xFFF5DADA), Color(0xFF753849), Color(0xFF35263B), Color(0xFFFFFAF7), Color(0xFF4A3F4D), Color(0xFFF5DADA), Color(0xFF753849))
private val Honey = MobileThemeSpec(false, Color(0xFFF7F1E6), Color(0xFFFFFDF7), Color(0xFFFAF4E8), Color(0xFFEADFC6), Color(0xFFE7DCC2), Color(0xFF3A3122), Color(0xFF8B8068), Color(0xFFB07C10), Color(0xFFF5E6C4), Color(0xFF6E4D09), Color(0xFF33270F), Color(0xFFFFFDF7), Color(0xFF3A3122), Color(0xFFF5E6C4), Color(0xFF6E4D09))
private val Lilac = MobileThemeSpec(false, Color(0xFFF2F0F8), Color(0xFFFDFCFF), Color(0xFFF6F4FB), Color(0xFFDFD9EE), Color(0xFFDED8EA), Color(0xFF34304A), Color(0xFF7E7894), Color(0xFF7A5BC4), Color(0xFFE8DEF8), Color(0xFF4B3389), Color(0xFF2B2440), Color(0xFFFDFCFF), Color(0xFF34304A), Color(0xFFE8DEF8), Color(0xFF4B3389))
private val Graphite = MobileThemeSpec(true, Color(0xFF181A1F), Color(0xFF23262D), Color(0xFF202329), Color(0xFF343842), Color(0xFF393E49), Color(0xFFEEF0F3), Color(0xFFA8B0BC), Color(0xFF8B9BB4), Color(0xFF3A4350), Color(0xFFEEF3F8), Color(0xFF111318), Color(0xFF2B2F38), Color(0xFFEEF0F3), Color(0xFF3A4350), Color(0xFFEEF3F8))
private val Midnight = MobileThemeSpec(true, Color(0xFF0B1120), Color(0xFF111827), Color(0xFF101827), Color(0xFF1E293B), Color(0xFF26354D), Color(0xFFF8FAFC), Color(0xFF94A3B8), Color(0xFF38BDF8), Color(0xFF12324A), Color(0xFFDFF6FF), Color(0xFF050814), Color(0xFF1E293B), Color(0xFFF8FAFC), Color(0xFF12324A), Color(0xFFDFF6FF))
private val Forest = MobileThemeSpec(true, Color(0xFF101813), Color(0xFF17231C), Color(0xFF152018), Color(0xFF263A2E), Color(0xFF2F4638), Color(0xFFF0F6EF), Color(0xFFA6B8AA), Color(0xFF7CCF9B), Color(0xFF264936), Color(0xFFE2F8E9), Color(0xFF0B120E), Color(0xFF203126), Color(0xFFF0F6EF), Color(0xFF264936), Color(0xFFE2F8E9))
private val Plum = MobileThemeSpec(true, Color(0xFF151019), Color(0xFF1F1826), Color(0xFF1C1522), Color(0xFF332945), Color(0xFF3A2F4B), Color(0xFFF2EEF6), Color(0xFFA89DB8), Color(0xFFB48AE0), Color(0xFF3E2F58), Color(0xFFECDFFB), Color(0xFF0D0A11), Color(0xFF2A2135), Color(0xFFF2EEF6), Color(0xFF3E2F58), Color(0xFFECDFFB))
private val Ember = MobileThemeSpec(true, Color(0xFF181210), Color(0xFF231A15), Color(0xFF201813), Color(0xFF3B2C21), Color(0xFF423227), Color(0xFFF6EFE9), Color(0xFFB4A294), Color(0xFFE1854C), Color(0xFF4F3320), Color(0xFFFAE3CF), Color(0xFF0F0B09), Color(0xFF2E221B), Color(0xFFF6EFE9), Color(0xFF4F3320), Color(0xFFFAE3CF))

val LocalChatColors = staticCompositionLocalOf { chatColors(IndigoLight) }

@Composable
fun MeronTheme(
    appearanceMode: AppAppearanceMode = AppAppearanceMode.System,
    content: @Composable () -> Unit,
) {
    val systemDark = isSystemInDarkTheme()
    val spec = mobileThemeSpec(appearanceMode, systemDark)
    androidx.compose.runtime.CompositionLocalProvider(
        LocalChatColors provides chatColors(spec),
    ) {
        MaterialTheme(colorScheme = materialColors(spec), content = content)
    }
}

private fun mobileThemeSpec(mode: AppAppearanceMode, systemDark: Boolean): MobileThemeSpec = when (mode) {
    AppAppearanceMode.System -> if (systemDark) IndigoDark else IndigoLight
    AppAppearanceMode.Indigo -> IndigoLight
    AppAppearanceMode.IndigoDark -> IndigoDark
    AppAppearanceMode.Light -> MeronLight
    AppAppearanceMode.Dark -> MeronDark
    AppAppearanceMode.Mist -> Mist
    AppAppearanceMode.Paper -> Paper
    AppAppearanceMode.Dawn -> Dawn
    AppAppearanceMode.Honey -> Honey
    AppAppearanceMode.Lilac -> Lilac
    AppAppearanceMode.Graphite -> Graphite
    AppAppearanceMode.Midnight -> Midnight
    AppAppearanceMode.Forest -> Forest
    AppAppearanceMode.Plum -> Plum
    AppAppearanceMode.Ember -> Ember
}

private fun materialColors(spec: MobileThemeSpec) = if (spec.dark) {
    darkColorScheme(
        primary = spec.accent,
        onPrimary = Color.White,
        primaryContainer = spec.accentContainer,
        onPrimaryContainer = spec.onAccentContainer,
        secondary = spec.textSecondary,
        onSecondary = spec.bgApp,
        secondaryContainer = spec.bgActive,
        onSecondaryContainer = spec.textPrimary,
        background = spec.bgApp,
        onBackground = spec.textPrimary,
        surface = spec.bgChats,
        onSurface = spec.textPrimary,
        surfaceVariant = spec.bgActive,
        onSurfaceVariant = spec.textSecondary,
        surfaceContainer = spec.bgRaised,
        surfaceContainerHigh = spec.bgActive,
        outline = spec.border,
        outlineVariant = spec.border,
        error = Color(0xFFF87171),
        onError = Color(0xFF0F172A),
    )
} else {
    lightColorScheme(
        primary = spec.accent,
        onPrimary = Color.White,
        primaryContainer = spec.accentContainer,
        onPrimaryContainer = spec.onAccentContainer,
        secondary = spec.textSecondary,
        onSecondary = Color.White,
        secondaryContainer = spec.bgActive,
        onSecondaryContainer = spec.textPrimary,
        background = spec.bgApp,
        onBackground = spec.textPrimary,
        surface = spec.bgChats,
        onSurface = spec.textPrimary,
        surfaceVariant = spec.bgRaised,
        onSurfaceVariant = spec.textSecondary,
        surfaceContainer = spec.bgRaised,
        surfaceContainerHigh = spec.bgActive,
        outline = spec.border,
        outlineVariant = spec.border,
        error = Color(0xFFDC2626),
        onError = Color.White,
    )
}

private fun chatColors(spec: MobileThemeSpec): ChatColors = ChatColors(
    sidebar = spec.sidebar,
    onSidebar = Color(0xFFF8FAFC),
    onSidebarMuted = if (spec.dark) Color(0xFFA8B0BC) else Color(0xFFCBD5E1),
    bubbleIn = spec.bubbleIn,
    bubbleInText = spec.bubbleInText,
    bubbleOut = spec.bubbleOut,
    bubbleOutText = spec.bubbleOutText,
    star = if (spec.dark) Color(0xFFFBBF24) else Color(0xFFF59E0B),
)
