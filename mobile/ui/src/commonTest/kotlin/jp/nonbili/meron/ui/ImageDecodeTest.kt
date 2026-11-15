package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ImageDecodeTest {
    private fun decodedBytes(
        width: Int,
        height: Int,
        maxBytes: Long = MAX_DECODED_IMAGE_BYTES,
        bytesPerPixel: Int = 4,
    ): Long {
        val (w, h) = scaledImageSize(width, height, imageDecodeScale(width, height, maxBytes, bytesPerPixel))
        return w.toLong() * h.toLong() * bytesPerPixel
    }

    @Test
    fun cameraFramesAreDecodedAtFullSize() {
        // 12 MP and 16 MP sensors, well under what a canvas refuses to draw.
        assertEquals(1.0, imageDecodeScale(4032, 3024))
        assertEquals(1.0, imageDecodeScale(4624, 3468))
    }

    @Test
    fun longScreenshotsAndPanoramasKeepTheirResolution() {
        assertEquals(1.0, imageDecodeScale(1080, 9000))
        assertEquals(1.0, imageDecodeScale(12000, 1000))
    }

    @Test
    fun thePictureFromTheCrashReportFitsTheBudget() {
        // 7676x5751 is 176 MB decoded; Android gives up above 100 MB.
        assertTrue(imageDecodeScale(7676, 5751) < 1.0)
        assertTrue(decodedBytes(7676, 5751) <= MAX_DECODED_IMAGE_BYTES)
    }

    @Test
    fun scalingLandsOnTheBudgetRatherThanFarBelowIt() {
        // Just over the budget: a power-of-two step would throw away three
        // quarters of the pixels.
        val bytes = decodedBytes(4200, 4100)
        assertTrue(bytes <= MAX_DECODED_IMAGE_BYTES)
        assertTrue(bytes > MAX_DECODED_IMAGE_BYTES * 95 / 100)
    }

    @Test
    fun halfFloatPixelsCountDouble() {
        // 4000x3000 fits at four bytes a pixel and is 96 MB at eight.
        assertEquals(1.0, imageDecodeScale(4000, 3000))
        assertTrue(imageDecodeScale(4000, 3000, bytesPerPixel = 8) < 1.0)
        assertTrue(decodedBytes(4000, 3000, bytesPerPixel = 8) <= MAX_DECODED_IMAGE_BYTES)
    }

    @Test
    fun aSmallerBudgetShrinksAPictureShownSmall() {
        assertTrue(decodedBytes(3000, 3000, AVATAR_IMAGE_BYTES) <= AVATAR_IMAGE_BYTES)
        assertEquals(1.0, imageDecodeScale(256, 256, AVATAR_IMAGE_BYTES))
    }

    @Test
    fun extremeShapesNeverScaleToNothing() {
        assertEquals(1 to 1, scaledImageSize(1, 100000, 0.000001))
    }

    @Test
    fun unknownDimensionsAreLeftToTheDecoder() {
        assertEquals(1.0, imageDecodeScale(-1, -1))
        assertEquals(1.0, imageDecodeScale(0, 500))
    }
}
