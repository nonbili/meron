package jp.nonbili.meron.ui

import android.annotation.SuppressLint
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView

@SuppressLint("SetJavaScriptEnabled")
@Composable
actual fun MailWebView(
    html: String,
    modifier: Modifier,
    onContentHeight: (Dp) -> Unit,
) {
    val latestOnHeight = rememberUpdatedState(onContentHeight)
    AndroidView(
        modifier = modifier,
        factory = { context ->
            WebView(context).apply {
                webViewClient = WebViewClient()
                // JS is enabled to run the height-reporting script; matches the
                // desktop reader, whose iframe also runs email scripts.
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = false
                settings.loadWithOverviewMode = false
                settings.useWideViewPort = false
                settings.defaultFontSize = 16
                // The view sizes to content, so it never scrolls internally.
                isVerticalScrollBarEnabled = false
                isHorizontalScrollBarEnabled = false
                addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun report(cssPx: Int) {
                            // contentHeight comes off the JS thread; bounce to the
                            // view (main) thread before touching Compose state.
                            post { latestOnHeight.value(cssPx.dp) }
                        }
                    },
                    "MeronHeight",
                )
            }
        },
        update = { webView ->
            webView.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        },
    )
}
