package jp.nonbili.meron.ui

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance

/** A theme the app can paint: a built-in [AppAppearanceMode] or an imported [CustomTheme]. */
sealed interface AppTheme {
    /** What the theme settings store: a built-in's name, or a custom theme's id. */
    val storageValue: String
    val label: String
    val isDark: Boolean
}

enum class AppAppearanceMode(
    override val storageValue: String,
    override val label: String,
) : AppTheme {
    Light("light", "Meron Light"),
    Dynamic("dynamic", "Material You"),
    Indigo("indigo", "Indigo"),
    Dark("dark", "Meron Dark"),
    DynamicDark("dynamic-dark", "Material You Dark"),
    IndigoDark("indigo-dark", "Indigo Dark"),
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
    ;

    override val isDark: Boolean get() = mobileThemeSpec(this).dark
}

/** The themes that take their colors from the system wallpaper palette (Material You). */
internal val AppAppearanceMode.isDynamic: Boolean
    get() = this == AppAppearanceMode.Dynamic || this == AppAppearanceMode.DynamicDark

/**
 * The theme setting: one fixed theme, or a light and a dark pick the app
 * switches between with the system appearance. Mirrors desktop's themeId /
 * themeFollowSystem / lightThemeId / darkThemeId.
 */
data class ThemeChoice(
    val fixed: AppTheme = AppAppearanceMode.Light,
    val followSystem: Boolean = false,
    val light: AppTheme = AppAppearanceMode.Light,
    val dark: AppTheme = AppAppearanceMode.Dark,
) {
    /** The theme to paint while the system is (or is not) dark. */
    fun resolve(systemDark: Boolean): AppTheme =
        when {
            !followSystem -> fixed
            systemDark -> dark
            else -> light
        }

    /** The themes the picker marks as chosen. */
    val chosen: Set<AppTheme> get() = if (followSystem) setOf(light, dark) else setOf(fixed)

    /** Pick [mode]: the fixed theme, or while following the system, the pick for its own appearance. */
    fun select(mode: AppTheme): ThemeChoice =
        when {
            !followSystem -> copy(fixed = mode)
            mode.isDark -> copy(dark = mode)
            else -> copy(light = mode)
        }

    /**
     * Turn following the system on or off without changing what is on screen
     * when the system already matches: the painted theme becomes the pick for
     * its appearance, or the fixed theme when turning off.
     */
    fun withFollowSystem(
        enabled: Boolean,
        systemDark: Boolean,
    ): ThemeChoice {
        if (enabled == followSystem) return this
        val current = resolve(systemDark)
        return when {
            !enabled -> copy(followSystem = false, fixed = current)
            current.isDark -> copy(followSystem = true, dark = current)
            else -> copy(followSystem = true, light = current)
        }
    }

    /** Every pick of [theme], which is going away, reset to the default for its appearance. */
    fun without(theme: AppTheme): ThemeChoice {
        val defaults = ThemeChoice()
        val replacement = if (theme.isDark) defaults.dark else defaults.light
        return ThemeChoice(
            fixed = if (fixed == theme) replacement else fixed,
            followSystem = followSystem,
            light = if (light == theme) defaults.light else light,
            dark = if (dark == theme) defaults.dark else dark,
        )
    }
}

/** Colors that have no Material slot: the chat bubbles and the sidebar. */
data class ChatColors(
    val sidebar: Color,
    val onSidebar: Color,
    val onSidebarMuted: Color,
    val sidebarAccent: Color,
    val bubbleIn: Color,
    val bubbleInText: Color,
    val bubbleOut: Color,
    val bubbleOutText: Color,
    val star: Color,
    val unreadBackground: Color,
    val unreadText: Color,
    val sidebarUnreadBackground: Color,
    val sidebarUnreadText: Color,
    /** The fill behind the selected drawer row. */
    val sidebarSelected: Color,
)

internal data class MobileThemeSpec(
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
    /** Drawer colors from the theme's own roles (Material You); null derives them like desktop's rail. */
    val sidebarColors: SidebarColors? = null,
)

internal data class SidebarColors(
    val text: Color,
    val textMuted: Color,
    val selected: Color,
    val selectedContent: Color,
    val unreadBackground: Color,
    val unreadText: Color,
)

// Built-in palettes mirror desktop/frontend/src/lib/themes.ts names and primary roles.
private val IndigoLight =
    MobileThemeSpec(
        false,
        Color(0xFFF1F5F9),
        Color.White,
        Color(0xFFF8FAFC),
        Color(0xFFDFE5ED),
        Color(0xFFDCE2EA),
        Color(0xFF0F172A),
        Color(0xFF607086),
        Color(0xFF6558CC),
        Color(0xFFE0E7FF),
        Color(0xFF312E81),
        Color(0xFFE3E9EE),
        Color.White,
        Color(0xFF0F172A),
        Color(0xFFE0E7FF),
        Color(0xFF312E81),
    )
private val IndigoDark =
    MobileThemeSpec(
        true,
        Color(0xFF090D16),
        Color(0xFF0F172A),
        Color(0xFF111B2E),
        Color(0xFF202B3D),
        Color(0xFF253042),
        Color(0xFFF8FAFC),
        Color(0xFF94A3B8),
        Color(0xFF897FE0),
        Color(0xFF2A2577),
        Color(0xFFE0E7FF),
        Color(0xFF05070C),
        Color(0xFF192435),
        Color(0xFFF8FAFC),
        Color(0xFF2A2577),
        Color(0xFFE0E7FF),
    )
private val MeronLight =
    MobileThemeSpec(
        false,
        Color(0xFFF0F2F1),
        Color.White,
        Color(0xFFF7F9F8),
        Color(0xFFE1E6E3),
        Color(0xFFDDE3DF),
        Color(0xFF1B211E),
        Color(0xFF65716B),
        Color(0xFF0E7A58),
        Color(0xFFDCEDE5),
        Color(0xFF14543E),
        Color(0xFFE6E9E7),
        Color.White,
        Color(0xFF1B211E),
        Color(0xFFDCEDE5),
        Color(0xFF14543E),
    )
private val MeronDark =
    MobileThemeSpec(
        true,
        Color(0xFF0C100E),
        Color(0xFF151B18),
        Color(0xFF111A15),
        Color(0xFF262F2A),
        Color(0xFF29352F),
        Color(0xFFF2F5F3),
        Color(0xFF98A39D),
        Color(0xFF40A984),
        Color(0xFF153F33),
        Color(0xFFD6EEE2),
        Color(0xFF060908),
        Color(0xFF1F2823),
        Color(0xFFF2F5F3),
        Color(0xFF153F33),
        Color(0xFFD6EEE2),
    )
private val Mist =
    MobileThemeSpec(
        false,
        Color(0xFFEDF4F7),
        Color.White,
        Color(0xFFF4FAFB),
        Color(0xFFD6E9EE),
        Color(0xFFD4E5EA),
        Color(0xFF14323C),
        Color(0xFF5B727B),
        Color(0xFF008292),
        Color(0xFFD3EEF2),
        Color(0xFF0E5663),
        Color(0xFFE0EAEE),
        Color.White,
        Color(0xFF14323C),
        Color(0xFFD3EEF2),
        Color(0xFF0E5663),
    )
private val Paper =
    MobileThemeSpec(
        false,
        Color(0xFFF4F1EA),
        Color(0xFFFFFDF8),
        Color(0xFFFAF6EE),
        Color(0xFFEAE2D5),
        Color(0xFFE8DECE),
        Color(0xFF2F3A3D),
        Color(0xFF6A6F6C),
        Color(0xFF64748B),
        Color(0xFFDFE7EC),
        Color(0xFF334155),
        Color(0xFFEAE6DC),
        Color(0xFFFFFDF8),
        Color(0xFF2F3A3D),
        Color(0xFFDFE7EC),
        Color(0xFF334155),
    )
private val Dawn =
    MobileThemeSpec(
        false,
        Color(0xFFF7EDE8),
        Color(0xFFFFFAF7),
        Color(0xFFFFF6F2),
        Color(0xFFEEDCD5),
        Color(0xFFEBD9D1),
        Color(0xFF4A3F4D),
        Color(0xFF766970),
        Color(0xFFAC5A72),
        Color(0xFFF8DCDC),
        Color(0xFF753849),
        Color(0xFFEFE1DA),
        Color(0xFFFFFAF7),
        Color(0xFF4A3F4D),
        Color(0xFFF8DCDC),
        Color(0xFF753849),
    )
private val Honey =
    MobileThemeSpec(
        false,
        Color(0xFFF7F1E6),
        Color(0xFFFFFDF7),
        Color(0xFFFAF4E8),
        Color(0xFFEDE2C9),
        Color(0xFFE9DEC4),
        Color(0xFF3A3122),
        Color(0xFF786D56),
        Color(0xFF9F6B00),
        Color(0xFFF4E5C3),
        Color(0xFF6E4D09),
        Color(0xFFEDE5D6),
        Color(0xFFFFFDF7),
        Color(0xFF3A3122),
        Color(0xFFF4E5C3),
        Color(0xFF6E4D09),
    )
private val Lilac =
    MobileThemeSpec(
        false,
        Color(0xFFF2F0F8),
        Color(0xFFFDFCFF),
        Color(0xFFF6F4FB),
        Color(0xFFE4DEF3),
        Color(0xFFE1DBED),
        Color(0xFF34304A),
        Color(0xFF6F6985),
        Color(0xFF7A5BC4),
        Color(0xFFEAE0FA),
        Color(0xFF4B3389),
        Color(0xFFE6E3EF),
        Color(0xFFFDFCFF),
        Color(0xFF34304A),
        Color(0xFFEAE0FA),
        Color(0xFF4B3389),
    )
private val Graphite =
    MobileThemeSpec(
        true,
        Color(0xFF181A1F),
        Color(0xFF23262D),
        Color(0xFF202329),
        Color(0xFF363A44),
        Color(0xFF3A3F4A),
        Color(0xFFEEF0F3),
        Color(0xFFA8B0BC),
        Color(0xFF8B9BB4),
        Color(0xFF3C4552),
        Color(0xFFEEF3F8),
        Color(0xFF111318),
        Color(0xFF2E323C),
        Color(0xFFEEF0F3),
        Color(0xFF3C4552),
        Color(0xFFEEF3F8),
    )
private val Midnight =
    MobileThemeSpec(
        true,
        Color(0xFF0B1120),
        Color(0xFF111827),
        Color(0xFF101827),
        Color(0xFF212C3E),
        Color(0xFF223148),
        Color(0xFFF8FAFC),
        Color(0xFF94A3B8),
        Color(0xFF55ADD5),
        Color(0xFF193851),
        Color(0xFFDFF6FF),
        Color(0xFF040712),
        Color(0xFF192436),
        Color(0xFFF8FAFC),
        Color(0xFF193851),
        Color(0xFFDFF6FF),
    )
private val Forest =
    MobileThemeSpec(
        true,
        Color(0xFF101813),
        Color(0xFF17231C),
        Color(0xFF152018),
        Color(0xFF25392D),
        Color(0xFF283F31),
        Color(0xFFF0F6EF),
        Color(0xFFA6B8AA),
        Color(0xFF7CCF9B),
        Color(0xFF234633),
        Color(0xFFE2F8E9),
        Color(0xFF09100C),
        Color(0xFF1F3025),
        Color(0xFFF0F6EF),
        Color(0xFF234633),
        Color(0xFFE2F8E9),
    )
private val Plum =
    MobileThemeSpec(
        true,
        Color(0xFF151019),
        Color(0xFF1F1826),
        Color(0xFF1C1522),
        Color(0xFF332945),
        Color(0xFF392E49),
        Color(0xFFF2EEF6),
        Color(0xFFA89DB8),
        Color(0xFFB48AE0),
        Color(0xFF3F3059),
        Color(0xFFECDFFB),
        Color(0xFF0B080F),
        Color(0xFF2C2337),
        Color(0xFFF2EEF6),
        Color(0xFF3F3059),
        Color(0xFFECDFFB),
    )
private val Ember =
    MobileThemeSpec(
        true,
        Color(0xFF181210),
        Color(0xFF231A15),
        Color(0xFF201813),
        Color(0xFF3B2C21),
        Color(0xFF413126),
        Color(0xFFF6EFE9),
        Color(0xFFB4A294),
        Color(0xFFE1854C),
        Color(0xFF4E321F),
        Color(0xFFFAE3CF),
        Color(0xFF0E0A08),
        Color(0xFF31251E),
        Color(0xFFF6EFE9),
        Color(0xFF4E321F),
        Color(0xFFFAE3CF),
    )

val LocalChatColors = staticCompositionLocalOf { chatColors(IndigoLight) }

/** Message body text size, as a percentage of the default (see AppFonts.kt). */
val LocalMessageFontScale = staticCompositionLocalOf { DEFAULT_MESSAGE_FONT_SCALE }

/** Whether HTML mail bodies are drawn darkened: the setting is on and the theme
 *  in effect is a dark one (see HtmlMessageBody). */
val LocalDarkMailBodies = staticCompositionLocalOf { false }

/** Whether conversation bubbles shrink over-wide HTML mail to fit (see
 *  MailWebView's `fitWideContent`). */
val LocalAutoFitMessages = staticCompositionLocalOf { false }

/** Whether chat bubbles grow to fit long bodies instead of scrolling inside a
 *  capped box (see MessageBubble). */
val LocalChatFullMessages = staticCompositionLocalOf { false }

/** The handful of colors a theme swatch paints, mirroring desktop's ThemeSwatch. */
internal data class ThemePreviewColors(
    val dark: Boolean,
    val bgApp: Color,
    val bgSideNav: Color,
    val bgChats: Color,
    val border: Color,
    val textPrimary: Color,
    val bubbleIn: Color,
    val bubbleOut: Color,
    val accent: Color,
)

/** Swatch colors for [mode], so a theme can be previewed without being applied. */
@Composable
internal fun themePreviewColors(
    mode: AppTheme,
): ThemePreviewColors =
    resolveThemeSpec(mode).spec.let { spec ->
        ThemePreviewColors(
            dark = spec.dark,
            bgApp = spec.bgApp,
            bgSideNav = spec.sidebar,
            bgChats = spec.bgChats,
            border = spec.border,
            textPrimary = spec.textPrimary,
            bubbleIn = spec.bubbleIn,
            bubbleOut = spec.bubbleOut,
            accent = spec.accent,
        )
    }

@Composable
fun MeronTheme(
    appearanceMode: AppTheme = AppAppearanceMode.Light,
    messageFontScale: Int = DEFAULT_MESSAGE_FONT_SCALE,
    darkMailBodies: Boolean = false,
    autoFitMessages: Boolean = false,
    chatFullMessages: Boolean = false,
    content: @Composable () -> Unit,
) {
    val resolved = resolveThemeSpec(appearanceMode)
    val spec = resolved.spec
    SyncSystemBarAppearance(spec.dark)
    androidx.compose.runtime.CompositionLocalProvider(
        LocalChatColors provides chatColors(spec),
        LocalMessageFontScale provides messageFontScale,
        LocalDarkMailBodies provides (darkMailBodies && spec.dark),
        LocalAutoFitMessages provides autoFitMessages,
        LocalChatFullMessages provides chatFullMessages,
    ) {
        MaterialTheme(colorScheme = resolved.scheme ?: materialColors(spec), content = content)
    }
}

private fun mobileThemeSpec(
    mode: AppAppearanceMode,
): MobileThemeSpec =
    when (mode) {
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

        // Stand-ins for when the system palette is unavailable; see resolveThemeSpec.
        AppAppearanceMode.Dynamic -> MeronLight

        AppAppearanceMode.DynamicDark -> MeronDark
    }

private class ResolvedTheme(
    val spec: MobileThemeSpec,
    /** The system's own Material scheme for a dynamic theme, used as is. */
    val scheme: ColorScheme? = null,
)

/**
 * The spec for [theme]. A custom theme derives it from its source colors. A
 * dynamic theme reads the system palette and derives the spec (for chat colors
 * and swatches) from it; without one it falls back to the Meron theme of the
 * same appearance.
 */
@Composable
private fun resolveThemeSpec(theme: AppTheme): ResolvedTheme {
    val mode =
        when (theme) {
            is CustomTheme -> return ResolvedTheme(remember(theme.source) { customThemeSpec(theme.source) })
            is AppAppearanceMode -> theme
        }
    val fallback = mobileThemeSpec(mode)
    if (!mode.isDynamic) return ResolvedTheme(fallback)
    val platform = platformDynamicColorScheme(fallback.dark) ?: return ResolvedTheme(fallback)
    // Material's background and surface are one color, which would melt the
    // message cards (drawn on surface) into the canvas behind them. Meron's own
    // themes set the canvas apart from the cards, so do the same here: a tinted
    // canvas under lighter cards, or a deeper one under them in dark.
    val canvas = if (fallback.dark) platform.surfaceContainerLowest else platform.surfaceContainer
    // Material's outlines (tone 50 and 80) draw much heavier lines than Meron's
    // own borders; soften them to the same weight against the cards.
    val scheme =
        platform.copy(
            background = canvas,
            outline = platform.onSurfaceVariant.copy(alpha = 0.55f),
            outlineVariant = lerp(platform.outlineVariant, platform.surface, 0.5f),
        )
    return ResolvedTheme(dynamicThemeSpec(scheme, fallback.dark), scheme)
}

private fun dynamicThemeSpec(
    scheme: ColorScheme,
    dark: Boolean,
) = MobileThemeSpec(
    dark = dark,
    bgApp = scheme.background,
    bgChats = scheme.surface,
    bgRaised = scheme.surfaceContainer,
    bgActive = scheme.surfaceContainerHigh,
    border = scheme.outlineVariant,
    textPrimary = scheme.onSurface,
    textSecondary = scheme.onSurfaceVariant,
    accent = scheme.primary,
    accentContainer = scheme.primaryContainer,
    onAccentContainer = scheme.onPrimaryContainer,
    // The drawer steps below the list like desktop's rail: a deeper container
    // than the canvas in light, the deepest one in dark.
    sidebar = if (dark) scheme.surfaceContainerLowest else scheme.surfaceContainerHigh,
    bubbleIn = scheme.surfaceContainerHigh,
    bubbleInText = scheme.onSurface,
    bubbleOut = scheme.primaryContainer,
    bubbleOutText = scheme.onPrimaryContainer,
    sidebarColors =
        SidebarColors(
            text = scheme.onSurface,
            textMuted = scheme.onSurfaceVariant,
            selected = scheme.secondaryContainer,
            selectedContent = scheme.onSecondaryContainer,
            unreadBackground = scheme.primaryContainer,
            unreadText = scheme.onPrimaryContainer,
        ),
)

/** Label for an opaque accent, matching desktop: white whenever it meets
 *  WCAG AA, else whichever of white and black contrasts more. */
internal fun accentLabelColor(color: Color): Color {
    val luminance = color.luminance()
    val white = 1.05f / (luminance + 0.05f)
    return if (white >= 4.5f || white >= (luminance + 0.05f) / 0.05f) Color.White else Color.Black
}

private fun materialColors(spec: MobileThemeSpec) =
    if (spec.dark) {
        darkColorScheme(
            primary = spec.accent,
            onPrimary = accentLabelColor(spec.accent),
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
            outline = spec.textSecondary.copy(alpha = 0.55f),
            outlineVariant = spec.border,
            error = Color(0xFFF87171),
            onError = Color(0xFF0F172A),
        )
    } else {
        lightColorScheme(
            primary = spec.accent,
            onPrimary = accentLabelColor(spec.accent),
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
            outline = spec.textSecondary.copy(alpha = 0.55f),
            outlineVariant = spec.border,
            error = Color(0xFFDC2626),
            onError = Color.White,
        )
    }

/** WCAG contrast ratio between two opaque colors. */
internal fun contrastRatio(
    a: Color,
    b: Color,
): Float {
    val la = a.luminance()
    val lb = b.luminance()
    return (maxOf(la, lb) + 0.05f) / (minOf(la, lb) + 0.05f)
}

/** [text] blended toward [toward] just until it reaches [min] contrast on [bg]. */
private fun readableOn(
    text: Color,
    bg: Color,
    toward: Color,
    min: Float = 4.5f,
): Color {
    for (step in 0..20) {
        val candidate = lerp(text, toward, step / 20f)
        if (contrastRatio(candidate, bg) >= min) return candidate
    }
    return toward
}

/** The accent darkened just enough for white numbers, like desktop's whiteLabelAccent. */
private fun whiteLabelAccent(accent: Color): Color {
    for (step in 0..50) {
        val candidate = Color.Black.copy(alpha = step / 50f).compositeOver(accent)
        if (contrastRatio(Color.White, candidate) >= 4.5f) return candidate
    }
    return Color.Black
}

/**
 * Drawer colors for Meron's own themes, mirroring desktop's rail: the drawer
 * wears the rail color, rows take the thread list's selection tint, and unread
 * counts are the rail's white numbers on the accent.
 */
private fun drawerColors(spec: MobileThemeSpec): SidebarColors {
    // Desktop's sideNavInkColor: the theme's text where it reads on the rail.
    val text =
        if (contrastRatio(spec.textPrimary, spec.sidebar) >= 4.5f) spec.textPrimary else accentLabelColor(spec.sidebar)
    return SidebarColors(
        text = text,
        textMuted = readableOn(spec.textSecondary, spec.sidebar, text),
        selected = spec.accent.copy(alpha = if (spec.dark) 0.2f else 0.13f),
        // An icon tint, so 3:1 is enough.
        selectedContent = readableOn(spec.accent, spec.sidebar, text, 3f),
        unreadBackground = whiteLabelAccent(spec.accent),
        unreadText = Color.White,
    )
}

/** A built-in theme's chat colors, outside composition (for tests). */
internal fun builtinChatColors(mode: AppAppearanceMode): ChatColors = chatColors(mobileThemeSpec(mode))

private fun chatColors(spec: MobileThemeSpec): ChatColors {
    val drawer = spec.sidebarColors ?: drawerColors(spec)
    return ChatColors(
        sidebar = spec.sidebar,
        onSidebar = drawer.text,
        onSidebarMuted = drawer.textMuted,
        sidebarAccent = drawer.selectedContent,
        bubbleIn = spec.bubbleIn,
        bubbleInText = spec.bubbleInText,
        bubbleOut = spec.bubbleOut,
        bubbleOutText = spec.bubbleOutText,
        star = if (spec.dark) Color(0xFFFBBF24) else Color(0xFFF59E0B),
        // Alpha compositing over opaque sRGB colors matches desktop's color-mix(in srgb).
        unreadBackground = spec.accent.copy(alpha = 0.18f).compositeOver(spec.bgChats),
        unreadText = spec.accent.copy(alpha = 0.55f).compositeOver(spec.textPrimary),
        sidebarUnreadBackground = drawer.unreadBackground,
        sidebarUnreadText = drawer.unreadText,
        sidebarSelected = drawer.selected,
    )
}
