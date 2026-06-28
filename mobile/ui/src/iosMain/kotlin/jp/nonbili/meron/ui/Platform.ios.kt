package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.ComposeFoundationFlags
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.text.input.PlatformImeOptions
import platform.Foundation.NSDate
import platform.Foundation.timeIntervalSince1970

actual fun currentTimeMillis(): Long = (NSDate().timeIntervalSince1970 * 1000.0).toLong()

actual val maskPasswordsByDefault: Boolean = false

@OptIn(ExperimentalComposeUiApi::class, ExperimentalFoundationApi::class)
actual val nativeTextKeyboardOptions: KeyboardOptions = run {
    ComposeFoundationFlags.isNewContextMenuEnabled = true
    KeyboardOptions(
        platformImeOptions = PlatformImeOptions {
            usingNativeTextInput(true)
        },
    )
}
