package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/** Keep section spacing outside connected groups, including conditional rows. */
internal class SettingsListScope {
    internal class Entry(
        val key: Any?,
        val grouped: Boolean,
        val content: @Composable () -> Unit,
    )

    internal val entries = mutableListOf<Entry>()

    fun item(
        key: Any? = null,
        content: @Composable () -> Unit,
    ) {
        entries += Entry(key, true, content)
    }

    fun <T> items(
        values: List<T>,
        key: (T) -> Any,
        content: @Composable (T) -> Unit,
    ) {
        values.forEach { value -> item(key(value)) { content(value) } }
    }

    fun <T> itemsIndexed(
        values: List<T>,
        content: @Composable (Int, T) -> Unit,
    ) {
        values.forEachIndexed { index, value -> item { content(index, value) } }
    }

    /** Free-standing content between groups, such as a page's introduction. */
    fun plain(content: @Composable () -> Unit) {
        entries += Entry(null, false, content)
    }

    fun section(title: String) {
        val first = entries.isEmpty()
        entries += Entry(null, false) { SettingsSectionLabel(title, first = first) }
    }
}

@Composable
internal fun SettingsList(
    modifier: Modifier,
    state: LazyListState,
    content: @Composable SettingsListScope.() -> Unit,
) {
    val scope = SettingsListScope()
    scope.content()
    val entries = scope.entries
    LazyColumn(
        modifier,
        state = state,
        contentPadding = PaddingValues(top = if (entries.firstOrNull()?.grouped == true) 8.dp else 0.dp, bottom = 24.dp + navigationBarInset()),
    ) {
        entries.forEachIndexed { index, entry ->
            item(key = entry.key) {
                if (entry.grouped) {
                    val first = index == 0 || !entries[index - 1].grouped
                    val last = index == entries.lastIndex || !entries[index + 1].grouped
                    Surface(
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp).padding(top = if (first) 0.dp else 2.dp),
                        shape =
                            RoundedCornerShape(
                                topStart = if (first) 16.dp else 4.dp,
                                topEnd = if (first) 16.dp else 4.dp,
                                bottomStart = if (last) 16.dp else 4.dp,
                                bottomEnd = if (last) 16.dp else 4.dp,
                            ),
                        color = MaterialTheme.colorScheme.surface,
                    ) {
                        entry.content()
                    }
                } else {
                    entry.content()
                }
            }
        }
    }
}
