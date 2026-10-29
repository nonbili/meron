package jp.nonbili.meron.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectVerticalDragGestures
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/** A release at least this fast downwards dismisses, however short the pull. */
private const val DISMISS_VELOCITY = 1000f

/**
 * Pulling a full-screen view down past the top of its content to close it:
 * the message reader back to its conversation, a conversation back to its list.
 * The view follows the finger with growing resistance, shrinking and fading as
 * it goes, and closes when let go far or fast enough; otherwise it springs back.
 */
@Stable
internal class PullToDismissState(
    private val scope: CoroutineScope,
    private val dismissDistancePx: Float,
    private val fallbackHeightPx: Float,
) {
    var distancePx by mutableFloatStateOf(0f)
        private set

    /** The height of the view being pulled, which it slides down by to close. */
    var heightPx by mutableFloatStateOf(0f)

    /** Whether the touch in progress went down with the content at its top. A
     *  scroll up that reaches the top mid-drag stops there instead of pulling. */
    internal var armed = false

    private var dismissed = false
    private var settling = false
    private var settle: Job? = null

    val canPull: Boolean get() = !dismissed && !settling

    /** How much of each pixel dragged the view moves, less the further it is out. */
    private fun resistance(): Float = if (heightPx > 0f) (1f - (distancePx / heightPx).coerceIn(0f, 1f)).coerceAtLeast(0.3f) else 0.8f

    /** Moves the view by a drag of [dy]; returns how much of it was taken. */
    fun dragBy(dy: Float): Float {
        if (!canPull) return 0f
        val resistance = resistance()
        // Pushing back up only takes what returns the view to rest.
        val taken = if (dy < 0f) dy.coerceAtLeast(-distancePx / resistance) else dy
        distancePx = (distancePx + taken * resistance).coerceAtLeast(0f)
        return taken
    }

    fun release(
        velocityY: Float,
        onDismiss: () -> Unit,
    ) {
        if (!canPull || distancePx == 0f) return
        settling = true
        settle =
            scope.launch {
                try {
                    if (distancePx >= dismissDistancePx || velocityY > DISMISS_VELOCITY) {
                        val target = if (heightPx > 0f) heightPx else fallbackHeightPx
                        animate(distancePx, target, velocityY, spring(stiffness = Spring.StiffnessMediumLow)) { value, _ ->
                            distancePx = value
                        }
                        dismissed = true
                        onDismiss()
                    } else {
                        animate(distancePx, 0f, velocityY, spring(stiffness = Spring.StiffnessMedium)) { value, _ ->
                            distancePx = value
                        }
                    }
                } finally {
                    settling = false
                }
            }
    }

    /** Stops a settle in flight, which must not close or move what shows next. */
    fun stop() {
        settle?.cancel()
        settle = null
    }

    /** Puts a view that was pulled away back in place, for when it shows again. */
    fun reset() {
        stop()
        distancePx = 0f
        dismissed = false
    }
}

@Composable
internal fun rememberPullToDismissState(key: Any? = Unit): PullToDismissState {
    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    return remember(key, density) {
        with(density) { PullToDismissState(scope, dismissDistancePx = 120.dp.toPx(), fallbackHeightPx = 800.dp.toPx()) }
    }
}

/** Moves, shrinks and fades the view being pulled; goes on its outermost layout. */
internal fun Modifier.pullToDismissLayer(state: PullToDismissState): Modifier =
    onSizeChanged { state.heightPx = it.height.toFloat() }
        .graphicsLayer {
            translationY = state.distancePx
            val progress = if (state.heightPx > 0f) (state.distancePx / state.heightPx).coerceIn(0f, 1f) else 0f
            alpha = 1f - progress * 0.4f
            scaleX = 1f - progress * 0.05f
            scaleY = 1f - progress * 0.05f
        }

/**
 * Pulls from the scrolling content inside: a drag down that started with it at
 * its top ([isAtTop], read as the finger goes down) moves the view instead of
 * the content. Flings never pull, so momentum carrying the content to its top
 * stops there.
 */
@Composable
internal fun Modifier.pullToDismiss(
    state: PullToDismissState,
    enabled: Boolean = true,
    isAtTop: () -> Boolean,
    onDismiss: () -> Unit,
): Modifier {
    if (!enabled) return this
    val atTop by rememberUpdatedState(isAtTop)
    val dismiss by rememberUpdatedState(onDismiss)
    val connection =
        remember(state) {
            object : NestedScrollConnection {
                override fun onPreScroll(
                    available: Offset,
                    source: NestedScrollSource,
                ): Offset {
                    if (source != NestedScrollSource.UserInput || !state.canPull) return Offset.Zero
                    val pulling = state.distancePx > 0f
                    if (available.y > 0f && (pulling || (state.armed && atTop()))) {
                        return Offset(0f, state.dragBy(available.y))
                    }
                    if (available.y < 0f && pulling) return Offset(0f, state.dragBy(available.y))
                    return Offset.Zero
                }

                override suspend fun onPreFling(available: Velocity): Velocity {
                    if (!state.canPull || state.distancePx == 0f) return Velocity.Zero
                    state.release(available.y, dismiss)
                    return available
                }
            }
        }
    return pointerInput(state) {
        awaitEachGesture {
            awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
            state.armed = atTop()
        }
    }.nestedScroll(connection)
}

/** Pulls from a bar that does not scroll, wherever the content is. */
@Composable
internal fun Modifier.pullToDismissHandle(
    state: PullToDismissState,
    onDismiss: () -> Unit,
): Modifier {
    val dismiss by rememberUpdatedState(onDismiss)
    return pointerInput(state) {
        detectVerticalDragGestures(
            onVerticalDrag = { _, dragAmount -> state.dragBy(dragAmount) },
            onDragEnd = { state.release(0f, dismiss) },
            onDragCancel = { state.release(0f, dismiss) },
        )
    }
}
