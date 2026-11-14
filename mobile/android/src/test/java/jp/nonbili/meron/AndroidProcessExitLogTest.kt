package jp.nonbili.meron

import android.app.ApplicationExitInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidProcessExitLogTest {
    private fun exit(
        timestamp: Long,
        reason: Int = ApplicationExitInfo.REASON_CRASH_NATIVE,
    ) = AndroidProcessExit(timestamp, "jp.nonbili.meron", 123, reason, 11, 100, "native fault")

    @Test
    fun historyIsWrittenChronologicallyWithoutRepeatingRecordedExits() {
        val history = listOf(exit(300), exit(200), exit(100))
        val fresh = unrecordedProcessExits(history, recordedKeys = setOf(exit(100).key))
        assertEquals(listOf(200L, 300L), fresh.map { it.timestamp })
        assertTrue(unrecordedProcessExits(history, recordedKeys = history.map { it.key }.toSet()).isEmpty())
    }

    @Test
    fun firstLaunchIncludesAvailableHistory() {
        assertEquals(listOf(exit(100)), unrecordedProcessExits(listOf(exit(100)), recordedKeys = emptySet()))
        assertTrue(unrecordedProcessExits(emptyList(), recordedKeys = emptySet()).isEmpty())
    }

    @Test
    fun clockMovingBackwardsDoesNotHideANewCrash() {
        val previouslyRecorded = exit(300)
        val afterClockCorrection = exit(100).copy(pid = 456)
        assertEquals(
            listOf(afterClockCorrection),
            unrecordedProcessExits(listOf(previouslyRecorded, afterClockCorrection), setOf(previouslyRecorded.key)),
        )
    }

    @Test
    fun exitsAtTheSameTimestampAreDeduplicatedIndependently() {
        val recorded = exit(100)
        val differentPid = recorded.copy(pid = 456)
        val differentProcess = recorded.copy(processName = "jp.nonbili.meron:renderer")
        assertEquals(
            listOf(differentPid, differentProcess),
            unrecordedProcessExits(listOf(recorded, differentPid, differentProcess), setOf(recorded.key)),
        )
    }

    @Test
    fun onlyCrashesAndAnrsOfferACrashReport() {
        for (reason in listOf(ApplicationExitInfo.REASON_CRASH, ApplicationExitInfo.REASON_CRASH_NATIVE, ApplicationExitInfo.REASON_ANR)) {
            assertTrue(exit(100, reason).isCrash)
        }
        for (reason in listOf(ApplicationExitInfo.REASON_USER_REQUESTED, ApplicationExitInfo.REASON_LOW_MEMORY, ApplicationExitInfo.REASON_SIGNALED)) {
            assertFalse(exit(100, reason).isCrash)
        }
    }

    @Test
    fun rendererExitDoesNotHideAnAppCrashOrCreateAnAppCrashReport() {
        val appCrash = exit(100)
        val rendererCrash = exit(200).copy(processName = "jp.nonbili.meron:renderer")
        assertEquals(appCrash, latestAppCrash(listOf(rendererCrash, appCrash), "jp.nonbili.meron"))
        assertNull(latestAppCrash(listOf(rendererCrash), "jp.nonbili.meron"))
    }

    @Test
    fun normalExitAfterAnOldCrashDoesNotOfferAStaleReport() {
        assertNull(
            latestAppCrash(listOf(exit(100), exit(200, ApplicationExitInfo.REASON_USER_REQUESTED)), "jp.nonbili.meron"),
        )
    }

    @Test
    fun summaryIdentifiesNativeSignalAndBoundsAndRedactsDescription() {
        val report = exit(100).copy(description = "mailbox alice@example.com\n" + "x".repeat(2000))
        val summary = redactMessage(report.summary())
        assertTrue(summary.contains("reason=CRASH_NATIVE status=11"))
        assertTrue(summary.contains("a***@example.com"))
        assertFalse(summary.contains("alice@example.com"))
        assertFalse(summary.contains('\n'))
        assertTrue(summary.length < 1200)
    }

    @Test
    fun descriptionIsRedactedBeforeTruncatingAnAddressAtTheLimit() {
        val report = exit(100).copy(description = "x".repeat(990) + " alice@example.com")
        assertFalse(report.summary().contains("alice"))
    }
}
