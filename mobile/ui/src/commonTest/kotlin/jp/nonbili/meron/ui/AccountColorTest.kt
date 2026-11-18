package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.Color
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class AccountColorTest {
    @Test
    fun parsesStoredHexColors() {
        assertEquals(Color(0xFF3B82F6), accountColorOrNull("#3b82f6"))
        assertEquals(Color(0xFF3B82F6), accountColorOrNull(" #3B82F6 "))
    }

    @Test
    fun unsetOrMalformedColorsDrawNoBar() {
        listOf("", "red", "#12345", "#3b82fg", "#3b82f6ff").forEach { assertNull(accountColorOrNull(it), it) }
    }

    @Test
    fun everyOfferedColorParses() {
        ACCOUNT_COLORS.forEach { (id, hex) -> assertEquals(true, accountColorOrNull(hex) != null, id) }
    }
}
