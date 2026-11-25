package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class ConversationToolbarTest {
    @Test
    fun untouchedInstallKeepsTheOldBar() {
        assertEquals(DEFAULT_CONVERSATION_TOOLBAR, loadConversationToolbar(StringPreferences()))
    }

    @Test
    fun keepsTheOrderItWasSavedIn() {
        val prefs = StringPreferences()
        val order =
            listOf(
                ConversationToolbarAction.Delete,
                ConversationToolbarAction.Archive,
                ConversationToolbarAction.Print,
                ConversationToolbarAction.Star,
            )
        saveConversationToolbar(prefs, order)
        assertEquals(order, loadConversationToolbar(prefs))
    }

    @Test
    fun nothingPinnedStaysEmpty() {
        val prefs = StringPreferences()
        saveConversationToolbar(prefs, emptyList())
        assertEquals(emptyList(), loadConversationToolbar(prefs))
    }

    @Test
    fun dropsUnknownAndRepeatedIds() {
        val prefs = StringPreferences()
        prefs.putString(CONVERSATION_TOOLBAR_PREF, "archive,openInNewWindow,archive,star")
        assertEquals(
            listOf(ConversationToolbarAction.Archive, ConversationToolbarAction.Star),
            loadConversationToolbar(prefs),
        )
    }

    private class StringPreferences : AppPreferences {
        private val values = mutableMapOf<String, String>()

        override fun getString(
            key: String,
            default: String,
        ): String = values[key] ?: default

        override fun putString(
            key: String,
            value: String,
        ) {
            values[key] = value
        }

        override fun getBoolean(
            key: String,
            default: Boolean,
        ): Boolean = default

        override fun putBoolean(
            key: String,
            value: Boolean,
        ) {}

        override fun getInt(
            key: String,
            default: Int,
        ): Int = default

        override fun putInt(
            key: String,
            value: Int,
        ) {}

        override fun getStringSet(
            key: String,
            default: Set<String>,
        ): Set<String> = default

        override fun putStringSet(
            key: String,
            value: Set<String>,
        ) {}

        override fun remove(key: String) {
            values.remove(key)
        }
    }
}
