package jp.nonbili.meron.ui

import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow

/** Bridges the platform "app returned to foreground" lifecycle event into the
 *  shared composition. The iOS host calls [signal] from
 *  `willEnterForegroundNotification`; the app collects [events] to refresh mail
 *  immediately instead of waiting for the next poll tick. Replays nothing and
 *  drops on overflow, so a missed collector never queues a stale refresh. */
object AppForegroundSignal {
    private val _events =
        MutableSharedFlow<Unit>(
            replay = 0,
            extraBufferCapacity = 1,
            onBufferOverflow = BufferOverflow.DROP_OLDEST,
        )

    val events: SharedFlow<Unit> = _events

    fun signal() {
        _events.tryEmit(Unit)
    }
}

/** Bridges the platform "notification permission dialog answered" event into the
 *  shared composition so permission-gated UI (Settings row, mail-screen banner)
 *  updates as soon as the user responds instead of on the next app restart. */
object NotificationPermissionSignal {
    private val _events =
        MutableSharedFlow<Unit>(
            replay = 0,
            extraBufferCapacity = 1,
            onBufferOverflow = BufferOverflow.DROP_OLDEST,
        )

    val events: SharedFlow<Unit> = _events

    fun signal() {
        _events.tryEmit(Unit)
    }
}
