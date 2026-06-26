package jp.nonbili.meron.ui

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.add
import kotlinx.serialization.json.addJsonObject
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

private val kanbanJson = Json { ignoreUnknownKeys = true }

private fun String.orFallback(fallback: String) = ifBlank { fallback }

@OptIn(ExperimentalUuidApi::class)
internal fun parseKanbanBoards(raw: String): List<KanbanBoardSpec> {
    if (raw.isBlank()) return emptyList()
    return runCatching {
        val array = kanbanJson.parseToJsonElement(raw) as? JsonArray ?: return emptyList()
        array.mapNotNull { element ->
            val obj = element.jsonObject
            val id = (obj["id"]?.asStringOrEmpty()).orEmpty().orFallback("kb-${Uuid.random()}")
            val name = (obj["name"]?.asStringOrEmpty()).orEmpty().orFallback("Kanban board")
            val avatarUrl = obj["avatarUrl"]?.asStringOrEmpty().orEmpty()
            val wallpaper = obj["wallpaper"]?.let { runCatching { it.jsonObject }.getOrNull() }
            val wallpaperPresetId = wallpaper?.get("presetId")?.asStringOrEmpty().orEmpty()
            val wallpaperUrl = wallpaper?.get("url")?.asStringOrEmpty().orEmpty()
            val columns =
                (obj["columns"]?.let { runCatching { it.jsonArray }.getOrNull() } ?: JsonArray(emptyList()))
                    .mapNotNull { col ->
                        val colObj = runCatching { col.jsonObject }.getOrNull() ?: return@mapNotNull null
                        val accountId = colObj["accountId"]?.asStringOrEmpty().orEmpty()
                        val folderId = colObj["folderId"]?.asStringOrEmpty().orEmpty()
                        if (accountId.isNotBlank() && folderId.isNotBlank()) KanbanColumnSpec(accountId, folderId) else null
                    }.distinctBy(::kanbanColumnKey)
            KanbanBoardSpec(id, name, columns, avatarUrl, wallpaperPresetId, wallpaperUrl)
        }
    }.getOrDefault(emptyList())
}

private fun kotlinx.serialization.json.JsonElement.asStringOrEmpty(): String =
    runCatching { (this as kotlinx.serialization.json.JsonPrimitive).content }.getOrDefault("")

internal fun serializeKanbanBoards(boards: List<KanbanBoardSpec>): String =
    buildJsonArray {
        boards.forEach { board ->
            addJsonObject {
                put("id", board.id)
                put("name", board.name)
                put(
                    "columns",
                    buildJsonArray {
                        board.columns.forEach { column ->
                            addJsonObject {
                                put("accountId", column.accountId)
                                put("folderId", column.folderId)
                            }
                        }
                    },
                )
                if (board.avatarUrl.isNotBlank()) put("avatarUrl", board.avatarUrl)
                when {
                    board.wallpaperUrl.isNotBlank() ->
                        put(
                            "wallpaper",
                            buildJsonObject {
                                put("kind", "custom")
                                put("url", board.wallpaperUrl)
                            },
                        )

                    board.wallpaperPresetId.isNotBlank() ->
                        put(
                            "wallpaper",
                            buildJsonObject {
                                put("kind", "preset")
                                put("presetId", board.wallpaperPresetId)
                            },
                        )
                }
            }
        }
    }.toString()

internal fun ensureKanbanDefaults(
    prefs: AppPreferences,
    boards: List<KanbanBoardSpec>,
    accounts: List<jp.nonbili.meron.shared.AccountSummary>,
): List<KanbanBoardSpec> {
    val next =
        if (boards.isEmpty()) {
            listOf(defaultKanbanBoard(accounts))
        } else {
            boards.mapIndexed { index, board ->
                if (index != 0) {
                    board
                } else {
                    val existing = board.columns.map(::kanbanColumnKey).toMutableSet()
                    val columns = board.columns.toMutableList()
                    val unified = KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                    if (existing.add(kanbanColumnKey(unified))) columns.add(0, unified)
                    accounts.forEach { account ->
                        val column = KanbanColumnSpec(account.id, INBOX_FOLDER)
                        if (existing.add(kanbanColumnKey(column))) columns.add(column)
                    }
                    board.copy(columns = columns)
                }
            }
        }
    if (next != boards) saveKanbanBoards(prefs, next)
    return next
}

internal fun loadKanbanBoards(
    prefs: AppPreferences,
    accounts: List<jp.nonbili.meron.shared.AccountSummary>,
): List<KanbanBoardSpec> = ensureKanbanDefaults(prefs, parseKanbanBoards(prefs.getString(KANBAN_BOARDS_PREF, "")), accounts)

internal fun saveKanbanBoards(
    prefs: AppPreferences,
    boards: List<KanbanBoardSpec>,
) = prefs.putString(KANBAN_BOARDS_PREF, serializeKanbanBoards(boards))

internal fun loadActiveKanbanBoardId(prefs: AppPreferences): String = prefs.getString(ACTIVE_KANBAN_BOARD_PREF, "")

internal fun saveActiveKanbanBoardId(
    prefs: AppPreferences,
    boardId: String,
) = prefs.putString(ACTIVE_KANBAN_BOARD_PREF, boardId)

internal fun loadKanbanFilter(prefs: AppPreferences): FilterMode =
    when (prefs.getString(KANBAN_FILTER_PREF, "all")) {
        "unread" -> FilterMode.Unread
        "starred" -> FilterMode.Starred
        else -> FilterMode.All
    }

internal fun saveKanbanFilter(
    prefs: AppPreferences,
    filter: FilterMode,
) = prefs.putString(
    KANBAN_FILTER_PREF,
    when (filter) {
        FilterMode.All -> "all"
        FilterMode.Unread -> "unread"
        FilterMode.Starred -> "starred"
    },
)

internal fun loadKanbanSearch(prefs: AppPreferences): String = prefs.getString(KANBAN_SEARCH_PREF, "")

internal fun saveKanbanSearch(
    prefs: AppPreferences,
    search: String,
) = prefs.putString(KANBAN_SEARCH_PREF, search)

internal fun loadKanbanSearchScope(prefs: AppPreferences): String = prefs.getString(KANBAN_SEARCH_SCOPE_PREF, "all").ifBlank { "all" }

internal fun saveKanbanSearchScope(
    prefs: AppPreferences,
    scope: String,
) = prefs.putString(KANBAN_SEARCH_SCOPE_PREF, scope.ifBlank { "all" })
