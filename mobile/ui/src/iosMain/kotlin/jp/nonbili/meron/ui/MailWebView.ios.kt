package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.UIKitView
import kotlinx.cinterop.ExperimentalForeignApi
import platform.CoreGraphics.CGRectMake
import platform.WebKit.WKWebView
import platform.WebKit.WKWebViewConfiguration

@OptIn(ExperimentalForeignApi::class)
@Composable
actual fun MailWebView(
    html: String,
    modifier: Modifier,
) {
    UIKitView(
        modifier = modifier,
        factory = {
            val config = WKWebViewConfiguration()
            config.defaultWebpagePreferences.allowsContentJavaScript = false
            WKWebView(frame = CGRectMake(0.0, 0.0, 0.0, 0.0), configuration = config)
        },
        update = { webView ->
            webView.loadHTMLString(html, baseURL = null)
        },
    )
}
