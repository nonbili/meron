package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/** Renders a complete HTML document (mail body). WKWebView on iOS, WebView on
 *  Android. JavaScript is disabled by the platform implementations. */
@Composable
expect fun MailWebView(
    html: String,
    modifier: Modifier,
)
