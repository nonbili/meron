package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class OpenPgpTest {
    @Test
    fun encryptionFlagsDistinguishMissingFromExplicitPlain() {
        val params = SendMailParams("a", "b@example.org", "subject", "body")
        assertFalse(params.toJson().contains("\"plain\""))
        assertTrue(params.copy(encrypt = true).toJson().contains("\"plain\":false"))
        assertTrue(params.copy(encrypt = false).toJson().contains("\"plain\":true"))
        val draft = SaveDraftParams("a", "draft", "b@example.org", "subject", "body", encrypt = true)
        assertTrue(draft.toJson().contains("\"encrypt\":true"))
    }

    @Test
    fun onlyAuthenticatedDecryptedOriginalsCanBeQuoted() {
        val message = MessageBody("m", from = "sender", to = "me", subject = "s", body = "secret")
        assertTrue(message.mayQuoteOriginal())
        for (trust in listOf("known", "verified")) {
            val crypto = MessageCrypto(encrypted = true, signed = true, trust = trust)
            assertTrue(message.copy(crypto = crypto).mayQuoteOriginal())
            assertFalse(message.copy(crypto = crypto.copy(partial = true)).mayQuoteOriginal())
            assertFalse(message.copy(crypto = crypto.copy(problem = "redirected")).mayQuoteOriginal())
            assertFalse(message.copy(crypto = crypto.copy(signed = false)).mayQuoteOriginal())
            assertFalse(message.copy(crypto = crypto.copy(verifiedConflict = true)).mayQuoteOriginal())
            assertFalse(message.copy(crypto = crypto.copy(weakHash = true)).mayQuoteOriginal())
        }
        assertFalse(message.copy(crypto = MessageCrypto(encrypted = true)).mayQuoteOriginal())
    }

    @Test
    fun messageCryptoIsParsedWithoutConfusingNestedProperties() {
        val messages = parseThreadReadResponse("""{"messages":[{"id":"m","from_addr":"a@example.org","to":"me@example.org","subject":"s","body":"b","crypto":{"encrypted":true,"signed":true,"trust":"known","signer":"ABC","problem":null,"key_changed":true}}]}""")
        assertEquals(true, messages.first().crypto?.encrypted)
        assertEquals("known", messages.first().crypto?.trust)
        assertEquals("", messages.first().crypto?.problem)
        assertEquals(true, messages.first().crypto?.keyChanged)
    }
}
