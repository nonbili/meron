package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.applyRemoteContentPolicy
import jp.nonbili.meron.shared.mailBodyCsp
import kotlin.math.roundToInt
import kotlin.random.Random

/** A one-off token for the viewer's `script-src`. Only has to be unguessable by
 *  the mail rendered beside it, which never sees this process. */
internal fun randomScriptNonce(): String = buildString { repeat(4) { append(Random.nextInt(1, Int.MAX_VALUE).toString(36)) } }

/** `hue-rotate(180deg)` as the matrix the engine applies, row by row. */
internal val HUE_TURN =
    listOf(
        floatArrayOf(-0.574f, 1.43f, 0.144f),
        floatArrayOf(0.426f, 0.43f, 0.144f),
        floatArrayOf(0.426f, 1.43f, -0.856f),
    )

/**
 * The color a darkened body has to be painted with to come out as [surface]:
 * the darkening filter run backwards, like desktop's darkenedFloor. The filter
 * can't reach every color — a very saturated surface falls outside what it
 * produces — and then this is the nearest it gets to, a shade duller than the
 * surface itself.
 */
internal fun darkenedFloor(surface: Color): Color {
    val channels = floatArrayOf(surface.red, surface.green, surface.blue)
    val (r, g, b) = HUE_TURN.map { row -> (1f - (row[0] * channels[0] + row[1] * channels[1] + row[2] * channels[2])).coerceIn(0f, 1f) }
    return Color(r, g, b)
}

@Composable
internal fun HtmlMessageBody(
    html: String,
    // What remembers whether this body's quote was opened (see QuoteFoldMemory):
    // the message id, so the bubble and the reader agree.
    quoteKey: String = html.hashCode().toString(),
    // Whether this message's remote content may load. The mail is spliced into
    // the document below, so its own baked CSP meta ends up outside that
    // document's head, where a meta policy is ignored: the head built here is
    // what actually enforces the decision on both platforms.
    allowRemote: Boolean,
    maxHeight: Dp = Dp.Unspecified,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit = {},
    fitWideContent: Boolean = false,
    onNaturalWidth: (Dp) -> Unit = {},
    mediaMissing: Int = 0,
    // What a darkened body turns its white into: the bubble, row or card the
    // view sits on. Inverted white is black, which caps nothing.
    darkSurface: Color = Color.Black,
) {
    // The WebView can't tell Compose how tall its content is, so a tiny script
    // reports document height through a platform bridge and we size the view to
    // it. The bubble caps the height (desktop uses 360px) and the WebView scrolls
    // past that; the full-screen reader passes no cap and shows the whole email.
    // Saveable so a message the conversation list scrolled out and back in
    // returns at its measured height: its web view is a fresh one, and growing
    // from the placeholder while on screen would move the list under the reader.
    var contentHeightDp by rememberSaveable(html) { mutableFloatStateOf(0f) }
    val contentHeight = contentHeightDp.dp
    // A WebView reaches neither the app's text sizes nor the system font-size
    // setting, so the reading typography is baked into the stylesheet below.
    // Sizing the text rather than zooming the page is what the overrides here
    // already assume: they flatten the mail's own sizes, so scaling the two
    // declarations scales every body, and images and tables keep fitting the
    // width they were laid out for.
    // Plain-text bodies are sized in sp, which carries the system font setting
    // on both platforms. Where the web view doesn't apply that setting itself,
    // fold it in here so an HTML mail and a text one stay the same size.
    val systemFontScale = if (MailWebViewFollowsSystemFontScale) 1f else LocalDensity.current.fontScale
    val bodyFontSize = scaledCssPx(MESSAGE_HTML_BASE_PX * systemFontScale, LocalMessageFontScale.current)
    // The script's overflow floor (see contentHeight in the script), saved with
    // the height it produced, along with what laid it out: the typography and
    // fitting here, and the width, which only the document knows and checks
    // itself. A floor from another layout would hold text that now runs
    // shorter at its old height, so it is handed back only to the same one.
    // Read once into the document: a later report must not rebuild the page
    // it came from.
    val overflowLayout = "$bodyFontSize|${LocalDensity.current.fontScale}|$fitWideContent"
    var savedOverflowLayout by rememberSaveable(html) { mutableStateOf("") }
    var savedOverflowExtent by rememberSaveable(html) { mutableIntStateOf(0) }
    var savedOverflowWidth by rememberSaveable(html) { mutableIntStateOf(0) }
    val initialOverflow =
        remember(html, overflowLayout) {
            if (savedOverflowLayout == overflowLayout) savedOverflowExtent to savedOverflowWidth else 0 to 0
        }
    // A fresh nonce per document admits the measurement script below and nothing
    // else: the mail is spliced into this page, so a script of its own that
    // survived the core's sanitiser would still have no way to name the token.
    val scriptNonce = remember(html, allowRemote) { randomScriptNonce() }
    val showQuotedLabel = tr("chat.showQuotedText")
    val hideQuotedLabel = tr("chat.hideQuotedText")
    val darkBody = LocalDarkMailBodies.current
    val mobileHtml =
        remember(
            html,
            quoteKey,
            allowRemote,
            scriptNonce,
            fitWideContent,
            darkBody,
            darkSurface,
            bodyFontSize,
            showQuotedLabel,
            hideQuotedLabel,
            initialOverflow,
        ) {
            val body = applyRemoteContentPolicy(html, allowRemote)
            // The state the reader left the quote in, read once per document:
            // toggles inside the page don't rebuild (and so reload) it, they
            // are reported back through onQuoteToggle instead. A folded quote
            // is folded from the first paint — the class is on the root before
            // the body parses, and the script below only places the toggle.
            val quoteOpen = QuoteFoldMemory.isOpen(quoteKey)
            val quoteClass = if (html.contains(HTML_QUOTE_ATTR) && !quoteOpen) "meron-quote-folded" else ""
            val rootClass = listOfNotNull(quoteClass.ifEmpty { null }, "meron-dark".takeIf { darkBody }).joinToString(" ")
            // Inline for the same reason as the height override below: a
            // sender's `html { background }` would otherwise sit, undarkened,
            // behind the inverted body. Not left transparent: a body
            // background under a transparent root propagates to the canvas,
            // outside the body's filter, so a mail declaring a white body
            // stayed white while its text inverted to white on it. The root
            // paints the surface the view sits on, which is what the body's
            // inverted white comes out as too.
            val backdrop = darkSurface.copy(alpha = 1f)
            val rootBackground = if (darkBody) " background: ${backdrop.toHex()} !important;" else ""
            val floor = darkenedFloor(backdrop)
            val floorChannels = listOf(floor.red, floor.green, floor.blue).joinToString(", ") { (it * 255).roundToInt().toString() }
            """
            <!doctype html>
            <!-- The self-sizing WebView needs its document boxes to follow the
                 message. Newsletter resets commonly force html/body to
                 height:100%, pinning them to the empty initial viewport. The
                 override goes inline rather than into the head stylesheet:
                 sender styles are parsed later, and between two equally
                 specific !important rules the later one wins, while an inline
                 declaration outranks every stylesheet rule of the same
                 importance wherever the sender's <style> sits. -->
            <html class="$rootClass" style="height: auto !important; min-height: 0 !important;$rootBackground">
            <head>
              <meta http-equiv="Content-Security-Policy" content="${mailBodyCsp(allowRemote, scriptNonce)}">
              <meta id="meron-viewport" name="viewport" content="width=device-width, initial-scale=1.0">
              <style>
                html, body {
                  margin: 0;
                  padding: 0;
                  width: 100%;
                  /* `anywhere` also shrinks min-content to a single glyph, so a
                     narrow table cell (a 32px spacer holding a name, say) would
                     wrap its text one character per line. `break-word` still
                     breaks long words that would overflow, but leaves intrinsic
                     widths alone. */
                  overflow-wrap: break-word;
                  word-break: normal;
                  font-size: $bodyFontSize;
                  line-height: 1.45;
                }
                body, p, div, span, td, th, li, a {
                  font-size: $bodyFontSize !important;
                  line-height: 1.45 !important;
                }
                /* Preheaders hide their inbox-preview text with an inline
                   font-size:0; the override above would resurrect it as a
                   column of stray characters. */
                [style*="font-size:0"]:not([style*="font-size:0."]),
                [style*="font-size: 0"]:not([style*="font-size: 0."]) {
                  font-size: 0 !important;
                }
                /* max-width alone keeps fixed-pixel layouts (width="600") inside
                   the bubble. Forcing width:auto on top of it would also beat
                   the width="100%" attribute every email layout table relies on,
                   shrinking rows to their content and stranding right-aligned
                   cells and full-width dividers. */
                table {
                  max-width: 100% !important;
                }
                /* ...but max-width cannot shrink a table below its min-content
                   width, and nested fixed-width tables make that constraint
                   circular: the outer table's cell is sized by the inner
                   <table width="640">, so the inner table's max-width:100%
                   resolves against a 640px cell and never clamps, leaving the
                   outer table a 640px min-content width to inherit. The page
                   then lays out wider than the view and is clipped, not
                   scrolled. Clearing the width only where it is declared in
                   pixels breaks that chain at its source while leaving the
                   width="100%" tables above untouched. */
                table[width]:not([width$="%"]) {
                  width: auto !important;
                }
                table.code .diff-line-num {
                  width: 35px !important;
                  min-width: 35px;
                  white-space: nowrap;
                }
                td.line_content pre,
                th.line_content pre {
                  margin: 0 !important;
                  padding: 0 !important;
                  border: 0 !important;
                  border-radius: 0;
                  overflow-wrap: anywhere;
                  white-space: pre-wrap;
                }
                td.line_content pre code,
                th.line_content pre code {
                  min-width: 0;
                }
                img {
                  max-width: 100% !important;
                  height: auto !important;
                }
                div[data-meron-image-grid] {
                  display: flex !important;
                  flex-wrap: wrap !important;
                  gap: 4px !important;
                }
                div[data-meron-image-grid] > * {
                  flex: 1 1 30% !important;
                  max-width: calc(33.333% - 3px) !important;
                  box-sizing: border-box !important;
                  margin: 0 !important;
                }
                /* The folded quoted tail and its toggle (see QuoteFold.kt). The
                   core strips sender `data-*` attributes and meron- classes, so
                   both are ours. */
                html.meron-quote-folded [data-meron-quote] {
                  display: none !important;
                }
                button.meron-quote-toggle {
                  display: block;
                  width: 32px;
                  height: 16px;
                  margin: 6px 0;
                  padding: 0;
                  border: 1px solid currentColor;
                  border-radius: 8px;
                  background: transparent;
                  color: inherit;
                  opacity: 0.5;
                  font: 700 11px/1 sans-serif;
                  letter-spacing: 1px;
                }
                /* Open: the chip stays lit, so it reads as a state rather than
                   as the same button as when folded. */
                button.meron-quote-toggle[aria-expanded="true"] {
                  opacity: 0.9;
                  background: color-mix(in srgb, currentColor 16%, transparent);
                }
                button.meron-quote-toggle::before {
                  content: '•••';
                }
                /* Dark bodies: the mail is drawn inverted, light text on dark,
                   with the hue turned back so links and brand colors stay
                   recognisable. Pictures are inverted a second time to come
                   out as sent (background ones are marked by applyDarkBody
                   below, which also drops the class for mail designed dark). */
                html.meron-dark body,
                html.meron-dark img,
                html.meron-dark video,
                html.meron-dark [data-meron-picture] {
                  filter: invert(1) hue-rotate(180deg);
                }
                /* ...but only once: inside a re-inverted picture an image is
                   already back to its own colors. */
                html.meron-dark [data-meron-picture] img {
                  filter: none;
                }
                /* Inverted white is black, not the surface the view sits on,
                   so a layer over the body caps how light it gets before the
                   filter runs: at the color that inverts to that surface (see
                   darkenedFloor). The layer is the first thing positioned in
                   the body, above everything in the flow and below whatever
                   comes after it with a filter of its own -- the pictures,
                   which a cap on the whole body would tint with the surface
                   instead. What else paints above it is capped one element at
                   a time (see capBackgrounds below). */
                html.meron-dark body::before {
                  content: '' !important;
                  display: block !important;
                  position: absolute !important;
                  inset: 0 !important;
                  background: ${floor.toHex()} !important;
                  mix-blend-mode: darken !important;
                  pointer-events: none !important;
                }
              </style>
            </head>
            <body style="height: auto !important; min-height: 0 !important;">$body
              <script nonce="$scriptNonce">
                (function () {
                  // Feed/newsletter HTML often lists photos as a bare run of
                  // sibling `<img>` (or single-image `<p>`/`<div>`) elements,
                  // which would otherwise stack one per row at full width.
                  // Wrap runs of 2+ into a flex grid so they tile 2-3 across.
                  function isImageOnlyBlock(el) {
                    if (!el || el.nodeType !== 1) return false;
                    if (el.tagName === 'IMG') return true;
                    if (el.children.length !== 1) return false;
                    for (var i = 0; i < el.childNodes.length; i++) {
                      var n = el.childNodes[i];
                      if (n.nodeType === 3 && n.textContent.trim().length > 0) return false;
                    }
                    return isImageOnlyBlock(el.children[0]);
                  }
                  function findImageBlock(img) {
                    var node = img;
                    while (node.parentElement && node.parentElement !== document.body) {
                      if (isImageOnlyBlock(node.parentElement)) {
                        node = node.parentElement;
                      } else {
                        break;
                      }
                    }
                    return node;
                  }
                  function groupConsecutiveImages() {
                    var imgs = Array.prototype.slice.call(document.querySelectorAll('img'));
                    var blocks = [];
                    var seen = [];
                    imgs.forEach(function (img) {
                      var block = findImageBlock(img);
                      if (seen.indexOf(block) === -1) {
                        seen.push(block);
                        blocks.push(block);
                      }
                    });
                    var i = 0;
                    while (i < blocks.length) {
                      var run = [blocks[i]];
                      var j = i + 1;
                      while (
                        j < blocks.length &&
                        run[run.length - 1].nextElementSibling === blocks[j] &&
                        run[run.length - 1].parentElement === blocks[j].parentElement
                      ) {
                        run.push(blocks[j]);
                        j++;
                      }
                      if (run.length > 1) {
                        var grid = document.createElement('div');
                        grid.setAttribute('data-meron-image-grid', '1');
                        run[0].parentNode.insertBefore(grid, run[0]);
                        run.forEach(function (block) {
                          grid.appendChild(block);
                        });
                      }
                      i = j;
                    }
                  }
                  // Mail bodies arrive as whole documents and routinely carry
                  // their own <meta name="viewport">, which lands in our body
                  // and, being later in the document, is the one Blink honours.
                  // A single stray `content="target-densitydpi=device-dpi"` --
                  // common in mail templates, and what Gemini's welcome mail
                  // ships -- declares no width at all, so with useWideViewPort
                  // on the layout falls back to the 980px desktop default and
                  // the whole page renders at ~0.37 scale: legible mail shrunk
                  // to nothing. Dropping every viewport but ours puts the width
                  // back under our control, which is also what applyWidthFit
                  // below assumes when it measures and rewrites.
                  function dropForeignViewports() {
                    var metas = document.querySelectorAll('meta[name="viewport"]');
                    var ours = null;
                    for (var i = 0; i < metas.length; i++) {
                      if (metas[i].id === 'meron-viewport') {
                        ours = metas[i];
                      } else if (metas[i].parentNode) {
                        metas[i].parentNode.removeChild(metas[i]);
                      }
                    }
                    // Removing a meta does not make Blink recompute the viewport
                    // -- the mail's description stays in effect over an empty
                    // head -- but writing to one does. Re-asserting the same
                    // content is what actually applies the width above.
                    if (ours) ours.setAttribute('content', ours.getAttribute('content'));
                  }
                  // The CSS overrides reflow most mail into the view. What they
                  // cannot shrink -- a wide <pre>, an oversized image, a fixed
                  // width in an inline style rather than the width attribute --
                  // would otherwise be clipped outright, because the view is
                  // sized to its content and never scrolls sideways. Widening
                  // the viewport to the content's natural width instead makes
                  // WebView scale the whole page down to fit.
                  var fitWide = ${if (fitWideContent) "true" else "false"};
                  var fitScale = 1;
                  var viewWidth = 0;
                  function visibleWidth() {
                    return (
                      (window.visualViewport && window.visualViewport.width) ||
                      window.innerWidth ||
                      0
                    );
                  }
                  function applyWidthFit() {
                    // Pin once: after the viewport widens, the page is no longer
                    // overflowing, so re-measuring would just undo the fit.
                    if (!fitWide || fitScale !== 1) return;
                    // By id, not by name: a mail's own viewport meta would win
                    // over ours in Blink, so rewriting the first match could
                    // rewrite a tag the engine is already ignoring.
                    var meta = document.getElementById('meron-viewport');
                    if (!meta) return;
                    // Captured before the viewport widens, so it stays the view's
                    // width in dp -- the denominator the height bridge needs.
                    if (!viewWidth) viewWidth = visibleWidth();
                    if (!viewWidth) return;
                    var natural = Math.max(
                      document.documentElement.scrollWidth || 0,
                      document.body ? document.body.scrollWidth : 0
                    );
                    // A little slack: sub-pixel table borders routinely round up
                    // and are not worth shrinking the whole page for.
                    if (natural <= viewWidth + 2) return;
                    // Widening the viewport alone does not rescale a page that has
                    // already loaded: the layout grows to the natural width but
                    // the scale stays 1, leaving the mail sideways-scrollable. So
                    // the fit scale is stated outright.
                    var width = Math.ceil(natural);
                    fitScale = viewWidth / width;
                    meta.setAttribute(
                      'content',
                      'width=' + width + ', initial-scale=' + fitScale + ', minimum-scale=' + fitScale
                    );
                  }
                  // The root element is the one that scrolls, so its scrollHeight
                  // never drops below the view's own height -- sizing off it pins
                  // a short mail to whatever height the view was first laid out
                  // at and lets the measurement only ever grow, padding a
                  // one-line message out with blank space. Measure the boxes,
                  // whose heights are auto and so track the content, and keep
                  // scrollHeight purely as an overflow signal for content that
                  // escapes the body (an absolutely positioned block whose
                  // containing block is the initial one reaches nothing else).
                  //
                  // That signal only says anything while the view is still
                  // shorter than the content: once it grows to fit, scrollHeight
                  // and clientHeight agree again and the reading is
                  // indistinguishable from an empty document. So an extent, once
                  // seen, is carried as a floor for the rest of this document --
                  // re-deriving it each pass is what would make the view
                  // oscillate between the overflow height and the empty box.
                  //
                  // A floor is only worth carrying for a scroll area the boxes
                  // cannot account for. A tall ordinary mail overflows the
                  // view's first layout too, and retaining that would pin it
                  // there: text that reflows shorter once the view settles at
                  // its width could never give the height back.
                  //
                  // The desktop frame applies the same rules; its arithmetic
                  // (and the tests pinning it) lives in chat/frameHeight.ts.
                  //
                  // A list item that scrolled out and back in gets a fresh
                  // document in a view already sized to the saved height, where
                  // the overflow fits and so never shows: the floor it had found
                  // is handed back, or the escaping content reads as the empty
                  // box and the item collapses before growing back. Only at the
                  // width it was found at, checked on the first measurement:
                  // narrower or wider, the content runs to a different length.
                  var overflowExtent = 0;
                  var seededOverflowExtent = ${initialOverflow.first};
                  var seededOverflowWidth = ${initialOverflow.second};
                  var layoutWidth = 0;
                  var reportedOverflowExtent = 0;
                  function contentHeight() {
                    var root = document.documentElement;
                    var body = document.body;
                    var h = 0;
                    // Rects are viewport-relative: add back how far a view
                    // shorter than the mail has scrolled down it.
                    var scrolled = window.scrollY || 0;
                    if (body) {
                      var rect = body.getBoundingClientRect();
                      var top = rect.top + scrolled;
                      h = Math.max(h, top + rect.height, top + body.scrollHeight);
                    }
                    if (root) {
                      var rootRect = root.getBoundingClientRect();
                      h = Math.max(h, rootRect.top + scrolled + rootRect.height);
                      if (
                        root.scrollHeight > root.clientHeight + 1 &&
                        root.scrollHeight > h + 1
                      ) {
                        overflowExtent = Math.max(overflowExtent, root.scrollHeight);
                      }
                    }
                    return Math.max(h, overflowExtent);
                  }
                  // What a document of plain flowing text needs: the body laid out
                  // at max-content, read and put back within the same task so
                  // nothing paints it. Anything whose layout follows the width it
                  // is given (tables, pictures, fixed widths) reports -1, and the
                  // bubble stays at full width for it.
                  function naturalWidth() {
                    var body = document.body;
                    if (!body || body.querySelector('table, img, video, iframe, svg, pre, [width], [style*="width"]')) return -1;
                    var style = body.style;
                    var w = [style.getPropertyValue('width'), style.getPropertyPriority('width')];
                    var mw = [style.getPropertyValue('max-width'), style.getPropertyPriority('max-width')];
                    style.setProperty('width', 'max-content', 'important');
                    style.setProperty('max-width', 'none', 'important');
                    var natural = Math.ceil(body.getBoundingClientRect().width);
                    if (w[0]) style.setProperty('width', w[0], w[1]); else style.removeProperty('width');
                    if (mw[0]) style.setProperty('max-width', mw[0], mw[1]); else style.removeProperty('max-width');
                    return natural > 0 ? Math.ceil(natural * fitScale) : -1;
                  }
                  // How far the reader has pinched in, where the view allows it.
                  // Taken from the two viewports rather than visualViewport.scale:
                  // the fit above already sets a page scale, and their ratio is 1
                  // at whatever scale the document rests at. The view is sized
                  // to its content and never scrolls down, so the height it is
                  // given has to grow with the zoom or the enlarged mail would
                  // be cut off at its unzoomed length.
                  var pinchZooms = ${if (MailWebViewPinchZooms) "true" else "false"};
                  function pinchZoom() {
                    var vv = window.visualViewport;
                    var layout = document.documentElement ? document.documentElement.clientWidth : 0;
                    if (!pinchZooms || !vv || !vv.width || !layout) return 1;
                    var zoom = layout / vv.width;
                    // Rounding in either width is not a zoom.
                    return zoom > 1.01 ? zoom : 1;
                  }
                  function report() {
                    // A lazy list can attach a fresh web view and load it before
                    // the view is laid out. At zero width every word wraps onto a
                    // line of its own, and a mail that needs 8000px reads as
                    // hundreds of thousands: reported, that sizes the list item
                    // off into the distance and the list jumps. The resize
                    // observer reports again once the view has its width.
                    if (!visibleWidth()) return;
                    if (!layoutWidth) {
                      // Before the fit pass, which widens the viewport.
                      layoutWidth = Math.round(visibleWidth());
                      if (seededOverflowWidth === layoutWidth) overflowExtent = seededOverflowExtent;
                      reportedOverflowExtent = overflowExtent;
                    }
                    applyWidthFit();
                    var nw = naturalWidth();
                    if (window.MeronWidth && window.MeronWidth.report) {
                      window.MeronWidth.report(nw);
                    } else if (
                      window.webkit &&
                      window.webkit.messageHandlers &&
                      window.webkit.messageHandlers.meronWidth
                    ) {
                      window.webkit.messageHandlers.meronWidth.postMessage(nw);
                    }
                    // The measurement is in the (possibly widened) layout
                    // viewport's CSS pixels; the view renders it at fitScale, so
                    // scale it back to dp or the view gets sized to a phantom tail.
                    var h = Math.ceil(contentHeight() * fitScale * pinchZoom());
                    if (overflowExtent !== reportedOverflowExtent) {
                      reportedOverflowExtent = overflowExtent;
                      if (window.MeronOverflow && window.MeronOverflow.report) {
                        window.MeronOverflow.report(overflowExtent, layoutWidth);
                      } else if (
                        window.webkit &&
                        window.webkit.messageHandlers &&
                        window.webkit.messageHandlers.meronOverflow
                      ) {
                        window.webkit.messageHandlers.meronOverflow.postMessage([overflowExtent, layoutWidth]);
                      }
                    }
                    if (window.MeronHeight && window.MeronHeight.report) {
                      window.MeronHeight.report(h);
                    } else if (
                      window.webkit &&
                      window.webkit.messageHandlers &&
                      window.webkit.messageHandlers.meronHeight
                    ) {
                      window.webkit.messageHandlers.meronHeight.postMessage(h);
                    }
                  }
                  document.addEventListener('click', function (event) {
                    var target = event.target;
                    var image = target && target.closest ? target.closest('img[src]') : null;
                    // A tap on a picture opens it in Meron's viewer, even inside a
                    // link; the link is a long-press away (see MailWebView).
                    if (image) {
                      // As written, which is what the gallery matches against; only
                      // a protocol-relative src gets a scheme, or it could not open.
                      var src = image.getAttribute('src');
                      if (src && src.indexOf('//') === 0) src = 'https:' + src;
                      if (src) {
                        event.preventDefault();
                        if (window.MeronImage && window.MeronImage.open) {
                          window.MeronImage.open(src);
                        } else if (
                          window.webkit &&
                          window.webkit.messageHandlers &&
                          window.webkit.messageHandlers.meronImage
                        ) {
                          window.webkit.messageHandlers.meronImage.postMessage(src);
                        }
                        return;
                      }
                    }
                    var anchor = target && target.closest ? target.closest('a[href]') : null;
                    if (!anchor) return;
                    var href = anchor.getAttribute('href');
                    if (!href || href.charAt(0) === '#') return;
                    event.preventDefault();
                    var url = anchor.href || href;
                    if (window.MeronLink && window.MeronLink.open) {
                      window.MeronLink.open(url);
                    } else if (
                      window.webkit &&
                      window.webkit.messageHandlers &&
                      window.webkit.messageHandlers.meronLink
                    ) {
                      window.webkit.messageHandlers.meronLink.postMessage(url);
                    }
                  });
                  // Put the "•••" toggle where the core-marked quote starts. The
                  // root already carries the folded class; opening it grows the
                  // document, which the ResizeObserver below reports.
                  var showQuotedLabel = ${jsStringLiteral(showQuotedLabel)};
                  var hideQuotedLabel = ${jsStringLiteral(hideQuotedLabel)};
                  var quoteInitiallyOpen = $quoteOpen;
                  function reportQuoteToggle(open) {
                    if (window.MeronQuote && window.MeronQuote.toggle) {
                      window.MeronQuote.toggle(open);
                    } else if (
                      window.webkit &&
                      window.webkit.messageHandlers &&
                      window.webkit.messageHandlers.meronQuote
                    ) {
                      window.webkit.messageHandlers.meronQuote.postMessage(open);
                    }
                  }
                  function installQuoteFold() {
                    var first = document.querySelector('[data-meron-quote]');
                    if (!first || !first.parentNode) return;
                    var root = document.documentElement;
                    var toggle = document.createElement('button');
                    toggle.type = 'button';
                    toggle.className = 'meron-quote-toggle';
                    first.parentNode.insertBefore(toggle, first);
                    function apply(open) {
                      root.classList.toggle('meron-quote-folded', !open);
                      toggle.setAttribute('aria-label', open ? hideQuotedLabel : showQuotedLabel);
                      toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
                    }
                    toggle.addEventListener('click', function (event) {
                      event.preventDefault();
                      event.stopPropagation();
                      var open = root.classList.contains('meron-quote-folded');
                      apply(open);
                      reportQuoteToggle(open);
                      report();
                    });
                    apply(quoteInitiallyOpen);
                  }
                  installQuoteFold();
                  // Before the first report: every width the script measures,
                  // and the view height derived from them, is read under the
                  // viewport this leaves in place.
                  // Dark bodies (see the meron-dark rules): a mail designed
                  // dark already -- a dark page, or light text written for a
                  // dark client -- would come out bright if inverted, so it
                  // keeps its own colors. Judged the way the desktop frames
                  // do: the page color the core hoisted or the body paints,
                  // else the tone most of the visible text is drawn in.
                  function colorParts(value) {
                    var match = /^rgba?\(([^)]*)\)/.exec((value || '').trim());
                    if (!match) return null;
                    var parts = match[1].split(/[\s,\/]+/).filter(Boolean).map(parseFloat);
                    return parts.length < 3 ? null : parts;
                  }
                  function colorTone(value) {
                    var parts = colorParts(value);
                    if (!parts || (parts.length > 3 && parts[3] < 0.6)) return null;
                    var lum = (0.2126 * parts[0] + 0.7152 * parts[1] + 0.0722 * parts[2]) / 255;
                    return lum > 0.55 ? 'light' : 'dark';
                  }
                  function bodyMeta(name) {
                    var meta = document.querySelector('meta[name="' + name + '"]');
                    return meta ? (meta.getAttribute('content') || '').trim() : '';
                  }
                  function canvasTone() {
                    var probe = document.createElement('span');
                    var declared = bodyMeta('meron-body-bg');
                    if (declared) {
                      probe.style.color = declared;
                      document.body.appendChild(probe);
                      var tone = colorTone(getComputedStyle(probe).color);
                      probe.remove();
                      if (tone) return tone;
                    }
                    return colorTone(getComputedStyle(document.body).backgroundColor);
                  }
                  function textTone() {
                    var light = 0;
                    var dark = 0;
                    (function walk(el) {
                      if (/^(STYLE|SCRIPT|BUTTON)$/.test(el.tagName)) return;
                      var style = getComputedStyle(el);
                      if (style.display === 'none' || style.visibility === 'hidden') return;
                      if (style.opacity === '0' || style.fontSize === '0px') return;
                      var tone = colorTone(style.color);
                      for (var i = 0; i < el.childNodes.length; i++) {
                        var node = el.childNodes[i];
                        if (node.nodeType === 1) {
                          walk(node);
                        } else if (node.nodeType === 3 && tone) {
                          var length = node.textContent.trim().length;
                          if (tone === 'light') light += length;
                          else dark += length;
                        }
                      }
                    })(document.body);
                    if (!light && !dark) return null;
                    return light > dark ? 'light' : 'dark';
                  }
                  // A picture drawn as a background is turned back to its own
                  // colors with the images -- but only where it is the whole
                  // of the element: turning a box back also turns back the
                  // text in it, which then reads as sent on a canvas that no
                  // longer is.
                  function markBackgroundPictures() {
                    var all = document.body.getElementsByTagName('*');
                    for (var i = 0; i < all.length; i++) {
                      var el = all[i];
                      if (getComputedStyle(el).backgroundImage.indexOf('url(') === -1) continue;
                      if (el.textContent.trim()) continue;
                      el.setAttribute('data-meron-picture', '');
                    }
                  }
                  // The layer over the body (see the meron-dark rules) caps
                  // what is in the flow, but a positioned box paints above that
                  // layer, and so does everything inside one: its white would
                  // invert to black, not to the surface. Such a background is
                  // held to the floor on the element itself; the pictures are
                  // turned back to their own colors, which a cap would then
                  // show through.
                  var darkFloor = [$floorChannels];
                  function capBackgrounds() {
                    var all = document.body.querySelectorAll('*');
                    // Written once the walk has read every element: a write in
                    // between would have the engine resolve the document's
                    // styles again for the next read.
                    var capped = [];
                    for (var i = 0; i < all.length; i++) {
                      var el = all[i];
                      if (el.matches('img, video, [data-meron-picture], [data-meron-picture] *')) continue;
                      var color = colorParts(getComputedStyle(el).backgroundColor);
                      if (!color || color[3] === 0) continue;
                      if (color[0] <= darkFloor[0] && color[1] <= darkFloor[1] && color[2] <= darkFloor[2]) continue;
                      var channels = [0, 1, 2].map(function (c) { return Math.min(color[c], darkFloor[c]); });
                      capped.push([el, 'rgba(' + channels.join(', ') + ', ' + (color.length > 3 ? color[3] : 1) + ')']);
                    }
                    capped.forEach(function (pair) {
                      pair[0].style.setProperty('background-color', pair[1], 'important');
                    });
                  }
                  // The sanitiser drops <body>, so the core hoists its colors
                  // into metas. They go back on every mail, dark rendering or
                  // not, and as the pair they are: white text written for a
                  // black <body> is white on the canvas without it, and a
                  // background alone, or the page left transparent with the
                  // default black text on it, is dark on dark. Put back as
                  // the inline declarations they were, with the priority they
                  // were written at; a surviving declaration of the mail's
                  // own is left alone.
                  function restoreDeclaredCanvas() {
                    var important = bodyMeta('meron-body-important').split(/\s+/);
                    [['background-color', 'meron-body-bg'], ['color', 'meron-body-fg']].forEach(function (pair) {
                      var value = bodyMeta(pair[1]);
                      if (!value || document.body.style.getPropertyValue(pair[0])) return;
                      document.body.style.setProperty(pair[0], value, important.indexOf(pair[0]) === -1 ? '' : 'important');
                    });
                  }
                  function applyDarkBody() {
                    var root = document.documentElement;
                    if (!root.classList.contains('meron-dark')) return;
                    var canvas = canvasTone();
                    if (canvas === 'dark') {
                      root.classList.remove('meron-dark');
                      return;
                    }
                    if (!canvas && textTone() === 'light') {
                      root.classList.remove('meron-dark');
                      return;
                    }
                    markBackgroundPictures();
                    capBackgrounds();
                  }
                  dropForeignViewports();
                  groupConsecutiveImages();
                  restoreDeclaredCanvas();
                  applyDarkBody();
                  window.addEventListener('load', report);
                  document.addEventListener('DOMContentLoaded', report);
                  if (window.ResizeObserver) {
                    new ResizeObserver(report).observe(document.documentElement);
                  }
                  if (pinchZooms && window.visualViewport) {
                    window.visualViewport.addEventListener('resize', report);
                  }
                  setTimeout(report, 300);
                })();
              </script>
            </body>
            </html>
            """.trimIndent()
        }
    val measured = contentHeight > 0.dp
    // Where the web view scrolls a long mail itself (Android, see
    // MailWebViewScrollsItself), it is never made taller than this: the bubble's
    // cap, or, uncapped, most of the window, so the conversation can still
    // bring its header and the next message into view around it.
    val selfScrollHeight =
        if (!MailWebViewScrollsItself || !LocalScrollLongHtmlMessages.current) {
            Dp.Unspecified
        } else if (maxHeight != Dp.Unspecified) {
            maxHeight
        } else {
            with(LocalDensity.current) {
                LocalWindowInfo.current.containerSize.height
                    .toDp()
            } * 0.75f
        }
    val scrollsItself = measured && selfScrollHeight != Dp.Unspecified && selfScrollHeight > 0.dp && contentHeight > selfScrollHeight
    val capped = !scrollsItself && maxHeight != Dp.Unspecified && measured && contentHeight > maxHeight
    val webViewModifier =
        Modifier
            .fillMaxWidth()
            .then(
                if (scrollsItself) {
                    Modifier.height(selfScrollHeight)
                } else if (measured) {
                    Modifier.height(contentHeight)
                } else {
                    // Fixed, not a minimum: in a lazy list the height is otherwise
                    // unbounded and the web view sizes itself to its own layout,
                    // which before the view has a width is hundreds of thousands
                    // of pixels of one-word lines.
                    Modifier.height(80.dp)
                },
            )

    // One call site whichever way the height falls: crossing the cap only swaps
    // the wrapper's modifiers. Were the web view composed in two places, growing
    // past the cap (opening a long quote does) would build a new one and reload
    // the page, losing whatever the reader just did in it.
    val htmlScrollState = rememberScrollState()
    Box(
        Modifier
            .fillMaxWidth()
            .then(
                if (capped) {
                    Modifier
                        .height(maxHeight)
                        .appScrollbar(htmlScrollState, endOffset = HtmlBubbleHorizontalPadding)
                        .verticalScroll(htmlScrollState)
                } else {
                    Modifier
                },
            ),
    ) {
        MailWebViewWithLinkMenu(
            html = mobileHtml,
            mediaMissing = mediaMissing,
            onContentHeight = { contentHeightDp = clampMailBodyHeight(it).value },
            onOverflowExtent = { extent, width ->
                savedOverflowLayout = overflowLayout
                savedOverflowExtent = extent
                savedOverflowWidth = width
            },
            onOpenUrl = onOpenUrl,
            onOpenImage = onOpenImage,
            onQuoteToggle = { open -> QuoteFoldMemory.setOpen(quoteKey, open) },
            onNaturalWidth = onNaturalWidth,
            modifier = webViewModifier,
            fitWideContent = fitWideContent,
            transparentBackground = darkBody,
            scrollsItself = scrollsItself,
        )
    }
}

/** The web view plus the link menu it asks for on long press. The menu is anchored to
 *  a box laid out exactly like the web view, so the reported press position places it. */
@Composable
internal fun MailWebViewWithLinkMenu(
    html: String,
    onContentHeight: (Dp) -> Unit,
    onOverflowExtent: (Int, Int) -> Unit,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit,
    onQuoteToggle: (Boolean) -> Unit,
    onNaturalWidth: (Dp) -> Unit,
    modifier: Modifier,
    fitWideContent: Boolean,
    transparentBackground: Boolean,
    mediaMissing: Int,
    scrollsItself: Boolean,
) {
    var menuTarget by remember { mutableStateOf<MessageLinkMenuTarget?>(null) }
    Box(modifier) {
        MailWebView(
            html = html,
            mediaMissing = mediaMissing,
            onContentHeight = onContentHeight,
            onOverflowExtent = onOverflowExtent,
            onOpenUrl = onOpenUrl,
            onOpenImage = onOpenImage,
            onLinkLongPress = { url, offset -> menuTarget = MessageLinkMenuTarget(url, offset) },
            fitWideContent = fitWideContent,
            transparentBackground = transparentBackground,
            onQuoteToggle = onQuoteToggle,
            onNaturalWidth = onNaturalWidth,
            scrollsItself = scrollsItself,
            modifier = Modifier.fillMaxSize(),
        )
        MessageLinkContextMenu(
            target = menuTarget,
            onDismiss = { menuTarget = null },
            onOpenUrl = onOpenUrl,
        )
    }
}
