package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals

class CoreEventStreamTest {
    @Test
    fun readyEventEnvelopeParsesToSharedCoreEvent() {
        val event =
            parseCoreEventEnvelope(
                """{"event":"ready","detail":{"version":"0.1.0","protocol":1}}""",
            )

        assertEquals("ready", event.name)
        assertEquals("""{"version":"0.1.0","protocol":1}""", event.detailJson)
    }

    @Test
    fun eventEnvelopePreservesNestedDetailJson() {
        val event =
            parseCoreEventEnvelope(
                """{"detail":{"account":"a1","changes":[{"folder":"INBOX","unread":2}]},"event":"mail.changed"}""",
            )

        assertEquals("mail.changed", event.name)
        assertEquals("""{"account":"a1","changes":[{"folder":"INBOX","unread":2}]}""", event.detailJson)
    }

    @Test
    fun eventEnvelopeDecodesEscapedEventName() {
        val event =
            parseCoreEventEnvelope(
                """{"event":"mail.changed\nnext","detail":{}}""",
            )

        assertEquals("mail.changed\nnext", event.name)
        assertEquals("{}", event.detailJson)
    }

    @Test
    fun subscribingReceivesPublishedEventsUntilClosed() {
        val stream = RecordingCoreEventStream()
        val received = mutableListOf<CoreEvent>()
        val handle = stream.subscribe { received += it }

        stream.publish("""{"event":"ready","detail":{"protocol":1}}""")
        handle.close()
        stream.publish("""{"event":"mail.changed","detail":{"account":"a1"}}""")

        assertEquals(
            listOf(CoreEvent("ready", """{"protocol":1}""")),
            received,
        )
    }
}

private class RecordingCoreEventStream : CoreEventStream {
    private val listeners = mutableListOf<(CoreEvent) -> Unit>()

    override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle {
        listeners += listener
        return CloseableHandle { listeners -= listener }
    }

    fun publish(eventJson: String) {
        val event = parseCoreEventEnvelope(eventJson)
        listeners.toList().forEach { it(event) }
    }
}
