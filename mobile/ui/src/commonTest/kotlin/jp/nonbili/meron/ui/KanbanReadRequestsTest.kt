package jp.nonbili.meron.ui

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KanbanReadRequestsTest {
    @Test
    fun overlappingColumnsShareWrites() =
        runBlocking {
            val requests = KanbanReadRequests()
            var calls = 0
            repeat(2) {
                assertEquals(
                    "{}",
                    requests.run("folder:unified:inbox") {
                        calls++
                        "{}"
                    },
                )
            }
            requests.run("folder:a:archive") {
                calls++
                "{}"
            }
            assertEquals(2, calls)
        }

    @Test
    fun partialFailureIsReportedAndNotRetriedForOverlappingColumns() =
        runBlocking {
            val requests = KanbanReadRequests()
            var calls = 0
            repeat(2) {
                assertFailsWith<IllegalStateException> {
                    requests.run("folder:unified:inbox") {
                        calls++
                        """{"ok":false,"failures":[{"account_id":"a","message":"Offline"}]}"""
                    }
                }
            }
            assertEquals(1, calls)
            assertEquals("{}", requests.run("folder:b:archive") { "{}" })
        }

    @Test
    fun cancellationDoesNotBecomeACachedFailure() =
        runBlocking {
            val requests = KanbanReadRequests()
            assertFailsWith<CancellationException> {
                requests.run("folder:a:inbox") { throw CancellationException() }
            }
            assertEquals("{}", requests.run("folder:a:inbox") { "{}" })
        }
}
