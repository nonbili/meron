package jp.nonbili.meron.shared

interface Localizer {
    fun text(
        key: String,
        args: Map<String, Any?> = emptyMap(),
    ): String
}

class MapLocalizer(
    private val messages: Map<String, String>,
    private val fallback: Localizer = MissingKeyLocalizer,
) : Localizer {
    override fun text(
        key: String,
        args: Map<String, Any?>,
    ): String {
        val template = messages[key] ?: return fallback.text(key, args)
        return args.entries.fold(template) { out, (name, value) ->
            out.replace("{$name}", value?.toString().orEmpty())
        }
    }
}

object MissingKeyLocalizer : Localizer {
    override fun text(
        key: String,
        args: Map<String, Any?>,
    ): String = key
}
