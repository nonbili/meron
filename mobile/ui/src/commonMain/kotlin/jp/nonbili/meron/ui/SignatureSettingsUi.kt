package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Draw
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.SignatureSpec
import jp.nonbili.meron.shared.plainTextToHtml
import jp.nonbili.meron.shared.signatureIsBlank
import jp.nonbili.meron.shared.signaturePlainText

// Signatures are stored as HTML so the desktop composer can send rich ones.
// This screen edits them as plain text, and only rewrites the stored HTML when
// the text actually changed — so opening the editor on a signature written with
// formatting on desktop and closing it again leaves that formatting intact.

private val accountSignatureModes = listOf("global", "none", "custom")

@Composable
private fun signatureModeLabel(mode: String): String =
    when (mode) {
        "none" -> tr("settings.signature.modeNone")
        "custom" -> tr("settings.signature.modeCustom")
        else -> tr("settings.signature.modeGlobal")
    }

/** First line of the signature, as the row's subtitle. */
private fun signaturePreview(html: String): String =
    signaturePlainText(html)
        .lineSequence()
        .firstOrNull()
        ?.trim()
        .orEmpty()

/** The app-wide signature row plus its editor. */
@Composable
internal fun SettingsSignatureRow(
    html: String,
    onSave: (String) -> Unit,
) {
    var editing by remember { mutableStateOf(false) }
    val preview = signaturePreview(html)
    SettingsRow(
        icon = Icons.Filled.Draw,
        title = tr("settings.signature.label"),
        subtitle = preview.ifBlank { tr("settings.signature.hint") },
        onClick = { editing = true },
        trailing = { if (preview.isBlank()) Text(tr("settings.signature.modeNone"), color = MaterialTheme.colorScheme.onSurfaceVariant) },
    )
    if (editing) {
        SignatureEditorDialog(
            initialHtml = html,
            onSave = {
                onSave(it)
                editing = false
            },
            onDismiss = { editing = false },
        )
    }
}

/**
 * One account's override: follow the app-wide signature, send none, or use its
 * own. The custom text is kept when the mode changes, so switching away and back
 * does not lose it.
 */
@Composable
internal fun SettingsAccountSignatureRow(
    spec: SignatureSpec,
    onSave: (SignatureSpec) -> Unit,
) {
    var editing by remember { mutableStateOf(false) }
    val hint = tr("settings.signature.accountHint")
    SettingsRow(
        icon = Icons.Filled.Draw,
        title = tr("settings.signature.label"),
        subtitle = if (spec.mode == "custom") signaturePreview(spec.html).ifBlank { hint } else hint,
        onClick = { editing = true },
        trailing = { Text(signatureModeLabel(spec.mode), color = MaterialTheme.colorScheme.onSurfaceVariant) },
    )
    if (editing) {
        AccountSignatureDialog(
            initial = spec,
            onSave = {
                onSave(it)
                editing = false
            },
            onDismiss = { editing = false },
        )
    }
}

/**
 * Convert edited plain text back to storable HTML, keeping [originalHtml] when
 * the text is untouched so rich markup survives a look at this editor.
 */
private fun savedSignatureHtml(
    originalHtml: String,
    text: String,
): String =
    when {
        text.trim() == signaturePlainText(originalHtml) -> originalHtml
        signatureIsBlank(plainTextToHtml(text.trim())) -> ""
        else -> plainTextToHtml(text.trim())
    }

@Composable
private fun SignatureEditorDialog(
    initialHtml: String,
    onSave: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var text by remember { mutableStateOf(signaturePlainText(initialHtml)) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("settings.signature.label")) },
        text = {
            val scrollState = rememberScrollState()
            Column(modifier = Modifier.heightIn(max = 480.dp).appScrollbar(scrollState).verticalScroll(scrollState)) {
                Text(
                    tr("settings.signature.hint"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    label = { Text(tr("settings.signature.label")) },
                    keyboardOptions = nativeTextKeyboardOptions,
                    modifier = Modifier.fillMaxWidth().heightIn(min = 120.dp).padding(top = 8.dp),
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(savedSignatureHtml(initialHtml, text)) }) { Text(tr("buttons.save")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

@Composable
private fun AccountSignatureDialog(
    initial: SignatureSpec,
    onSave: (SignatureSpec) -> Unit,
    onDismiss: () -> Unit,
) {
    var mode by remember { mutableStateOf(initial.mode) }
    var text by remember { mutableStateOf(signaturePlainText(initial.html)) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("settings.signature.label")) },
        text = {
            val scrollState = rememberScrollState()
            Column(modifier = Modifier.heightIn(max = 480.dp).appScrollbar(scrollState).verticalScroll(scrollState)) {
                Text(
                    tr("settings.signature.accountHint"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                for (option in accountSignatureModes) {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable { mode = option }
                            .padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = option == mode, onClick = { mode = option })
                        Text(signatureModeLabel(option), modifier = Modifier.padding(start = 8.dp))
                    }
                }
                if (mode == "custom") {
                    OutlinedTextField(
                        value = text,
                        onValueChange = { text = it },
                        label = { Text(tr("settings.signature.label")) },
                        keyboardOptions = nativeTextKeyboardOptions,
                        modifier = Modifier.fillMaxWidth().heightIn(min = 120.dp).padding(top = 8.dp),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onSave(SignatureSpec(mode = mode, html = savedSignatureHtml(initial.html, text))) },
            ) { Text(tr("buttons.save")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}
