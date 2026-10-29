package jp.nonbili.meron

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.CoreRequest
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileCommand
import jp.nonbili.meron.shared.coreErrorMessage
import jp.nonbili.meron.shared.parseCoreEventEnvelope
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.requireProtocolVersion
import jp.nonbili.meron.shared.toJson

/** Commands after which a message may no longer be unread, or no longer there:
 *  a sync that brought in another device's changes, or the app's own read,
 *  move and delete writes. */
private val mailChangingCommands =
    setOf(
        MobileCommand.Sync,
        MobileCommand.MarkRead,
        MobileCommand.MarkAllRead,
        MobileCommand.Archive,
        MobileCommand.Delete,
        MobileCommand.Move,
        MobileCommand.EmptyFolder,
        MobileCommand.FolderDelete,
        MobileCommand.AccountRemove,
        MobileCommand.RssSync,
        MobileCommand.RssMarkRead,
        MobileCommand.FeedRemove,
    )

class JniMeronCore(
    /** Called after a successful [mailChangingCommands] command. */
    private val onMailChanged: () -> Unit = {},
) : MeronCore {
    private val eventStream = JniCoreEventStream(::ensureLoaded)

    override suspend fun invoke(
        command: String,
        payloadJson: String,
    ): String {
        ensureLoaded()
        // Log command names and errors only — never payloads/responses, which
        // carry passwords and OAuth tokens.
        val logLookup = command != "avatar.resolve"
        if (logLookup) android.util.Log.i("MeronCore", "-> $command")
        val startedAt = System.currentTimeMillis()
        val response = MeronCoreNative.invokeJson(CoreRequest(1, command, payloadJson).toJson())
        val error = coreErrorMessage(response)
        if (error != null) {
            android.util.Log.w("MeronCore", "<- $command error after ${System.currentTimeMillis() - startedAt}ms: $error")
        } else {
            if (logLookup) android.util.Log.i("MeronCore", "<- $command ok after ${System.currentTimeMillis() - startedAt}ms")
            if (command in mailChangingCommands) onMailChanged()
        }
        return requireCoreOk(response)
    }

    override fun events(): CoreEventStream = eventStream

    override suspend fun protocolVersion(): Int {
        ensureLoaded()
        return MeronCoreNative.protocolVersion().also(::requireProtocolVersion)
    }

    fun isLoaded(): Boolean = MeronCoreNative.isLoaded()

    fun loadError(): String = MeronCoreNative.loadError()

    private fun ensureLoaded() {
        check(MeronCoreNative.isLoaded()) {
            val error = MeronCoreNative.loadError()
            if (error.isBlank()) "meron_core is not loaded" else error
        }
    }
}

private class JniCoreEventStream(
    private val ensureLoaded: () -> Unit,
) : CoreEventStream {
    override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle {
        ensureLoaded()
        val nativeListener =
            MeronCoreNative.CoreEventListener { eventJson ->
                listener(parseCoreEventEnvelope(eventJson))
            }
        MeronCoreNative.addCoreEventListener(nativeListener)
        MeronCoreNative.emitReadyEvent()
        return CloseableHandle {
            MeronCoreNative.removeCoreEventListener(nativeListener)
        }
    }
}
