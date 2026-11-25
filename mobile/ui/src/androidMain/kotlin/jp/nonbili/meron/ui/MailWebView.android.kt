package jp.nonbili.meron.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollDispatcher
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import jp.nonbili.meron.shared.isMeronOAuthCallbackScheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import java.io.ByteArrayInputStream
import java.io.File
import java.io.FileInputStream
import java.net.URI
import kotlin.math.abs
import kotlin.math.roundToInt

internal const val MAIL_WEB_VIEW_ORIGIN = "https://appassets.androidplatform.net/"

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
    transparentBackground: Boolean,
    onQuoteToggle: (Boolean) -> Unit,
    onNaturalWidth: (Dp) -> Unit,
    onOverflowExtent: (Int, Int) -> Unit,
    mediaMissing: Int,
    scrollsItself: Boolean,
) {
    val nestedScrollDispatcher = remember { NestedScrollDispatcher() }
    val nestedScrollConnection = remember { object : NestedScrollConnection {} }
    val gestureScope = rememberCoroutineScope()
    var touchView by remember { mutableStateOf<LongPressWebView?>(null) }
    val mediaRecovery = remember { MailMediaRecovery(mediaMissing) }
    val latestOnOverflowExtent = rememberUpdatedState(onOverflowExtent)
    val latestOnNaturalWidth = rememberUpdatedState(onNaturalWidth)
    val latestOnQuoteToggle = rememberUpdatedState(onQuoteToggle)
    val latestOnHeight = rememberUpdatedState(onContentHeight)
    val latestOnOpenUrl = rememberUpdatedState(onOpenUrl)
    val latestOnOpenImage = rememberUpdatedState(onOpenImage)
    val latestOnLinkLongPress = rememberUpdatedState(onLinkLongPress)
    // Inside NavHost this is the back-stack entry's lifecycle: RESUMED only
    // once the navigation transition has settled.
    val lifecycleState by LocalLifecycleOwner.current.lifecycle.currentStateFlow
        .collectAsState()
    // The conversation overlay is no navigation destination, so its fade never
    // shows in the lifecycle above.
    val drawable = LocalMailWebViewDrawable.current
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
        modifier =
            if (scrollsItself) {
                modifier
                    .nestedScroll(nestedScrollConnection, nestedScrollDispatcher)
                    .pointerInput(Unit) {
                        awaitEachGesture {
                            val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                            val slop = viewConfiguration.touchSlop
                            var displacement = Offset.Zero
                            while (true) {
                                val change = awaitPointerEvent(PointerEventPass.Initial).changes.singleOrNull()
                                if (change == null || change.id != down.id || !change.pressed || change.isConsumed) break
                                displacement += change.position - change.previousPosition
                                val dx = abs(displacement.x)
                                val dy = abs(displacement.y)
                                if (dx < slop && dy < slop) continue
                                // Claim vertical drags before the interop view and
                                // parent process this move. Sideways swipes still
                                // reach the conversation's gesture handler.
                                if (dy >= dx) {
                                    val overSlop = if (displacement.y < 0f) displacement.y + slop else displacement.y - slop
                                    touchView?.claimVerticalDrag(overSlop)
                                }
                                break
                            }
                        }
                    }
            } else {
                modifier
            },
        factory = { context ->
            LongPressWebView(context, nestedScrollDispatcher, gestureScope).apply {
                touchView = this
                webViewClient =
                    object : WebViewClient() {
                        override fun onPageFinished(
                            view: WebView?,
                            url: String?,
                        ) {
                            view?.retryRecoveredMedia(mediaRecovery)
                        }

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
                // The page is served from an https origin, and WebView's default
                // blocks every http: subresource on one -- so a mail whose images
                // are http: stays blank even after its remote content is allowed.
                // Whether remote content loads at all is the CSP's call.
                settings.mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
                // Pinch to zoom, for mail whose text or pictures stay too small
                // at the size they fit the view in. The on-screen +/- buttons
                // are the deprecated half of the same switch.
                settings.setSupportZoom(true)
                settings.builtInZoomControls = true
                settings.displayZoomControls = false
                // Sized to content, the view never scrolls internally -- except
                // sideways once zoomed in, or down a long mail it is capped
                // short of (see LongPressWebView and scrollsItself).
                isVerticalScrollBarEnabled = false
                isHorizontalScrollBarEnabled = false
                setOnLongClickListener {
                    val hit = hitTestResult
                    // The menu is Compose-side so it can open at the press, which a
                    // PopupMenu anchored to the (full-page) web view cannot do.
                    val scale = context.resources.displayMetrics.density
                    val pressAt = DpOffset((lastTouchX / scale).dp, (lastTouchY / scale).dp)
                    val imageRef = webViewImageUrl(hit.type, hit.extra)
                    if (hit.type == WebView.HitTestResult.SRC_IMAGE_ANCHOR_TYPE) {
                        // A tap opens the picture; a long-press offers its link, for
                        // a reader who wants where it points. The hit test only
                        // carries the image's src, so ask for the anchor's href. A
                        // link we would not open (cid:, javascript:) keeps the
                        // picture behaviour.
                        requestFocusNodeHref(
                            Handler(Looper.getMainLooper()) { message ->
                                val href = message.data.getString("url")?.let(::externalMailUrl)
                                when {
                                    href != null -> latestOnLinkLongPress.value(href, pressAt)
                                    imageRef != null -> latestOnOpenImage.value(imageRef)
                                }
                                true
                            }.obtainMessage(),
                        )
                        return@setOnLongClickListener true
                    }
                    imageRef?.let { ref ->
                        latestOnOpenImage.value(ref)
                        return@setOnLongClickListener true
                    }
                    val url = webViewLinkUrl(hit.type, hit.extra) ?: return@setOnLongClickListener false
                    latestOnLinkLongPress.value(url, pressAt)
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
                        fun report(cssPx: Int) {
                            post { latestOnNaturalWidth.value(cssPx.dp) }
                        }
                    },
                    "MeronWidth",
                )
                addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun report(
                            cssPx: Int,
                            widthCssPx: Int,
                        ) {
                            post { latestOnOverflowExtent.value(cssPx, widthCssPx) }
                        }
                    },
                    "MeronOverflow",
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
                addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun toggle(open: Boolean) {
                            post { latestOnQuoteToggle.value(open) }
                        }
                    },
                    "MeronQuote",
                )
            }
        },
        update = { webView ->
            // WebView draws through a GL functor that hwui cannot render into
            // the offscreen layers used by navigation transitions and fades —
            // doing so crashes natively (SkSurface::getCanvas in
            // GLFunctorDrawable) on GL-pipeline devices. Skip drawing until the
            // transition settles; the page keeps loading and measuring while
            // INVISIBLE.
            webView.visibility =
                if (drawable && lifecycleState.isAtLeast(Lifecycle.State.RESUMED)) View.VISIBLE else View.INVISIBLE
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
            // Both ways explicitly, as with the settings above: the view is
            // retained, and white is WebView's own default.
            // Shorter than its document, the view scrolls it itself and shows
            // where it is; sized to the document, there is nothing to scroll.
            webView.scrollsItself = scrollsItself
            webView.isVerticalScrollBarEnabled = scrollsItself
            webView.setBackgroundColor(if (transparentBackground) Color.TRANSPARENT else Color.WHITE)
            val documentChanged = webView.tag != html
            mediaRecovery.update(mediaMissing, documentChanged)
            if (documentChanged) {
                webView.tag = html
                webView.loadDataWithBaseURL(MAIL_WEB_VIEW_ORIGIN, mailHtmlWithMediaGeneration(html, mediaRecovery.generation), "text/html", "UTF-8", null)
            }
            webView.retryRecoveredMedia(mediaRecovery)
        },
    )
}

private fun WebView.retryRecoveredMedia(recovery: MailMediaRecovery) {
    val request = recovery.request() ?: return
    evaluateJavascript(request.script) { result -> recovery.acknowledge(request, result == "true") }
}

internal fun localMailMediaResponse(
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

// Read by pattern, not with java.net.URI: URI rejects characters a browser takes
// in its stride (`|`, `{`, `^`, a space), which tracking links in mail are full
// of, and a link it failed to parse used to be dropped without a word.
private val urlScheme = Regex("""^([a-z][a-z0-9+.-]*):""", RegexOption.IGNORE_CASE)
private val urlHost = Regex("""^[a-z][a-z0-9+.-]*://(?:[^/?#@]*@)?([^/?#:]*)""", RegexOption.IGNORE_CASE)

internal fun externalMailUrl(url: String): String? {
    val value = url.trim()
    if (value.isEmpty()) return null
    val host = urlHost.find(value)?.groupValues?.get(1)
    if (isMailWebViewOrigin(host)) return null
    val scheme =
        urlScheme
            .find(value)
            ?.groupValues
            ?.get(1)
            ?.lowercase() ?: return null
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

internal fun isMailWebViewOrigin(host: String?): Boolean = host.equals("appassets.androidplatform.net", ignoreCase = true)

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
 *  press position, and without it a link menu can only be placed at the view corner.
 *
 *  Also keeps the gestures that are the page's own away from the list or reader
 *  scrolling around it, which would otherwise take the touch over (and cancel it
 *  here) as soon as a finger drifted vertically: a pinch, and a sideways drag
 *  across a page zoomed wider than the view, or a vertical drag inside a
 *  capped body when internal scrolling is enabled. */
private class LongPressWebView(
    context: Context,
    private val nestedScrollDispatcher: NestedScrollDispatcher,
    private val gestureScope: CoroutineScope,
) : WebView(context) {
    var scrollsItself = false
        set(value) {
            field = value
            if (!value) parentFling?.cancel()
        }
    var lastTouchX = 0f
        private set
    var lastTouchY = 0f
        private set
    private val touchConfiguration = ViewConfiguration.get(context)
    private val touchSlop = touchConfiguration.scaledTouchSlop
    private val velocityTracker = VelocityTracker()
    private var parentFling: Job? = null
    private var lastParentScrollY = 0f
    private var holdsVerticalTouch = false
    private var initialVerticalDrag: Float? = null
    private var previousRawY = 0f

    fun claimVerticalDrag(overSlop: Float) {
        if (scrollsItself && (canScrollVertically(1) || canScrollVertically(-1))) {
            holdsVerticalTouch = true
            initialVerticalDrag = overSlop
            parent?.requestDisallowInterceptTouchEvent(true)
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        var unconsumedY = 0f
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                parentFling?.cancel()
                lastTouchX = event.x
                lastTouchY = event.y
                previousRawY = event.rawY
                holdsVerticalTouch = false
                lastParentScrollY = 0f
                if (scrollsItself) {
                    velocityTracker.resetTracking()
                    trackVelocity(event)
                }
            }

            MotionEvent.ACTION_POINTER_DOWN -> {
                holdsVerticalTouch = false
                parent?.requestDisallowInterceptTouchEvent(true)
            }

            MotionEvent.ACTION_MOVE -> {
                val dx = event.x - lastTouchX
                val dy = event.y - lastTouchY
                if (ownsDrag(dx, dy, touchSlop, canScrollHorizontally(if (dx < 0) 1 else -1))) {
                    parent?.requestDisallowInterceptTouchEvent(true)
                }
                if (scrollsItself) trackVelocity(event)
                unconsumedY = unconsumedVerticalDrag(event.rawY)
            }
        }
        val handled = super.onTouchEvent(event)
        if (unconsumedY != 0f) {
            // Carry on scrolling the list or reader at the body's edge
            // without starting a pull-to-dismiss gesture.
            lastParentScrollY =
                nestedScrollDispatcher
                    .dispatchPostScroll(Offset.Zero, Offset(0f, unconsumedY), NestedScrollSource.SideEffect)
                    .y
        }
        if (event.actionMasked == MotionEvent.ACTION_UP && scrollsItself && holdsVerticalTouch && lastParentScrollY != 0f) {
            trackVelocity(event)
            flingParent()
        }
        return handled
    }

    private fun trackVelocity(event: MotionEvent) {
        velocityTracker.addPosition(event.eventTime, Offset(event.rawX, event.rawY))
    }

    private fun flingParent() {
        val maxVelocity = touchConfiguration.scaledMaximumFlingVelocity.toFloat()
        val velocityY = velocityTracker.calculateVelocity(Velocity(maxVelocity, maxVelocity)).y
        if (abs(velocityY) < touchConfiguration.scaledMinimumFlingVelocity || velocityY * lastParentScrollY <= 0f) return
        if (canScrollVertically(if (velocityY < 0f) 1 else -1)) return
        parentFling =
            gestureScope.launch {
                nestedScrollDispatcher.dispatchPostFling(Velocity.Zero, Velocity(0f, velocityY))
            }
    }

    private fun unconsumedVerticalDrag(rawY: Float): Float {
        // The outer scroller can move this view during the gesture, so use
        // screen coordinates for per-event deltas.
        val deltaY = rawY - previousRawY
        previousRawY = rawY
        if (!scrollsItself || !holdsVerticalTouch) return 0f
        val dragY = initialVerticalDrag ?: deltaY
        initialVerticalDrag = null
        if (dragY == 0f) return 0f
        lastParentScrollY = 0f
        // Compose has already seen consumed pointer changes. Hand off the
        // movement through nested scrolling while WebView retains the touch.
        return if (canScrollVertically(if (dragY < 0) 1 else -1)) 0f else dragY
    }
}

/** Whether a one-finger drag of ([dx], [dy]) from where it went down pans the
 *  page along x rather than scrolling what the web view sits in. */
internal fun ownsDrag(
    dx: Float,
    dy: Float,
    touchSlop: Int,
    canPan: Boolean,
): Boolean = canPan && abs(dx) > touchSlop && abs(dx) > abs(dy)

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

@Suppress("DEPRECATION")
internal fun webViewImageUrl(
    hitType: Int,
    extra: String?,
): String? = if (hitType == WebView.HitTestResult.IMAGE_TYPE || hitType == WebView.HitTestResult.SRC_IMAGE_ANCHOR_TYPE) extra?.let(::mailImageRef) else null

internal actual val MailWebViewFollowsSystemFontScale: Boolean = true

internal actual val MailWebViewFitsWideContent: Boolean = true

internal actual val MailWebViewPinchZooms: Boolean = true

internal actual val MailWebViewScrollsItself: Boolean = true
