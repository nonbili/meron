package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset

/** Renders a complete HTML document (mail body). WKWebView on iOS, WebView on
 *  Android. JavaScript is enabled so the embedded measurement script can report
 *  content height via [onContentHeight] (in dp), letting the caller size the
 *  view to fit the email.
 *
 *  [onLinkLongPress] reports a long press on a link, with the press position
 *  relative to the view's top-left corner, so the caller can show its own menu
 *  at the finger. iOS ignores it: WKWebView shows the native link menu itself.
 *
 *  [fitWideContent] lets a page that is still wider than the view after the CSS
 *  reflow overrides shrink to fit instead of being clipped, the way Gmail renders
 *  fixed-width mail: the page lays out at its natural width and is scaled down,
 *  with WebView's text autosizing inflating fonts so the smaller scale stays
 *  readable. The height bridge reports pre-scale CSS pixels, so the script
 *  multiplies by the fit scale to keep reported height in dp. Only the
 *  full-screen reader enables it — in a chat bubble a 640px mail would scale to
 *  a thumbnail, so bubbles reflow only unless the reader opts in (the auto-fit
 *  setting). iOS ignores it (see MailWebView.ios.kt).
 *
 *  [transparentBackground] lets whatever is behind the view show through where
 *  the page paints nothing, which darkened mail relies on. WKWebView is already
 *  non-opaque, so only Android acts on it. */
@Composable
expect fun MailWebView(
    html: String,
    modifier: Modifier,
    onContentHeight: (Dp) -> Unit,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit = {},
    onLinkLongPress: (String, DpOffset) -> Unit = { _, _ -> },
    fitWideContent: Boolean = false,
    transparentBackground: Boolean = false,
    /** The document's quote toggle was tapped: true when the quote is now open. */
    onQuoteToggle: (Boolean) -> Unit = {},
    /** The width a document of plain flowing text needs, in dp, so the caller can
     *  shrink its bubble to it; zero or less for a document that lays out against
     *  the width it is given (tables, pictures) and should fill it. */
    onNaturalWidth: (Dp) -> Unit = {},
    /** The measurement script's overflow floor, in layout CSS px, whenever it
     *  grows, with the CSS width it was measured at: content escaping the body
     *  that the caller hands back to the next document for the same mail. */
    onOverflowExtent: (extent: Int, width: Int) -> Unit = { _, _ -> },
    /** Retry failed images when the core restores attachment files. */
    mediaMissing: Int = 0,
)

/**
 * Whether a [MailWebView] below may draw. False while something above it is
 * compositing through an offscreen layer (a fade), which Android's WebView
 * cannot be drawn into: hwui crashes natively in its GL functor. Such a view
 * stays laid out, loading and measuring, and shows once this turns true.
 */
internal val LocalMailWebViewDrawable = compositionLocalOf { true }

/**
 * Whether the platform's web view already sizes CSS text by the system font
 * setting, the way `sp` sizes are scaled for the rest of the app.
 *
 * Android's WebView does: at a 1.5 system font scale, measured body line
 * heights grow by the same ~1.5 the Compose UI grows by. WKWebView does not —
 * Dynamic Type reaches Compose (`Density.fontScale`, mapped from the trait
 * collection's content size category) but never the web content, so message
 * bodies there have to be scaled by hand or HTML mail would stay at one size
 * while every plain-text body around it grew.
 */
internal expect val MailWebViewFollowsSystemFontScale: Boolean

/** Whether [MailWebView]'s `fitWideContent` does anything here, so a setting
 *  built on it is only offered where it works. */
internal expect val MailWebViewFitsWideContent: Boolean

/** Whether the reader can pinch [MailWebView] to zoom its page, in which case
 *  the document reports a height that grows with the zoom. */
internal expect val MailWebViewPinchZooms: Boolean

// Native update and navigation callbacks share this state. A recovery signal
// stays pending until the matching, parsed document installs its retry hooks.
internal class MailMediaRecovery(
    initialMissing: Int,
) {
    var generation: Int = 0
        private set
    private var missing = initialMissing
    private var revision = 0
    private var pending = false

    fun update(
        nextMissing: Int,
        documentChanged: Boolean,
    ) {
        if (documentChanged) {
            generation++
            revision++
        }
        if (nextMissing < missing) {
            pending = true
            revision++
        }
        missing = nextMissing
    }

    fun request(): MailMediaRecoveryRequest? = if (pending) MailMediaRecoveryRequest(generation, revision) else null

    fun acknowledge(
        request: MailMediaRecoveryRequest,
        installed: Boolean,
    ) {
        if (installed && request.generation == generation && request.revision == revision) pending = false
    }
}

internal data class MailMediaRecoveryRequest(
    val generation: Int,
    val revision: Int,
) {
    val script: String
        get() = RetryFailedMailImagesScript.replace("__GENERATION__", generation.toString()).replace("__REVISION__", revision.toString())
}

internal fun mailHtmlWithMediaGeneration(
    html: String,
    generation: Int,
): String = Regex("(?i)<html(?=\\s|>)").replaceFirst(html, "<html data-meron-media-generation=\"$generation\"")

internal val RetryFailedMailImagesScript =
    """
    (function() {
      if (document.documentElement.getAttribute('data-meron-media-generation') !== '__GENERATION__') return false;
      var revision = __REVISION__;
      if (window.__meronMediaRecoveryRevision === revision) return true;
      window.__meronMediaRecoveryRevision = revision;
      function install() {
        if (window.__meronMediaRecoveryRevision !== revision) return;
        document.querySelectorAll('img[src]').forEach(function(image) {
          if (image.complete && image.naturalWidth > 0) return;
          function retry() {
            if (window.__meronMediaRecoveryRevision !== revision || image.naturalWidth > 0) return;
            var src = image.getAttribute('src');
            if (!src) return;
            image.removeAttribute('src');
            image.setAttribute('src', src);
          }
          if (image.complete) retry();
          else {
            image.addEventListener('error', retry, { once: true });
            image.addEventListener('load', function() { image.removeEventListener('error', retry); }, { once: true });
          }
        });
      }
      if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', install, { once: true });
      else install();
      return true;
    })();
    """.trimIndent()
