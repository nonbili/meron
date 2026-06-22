package jp.nonbili.meron

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

// Palette mirrors the desktop "Indigo" / "Indigo Dark" themes in
// frontend/src/index.css so the mobile shell shares Meron's visual identity.

private val Accent = Color(0xFF4F46E5)
private val AccentDark = Color(0xFF6366F1)

private val LightColors = lightColorScheme(
    primary = Accent,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFE0E7FF),
    onPrimaryContainer = Color(0xFF312E81),
    secondary = Color(0xFF64748B),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFE2E8F0),
    onSecondaryContainer = Color(0xFF0F172A),
    background = Color(0xFFF1F5F9),
    onBackground = Color(0xFF0F172A),
    surface = Color(0xFFFFFFFF),
    onSurface = Color(0xFF0F172A),
    surfaceVariant = Color(0xFFF1F5F9),
    onSurfaceVariant = Color(0xFF64748B),
    surfaceContainer = Color(0xFFF8FAFC),
    surfaceContainerHigh = Color(0xFFF1F5F9),
    outline = Color(0xFFCBD5E1),
    outlineVariant = Color(0xFFE2E8F0),
    error = Color(0xFFDC2626),
    onError = Color.White,
)

private val DarkColors = darkColorScheme(
    primary = AccentDark,
    onPrimary = Color.White,
    primaryContainer = Color(0xFF312E81),
    onPrimaryContainer = Color(0xFFE0E7FF),
    secondary = Color(0xFF94A3B8),
    onSecondary = Color(0xFF0F172A),
    secondaryContainer = Color(0xFF1E293B),
    onSecondaryContainer = Color(0xFFF8FAFC),
    background = Color(0xFF090D16),
    onBackground = Color(0xFFF8FAFC),
    surface = Color(0xFF0F172A),
    onSurface = Color(0xFFF8FAFC),
    surfaceVariant = Color(0xFF1E293B),
    onSurfaceVariant = Color(0xFF94A3B8),
    surfaceContainer = Color(0xFF111B2E),
    surfaceContainerHigh = Color(0xFF1E293B),
    outline = Color(0xFF334155),
    outlineVariant = Color(0xFF1E293B),
    error = Color(0xFFF87171),
    onError = Color(0xFF0F172A),
)

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

private val LightChat = ChatColors(
    sidebar = Color(0xFF0F172A),
    onSidebar = Color(0xFFF8FAFC),
    onSidebarMuted = Color(0xFF94A3B8),
    bubbleIn = Color(0xFFFFFFFF),
    bubbleInText = Color(0xFF0F172A),
    bubbleOut = Color(0xFFE0E7FF),
    bubbleOutText = Color(0xFF312E81),
    star = Color(0xFFF59E0B),
)

private val DarkChat = ChatColors(
    sidebar = Color(0xFF05070C),
    onSidebar = Color(0xFFF8FAFC),
    onSidebarMuted = Color(0xFF94A3B8),
    bubbleIn = Color(0xFF1E293B),
    bubbleInText = Color(0xFFF8FAFC),
    bubbleOut = Color(0xFF312E81),
    bubbleOutText = Color(0xFFE0E7FF),
    star = Color(0xFFFBBF24),
)

val LocalChatColors = staticCompositionLocalOf { LightChat }

@Composable
fun MeronTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    androidx.compose.runtime.CompositionLocalProvider(
        LocalChatColors provides if (dark) DarkChat else LightChat,
    ) {
        MaterialTheme(colorScheme = if (dark) DarkColors else LightColors, content = content)
    }
}
