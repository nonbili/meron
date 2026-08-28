package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.isUnspecified
import kotlin.math.abs
import kotlin.math.floor
import kotlin.math.round

// How large message text is drawn.
//
// Desktop (desktop/frontend/src/lib/fonts.ts) also picks font families and scales the
// whole app. Neither ports. There is no installed-font list here, and the
// families Compose offers without bundling one are the platform's own generics,
// so a family setting could only trade the system font for its serif or
// monospace sibling. App text is already scaled system-wide by the display
// settings, which `sp` sizes follow — only message bodies escape that, because
// a WebView renders them, so the size setting is theirs alone. That escape is
// per-platform: see MailWebViewFollowsSystemFontScale.

const val DEFAULT_MESSAGE_FONT_SCALE = 100

/** Base size message bodies render at before any scaling, in CSS pixels. */
const val MESSAGE_HTML_BASE_PX = 16.0

/**
 * The sizes the message text slider snaps to, as a percentage of the default.
 * Narrower at the top than desktop's 400%: a phone body is already laid out at
 * the screen's width, so past roughly double size the mail reflows into a
 * column of single words.
 */
val MESSAGE_FONT_SCALE_STEPS = listOf(80, 90, 100, 115, 130, 150, 175, 200)

/** Snap a stored or slid scale onto the nearest step. */
fun coerceMessageFontScale(value: Int): Int = MESSAGE_FONT_SCALE_STEPS.minBy { abs(it - value) }

/** A CSS pixel size scaled by [scale] percent, rounded to a tenth. */
internal fun scaledCssPx(
    basePx: Double,
    scale: Int,
): String {
    val scaled = round(basePx * coerceMessageFontScale(scale) / 10.0) / 10.0
    return if (scaled == floor(scaled)) "${scaled.toInt()}px" else "${scaled}px"
}

private fun TextUnit.scaledBy(scale: Int): TextUnit = if (isUnspecified) this else this * (coerceMessageFontScale(scale) / 100f)

/**
 * [base] at the reading size the user picked. The plain-text counterpart of the
 * stylesheet the WebView bodies get, so a text-only mail and an HTML one read
 * at the same size.
 */
@Composable
@ReadOnlyComposable
internal fun messageBodyTextStyle(base: TextStyle): TextStyle = messageBodyTextStyle(base, LocalMessageFontScale.current)

/** [messageBodyTextStyle] at an explicit scale, for the settings preview. */
internal fun messageBodyTextStyle(
    base: TextStyle,
    scale: Int,
): TextStyle =
    base.copy(
        fontSize = base.fontSize.scaledBy(scale),
        lineHeight = base.lineHeight.scaledBy(scale),
    )
