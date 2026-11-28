package jp.nonbili.meron.ui

import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.Composable

// The iOS text menu takes no items of the app's own.
@Composable
internal actual fun QuotableSelectionContainer(
    onQuote: ((String) -> Unit)?,
    content: @Composable () -> Unit,
) {
    SelectionContainer(content = content)
}
