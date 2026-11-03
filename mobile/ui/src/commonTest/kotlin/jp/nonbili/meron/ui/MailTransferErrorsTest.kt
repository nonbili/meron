package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MailTransferErrorsTest {
    private fun explain(
        detail: String?,
        fallback: String = "Copy failed",
    ): String =
        mailTransferError(detail?.let { RuntimeException(it) }, fallback) { key, fields ->
            localizedString("en", key, fields)
        }

    @Test
    fun sizeRejectionIncludesConfirmedProgressAndDuplicateWarning() {
        val result =
            explain(
                "Copy progress: 1/3 confirmed; message size: 12000000 bytes: IMAP APPEND: " +
                    "[LIMIT] Mail size is larger than the maximum size allowed by server configuration",
            )
        assertTrue(result.contains("destination server rejected a message"))
        assertTrue(result.contains("12000000 bytes"))
        assertTrue(result.contains("1 of 3 messages confirmed copied"))
        assertTrue(result.contains("retrying may create duplicates"))
        assertFalse(result.contains("APPEND"))
        assertFalse(result.contains("{bytes"))
    }

    @Test
    fun knownRejectionsExplainTheNextAction() {
        mapOf(
            "[TOOBIG] message rejected" to "size limit",
            "[OVERQUOTA] quota exceeded" to "mailbox is full",
            "[TRYCREATE] missing mailbox" to "folder no longer exists",
            "[NONEXISTENT] missing" to "folder no longer exists",
            "[NOPERM] denied" to "denied access",
            "[AUTHENTICATIONFAILED] bad credentials" to "Reconnect",
            "account needs reconnect: acc" to "Reconnect",
        ).forEach { (detail, expected) ->
            assertTrue(explain(detail).contains(expected), detail)
        }
    }

    @Test
    fun interruptedWritesWarnToCheckDestination() {
        listOf("operation timeout", "UID MOVE: connection lost").forEach { detail ->
            val result = explain(detail, "Move failed")
            assertTrue(result.startsWith("Move failed:"))
            assertTrue(result.contains("Check the destination before retrying"))
        }
    }

    @Test
    fun zeroConfirmedCopiesDoesNotInventProgressOrLimit() {
        val result = explain("Copy progress: 0/2 confirmed; message size: 123 bytes: [TOOBIG]")
        assertTrue(result.contains("0 of 2 messages confirmed copied"))
        assertTrue(result.contains("123 bytes"))
        assertFalse(result.contains("duplicates"))
        assertFalse(result.contains("10M"))
    }

    @Test
    fun sourceRemovalFailureExplainsThatMessagesWereCopied() {
        val result =
            explain(
                "Move copied messages but could not confirm removal from the source folder: UID STORE: [NOPERM] permission denied",
                "Move failed",
            )
        assertTrue(result.contains("denied access"))
        assertTrue(result.contains("Messages were copied to the destination"))
        assertTrue(result.contains("removal from the source folder could not be confirmed"))
        assertTrue(result.contains("retrying may create duplicates"))
    }

    @Test
    fun unfamiliarDiagnosticsAndMissingDetailsArePreserved() {
        assertEquals("server policy: custom restriction", explain("server policy: custom restriction"))
        assertEquals("Copy failed", explain(null))
        assertEquals("Copy failed", explain(""))
    }

    @Test
    fun numberPlaceholdersRenderInNonEnglishCatalogsToo() {
        assertEquals(
            "3 件中 1 件のメッセージのコピーを確認しました。",
            localizedString("ja", "mail.transferError.progress", mapOf("copied" to 1, "total" to 3)),
        )
    }
}
