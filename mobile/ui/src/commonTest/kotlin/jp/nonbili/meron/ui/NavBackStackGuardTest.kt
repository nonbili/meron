package jp.nonbili.meron.ui

import androidx.lifecycle.Lifecycle
import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class NavBackStackGuardTest {
    @Test
    fun testAllowsMutationWithoutAnyEntry() {
        assertTrue(canMutateBackStack(null))
    }

    @Test
    fun testAllowsMutationWhenEntryResumed() {
        assertTrue(canMutateBackStack(Lifecycle.State.RESUMED))
    }

    @Test
    fun testBlocksMutationDuringTransition() {
        assertFalse(canMutateBackStack(Lifecycle.State.STARTED))
        assertFalse(canMutateBackStack(Lifecycle.State.CREATED))
    }

    @Test
    fun testBlocksMutationOnDestroyedEntry() {
        assertFalse(canMutateBackStack(Lifecycle.State.DESTROYED))
        assertFalse(canMutateBackStack(Lifecycle.State.INITIALIZED))
    }
}
