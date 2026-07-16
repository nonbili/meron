package jp.nonbili.meron.ui

import androidx.compose.ui.unit.dp
import kotlin.test.Test
import kotlin.test.assertEquals

class MailBodyHeightTest {
    @Test
    fun testKeepsOrdinaryHeight() {
        assertEquals(1234.dp, clampMailBodyHeight(1234.dp))
    }

    @Test
    fun testClampsNegativeReportToZero() {
        assertEquals(0.dp, clampMailBodyHeight((-50).dp))
    }

    @Test
    fun testClampsGiantReportToMax() {
        assertEquals(MailBodyMaxReportedHeight, clampMailBodyHeight(1_000_000.dp))
    }

    @Test
    fun testKeepsMaxItself() {
        assertEquals(MailBodyMaxReportedHeight, clampMailBodyHeight(MailBodyMaxReportedHeight))
    }
}
