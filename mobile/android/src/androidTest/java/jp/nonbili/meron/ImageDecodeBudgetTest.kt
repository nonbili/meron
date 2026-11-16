package jp.nonbili.meron

import android.graphics.Bitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.test.ext.junit.runners.AndroidJUnit4
import jp.nonbili.meron.ui.decodeImageBitmap
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.ByteArrayOutputStream

/** The real decoder has to honour the options the budget turns into. */
@RunWith(AndroidJUnit4::class)
class ImageDecodeBudgetTest {
    private fun png(
        width: Int,
        height: Int,
    ): ByteArray {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        return ByteArrayOutputStream().also { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
    }

    @Test
    fun anOversizedImageIsDecodedWithinItsBudget() {
        val budget = 300L * 200L * 4L
        val decoded = decodeImageBitmap(png(1200, 800), budget)
        assertNotNull(decoded)
        val bytes = decoded!!.asAndroidBitmap().byteCount.toLong()
        assertTrue("decoded to $bytes bytes, over the budget of $budget", bytes <= budget)
        assertTrue("decoded to $bytes bytes, far below the budget of $budget", bytes > budget * 9 / 10)
        // Scaled, not cropped.
        assertEquals(1.5, decoded.width.toDouble() / decoded.height, 0.02)
    }

    @Test
    fun anImageWithinItsBudgetIsDecodedAtFullSize() {
        val decoded = decodeImageBitmap(png(1200, 800))
        assertEquals(1200, decoded?.width)
        assertEquals(800, decoded?.height)
    }
}
