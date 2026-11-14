package jp.nonbili.meron

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import jp.nonbili.meron.ui.APP_ICON_BLUE
import jp.nonbili.meron.ui.APP_ICON_DARK
import jp.nonbili.meron.ui.APP_ICON_DEFAULT
import jp.nonbili.meron.ui.APP_ICON_LIGHT

/**
 * The launcher icon is whichever of the manifest's launcher aliases is
 * enabled: an app cannot repaint its icon, only choose between entries that
 * ship with it. The package manager remembers the choice, so nothing is stored
 * here.
 */
internal object AndroidAppIcon {
    // Alias names live in the manifest's namespace, which is not the
    // application id on builds that add a suffix to it.
    private const val NAMESPACE = "jp.nonbili.meron"

    private val aliases =
        linkedMapOf(
            APP_ICON_DEFAULT to "ComposeMainActivity",
            APP_ICON_BLUE to "LauncherIconBlue",
            APP_ICON_LIGHT to "LauncherIconLight",
            APP_ICON_DARK to "LauncherIconDark",
        )

    val styles: List<String> = aliases.keys.toList()

    private fun component(
        context: Context,
        alias: String,
    ) = ComponentName(context.packageName, "$NAMESPACE.$alias")

    fun current(context: Context): String {
        val packages = context.packageManager
        return aliases.entries
            .firstOrNull { (style, alias) ->
                style != APP_ICON_DEFAULT &&
                    packages.getComponentEnabledSetting(component(context, alias)) == PackageManager.COMPONENT_ENABLED_STATE_ENABLED
            }?.key ?: APP_ICON_DEFAULT
    }

    fun set(
        context: Context,
        style: String,
    ) {
        val chosen = aliases[style] ?: return
        val packages = context.packageManager

        fun apply(
            alias: String,
            state: Int,
        ) = packages.setComponentEnabledSetting(component(context, alias), state, PackageManager.DONT_KILL_APP)
        // Enable before disabling, so the app is never without a launcher entry.
        apply(chosen, PackageManager.COMPONENT_ENABLED_STATE_ENABLED)
        aliases.values.filter { it != chosen }.forEach { apply(it, PackageManager.COMPONENT_ENABLED_STATE_DISABLED) }
    }
}
