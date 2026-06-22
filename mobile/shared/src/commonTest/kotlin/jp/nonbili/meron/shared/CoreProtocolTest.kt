package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class CoreProtocolTest {
    @Test
    fun pingRequestUsesSharedProtocolShape() {
        assertEquals(
            """{"id":7,"method":"ping","params":{}}""",
            pingRequest(id = 7).toJson(),
        )
    }

    @Test
    fun requestJsonEscapesMethod() {
        assertEquals(
            """{"id":1,"method":"mail.threadRead\nnext","params":{"ok":true}}""",
            CoreRequest(1, "mail.threadRead\nnext", """{"ok":true}""").toJson(),
        )
    }

    @Test
    fun protocolVersionMustMatchRustCore() {
        requireProtocolVersion(EXPECTED_PROTOCOL_VERSION)
        assertFailsWith<IllegalArgumentException> {
            requireProtocolVersion(EXPECTED_PROTOCOL_VERSION + 1)
        }
    }
}
