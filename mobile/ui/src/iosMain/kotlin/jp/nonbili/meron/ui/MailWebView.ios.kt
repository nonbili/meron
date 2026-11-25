package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.UIKitView
import kotlinx.cinterop.BetaInteropApi
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.ObjCAction
import kotlinx.cinterop.useContents
import platform.CoreGraphics.CGPointMake
import platform.CoreGraphics.CGRectMake
import platform.Foundation.NSNumber
import platform.Foundation.NSSelectorFromString
import platform.UIKit.UIGestureRecognizer
import platform.UIKit.UIGestureRecognizerDelegateProtocol
import platform.UIKit.UIGestureRecognizerStateBegan
import platform.UIKit.UIGestureRecognizerStateChanged
import platform.UIKit.UIPanGestureRecognizer
import platform.UIKit.UIScrollView
import platform.WebKit.WKNavigation
import platform.WebKit.WKNavigationDelegateProtocol
import platform.WebKit.WKScriptMessage
import platform.WebKit.WKScriptMessageHandlerProtocol
import platform.WebKit.WKUserContentController
import platform.WebKit.WKWebView
import platform.WebKit.WKWebViewConfiguration
import platform.darwin.NSObject
import kotlin.math.abs

@OptIn(ExperimentalForeignApi::class)
@Composable
actual fun MailWebView(
    html: String,
    modifier: Modifier,
    onContentHeight: (Dp) -> Unit,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit,
    onLinkLongPress: (String, DpOffset) -> Unit,
    // Unused: shrink-to-fit is driven by Android's useWideViewPort/text
    // autosizing, which WKWebView has no equivalent for. The script's fit pass
    // is gated on the same flag and stays off here, so iOS keeps reflow-only
    // rendering and the height bridge's scale-1 assumption holds.
    @Suppress("UNUSED_PARAMETER") fitWideContent: Boolean,
    // Unused: the view is always non-opaque (see the factory below).
    @Suppress("UNUSED_PARAMETER") transparentBackground: Boolean,
    onQuoteToggle: (Boolean) -> Unit,
    onNaturalWidth: (Dp) -> Unit,
    onOverflowExtent: (Int, Int) -> Unit,
    mediaMissing: Int,
    // Unused: iOS keeps the view sized to the document (MailWebViewScrollsItself).
    @Suppress("UNUSED_PARAMETER") scrollsItself: Boolean,
) {
    val latestOnOverflowExtent = rememberUpdatedState(onOverflowExtent)
    val latestOnNaturalWidth = rememberUpdatedState(onNaturalWidth)
    val latestOnHeight = rememberUpdatedState(onContentHeight)
    val latestOnOpenUrl = rememberUpdatedState(onOpenUrl)
    val latestOnOpenImage = rememberUpdatedState(onOpenImage)
    val latestOnQuoteToggle = rememberUpdatedState(onQuoteToggle)
    // The document last handed to this web view. `update` runs again on
    // recomposition (every height report recomposes), and reloading the same
    // page would reset what the reader did in it — an opened quote, for one.
    val loadedHtml = remember { LoadedHtml() }
    val mediaRecovery = remember { MailMediaRecovery(mediaMissing) }
    val mediaNavigationDelegate = remember { MediaRecoveryNavigationDelegate(mediaRecovery) }
    // Held here because a gesture recognizer retains neither its target nor
    // its delegate.
    val zoomedPan = remember { ZoomedPanHandler() }
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
                scriptMessageHandler = HeightMessageHandler { cssPx -> latestOnNaturalWidth.value(cssPx.dp) },
                name = "meronWidth",
            )
            config.userContentController.addScriptMessageHandler(
                scriptMessageHandler = OverflowMessageHandler { extent, width -> latestOnOverflowExtent.value(extent, width) },
                name = "meronOverflow",
            )
            config.userContentController.addScriptMessageHandler(
                scriptMessageHandler = LinkMessageHandler { url -> latestOnOpenUrl.value(url) },
                name = "meronLink",
            )
            config.userContentController.addScriptMessageHandler(
                scriptMessageHandler = ImageMessageHandler { src -> latestOnOpenImage.value(src) },
                name = "meronImage",
            )
            config.userContentController.addScriptMessageHandler(
                scriptMessageHandler = QuoteMessageHandler { open -> latestOnQuoteToggle.value(open) },
                name = "meronQuote",
            )
            WKWebView(frame = CGRectMake(0.0, 0.0, 0.0, 0.0), configuration = config).apply {
                navigationDelegate = mediaNavigationDelegate
                // Compose owns capped bubble scrolling; the web view is measured
                // to its full content height so its native scroll view would fight
                // the parent LazyColumn for vertical drags. Its pinch is a
                // separate recognizer and stays on, so the page still zooms; the
                // sideways pan a zoomed page then needs is ZoomedPanHandler's.
                scrollView.scrollEnabled = false
                addGestureRecognizer(
                    UIPanGestureRecognizer(target = zoomedPan, action = NSSelectorFromString("handlePan:")).apply {
                        maximumNumberOfTouches = 1u
                        delegate = zoomedPan
                    },
                )
                setOpaque(false)
            }
        },
        update = { webView ->
            val documentChanged = loadedHtml.value != html
            mediaRecovery.update(mediaMissing, documentChanged)
            if (documentChanged) {
                loadedHtml.value = html
                webView.loadHTMLString(mailHtmlWithMediaGeneration(html, mediaRecovery.generation), baseURL = null)
            }
            webView.retryRecoveredMedia(mediaRecovery)
        },
    )
}

@OptIn(ExperimentalForeignApi::class)
private fun WKWebView.retryRecoveredMedia(recovery: MailMediaRecovery) {
    val request = recovery.request() ?: return
    evaluateJavaScript(request.script) { result, error ->
        recovery.acknowledge(request, error == null && (result as? NSNumber)?.boolValue == true)
    }
}

@OptIn(ExperimentalForeignApi::class)
private class MediaRecoveryNavigationDelegate(
    private val recovery: MailMediaRecovery,
) : NSObject(),
    WKNavigationDelegateProtocol {
    override fun webView(
        webView: WKWebView,
        didFinishNavigation: WKNavigation?,
    ) {
        webView.retryRecoveredMedia(recovery)
    }
}

private class LoadedHtml(
    var value: String? = null,
)

/** Pans a page zoomed wider than the view sideways, in place of the scroll
 *  view's own pan (off, see the factory above). It begins only for a drag the
 *  page can follow, so a vertical drag, or one past the page's edge, is left to
 *  the list or reader scrolling around the web view. */
@OptIn(ExperimentalForeignApi::class)
private class ZoomedPanHandler :
    NSObject(),
    UIGestureRecognizerDelegateProtocol {
    private var startOffsetX = 0.0

    override fun gestureRecognizerShouldBegin(gestureRecognizer: UIGestureRecognizer): Boolean {
        val pan = gestureRecognizer as? UIPanGestureRecognizer ?: return false
        val scrollView = pan.scrollView() ?: return false
        val (vx, vy) = pan.velocityInView(pan.view).useContents { x to y }
        if (abs(vx) <= abs(vy)) return false
        val offsetX = scrollView.contentOffset.useContents { x }
        // A finger moving right brings in what lies to the left.
        return if (vx > 0) offsetX > 0.5 else offsetX < scrollView.maxOffsetX() - 0.5
    }

    @OptIn(BetaInteropApi::class)
    @ObjCAction
    fun handlePan(pan: UIPanGestureRecognizer) {
        val scrollView = pan.scrollView() ?: return
        when (pan.state) {
            UIGestureRecognizerStateBegan -> {
                startOffsetX = scrollView.contentOffset.useContents { x }
            }

            UIGestureRecognizerStateChanged -> {
                val dx = pan.translationInView(pan.view).useContents { x }
                val x = (startOffsetX - dx).coerceIn(0.0, scrollView.maxOffsetX().coerceAtLeast(0.0))
                val y = scrollView.contentOffset.useContents { y }
                scrollView.setContentOffset(CGPointMake(x, y), animated = false)
            }
        }
    }

    private fun UIPanGestureRecognizer.scrollView(): UIScrollView? = (view as? WKWebView)?.scrollView

    private fun UIScrollView.maxOffsetX(): Double = contentSize.useContents { width } - bounds.useContents { size.width }
}

private class QuoteMessageHandler(
    private val onToggle: (Boolean) -> Unit,
) : NSObject(),
    WKScriptMessageHandlerProtocol {
    override fun userContentController(
        userContentController: WKUserContentController,
        didReceiveScriptMessage: WKScriptMessage,
    ) {
        (didReceiveScriptMessage.body as? NSNumber)?.let { onToggle(it.boolValue) }
    }
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

/** Receives `[extent, width]`. */
private class OverflowMessageHandler(
    private val onOverflow: (Int, Int) -> Unit,
) : NSObject(),
    WKScriptMessageHandlerProtocol {
    override fun userContentController(
        userContentController: WKUserContentController,
        didReceiveScriptMessage: WKScriptMessage,
    ) {
        val values = (didReceiveScriptMessage.body as? List<*>)?.map { (it as? NSNumber)?.intValue } ?: return
        val extent = values.getOrNull(0) ?: return
        val width = values.getOrNull(1) ?: return
        onOverflow(extent, width)
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

private class ImageMessageHandler(
    private val onOpenImage: (String) -> Unit,
) : NSObject(),
    WKScriptMessageHandlerProtocol {
    override fun userContentController(
        userContentController: WKUserContentController,
        didReceiveScriptMessage: WKScriptMessage,
    ) {
        (didReceiveScriptMessage.body as? String)?.takeIf { it.isNotBlank() }?.let(onOpenImage)
    }
}

internal actual val MailWebViewFollowsSystemFontScale: Boolean = false

// WKWebView has no shrink-to-fit counterpart (see fitWideContent above).
internal actual val MailWebViewFitsWideContent: Boolean = false

internal actual val MailWebViewPinchZooms: Boolean = true

internal actual val MailWebViewScrollsItself: Boolean = false
