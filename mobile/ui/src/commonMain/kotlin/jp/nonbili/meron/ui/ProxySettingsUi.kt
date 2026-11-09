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
import androidx.compose.material.icons.filled.Lan
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.ProxySpec

/** Modes offered app-wide, and the extra two an account can pick from. */
private val appProxyModes = listOf("off", "http", "socks5")
private val accountProxyModes = listOf("global", "direct", "http", "socks5")

@Composable
private fun proxyModeLabel(mode: String): String =
    when (mode) {
        "http" -> tr("settings.network.modeHttp")
        "socks5" -> tr("settings.network.modeSocks5")
        "global" -> tr("settings.network.modeGlobal")
        "direct" -> tr("settings.network.modeDirect")
        else -> tr("settings.network.modeOff")
    }

/** Row subtitle: the endpoint when one is configured, else the mode alone. */
@Composable
private fun proxySummary(spec: ProxySpec): String =
    when {
        spec.usable -> "${proxyModeLabel(spec.mode)} · ${spec.host}:${spec.port}"
        spec.mode == "http" || spec.mode == "socks5" -> tr("settings.network.incomplete")
        else -> proxyModeLabel(spec.mode)
    }

/**
 * The proxy row plus its editor. Used for the app-wide proxy and, with
 * [accountScoped] set, for a single account's override — the only difference is
 * which modes are offered and the hint text.
 *
 * Saving is explicit (the dialog's confirm button) rather than per-keystroke:
 * a half-typed host would otherwise be pushed to the core and break the next
 * connection attempt.
 */
@Composable
internal fun SettingsProxyRow(
    spec: ProxySpec,
    accountScoped: Boolean,
    onSave: (ProxySpec) -> Unit,
) {
    var editing by remember { mutableStateOf(false) }
    SettingsRow(
        icon = Icons.Filled.Lan,
        title = tr("settings.network.proxy"),
        subtitle = proxySummary(spec),
        onClick = { editing = true },
        trailing = { Text(proxyModeLabel(spec.mode), color = MaterialTheme.colorScheme.onSurfaceVariant) },
    )
    if (editing) {
        ProxyEditorDialog(
            initial = spec,
            accountScoped = accountScoped,
            onSave = {
                onSave(it)
                editing = false
            },
            onDismiss = { editing = false },
        )
    }
}

@Composable
private fun ProxyEditorDialog(
    initial: ProxySpec,
    accountScoped: Boolean,
    onSave: (ProxySpec) -> Unit,
    onDismiss: () -> Unit,
) {
    var mode by remember { mutableStateOf(initial.mode) }
    var host by remember { mutableStateOf(initial.host) }
    var portText by remember { mutableStateOf(if (initial.port > 0) initial.port.toString() else "") }
    var username by remember { mutableStateOf(initial.username) }
    var password by remember { mutableStateOf(initial.password) }
    val needsEndpoint = mode == "http" || mode == "socks5"
    val port = portText.toIntOrNull() ?: 0
    val canSave = !needsEndpoint || (host.isNotBlank() && port in 1..65535)

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("settings.network.proxy")) },
        text = {
            val scrollState = rememberScrollState()
            Column(modifier = Modifier.heightIn(max = 480.dp).appScrollbar(scrollState).verticalScroll(scrollState)) {
                Text(
                    if (accountScoped) tr("settings.network.accountProxyHint") else tr("settings.network.proxyHint"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                for (option in if (accountScoped) accountProxyModes else appProxyModes) {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable { mode = option }
                            .padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = option == mode, onClick = { mode = option })
                        Text(proxyModeLabel(option), modifier = Modifier.padding(start = 8.dp))
                    }
                }
                if (needsEndpoint) {
                    OutlinedTextField(
                        value = host,
                        onValueChange = { host = it.trim() },
                        label = { Text(tr("settings.network.host")) },
                        singleLine = true,
                        keyboardOptions = nativeTextKeyboardOptions,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    OutlinedTextField(
                        value = portText,
                        onValueChange = { portText = it.filter(Char::isDigit).take(5) },
                        label = { Text(tr("accounts.fields.port")) },
                        singleLine = true,
                        keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Number),
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    OutlinedTextField(
                        value = username,
                        onValueChange = { username = it },
                        label = { Text(tr("accounts.fields.username")) },
                        supportingText = { Text(tr("settings.network.optional")) },
                        singleLine = true,
                        keyboardOptions = nativeTextKeyboardOptions,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        label = { Text(tr("accounts.fields.password")) },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Password),
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = canSave,
                onClick = {
                    // Modes without an endpoint drop any leftover credentials
                    // rather than keeping them around unused in the store.
                    onSave(
                        if (needsEndpoint) {
                            ProxySpec(mode, host.trim(), port, username, password)
                        } else {
                            ProxySpec(mode = mode)
                        },
                    )
                },
            ) { Text(tr("buttons.save")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}
