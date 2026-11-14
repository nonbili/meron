package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ImageDecodeAndroidTest {
    @Test
    fun ordinaryImagesAreDecodedAtFullSize() {
        assertEquals(1, imageSampleSize(1920, 1080))
        assertEquals(1, imageSampleSize(4000, 3000))
    }

    @Test
    fun oversizedImagesAreShrunkBelowWhatACanvasRefusesToDraw() {
        // The picture from the crash report: 44 megapixels, 176 MB decoded.
        val sample = imageSampleSize(7676, 5751)
        assertEquals(2, sample)
        assertTrue((7676L / sample) * (5751L / sample) * 4 < 100L * 1024 * 1024)
        assertEquals(4, imageSampleSize(16000, 12000))
    }

    @Test
    fun longThinImagesAreBoundedBySideEvenWhenSmallInArea() {
        assertEquals(4, imageSampleSize(600, 30000))
        assertEquals(4, imageSampleSize(30000, 600))
    }

    @Test
    fun unknownDimensionsAreLeftToTheDecoder() {
        assertEquals(1, imageSampleSize(-1, -1))
        assertEquals(1, imageSampleSize(0, 500))
    }
}
