package jp.nonbili.meron.ui

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import kotlin.test.Test
import kotlin.test.assertEquals

class SenderImageTest {
    @Test
    fun requestSizeRoundsUpToSharedPowerOfTwo() {
        assertEquals(64, senderImageRequestPx(0))
        assertEquals(64, senderImageRequestPx(38))
        assertEquals(64, senderImageRequestPx(64))
        assertEquals(128, senderImageRequestPx(65))
        // 38dp and 40dp avatars at 3x share one picture.
        assertEquals(128, senderImageRequestPx(114))
        assertEquals(128, senderImageRequestPx(120))
        assertEquals(128, senderImageRequestPx(128))
        assertEquals(256, senderImageRequestPx(144))
        // Core clamps anything above what it serves.
        assertEquals(1024, senderImageRequestPx(600))
    }

    @Test
    fun imageAddressKeepsTheBareAddressWhole() {
        assertEquals("billing=receipts@example.com", senderImageAddress("billing=receipts@example.com", "Billing"))
        assertEquals("juno@example.com", senderImageAddress(" juno@example.com ", "Juno Vale"))
        // Without an address, fall back to one in the label.
        assertEquals("juno@example.com", senderImageAddress("", "Juno <juno@example.com>"))
        assertEquals(null, senderImageAddress("", "Juno Vale"))
    }

    @Test
    fun duplicateLoadsShareOneRun() =
        runBlocking {
            val loads = InFlightLoads<String>()
            val gate = CompletableDeferred<Unit>()
            var runs = 0
            val first =
                async {
                    loads.load("k") {
                        runs++
                        gate.await()
                        "image"
                    }
                }
            yield()
            val second =
                async {
                    loads.load("k") {
                        runs++
                        "other"
                    }
                }
            yield()
            gate.complete(Unit)
            assertEquals("image", first.await())
            assertEquals("image", second.await())
            assertEquals(1, runs)
        }

    @Test
    fun waiterLoadsItselfWhenTheOwnerIsCancelled() =
        runBlocking {
            val loads = InFlightLoads<String>()
            val first =
                async {
                    loads.load("k") {
                        CompletableDeferred<Unit>().await()
                        "never"
                    }
                }
            yield()
            val second = async { loads.load("k") { "image" } }
            yield()
            first.cancel()
            assertEquals("image", second.await())
        }
}
