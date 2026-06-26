package jp.nonbili.meron.ui

/** Key-value persistence, one instance per namespace (app vs kanban). Backed by
 *  SharedPreferences on Android and NSUserDefaults on iOS. */
interface AppPreferences {
    fun getString(
        key: String,
        default: String,
    ): String

    fun putString(
        key: String,
        value: String,
    )

    fun getBoolean(
        key: String,
        default: Boolean,
    ): Boolean

    fun putBoolean(
        key: String,
        value: Boolean,
    )

    fun getInt(
        key: String,
        default: Int,
    ): Int

    fun putInt(
        key: String,
        value: Int,
    )

    fun getStringSet(
        key: String,
        default: Set<String>,
    ): Set<String>

    fun putStringSet(
        key: String,
        value: Set<String>,
    )

    fun remove(key: String)
}
