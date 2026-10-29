package jp.nonbili.meron.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.sign

/** How far past the plain touch slop a drag must run sideways before the swipe
 *  claims it. What scrolls inside the conversation (the list vertically, the
 *  attachment strip and a zoomed mail body sideways) claims at the plain slop,
 *  so it always gets the first say. */
private const val CLAIM_SLOP_FACTOR = 2f

/** A drag towards a side with no conversation behind it moves this much. */
private const val EDGE_RESISTANCE = 0.2f

/** The share of the width a released drag must cover to page over. */
private const val COMMIT_FRACTION = 0.3f

/**
 * Paging between conversations by swiping the open one sideways, as Gmail does:
 * towards the start for the next (older) conversation in the list it was opened
 * from, towards the end for the previous (newer) one. The conversations either
 * side are laid out a page width away and move with [offsetPx], so the one
 * being swiped to shows as it is.
 *
 * Held above the pages, so the settle outlives the page that started it.
 */
@Stable
internal class ThreadSwipeState(
    private val scope: CoroutineScope,
) {
    var offsetPx by mutableFloatStateOf(0f)
        internal set
    var settling by mutableStateOf(false)
        private set

    /** The width the pages are laid out a step apart by. */
    var pageWidthPx by mutableFloatStateOf(0f)

    /** Lets go of a drag: slides the conversation beside it fully in and
     *  [open]s it, which puts it at the centre as the offset returns to zero in
     *  the same frame; or, with no [open], springs back. */
    fun release(
        velocityPx: Float,
        widthPx: Float,
        open: (() -> Unit)?,
    ) {
        scope.launch {
            settling = true
            try {
                if (open != null && widthPx > 0f) {
                    // The neighbour sits a page width away on the far side.
                    val through = sign(offsetPx) * widthPx
                    animate(offsetPx, through, velocityPx, spring(stiffness = Spring.StiffnessMediumLow)) { value, _ -> offsetPx = value }
                    open()
                } else {
                    animate(offsetPx, 0f, velocityPx, spring(stiffness = Spring.StiffnessMedium)) { value, _ -> offsetPx = value }
                }
            } finally {
                offsetPx = 0f
                settling = false
            }
        }
    }
}

@Composable
internal fun rememberThreadSwipeState(): ThreadSwipeState {
    val scope = rememberCoroutineScope()
    return remember { ThreadSwipeState(scope) }
}

/** Drives [state] from a sideways drag and pages to [onNext] or [onPrevious]
 *  when it is let go far or fast enough; null where there is no conversation
 *  that way, which leaves only a stiff rubber band. The pages read
 *  [ThreadSwipeState.offsetPx] to move. */
@Composable
internal fun Modifier.threadSwipe(
    state: ThreadSwipeState,
    onPrevious: (() -> Unit)?,
    onNext: (() -> Unit)?,
): Modifier {
    val previous by rememberUpdatedState(onPrevious)
    val next by rememberUpdatedState(onNext)
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    return pointerInput(state, rtl) {
        val slop = viewConfiguration.touchSlop
        val flingVelocity = 400.dp.toPx()
        awaitEachGesture {
            val down = awaitFirstDown(requireUnconsumed = false)
            if (state.settling) return@awaitEachGesture
            val tracker = VelocityTracker()
            tracker.addPosition(down.uptimeMillis, down.position)
            var dx = 0f
            var dy = 0f
            while (true) {
                val event = awaitPointerEvent()
                // A second finger is a pinch, the mail body's.
                if (event.changes.size > 1) return@awaitEachGesture
                val change = event.changes.firstOrNull { it.id == down.id } ?: return@awaitEachGesture
                if (!change.pressed || change.isConsumed) return@awaitEachGesture
                val delta = change.positionChange()
                dx += delta.x
                dy += delta.y
                tracker.addPosition(change.uptimeMillis, change.position)
                if (abs(dy) > slop && abs(dy) >= abs(dx)) return@awaitEachGesture
                if (abs(dx) > slop * CLAIM_SLOP_FACTOR && abs(dx) > abs(dy) * 2) {
                    change.consume()
                    break
                }
            }

            // Whether content dragged by [offset] is uncovering the next conversation.
            fun towardNext(offset: Float) = if (rtl) offset > 0f else offset < 0f

            fun target(offset: Float) = if (towardNext(offset)) next else previous
            var drag = 0f
            var released = false
            try {
                while (true) {
                    val event = awaitPointerEvent()
                    val change = event.changes.firstOrNull { it.id == down.id } ?: break
                    tracker.addPosition(change.uptimeMillis, change.position)
                    // The lift carries the last stretch of movement too.
                    drag += change.positionChange().x
                    change.consume()
                    state.offsetPx = if (target(drag) != null) drag else drag * EDGE_RESISTANCE
                    if (!change.pressed) break
                }
                val velocity = tracker.calculateVelocity().x
                val offset = state.offsetPx
                val width = state.pageWidthPx.takeIf { it > 0f } ?: size.width.toFloat()
                val flung = abs(velocity) > flingVelocity && sign(velocity) == sign(offset)
                val open = target(offset)?.takeIf { abs(offset) > width * COMMIT_FRACTION || flung }
                released = true
                state.release(velocity, width, open)
            } finally {
                // A cancelled gesture must not leave the conversation half off screen.
                if (!released) state.release(0f, 0f, null)
            }
        }
    }
}

/** Swallows every touch before anything inside sees it, for a page shown beside
 *  the open conversation: its buttons act on the open one. */
internal fun Modifier.blockPointerInput(): Modifier =
    pointerInput(Unit) {
        awaitPointerEventScope {
            while (true) {
                awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() }
            }
        }
    }
