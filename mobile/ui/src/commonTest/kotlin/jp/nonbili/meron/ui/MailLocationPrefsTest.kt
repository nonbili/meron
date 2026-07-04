package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class MailLocationPrefsTest {
    @Test
    fun defaultsToUnifiedInboxWhenNoMailboxWasSaved() {
        val prefs = FakePreferences()

        assertEquals(UNIFIED_ACCOUNT_ID, loadLastMailAccountId(prefs))
        assertEquals(INBOX_FOLDER, loadLastMailFolder(prefs))
    }

    @Test
    fun savesAndRestoresAccountMailbox() {
        val prefs = FakePreferences()

        saveLastMailLocation(prefs, "acct-1", "Archive")

        assertEquals("acct-1", loadLastMailAccountId(prefs))
        assertEquals("Archive", loadLastMailFolder(prefs))
    }

    @Test
    fun blankSavedValuesFallBackToUnifiedInbox() {
        val prefs = FakePreferences()

        saveLastMailLocation(prefs, "", "")

        assertEquals(UNIFIED_ACCOUNT_ID, loadLastMailAccountId(prefs))
        assertEquals(INBOX_FOLDER, loadLastMailFolder(prefs))
    }

    private class FakePreferences : AppPreferences {
        private val strings = mutableMapOf<String, String>()
        private val booleans = mutableMapOf<String, Boolean>()
        private val ints = mutableMapOf<String, Int>()
        private val stringSets = mutableMapOf<String, Set<String>>()

        override fun getString(
            key: String,
            default: String,
        ): String = strings[key] ?: default

        override fun putString(
            key: String,
            value: String,
        ) {
            strings[key] = value
        }

        override fun getBoolean(
            key: String,
            default: Boolean,
        ): Boolean = booleans[key] ?: default

        override fun putBoolean(
            key: String,
            value: Boolean,
        ) {
            booleans[key] = value
        }

        override fun getInt(
            key: String,
            default: Int,
        ): Int = ints[key] ?: default

        override fun putInt(
            key: String,
            value: Int,
        ) {
            ints[key] = value
        }

        override fun getStringSet(
            key: String,
            default: Set<String>,
        ): Set<String> = stringSets[key] ?: default

        override fun putStringSet(
            key: String,
            value: Set<String>,
        ) {
            stringSets[key] = value
        }

        override fun remove(key: String) {
            strings.remove(key)
            booleans.remove(key)
            ints.remove(key)
            stringSets.remove(key)
        }
    }
}
