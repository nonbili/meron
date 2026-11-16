package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ImageDecodeAndroidTest {
    private val shapes = listOf(7676 to 5751, 4200 to 4100, 1200 to 800, 3001 to 2999, 12345 to 677, 9999 to 9999)
    private val budgets = listOf(MAX_DECODED_IMAGE_BYTES, AVATAR_IMAGE_BYTES, 300L * 200L * 4L)

    /** The worst-case size BitmapFactory produces with these options. */
    private fun decodedBytes(
        width: Int,
        height: Int,
        maxBytes: Long,
    ): Long {
        val options = imageDecodeOptions(width, height, maxBytes)
        if (options.inSampleSize == 0) return width.toLong() * height * 4
        val sample = options.inSampleSize
        val w = densityScaled((width + sample - 1) / sample, options.inTargetDensity)
        val h = densityScaled((height + sample - 1) / sample, options.inTargetDensity)
        return w.toLong() * h * 4
    }

    @Test
    fun anImageWithinBudgetIsDecodedUntouched() {
        val options = imageDecodeOptions(1200, 800, MAX_DECODED_IMAGE_BYTES)
        assertEquals(0, options.inSampleSize)
        assertEquals(0, options.inDensity)
    }

    @Test
    fun theDecodedImageLandsOnTheBudgetWithRoundingAccountedFor() {
        for ((width, height) in shapes) {
            for (budget in budgets) {
                if (width.toLong() * height * 4 <= budget) continue
                val bytes = decodedBytes(width, height, budget)
                assertTrue(bytes <= budget, "${width}x$height into $budget: $bytes bytes, over the budget")
                assertTrue(bytes > budget * 95 / 100, "${width}x$height into $budget: only $bytes bytes")
            }
        }
    }

    @Test
    fun samplingNeverGoesBelowTheTarget() {
        // The sample step is the cheap one; it must leave the density step
        // something to scale down from, never up.
        for ((width, height) in shapes) {
            for (budget in budgets) {
                val options = imageDecodeOptions(width, height, budget)
                if (options.inSampleSize == 0) continue
                assertTrue(options.inTargetDensity <= options.inDensity, "${width}x$height into $budget scaled up")
            }
        }
    }
}
