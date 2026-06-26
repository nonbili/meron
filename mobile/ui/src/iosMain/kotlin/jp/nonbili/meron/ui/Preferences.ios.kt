package jp.nonbili.meron.ui

import platform.Foundation.NSUserDefaults

/** NSUserDefaults-backed [AppPreferences]; keys are prefixed by namespace. */
class IosAppPreferences(
    private val namespace: String,
) : AppPreferences {
    private val defaults = NSUserDefaults.standardUserDefaults

    private fun k(key: String) = "$namespace.$key"

    override fun getString(
        key: String,
        default: String,
    ): String = defaults.stringForKey(k(key)) ?: default

    override fun putString(
        key: String,
        value: String,
    ) = defaults.setObject(value, forKey = k(key))

    override fun getBoolean(
        key: String,
        default: Boolean,
    ): Boolean = if (defaults.objectForKey(k(key)) == null) default else defaults.boolForKey(k(key))

    override fun putBoolean(
        key: String,
        value: Boolean,
    ) = defaults.setBool(value, forKey = k(key))

    override fun getInt(
        key: String,
        default: Int,
    ): Int = if (defaults.objectForKey(k(key)) == null) default else defaults.integerForKey(k(key)).toInt()

    override fun putInt(
        key: String,
        value: Int,
    ) = defaults.setInteger(value.toLong(), forKey = k(key))

    override fun getStringSet(
        key: String,
        default: Set<String>,
    ): Set<String> {
        val array = defaults.arrayForKey(k(key)) ?: return default
        return array.filterIsInstance<String>().toSet()
    }

    override fun putStringSet(
        key: String,
        value: Set<String>,
    ) = defaults.setObject(value.toList(), forKey = k(key))

    override fun remove(key: String) = defaults.removeObjectForKey(k(key))
}
