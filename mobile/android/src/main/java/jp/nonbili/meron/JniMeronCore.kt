package jp.nonbili.meron

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.CoreRequest
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.parseCoreEventEnvelope
import jp.nonbili.meron.shared.requireProtocolVersion
import jp.nonbili.meron.shared.toJson

class JniMeronCore : MeronCore {
    private val eventStream = JniCoreEventStream(::ensureLoaded)

    override suspend fun invoke(command: String, payloadJson: String): String {
        ensureLoaded()
        // Log command names and errors only — never payloads/responses, which
        // carry passwords and OAuth tokens.
        android.util.Log.i("MeronCore", "-> $command")
        val response = MeronCoreNative.invokeJson(CoreRequest(1, command, payloadJson).toJson())
        if (response.contains("\"error\"")) {
            android.util.Log.w("MeronCore", "<- $command error: ${response.take(300)}")
        }
        return response
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
        val nativeListener = MeronCoreNative.CoreEventListener { eventJson ->
            listener(parseCoreEventEnvelope(eventJson))
        }
        MeronCoreNative.addCoreEventListener(nativeListener)
        MeronCoreNative.emitReadyEvent()
        return CloseableHandle {
            MeronCoreNative.removeCoreEventListener(nativeListener)
        }
    }
}
