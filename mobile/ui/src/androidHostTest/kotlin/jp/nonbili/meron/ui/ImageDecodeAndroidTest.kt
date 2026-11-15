package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ImageDecodeAndroidTest {
    /** The fraction of each side the decoder keeps with these options. */
    private fun applied(scale: Double): Double {
        val options = imageDecodeOptions(scale)
        val sample = maxOf(1, options.inSampleSize)
        val density = if (options.inDensity > 0) options.inTargetDensity.toDouble() / options.inDensity else 1.0
        return density / sample
    }

    @Test
    fun anImageWithinBudgetIsDecodedUntouched() {
        val options = imageDecodeOptions(1.0)
        assertEquals(0, options.inSampleSize)
        assertEquals(0, options.inDensity)
    }

    @Test
    fun theDecoderIsToldTheScaleThatWasAskedFor() {
        for (scale in listOf(0.97, 0.61, 0.5, 0.33, 0.25, 0.1, 0.013)) {
            val got = applied(scale)
            assertTrue(got <= scale, "scale $scale: decoder would keep $got, over the budget")
            assertTrue(got > scale * 0.99, "scale $scale: decoder would keep only $got")
        }
    }

    @Test
    fun samplingNeverGoesBelowTheTarget() {
        // The sample step is the cheap one; it must leave the density step
        // something to scale down from, never up.
        for (scale in listOf(0.97, 0.61, 0.5, 0.33, 0.25, 0.1)) {
            val sample = imageDecodeOptions(scale).inSampleSize
            assertTrue(1.0 / sample >= scale, "scale $scale sampled by $sample")
        }
    }
}
