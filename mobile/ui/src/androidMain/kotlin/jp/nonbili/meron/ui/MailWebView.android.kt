package jp.nonbili.meron.ui

import android.annotation.SuppressLint
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
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
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit,
) {
    val latestOnHeight = rememberUpdatedState(onContentHeight)
    val latestOnOpenUrl = rememberUpdatedState(onOpenUrl)
    val latestOnOpenImage = rememberUpdatedState(onOpenImage)
    AndroidView(
        modifier = modifier,
        factory = { context ->
            WebView(context).apply {
                webViewClient =
                    object : WebViewClient() {
                        override fun shouldOverrideUrlLoading(
                            view: WebView?,
                            request: WebResourceRequest?,
                        ): Boolean {
                            val url = request?.url?.toString().orEmpty()
                            if (url.isBlank()) return false
                            latestOnOpenUrl.value(url)
                            return true
                        }

                        @Deprecated("Deprecated in Android SDK")
                        override fun shouldOverrideUrlLoading(
                            view: WebView?,
                            url: String?,
                        ): Boolean {
                            if (url.isNullOrBlank()) return false
                            latestOnOpenUrl.value(url)
                            return true
                        }
                    }
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
                addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun open(url: String) {
                            if (url.isNotBlank()) post { latestOnOpenUrl.value(url) }
                        }
                    },
                    "MeronLink",
                )
                addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun open(src: String) {
                            if (src.isNotBlank()) post { latestOnOpenImage.value(src) }
                        }
                    },
                    "MeronImage",
                )
            }
        },
        update = { webView ->
            webView.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        },
    )
}
