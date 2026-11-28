package jp.nonbili.meron.ui

import android.os.SystemClock
import androidx.compose.foundation.text.contextmenu.builder.item
import androidx.compose.foundation.text.contextmenu.data.TextContextMenuItem
import androidx.compose.foundation.text.contextmenu.data.TextContextMenuKeys
import androidx.compose.foundation.text.contextmenu.modifier.appendTextContextMenuComponents
import androidx.compose.foundation.text.contextmenu.modifier.filterTextContextMenuComponents
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.Clipboard
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.NativeClipboard

private object QuoteInReplyKey

// Compose's selection menu has no way to read the selection, but its Copy item
// writes it to the clipboard. "Quote in reply" runs that Copy against this
// clipboard, which hands the text over instead of putting it on the system one.
@Composable
internal actual fun QuotableSelectionContainer(
    onQuote: ((String) -> Unit)?,
    content: @Composable () -> Unit,
) {
    if (onQuote == null) {
        SelectionContainer(content = content)
        return
    }
    val latestOnQuote = rememberUpdatedState(onQuote)
    val system = LocalClipboard.current
    val clipboard = remember(system) { SelectionCapturingClipboard(system) }
    val copy = remember { arrayOfNulls<TextContextMenuItem>(1) }
    val label = tr("chat.actions.quoteInReply")
    CompositionLocalProvider(LocalClipboard provides clipboard) {
        SelectionContainer(
            modifier =
                Modifier
                    .filterTextContextMenuComponents { component ->
                        if (component is TextContextMenuItem && component.key == TextContextMenuKeys.CopyKey) {
                            copy[0] = component
                        }
                        true
                    }.appendTextContextMenuComponents {
                        item(key = QuoteInReplyKey, label = label) {
                            val copyItem = copy[0]
                            if (copyItem != null) {
                                clipboard.captureNext { latestOnQuote.value(it) }
                                copyItem.onClick(this)
                            }
                            close()
                        }
                    },
            content = content,
        )
    }
}

private class SelectionCapturingClipboard(
    private val system: Clipboard,
) : Clipboard {
    private var capture: ((String) -> Unit)? = null
    private var captureUntil = 0L

    /** Hand the next write, if it comes within a moment (the copy is launched,
     *  not immediate), to [onText] instead of the system clipboard. */
    fun captureNext(onText: (String) -> Unit) {
        capture = onText
        captureUntil = SystemClock.uptimeMillis() + 1_000
    }

    // Required by the interface; nothing here reads it.
    @Deprecated("Use platform-specific extension to get platform reference.")
    @Suppress("DEPRECATION")
    override val nativeClipboard: NativeClipboard get() = system.nativeClipboard

    override suspend fun getClipEntry(): ClipEntry? = system.getClipEntry()

    override suspend fun setClipEntry(clipEntry: ClipEntry?) {
        val onText = capture?.takeIf { SystemClock.uptimeMillis() <= captureUntil }
        capture = null
        if (onText == null) {
            system.setClipEntry(clipEntry)
            return
        }
        val clip = clipEntry?.clipData
        val text = if (clip != null && clip.itemCount > 0) clip.getItemAt(0).text?.toString() else null
        if (!text.isNullOrBlank()) onText(text)
    }
}
