package jp.nonbili.meron.ui

import androidx.compose.foundation.text.KeyboardOptions

actual fun currentTimeMillis(): Long = System.currentTimeMillis()

actual val maskPasswordsByDefault: Boolean = true

actual val nativeTextKeyboardOptions: KeyboardOptions = KeyboardOptions.Default
