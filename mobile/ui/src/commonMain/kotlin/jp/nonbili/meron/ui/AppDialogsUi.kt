package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.ServerCertificate
import jp.nonbili.meron.shared.certificateCommonName
import jp.nonbili.meron.shared.formatCertificateFingerprint

@Composable
internal fun AddFeedDialog(
    url: String,
    onUrlChange: (String) -> Unit,
    error: String,
    submitting: Boolean,
    onAdd: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("feeds.actions.addFeed")) },
        text = {
            OutlinedTextField(
                value = url,
                onValueChange = onUrlChange,
                label = { Text(tr("feeds.url")) },
                supportingText = error.takeIf { it.isNotBlank() }?.let { message -> { Text(message) } },
                isError = error.isNotBlank(),
                singleLine = true,
                enabled = !submitting,
                colors =
                    OutlinedTextFieldDefaults.colors(
                        unfocusedBorderColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f),
                    ),
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(
                onClick = onAdd,
                enabled = url.isNotBlank() && !submitting,
            ) {
                if (submitting) {
                    CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                } else {
                    Text(tr("common.add"))
                }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !submitting) {
                Text(tr("buttons.cancel"))
            }
        },
    )
}

/** Names a feed in the list. An empty name restores the feed's own title. */
@Composable
internal fun RenameFeedDialog(
    name: String,
    onNameChange: (String) -> Unit,
    onSave: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("mobile.actions.renameFeed")) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = onNameChange,
                label = { Text(tr("feeds.name")) },
                supportingText = { Text(tr("feeds.nameHint")) },
                singleLine = true,
                colors =
                    OutlinedTextFieldDefaults.colors(
                        unfocusedBorderColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f),
                    ),
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = { TextButton(onClick = onSave) { Text(tr("buttons.save")) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

/**
 * Confirms emptying a Trash or Junk folder. The delete is permanent — there is no
 * Trash left to restore from — so the action always goes through this dialog.
 */
@Composable
internal fun EmptyFolderDialog(
    folderName: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("threads.emptyFolder.confirmTitle", mapOf("folder" to folderName))) },
        text = { Text(tr("threads.emptyFolder.confirmMessage", mapOf("folder" to folderName))) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(
                    tr("threads.emptyFolder.confirmButton"),
                    color = MaterialTheme.colorScheme.error,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(tr("buttons.cancel"))
            }
        },
    )
}

/**
 * Confirms deleting a folder on the server. The folder's mail goes with it and the
 * server keeps no copy, so the action always goes through this dialog.
 */
@Composable
internal fun DeleteFolderDialog(
    folderName: String,
    nested: Int,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("folders.delete.confirmTitle", mapOf("folder" to folderName))) },
        text = {
            Text(
                if (nested > 0) {
                    tr("folders.delete.confirmMessageNested", mapOf("folder" to folderName, "count" to nested))
                } else {
                    tr("folders.delete.confirmMessage", mapOf("folder" to folderName))
                },
            )
        },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(
                    tr("folders.delete.confirmButton"),
                    color = MaterialTheme.colorScheme.error,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(tr("buttons.cancel"))
            }
        },
    )
}

/**
 * Asks the user to accept a server certificate that could not be validated
 * against the public roots — a local Proton Mail Bridge serves a self-signed
 * one. They compare the fingerprint against the one the server is supposed to
 * have; accepting pins that exact certificate for this account and nothing else.
 */
@Composable
internal fun CertificateTrustDialog(
    server: String,
    certificate: ServerCertificate,
    busy: Boolean,
    onTrust: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(tr("accounts.certificate.title")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(tr("accounts.certificate.body", mapOf("server" to server)))
                CertificateRow(tr("accounts.certificate.issuedTo"), certificateCommonName(certificate.subject))
                CertificateRow(tr("accounts.certificate.issuedBy"), certificateCommonName(certificate.issuer))
                CertificateRow(tr("accounts.certificate.expires"), certificate.notAfter)
                CertificateRow(
                    tr("accounts.certificate.fingerprint"),
                    formatCertificateFingerprint(certificate.fingerprint),
                    monospace = true,
                )
            }
        },
        confirmButton = {
            TextButton(onClick = onTrust, enabled = !busy) {
                if (busy) {
                    CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                } else {
                    Text(tr("accounts.certificate.trust"))
                }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !busy) {
                Text(tr("buttons.cancel"))
            }
        },
    )
}

@Composable
private fun CertificateRow(
    label: String,
    value: String,
    monospace: Boolean = false,
) {
    if (value.isBlank()) return
    Column {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            value,
            style =
                if (monospace) {
                    MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace)
                } else {
                    MaterialTheme.typography.bodyMedium
                },
        )
    }
}

/**
 * Offered on the launch after a crash. Nothing has been sent at this point —
 * "send" opens the platform share sheet with the redacted diagnostic log, so
 * the user sees the contents and picks the recipient.
 */
@Composable
internal fun CrashReportDialog(
    summary: String,
    onSend: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("crash.title")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(tr("crash.body"))
                if (summary.isNotBlank()) {
                    Text(
                        summary,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onSend) {
                Text(tr("crash.send"))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(tr("crash.notNow"))
            }
        },
    )
}

/**
 * Asks for a folder's name: a new folder's (with [hint] on nesting it), or the
 * new name of one being renamed.
 */
@Composable
internal fun FolderNameDialog(
    title: String,
    confirmLabel: String,
    name: String,
    onNameChange: (String) -> Unit,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    hint: String? = null,
) {
    val canConfirm = name.isNotBlank()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = onNameChange,
                label = { Text(tr("folders.namePlaceholder")) },
                supportingText = hint?.let { { Text(it) } },
                singleLine = true,
                keyboardOptions = nativeTextKeyboardOptions.copy(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (canConfirm) onConfirm() }),
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(onClick = onConfirm, enabled = canConfirm) {
                Text(confirmLabel)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(tr("buttons.cancel"))
            }
        },
    )
}
