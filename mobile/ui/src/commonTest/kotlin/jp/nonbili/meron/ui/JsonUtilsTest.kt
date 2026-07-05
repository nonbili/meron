package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class JsonUtilsTest {
    @Test
    fun jsonStringValueExtractsBasicKey() {
        val json = """{"name":"Alice","age":30}"""

        assertEquals("Alice", json.jsonStringValue("name"))
    }

    @Test
    fun jsonStringValueReturnsEmptyForMissingKey() {
        val json = """{"name":"Alice"}"""

        assertEquals("", json.jsonStringValue("missing"))
    }

    @Test
    fun jsonStringValueReturnsEmptyForMalformedInput() {
        assertEquals("", "not json at all".jsonStringValue("name"))
        assertEquals("", "".jsonStringValue("name"))
    }

    @Test
    fun jsonStringValueDecodesEscapedQuote() {
        val json = """{"message":"She said \"hi\""}"""

        assertEquals("She said \"hi\"", json.jsonStringValue("message"))
    }

    @Test
    fun jsonStringValueDecodesEscapedBackslash() {
        val json = """{"path":"C:\\Users\\alice"}"""

        assertEquals("C:\\Users\\alice", json.jsonStringValue("path"))
    }

    @Test
    fun jsonStringValueDecodesEscapedNewlineAndTab() {
        val json = """{"text":"line1\nline2\tindented"}"""

        assertEquals("line1\nline2\tindented", json.jsonStringValue("text"))
    }

    @Test
    fun jsonStringValueDecodesEscapedCarriageReturnAndFormFeed() {
        val json = """{"text":"a\rb\fc"}"""

        assertEquals("a\rb\u000cc", json.jsonStringValue("text"))
    }

    @Test
    fun jsonStringValueDecodesEscapedForwardSlashAndBackspace() {
        val json = """{"text":"a\/b\bc"}"""

        assertEquals("a/b\bc", json.jsonStringValue("text"))
    }

    @Test
    fun jsonIntValueExtractsBasicKey() {
        val json = """{"count":42}"""

        assertEquals(42, json.jsonIntValue("count", -1))
    }

    @Test
    fun jsonIntValueExtractsNegativeNumber() {
        val json = """{"offset":-17}"""

        assertEquals(-17, json.jsonIntValue("offset", 0))
    }

    @Test
    fun jsonIntValueReturnsDefaultForMissingKey() {
        val json = """{"count":42}"""

        assertEquals(99, json.jsonIntValue("missing", 99))
    }

    @Test
    fun jsonIntValueReturnsDefaultForMalformedInput() {
        assertEquals(7, "not json at all".jsonIntValue("count", 7))
        assertEquals(7, "".jsonIntValue("count", 7))
    }

    @Test
    fun decodeJsonStringHandlesPlainTextUnchanged() {
        assertEquals("plain text", "plain text".decodeJsonString())
    }

    @Test
    fun decodeJsonStringHandlesAllEscapeSequences() {
        assertEquals("\"", "\\\"".decodeJsonString())
        assertEquals("\\", "\\\\".decodeJsonString())
        assertEquals("/", "\\/".decodeJsonString())
        assertEquals("\b", "\\b".decodeJsonString())
        assertEquals("\u000c", "\\f".decodeJsonString())
        assertEquals("\n", "\\n".decodeJsonString())
        assertEquals("\r", "\\r".decodeJsonString())
        assertEquals("\t", "\\t".decodeJsonString())
    }

    @Test
    fun decodeJsonStringPassesThroughUnknownEscapeCharacter() {
        assertEquals("x", "\\x".decodeJsonString())
    }

    @Test
    fun decodeJsonStringHandlesTrailingBackslashWithoutCrashing() {
        assertEquals("abc\\", "abc\\".decodeJsonString())
    }
}
