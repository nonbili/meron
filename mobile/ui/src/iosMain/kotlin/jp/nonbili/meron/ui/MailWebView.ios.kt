package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.UIKitView
import kotlinx.cinterop.ExperimentalForeignApi
import platform.CoreGraphics.CGRectMake
import platform.Foundation.NSNumber
import platform.WebKit.WKScriptMessage
import platform.WebKit.WKScriptMessageHandlerProtocol
import platform.WebKit.WKUserContentController
import platform.WebKit.WKWebView
import platform.WebKit.WKWebViewConfiguration
import platform.darwin.NSObject

@OptIn(ExperimentalForeignApi::class)
@Composable
actual fun MailWebView(
    html: String,
    modifier: Modifier,
    onContentHeight: (Dp) -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    val latestOnHeight = rememberUpdatedState(onContentHeight)
    val latestOnOpenUrl = rememberUpdatedState(onOpenUrl)
    UIKitView(
        modifier = modifier,
        factory = {
            val config = WKWebViewConfiguration()
            // JS runs the height-reporting script; matches the desktop reader,
            // whose iframe also runs email scripts.
            config.defaultWebpagePreferences.allowsContentJavaScript = true
            config.userContentController.addScriptMessageHandler(
                scriptMessageHandler = HeightMessageHandler { cssPx -> latestOnHeight.value(cssPx.dp) },
                name = "meronHeight",
            )
            config.userContentController.addScriptMessageHandler(
                scriptMessageHandler = LinkMessageHandler { url -> latestOnOpenUrl.value(url) },
                name = "meronLink",
            )
            WKWebView(frame = CGRectMake(0.0, 0.0, 0.0, 0.0), configuration = config).apply {
                // Compose owns capped bubble scrolling; the web view is measured
                // to its full content height so its native scroll view would fight
                // the parent LazyColumn for vertical drags.
                scrollView.scrollEnabled = false
                setOpaque(false)
            }
        },
        update = { webView ->
            webView.loadHTMLString(html, baseURL = null)
        },
    )
}

private class HeightMessageHandler(
    private val onHeight: (Int) -> Unit,
) : NSObject(),
    WKScriptMessageHandlerProtocol {
    override fun userContentController(
        userContentController: WKUserContentController,
        didReceiveScriptMessage: WKScriptMessage,
    ) {
        (didReceiveScriptMessage.body as? NSNumber)?.let { onHeight(it.intValue) }
    }
}

private class LinkMessageHandler(
    private val onOpenUrl: (String) -> Unit,
) : NSObject(),
    WKScriptMessageHandlerProtocol {
    override fun userContentController(
        userContentController: WKUserContentController,
        didReceiveScriptMessage: WKScriptMessage,
    ) {
        (didReceiveScriptMessage.body as? String)?.takeIf { it.isNotBlank() }?.let(onOpenUrl)
    }
}
