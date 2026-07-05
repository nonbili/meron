package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
internal fun ThemePickerDialog(
    current: AppAppearanceMode,
    onSelect: (AppAppearanceMode) -> Unit,
    onDismiss: () -> Unit,
) {
    val selectableModes = AppAppearanceMode.entries.filterNot { it == AppAppearanceMode.System }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("common.theme")) },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 480.dp)) {
                items(selectableModes) { mode ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onSelect(mode)
                                onDismiss()
                            }.padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = mode == current, onClick = {
                            onSelect(mode)
                            onDismiss()
                        })
                        Text(mode.label, modifier = Modifier.padding(start = 8.dp))
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.done")) } },
    )
}

@Composable
internal fun LanguagePickerDialog(
    currentTag: String,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("settings.language.label")) },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 480.dp)) {
                item {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onSelect("")
                                onDismiss()
                            }.padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = currentTag.isBlank(), onClick = {
                            onSelect("")
                            onDismiss()
                        })
                        Text(tr("settings.language.system"), modifier = Modifier.padding(start = 8.dp))
                    }
                }
                items(supportedAppLanguageTags) { tag ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onSelect(tag)
                                onDismiss()
                            }.padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = tag == currentTag, onClick = {
                            onSelect(tag)
                            onDismiss()
                        })
                        Text(appLanguageDisplayName(tag), modifier = Modifier.padding(start = 8.dp))
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.done")) } },
    )
}
