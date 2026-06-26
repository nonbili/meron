package jp.nonbili.meron.ui

import android.content.Context

/** SharedPreferences-backed [AppPreferences]; one instance per namespace. */
class AndroidAppPreferences(
    context: Context,
    name: String,
) : AppPreferences {
    private val prefs = context.getSharedPreferences(name, Context.MODE_PRIVATE)

    override fun getString(
        key: String,
        default: String,
    ): String = prefs.getString(key, default) ?: default

    override fun putString(
        key: String,
        value: String,
    ) = prefs.edit().putString(key, value).apply()

    override fun getBoolean(
        key: String,
        default: Boolean,
    ): Boolean = prefs.getBoolean(key, default)

    override fun putBoolean(
        key: String,
        value: Boolean,
    ) = prefs.edit().putBoolean(key, value).apply()

    override fun getInt(
        key: String,
        default: Int,
    ): Int = prefs.getInt(key, default)

    override fun putInt(
        key: String,
        value: Int,
    ) = prefs.edit().putInt(key, value).apply()

    override fun getStringSet(
        key: String,
        default: Set<String>,
    ): Set<String> = prefs.getStringSet(key, default) ?: default

    override fun putStringSet(
        key: String,
        value: Set<String>,
    ) = prefs.edit().putStringSet(key, value).apply()

    override fun remove(key: String) = prefs.edit().remove(key).apply()
}
