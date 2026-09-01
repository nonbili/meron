package jp.nonbili.meron.ui

import android.annotation.SuppressLint
import android.content.Context
import android.view.MotionEvent
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import jp.nonbili.meron.shared.isMeronOAuthCallbackScheme
import java.io.ByteArrayInputStream
import java.io.File
import java.io.FileInputStream
import java.net.URI
import kotlin.math.roundToInt

private const val MAIL_WEB_VIEW_ORIGIN = "https://appassets.androidplatform.net/"

@SuppressLint("SetJavaScriptEnabled")
@Composable
actual fun MailWebView(
    html: String,
    modifier: Modifier,
    onContentHeight: (Dp) -> Unit,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit,
    onLinkLongPress: (String, DpOffset) -> Unit,
    fitWideContent: Boolean,
) {
    val latestOnHeight = rememberUpdatedState(onContentHeight)
    val latestOnOpenUrl = rememberUpdatedState(onOpenUrl)
    val latestOnOpenImage = rememberUpdatedState(onOpenImage)
    val latestOnLinkLongPress = rememberUpdatedState(onLinkLongPress)
    // Inside NavHost this is the back-stack entry's lifecycle: RESUMED only
    // once the navigation transition has settled.
    val lifecycleState by LocalLifecycleOwner.current.lifecycle.currentStateFlow
        .collectAsState()
    // Read in the composable, not the update block, so a system font size change
    // recomposes this view and re-applies the zoom below.
    //
    // Taken from Compose's own sp conversion rather than from
    // Configuration.fontScale: since Android 14 the accessibility font sizes
    // are non-linear -- larger text scales less than small text -- and the raw
    // scale is documented as informational for that reason. Measured at a 200%
    // system size, Compose grows a message's text by 1.74x while a
    // fontScale-derived zoom grows the HTML by a flat 2.0x, so mail bodies
    // would outrun the plain-text ones beside them. Converting the base body
    // size the stylesheet uses puts the two back on the same curve. The whole
    // document scales by that one factor, which is what keeps headings and
    // other elements the stylesheet does not size scaling at all.
    val htmlTextZoom =
        with(LocalDensity.current) {
            val basePx = MESSAGE_HTML_BASE_PX.toFloat()
            (basePx.sp.toDp().value / basePx * 100).roundToInt()
        }
    AndroidView(
        modifier = modifier,
        factory = { context ->
            LongPressWebView(context).apply {
                webViewClient =
                    object : WebViewClient() {
                        override fun shouldOverrideUrlLoading(
                            view: WebView?,
                            request: WebResourceRequest?,
                        ): Boolean {
                            val url = request?.url?.toString().orEmpty()
                            if (url.isBlank()) return false
                            val externalUrl = externalMailUrl(url) ?: return true
                            latestOnOpenUrl.value(externalUrl)
                            return true
                        }

                        @Deprecated("Deprecated in Android SDK")
                        override fun shouldOverrideUrlLoading(
                            view: WebView?,
                            url: String?,
                        ): Boolean {
                            if (url.isNullOrBlank()) return false
                            val externalUrl = externalMailUrl(url) ?: return true
                            latestOnOpenUrl.value(externalUrl)
                            return true
                        }

                        override fun shouldInterceptRequest(
                            view: WebView?,
                            request: WebResourceRequest?,
                        ): WebResourceResponse? {
                            val uri = request?.url ?: return super.shouldInterceptRequest(view, request)
                            localMailMediaResponse(context, uri.scheme, uri.host, uri.path)?.let { return it }
                            if (isMailWebViewOrigin(uri.host)) {
                                return WebResourceResponse("text/plain", "UTF-8", ByteArrayInputStream(ByteArray(0)))
                            }
                            return super.shouldInterceptRequest(view, request)
                        }
                    }
                // JS is enabled to run the height-reporting script; matches the
                // desktop reader, whose iframe also runs email scripts.
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = false
                settings.defaultFontSize = 16
                // The view sizes to content, so it never scrolls internally.
                isVerticalScrollBarEnabled = false
                isHorizontalScrollBarEnabled = false
                setOnLongClickListener {
                    val url = webViewLinkUrl(hitTestResult.type, hitTestResult.extra) ?: return@setOnLongClickListener false
                    // The menu is Compose-side so it can open at the press, which a
                    // PopupMenu anchored to the (full-page) web view cannot do.
                    val scale = context.resources.displayMetrics.density
                    latestOnLinkLongPress.value(
                        url,
                        DpOffset((lastTouchX / scale).dp, (lastTouchY / scale).dp),
                    )
                    true
                }
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
                            externalMailUrl(url)?.let { externalUrl ->
                                post { latestOnOpenUrl.value(externalUrl) }
                            }
                        }
                    },
                    "MeronLink",
                )
                addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun open(src: String) {
                            mailImageRef(src)?.let { imageRef ->
                                post { latestOnOpenImage.value(imageRef) }
                            }
                        }
                    },
                    "MeronImage",
                )
            }
        },
        update = { webView ->
            // WebView draws through a GL functor that hwui cannot render into
            // the offscreen layers used by navigation transitions — doing so
            // crashes natively (SkSurface::getCanvas in GLFunctorDrawable) on
            // GL-pipeline devices. Skip drawing until the transition settles;
            // the page keeps loading and measuring while INVISIBLE.
            webView.visibility =
                if (lifecycleState.isAtLeast(Lifecycle.State.RESUMED)) View.VISIBLE else View.INVISIBLE
            // Set here rather than in the factory: the view is retained across
            // recompositions, so a factory-only assignment would leave a mode
            // change applying to the reloaded html below but not to the settings
            // that html assumes. Both branches are explicit for the same reason —
            // switching back has to actually switch back. Assigned before the
            // load so the page never parses under the previous mode.
            with(webView.settings) {
                // Off, the viewport meta in the document is ignored outright and
                // the page always renders at scale 1, which is what keeps the
                // height bridge's CSS-px-to-dp identity exact. On, the script may
                // widen the viewport to the content's natural width so an
                // over-wide mail shrinks to fit rather than being clipped.
                loadWithOverviewMode = fitWideContent
                useWideViewPort = fitWideContent
                // Fitting a 640px mail into a phone means ~0.56 scale, which would
                // leave 15px body text at ~8px. Text autosizing inflates fonts
                // within the preserved table structure so the shrunken page stays
                // readable — the half of Gmail's approach that makes the other
                // half usable. NORMAL is the modern WebView default the reflow-only
                // path has always rendered under.
                layoutAlgorithm =
                    if (fitWideContent) {
                        WebSettings.LayoutAlgorithm.TEXT_AUTOSIZING
                    } else {
                        WebSettings.LayoutAlgorithm.NORMAL
                    }
                // WebView seeds its text zoom from the configuration's font
                // scale when it is constructed and never revisits it. The
                // activity handles fontScale itself (see AndroidManifest) so
                // this view outlives the change: without re-asserting it here,
                // shrinking or growing the system font size would move every
                // sp-sized body in the app and leave the open HTML one behind.
                textZoom = htmlTextZoom
            }
            if (webView.tag != html) {
                webView.tag = html
                webView.loadDataWithBaseURL(MAIL_WEB_VIEW_ORIGIN, html, "text/html", "UTF-8", null)
            }
        },
    )
}

private fun localMailMediaResponse(
    context: Context,
    scheme: String?,
    host: String?,
    path: String?,
): WebResourceResponse? {
    if (scheme != "https" || host != "appassets.androidplatform.net" || path == null) return null
    val relative = path.removePrefix("/media/")
    if (relative == path || relative.isBlank() || relative.split('/').any { it == ".." || it.isBlank() }) return null
    return runCatching {
        val root = androidMediaRoot(context, relative)
        val file = File(root, relative).canonicalFile
        if (!file.startsWith(root) || !file.isFile) return null
        WebResourceResponse(mailMediaMimeType(file.name), null, FileInputStream(file))
    }.getOrNull()
}

internal fun mailMediaMimeType(filename: String): String =
    when (filename.substringAfterLast('.', "").lowercase()) {
        "jpg", "jpeg" -> "image/jpeg"
        "png" -> "image/png"
        "gif" -> "image/gif"
        "webp" -> "image/webp"
        "svg" -> "image/svg+xml"
        else -> "application/octet-stream"
    }

internal fun externalMailUrl(url: String): String? {
    val value = url.trim()
    if (value.isEmpty()) return null
    val host = runCatching { URI(value).host }.getOrNull()
    if (isMailWebViewOrigin(host)) return null
    val scheme = runCatching { URI(value).scheme?.lowercase() }.getOrNull() ?: return null
    return value.takeUnless {
        scheme in setOf("intent", "javascript", "file", "content", "data", "cid", "about") ||
            isMeronOAuthCallbackScheme(scheme)
    }
}

internal fun mailImageRef(src: String): String? {
    val value = src.trim()
    if (value.isEmpty()) return null
    val uri = runCatching { URI(encodeIllegalUriCharacters(value)) }.getOrNull() ?: return null
    if (isMailWebViewOrigin(uri.host)) return uri.path?.takeIf { it.startsWith("/media/") }
    if (value.startsWith("/media/")) return value
    return value.takeIf { uri.scheme?.lowercase() in setOf("http", "https") }
}

private fun isMailWebViewOrigin(host: String?): Boolean = host.equals("appassets.androidplatform.net", ignoreCase = true)

private fun encodeIllegalUriCharacters(value: String): String =
    buildString(value.length) {
        value.forEach { char ->
            append(
                when (char) {
                    ' ' -> "%20"
                    '|' -> "%7C"
                    '^' -> "%5E"
                    '{' -> "%7B"
                    '}' -> "%7D"
                    '[' -> "%5B"
                    ']' -> "%5D"
                    '`' -> "%60"
                    else -> char
                },
            )
        }
    }

/** Remembers where the finger went down: [View.OnLongClickListener] is not told the
 *  press position, and without it a link menu can only be placed at the view corner. */
private class LongPressWebView(
    context: Context,
) : WebView(context) {
    var lastTouchX = 0f
        private set
    var lastTouchY = 0f
        private set

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_DOWN) {
            lastTouchX = event.x
            lastTouchY = event.y
        }
        return super.onTouchEvent(event)
    }
}

@Suppress("DEPRECATION")
internal fun webViewLinkUrl(
    hitType: Int,
    extra: String?,
): String? =
    extra
        ?.trim()
        ?.takeIf {
            it.isNotEmpty() &&
                (
                    hitType == WebView.HitTestResult.ANCHOR_TYPE ||
                        hitType == WebView.HitTestResult.SRC_ANCHOR_TYPE
                )
        }

internal actual val MailWebViewFollowsSystemFontScale: Boolean = true
