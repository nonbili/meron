package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class ContactsRouteTest {
    @Test
    fun contactsCanBeRestoredAsLastTopLevelScreen() {
        val prefs = MemoryAppPreferences()
        saveLastTopScreen(prefs, Screen.Contacts)
        assertEquals(Screen.Contacts, loadLastTopScreen(prefs))
    }

    @Test
    fun contactsRouteRoundTrips() {
        assertEquals(AppRoutes.Contacts, Screen.Contacts.route())
        assertEquals(Screen.Contacts, appRouteToScreen(AppRoutes.Contacts))
    }

    @Test
    fun restoredContactsCanExitAfterReturningFromHistory() {
        val prefs = MemoryAppPreferences()
        saveLastTopScreen(prefs, Screen.Contacts)
        val restoredRoot = loadLastTopScreen(prefs)
        // Opening history records Contacts as the thread return destination.
        val threadReturnScreen = restoredRoot
        val directoryAfterThreadBack = appRouteToScreen(restoredRoot.route())!!
        // Detail Back only clears the selection; directory Back must leave root.
        assertEquals(Screen.Mail, rootBackDestination(directoryAfterThreadBack, threadReturnScreen))
    }
}
