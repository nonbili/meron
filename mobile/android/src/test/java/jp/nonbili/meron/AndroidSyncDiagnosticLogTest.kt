package jp.nonbili.meron

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class AndroidSyncDiagnosticLogTest {
    @Test
    fun batchPreservesEveryEntryWhenTheirCombinedSizeExceedsTheSingleEntryLimit() {
        val file = File.createTempFile("diag", ".log").apply { deleteOnExit() }
        file.writeText("existing entry\n")
        val messages = (1..16).map { "exit $it " + "x".repeat(1000) }
        AndroidSyncDiagnosticLog.appendEntries(file, messages, "timestamp")
        assertEquals(listOf("existing entry") + messages.map { "timestamp $it" }, file.readLines())
    }

    @Test
    fun batchRetainsTheNewestFiveHundredLines() {
        val file = File.createTempFile("diag", ".log").apply { deleteOnExit() }
        file.writeText((1..500).joinToString("\n") { "old $it" } + "\n")
        AndroidSyncDiagnosticLog.appendEntries(file, listOf("new 1", "new 2"), "timestamp")
        val lines = file.readLines()
        assertEquals(500, lines.size)
        assertEquals("old 3", lines.first())
        assertEquals(listOf("timestamp new 1", "timestamp new 2"), lines.takeLast(2))
    }

    @Test
    fun testReadTailOfMissingFileIsEmpty() {
        assertEquals("", AndroidSyncDiagnosticLog.readTail(File("does-not-exist-${System.nanoTime()}.log")))
    }

    @Test
    fun testReadTailKeepsASmallLogWhole() {
        val file = File.createTempFile("diag", ".log").apply { deleteOnExit() }
        file.writeText("a\nb\n")
        assertEquals("a\nb\n", AndroidSyncDiagnosticLog.readTail(file))
    }

    @Test
    fun testReadTailOfALargeLogStartsOnAWholeLine() {
        val file = File.createTempFile("diag", ".log").apply { deleteOnExit() }
        file.writeText("x".repeat(1 shl 20) + "\nnewest\n")
        assertEquals("newest\n", AndroidSyncDiagnosticLog.readTail(file, maxBytes = 1024))
    }

    @Test
    fun testReadTailNeverStartsInsideAUtf8Sequence() {
        // One long line of 3-byte characters: no newline to cut at, so the
        // cut lands mid-character and must skip to the next lead byte.
        val file = File.createTempFile("diag", ".log").apply { deleteOnExit() }
        file.writeText("あ".repeat(1000))
        val tail = AndroidSyncDiagnosticLog.readTail(file, maxBytes = 1000)
        assertTrue(tail.isNotEmpty())
        assertTrue(tail.all { it == 'あ' })
    }
}
