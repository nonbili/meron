package jp.nonbili.meron.ui

import kotlinx.coroutines.CancellationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

// One board action executes sequentially and shares both successful and failed
// writes across overlapping columns, so a failed target is not retried implicitly.
internal class KanbanReadRequests {
    private val results = mutableMapOf<String, Result<String>>()

    suspend fun run(
        key: String,
        write: suspend () -> String,
    ): String {
        val result =
            results[key] ?: runCatching {
                val response = write()
                val value = Json.parseToJsonElement(response) as? JsonObject
                check(value?.get("ok") != JsonPrimitive(false) && (value?.get("failures") as? JsonArray).isNullOrEmpty()) {
                    "Mark read failed: $response"
                }
                response
            }.also {
                val error = it.exceptionOrNull()
                if (error is CancellationException) throw error
                results[key] = it
            }
        return result.getOrThrow()
    }
}
