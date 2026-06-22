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
        return MeronCoreNative.invokeJson(CoreRequest(1, command, payloadJson).toJson())
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
