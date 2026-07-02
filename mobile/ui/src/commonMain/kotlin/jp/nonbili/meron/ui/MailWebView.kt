package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp

/** Renders a complete HTML document (mail body). WKWebView on iOS, WebView on
 *  Android. JavaScript is enabled so the embedded measurement script can report
 *  content height via [onContentHeight] (in dp), letting the caller size the
 *  view to fit the email. */
@Composable
expect fun MailWebView(
    html: String,
    modifier: Modifier,
    onContentHeight: (Dp) -> Unit,
    onOpenUrl: (String) -> Unit,
)
