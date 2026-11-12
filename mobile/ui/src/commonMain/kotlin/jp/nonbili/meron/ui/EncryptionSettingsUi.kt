package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Autorenew
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.Domain
import androidx.compose.material.icons.filled.Draw
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.RestartAlt
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.UploadFile
import androidx.compose.material.icons.filled.VerifiedUser
import androidx.compose.material.icons.filled.VpnKey
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.AccountSummary
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlin.io.encoding.Base64

private val VerifiedColor = Color(0xFF278049)

private fun normalizeFingerprint(value: String) = value.filter { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }.uppercase()

/** 40 hex digits as ten groups of four, the way they are read aloud. */
internal fun formatFingerprint(fingerprint: String) = normalizeFingerprint(fingerprint).chunked(4).joinToString(" ")

/** The last 16 hex digits, enough to tell keys apart in a list. */
internal fun shortFingerprint(fingerprint: String) = normalizeFingerprint(fingerprint).takeLast(16).chunked(4).joinToString(" ")

private fun fingerprintsMatch(
    pasted: String,
    fingerprint: String,
): Boolean = normalizeFingerprint(pasted).let { it.length >= 32 && it == normalizeFingerprint(fingerprint) }

private fun JsonObject.objects(key: String) = (get(key) as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }

private fun JsonObject.strings(key: String) = (get(key) as? JsonArray).orEmpty().mapNotNull { (it as? JsonPrimitive)?.contentOrNull }

/** OpenPGP address matching in the core folds ASCII letters only. */
private fun keyAddress(address: String): String = address.trim().map { if (it in 'A'..'Z') it.lowercaseChar() else it }.joinToString("")

/** Match the addresses used by the core when it creates an account's key. */
internal fun encryptionAddresses(account: AccountSummary): List<String> =
    (listOf(account.email) + account.aliases.map { it.email })
        .map(::keyAddress)
        // Match the core's email_of_user_id, including while an alias is being edited.
        .filter { '@' in it && it.none { char -> char.isWhitespace() || char == '<' || char == '>' } }
        .distinct()

/** Every missing identity matters, even when the key includes the primary address. */
internal fun missingKeyAddresses(
    key: JsonObject,
    addresses: List<String>,
): List<String> {
    val emails = key.strings("emails").map(::keyAddress).toSet()
    return addresses.filter { keyAddress(it) !in emails }
}

/** Why a key can't be used (`expired`, `revoked`, …), blank when it can. */
private fun JsonObject.unusable() = (get("unusable") as? JsonPrimitive)?.takeIf { it !is JsonNull }?.contentOrNull.orEmpty()

private sealed interface EnableStep {
    data object Choose : EnableStep

    data class Passphrase(
        val key: JsonObject,
        val fingerprint: String?,
        val wrong: Boolean,
    ) : EnableStep

    /** The picked file is a backup encrypted with a code (OpenKeychain, Autocrypt Setup Message). */
    data class BackupCode(
        val wrong: Boolean,
    ) : EnableStep

    data class Pick(
        val keys: List<JsonObject>,
    ) : EnableStep
}

private sealed interface AccountEncryptionDialog {
    data object Enable : AccountEncryptionDialog

    data object Disable : AccountEncryptionDialog

    data object ExportSecret : AccountEncryptionDialog

    data class DeleteKey(
        val fingerprint: String,
    ) : AccountEncryptionDialog
}

/**
 * End-to-end encryption for one account, as the desktop account card has it:
 * the switch and, once there is a key, what can be done with it. Held outside
 * the rows because each row of a settings group is its own list item.
 */
internal class AccountEncryption(
    private val state: MeronMobileState,
    private val services: PlatformServices,
    private val scope: CoroutineScope,
    val account: String,
    val address: String,
) {
    private var info by mutableStateOf<JsonObject?>(null)
    var busy by mutableStateOf(false)
        private set
    var error by mutableStateOf("")
        private set
    private var dialog by mutableStateOf<AccountEncryptionDialog?>(null)
    private var step by mutableStateOf<EnableStep>(EnableStep.Choose)
    private var file: ByteArray? = null

    /** The code of a backup file, kept while its key is chosen and unlocked. */
    private var backupCode = ""
    private val params = buildJsonObject { put("account", account) }

    val loaded get() = info != null
    val enabled get() = info?.flag("enabled") == true

    /** Mail that isn't encrypted goes out signed. */
    val signPlain get() = info?.flag("sign_plain") == true
    val key get() = info?.get("key") as? JsonObject
    val previous get() = info?.objects("previous").orEmpty()
    val addresses
        get() = state.coreAccounts.firstOrNull { it.id == account }?.let(::encryptionAddresses) ?: listOf(address)
    val missingAddresses get() = key?.let { missingKeyAddresses(it, addresses) }.orEmpty()

    private suspend fun refresh() {
        info = state.pgp("pgp.account.state", params)
    }

    private fun run(action: suspend () -> Unit) {
        if (busy) return
        busy = true
        error = ""
        scope.launch {
            try {
                action()
                refresh()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.message.orEmpty()
            } finally {
                busy = false
            }
        }
    }

    fun load() = run {}

    fun toggle() {
        if (!loaded) return
        when {
            enabled -> dialog = AccountEncryptionDialog.Disable

            // A key kept from before: switch back on without asking again.
            key != null -> run { state.pgp("pgp.account.enable", params) }

            else -> openEnable()
        }
    }

    fun toggleSignPlain() {
        val sign = !signPlain
        run {
            state.pgp(
                "pgp.account.setSignPlain",
                buildJsonObject {
                    put("account", account)
                    put("sign", sign)
                },
            )
        }
    }

    fun openEnable() {
        error = ""
        step = EnableStep.Choose
        file = null
        backupCode = ""
        dialog = AccountEncryptionDialog.Enable
    }

    fun copyFingerprint() {
        val fingerprint = key?.text("fingerprint") ?: return
        services.copyText(state.trs("crypto.fingerprint"), formatFingerprint(fingerprint))
    }

    fun exportPublic() =
        run {
            val armored = state.pgp("pgp.account.exportPublic", params).text("armored")
            services.saveFile(armored.encodeToByteArray(), "${fileSlug()}-public.asc", "application/pgp-keys")
        }

    fun askExportSecret() {
        error = ""
        dialog = AccountEncryptionDialog.ExportSecret
    }

    fun askDelete(fingerprint: String) {
        dialog = AccountEncryptionDialog.DeleteKey(fingerprint)
    }

    private fun fileSlug() = address.replace(Regex("[^a-zA-Z0-9.@_-]+"), "-").ifBlank { "meron" }

    private fun close() {
        dialog = null
        file = null
        backupCode = ""
        step = EnableStep.Choose
    }

    private fun create() =
        run {
            state.pgp("pgp.account.create", params)
            close()
        }

    private fun pickExisting() {
        services.pickFile(listOf("*/*")) { picked ->
            if (picked != null) {
                file = picked.bytes
                backupCode = ""
                tryImport(null, null)
            }
        }
    }

    private fun tryImport(
        passphrase: String?,
        fingerprint: String?,
    ) = run {
        val data = file ?: return@run
        val result =
            state.pgp(
                "pgp.account.importKey",
                buildJsonObject {
                    put("account", account)
                    put("data_b64", Base64.encode(data))
                    put("passphrase", passphrase.orEmpty())
                    put("backup_code", backupCode)
                    if (!fingerprint.isNullOrBlank()) put("fingerprint", fingerprint)
                },
            )["result"] as? JsonObject ?: JsonObject(emptyMap())
        when (result.text("status")) {
            "imported" -> {
                close()
                val imported = result["key"] as? JsonObject
                val missing = imported?.let { missingKeyAddresses(it, addresses) }.orEmpty()
                if (missing.isNotEmpty()) {
                    error = state.trs("crypto.enable.addressMismatch", mapOf("address" to missing.joinToString()))
                } else if (!result.flag("address_matches")) {
                    error = state.trs("crypto.enable.addressMismatch", mapOf("address" to address))
                }
            }

            "needs_passphrase" -> {
                step =
                    EnableStep.Passphrase(
                        key = result["key"] as? JsonObject ?: JsonObject(emptyMap()),
                        fingerprint = fingerprint,
                        wrong = passphrase != null,
                    )
            }

            "needs_backup_code" -> {
                step = EnableStep.BackupCode(wrong = result.flag("wrong"))
            }

            "choose" -> {
                step = EnableStep.Pick(result.objects("keys"))
            }

            "on_card" -> {
                step = EnableStep.Choose
                error = state.trs("crypto.enable.onCard")
            }

            else -> {
                step = EnableStep.Choose
                error = state.trs("crypto.enable.noSecretKey")
            }
        }
    }

    /** An error raised while a dialog is open is shown in that dialog. */
    val rowError get() = error.takeIf { dialog == null }.orEmpty()

    @Composable
    fun Dialogs() {
        when (val current = dialog) {
            AccountEncryptionDialog.Enable -> {
                EnableDialog()
            }

            AccountEncryptionDialog.Disable -> {
                AlertDialog(
                    onDismissRequest = { dialog = null },
                    title = { Text(tr("crypto.account.disableTitle")) },
                    text = { Text(tr("crypto.account.disableMessage")) },
                    confirmButton = {
                        TextButton(onClick = {
                            dialog = null
                            run { state.pgp("pgp.account.disable", params) }
                        }) { Text(tr("crypto.account.disable")) }
                    },
                    dismissButton = { TextButton(onClick = { dialog = null }) { Text(tr("buttons.cancel")) } },
                )
            }

            AccountEncryptionDialog.ExportSecret -> {
                ExportSecretDialog(
                    onDismiss = { dialog = null },
                    onExport = { passphrase ->
                        dialog = null
                        run {
                            val armored =
                                state
                                    .pgp(
                                        "pgp.account.exportSecret",
                                        buildJsonObject {
                                            put("account", account)
                                            put("passphrase", passphrase)
                                        },
                                    ).text("armored")
                            services.saveFile(armored.encodeToByteArray(), "${fileSlug()}-secret.asc", "application/pgp-keys")
                        }
                    },
                )
            }

            is AccountEncryptionDialog.DeleteKey -> {
                AlertDialog(
                    onDismissRequest = { dialog = null },
                    title = { Text(tr("crypto.account.deleteTitle")) },
                    text = {
                        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                            Text(tr("crypto.account.deleteMessage"))
                            FingerprintBlock(current.fingerprint)
                        }
                    },
                    confirmButton = {
                        TextButton(onClick = {
                            dialog = null
                            run {
                                state.pgp(
                                    "pgp.account.deleteKey",
                                    buildJsonObject {
                                        put("account", account)
                                        put("fingerprint", current.fingerprint)
                                    },
                                )
                            }
                        }) { Text(tr("crypto.account.delete"), color = MaterialTheme.colorScheme.error) }
                    },
                    dismissButton = { TextButton(onClick = { dialog = null }) { Text(tr("buttons.cancel")) } },
                )
            }

            null -> {}
        }
    }

    /**
     * Turning encryption on asks one question: a new key, or the one the user
     * already has. A passphrase or a choice of key follows only if the picked
     * file needs it.
     */
    @Composable
    private fun EnableDialog() {
        var passphrase by remember(step) { mutableStateOf("") }
        var code by remember(step) { mutableStateOf("") }
        val current = step
        AlertDialog(
            onDismissRequest = { if (!busy) close() },
            icon = { Icon(Icons.Filled.Lock, contentDescription = null) },
            title = { Text(tr(if (key == null) "crypto.enable.title" else "crypto.account.replace")) },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    when (current) {
                        EnableStep.Choose -> {
                            Text(
                                tr("crypto.enable.question", mapOf("address" to address)),
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurface,
                            )
                            if (addresses.size > 1) {
                                Text(
                                    tr("crypto.enable.aliases", mapOf("addresses" to addresses.joinToString())),
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                            }
                            NoteBox(tr("crypto.enable.otherDevices"))
                            Button(onClick = { create() }, enabled = !busy, modifier = Modifier.fillMaxWidth()) {
                                Text(tr("crypto.enable.create"))
                            }
                            OutlinedButton(
                                onClick = { pickExisting() },
                                enabled = !busy,
                                colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.primary),
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text(tr("crypto.enable.useExisting"))
                            }
                            Text(
                                tr(if (key == null) "crypto.enable.backupHint" else "crypto.account.replaceHint"),
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }

                        is EnableStep.Passphrase -> {
                            val label =
                                current.key
                                    .strings("user_ids")
                                    .firstOrNull()
                                    .orEmpty()
                            Text(tr("crypto.enable.passphraseFor", mapOf("key" to label)))
                            FingerprintBlock(current.key.text("fingerprint"))
                            OutlinedTextField(
                                passphrase,
                                { passphrase = it },
                                label = { Text(tr("crypto.enable.passphrase")) },
                                singleLine = true,
                                isError = current.wrong && passphrase.isEmpty(),
                                supportingText = {
                                    Text(tr(if (current.wrong) "crypto.enable.wrongPassphrase" else "crypto.enable.passphraseOnce"))
                                },
                                visualTransformation = PasswordVisualTransformation(),
                                keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Password),
                                modifier = Modifier.fillMaxWidth(),
                            )
                        }

                        is EnableStep.BackupCode -> {
                            Text(tr("crypto.enable.backupCodeFor"))
                            OutlinedTextField(
                                code,
                                { code = it },
                                label = { Text(tr("crypto.enable.backupCode")) },
                                isError = current.wrong && code.isEmpty(),
                                supportingText = {
                                    Text(tr(if (current.wrong) "crypto.enable.wrongBackupCode" else "crypto.enable.backupCodeHint"))
                                },
                                textStyle = LocalTextStyle.current.copy(fontFamily = FontFamily.Monospace),
                                keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Password),
                                modifier = Modifier.fillMaxWidth(),
                            )
                        }

                        is EnableStep.Pick -> {
                            Text(tr("crypto.enable.pickKey"))
                            current.keys.forEach { candidate ->
                                Surface(
                                    color = MaterialTheme.colorScheme.surface,
                                    shape = RoundedCornerShape(12.dp),
                                    enabled = !busy,
                                    onClick = { tryImport(null, candidate.text("fingerprint")) },
                                    modifier = Modifier.fillMaxWidth(),
                                ) {
                                    Column(Modifier.padding(12.dp)) {
                                        Text(
                                            (candidate.strings("user_ids") + candidate.strings("emails")).firstOrNull() ?: "—",
                                            style = MaterialTheme.typography.bodyMedium,
                                            fontWeight = FontWeight.SemiBold,
                                        )
                                        Text(
                                            formatFingerprint(candidate.text("fingerprint")),
                                            style = MaterialTheme.typography.bodySmall,
                                            fontFamily = FontFamily.Monospace,
                                        )
                                    }
                                }
                            }
                        }
                    }
                    if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
                    if (error.isNotBlank()) {
                        Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                }
            },
            confirmButton = {
                if (current is EnableStep.Passphrase) {
                    TextButton(
                        enabled = !busy && passphrase.isNotEmpty(),
                        onClick = { tryImport(passphrase, current.fingerprint ?: current.key.text("fingerprint")) },
                    ) { Text(tr("crypto.enable.unlock")) }
                }
                if (current is EnableStep.BackupCode) {
                    TextButton(
                        enabled = !busy && code.isNotBlank(),
                        onClick = {
                            backupCode = code
                            tryImport(null, null)
                        },
                    ) { Text(tr("crypto.enable.unlock")) }
                }
            },
            dismissButton = { TextButton(enabled = !busy, onClick = { close() }) { Text(tr("buttons.cancel")) } },
        )
    }
}

@Composable
internal fun rememberAccountEncryption(
    account: String,
    address: String,
): AccountEncryption? {
    val state = LocalCryptoState.current ?: return null
    val services = LocalPlatformServices.current
    val scope = rememberCoroutineScope()
    val encryption = remember(account) { AccountEncryption(state, services, scope, account, address) }
    LaunchedEffect(encryption) { encryption.load() }
    return encryption
}

/** The "Encrypted email" group of an account's settings. */
@Composable
internal fun SettingsListScope.AccountEncryptionSection(encryption: AccountEncryption?) {
    encryption ?: return
    section(tr("crypto.account.title"))
    item {
        Column {
            ListItem(
                headlineContent = { Text(tr("crypto.account.switch")) },
                supportingContent = { Text(tr("crypto.account.switchHint")) },
                leadingContent = {
                    Icon(Icons.Filled.Lock, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                },
                trailingContent = {
                    Switch(
                        checked = encryption.enabled,
                        enabled = encryption.loaded && !encryption.busy,
                        onCheckedChange = { encryption.toggle() },
                        colors = settingsSwitchColors(),
                    )
                },
                modifier = Modifier.heightIn(min = 56.dp).clickable(enabled = !encryption.busy) { encryption.toggle() },
            )
            if (encryption.busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        }
    }
    if (encryption.rowError.isNotBlank()) {
        item { WarningRow(encryption.rowError) }
    }
    val key = encryption.key
    if (key != null && encryption.enabled) {
        item {
            ListItem(
                headlineContent = { Text(tr("crypto.account.signPlain")) },
                supportingContent = { Text(tr("crypto.account.signPlainHint")) },
                leadingContent = {
                    Icon(Icons.Filled.Draw, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                },
                trailingContent = {
                    Switch(
                        checked = encryption.signPlain,
                        enabled = !encryption.busy,
                        onCheckedChange = { encryption.toggleSignPlain() },
                        colors = settingsSwitchColors(),
                    )
                },
                modifier = Modifier.heightIn(min = 56.dp).clickable(enabled = !encryption.busy) { encryption.toggleSignPlain() },
            )
        }
    }
    if (key != null) CurrentKeyRows(encryption, key)
    // Earlier keys outlive the current one: deleting it promotes none of them.
    encryption.previous.forEach { earlier ->
        val fingerprint = earlier.text("fingerprint")
        item(key = "earlier-key:$fingerprint") {
            ListItem(
                headlineContent = {
                    SettingsHeadline(title = tr("crypto.account.previousKey"), hint = tr("crypto.account.previousKeyHint"))
                },
                supportingContent = {
                    Text(
                        shortFingerprint(fingerprint),
                        style = MaterialTheme.typography.bodySmall,
                        fontFamily = FontFamily.Monospace,
                    )
                },
                leadingContent = {
                    Icon(Icons.Filled.History, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                },
                trailingContent = {
                    IconButton(onClick = { encryption.askDelete(fingerprint) }) {
                        Icon(Icons.Filled.Delete, contentDescription = tr("crypto.account.delete"))
                    }
                },
                modifier = Modifier.heightIn(min = 56.dp),
            )
        }
    }
    if (key != null) {
        item {
            SettingsRow(
                icon = Icons.Filled.Delete,
                title = tr("crypto.account.delete"),
                hint = tr("crypto.account.deleteHint"),
                onClick = { encryption.askDelete(key.text("fingerprint")) },
                destructive = true,
            )
        }
    }
}

/** What can be done with the key in use: read it out, export it, replace it. */
@Composable
private fun SettingsListScope.CurrentKeyRows(
    encryption: AccountEncryption,
    key: JsonObject,
) {
    item {
        ListItem(
            headlineContent = { SettingsHeadline(title = tr("crypto.account.yourKey"), hint = tr("crypto.account.yourKeyHint")) },
            supportingContent = {
                Text(
                    formatFingerprint(key.text("fingerprint")).chunked(25).joinToString("\n") { it.trim() },
                    style = MaterialTheme.typography.bodySmall,
                    fontFamily = FontFamily.Monospace,
                )
            },
            leadingContent = {
                Icon(Icons.Filled.Fingerprint, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            },
            trailingContent = {
                IconButton(onClick = { encryption.copyFingerprint() }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = tr("crypto.account.copyFingerprint"))
                }
            },
            modifier = Modifier.heightIn(min = 56.dp),
        )
    }
    val unusable = key.unusable()
    val missingAddresses = encryption.missingAddresses
    if (missingAddresses.isNotEmpty()) {
        item { WarningRow(tr("crypto.account.missingAddresses", mapOf("addresses" to missingAddresses.joinToString()))) }
    }
    if (unusable.isNotBlank()) {
        item { WarningRow(tr("crypto.account.unusable.$unusable") + ". " + tr("crypto.toggle.ownKeyUnusable")) }
    }
    item {
        SettingsRow(
            icon = Icons.Filled.Share,
            title = tr("crypto.account.exportPublic"),
            hint = tr("crypto.account.exportPublicHint"),
            onClick = { encryption.exportPublic() },
            trailing = { RowAction(tr("common.export")) },
        )
    }
    item {
        SettingsRow(
            icon = Icons.Filled.VpnKey,
            title = tr("crypto.account.exportSecret"),
            hint = tr("crypto.account.exportSecretHint"),
            onClick = { encryption.askExportSecret() },
            trailing = { RowAction(tr("common.export")) },
        )
    }
    item {
        SettingsRow(
            icon = Icons.Filled.Autorenew,
            title = tr("crypto.account.replace"),
            hint = tr("crypto.account.replaceHint"),
            onClick = { encryption.openEnable() },
        )
    }
}

@Composable
private fun RowAction(label: String) {
    Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
}

@Composable
private fun WarningRow(text: String) {
    ListItem(
        headlineContent = { Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error) },
        leadingContent = { Icon(Icons.Filled.Warning, contentDescription = null, tint = MaterialTheme.colorScheme.error) },
    )
}

/** A caveat worth reading before going on, set apart from the body text. */
@Composable
private fun NoteBox(text: String) {
    // Dialogs sit on the same tone as surfaceVariant, so cards inside them use
    // the plain surface to stand out.
    Surface(
        color = MaterialTheme.colorScheme.surface,
        contentColor = MaterialTheme.colorScheme.onSurface,
        shape = RoundedCornerShape(12.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Icon(
                Icons.Filled.Info,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(18.dp),
            )
            Text(text, style = MaterialTheme.typography.bodySmall)
        }
    }
}

/** A fingerprint as two rows of five groups, to be compared group by group. */
@Composable
private fun FingerprintBlock(
    fingerprint: String,
    label: String? = null,
) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (label != null) {
            Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Surface(
            color = MaterialTheme.colorScheme.surface,
            contentColor = MaterialTheme.colorScheme.onSurface,
            shape = RoundedCornerShape(12.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(Modifier.padding(horizontal = 12.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                normalizeFingerprint(fingerprint).chunked(4).chunked(5).forEach { groups ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        groups.forEach { Text(it, style = MaterialTheme.typography.bodyMedium, fontFamily = FontFamily.Monospace) }
                    }
                }
            }
        }
    }
}

/** A new passphrase, typed twice, for a secret-key export, after a warning. */
@Composable
private fun ExportSecretDialog(
    onDismiss: () -> Unit,
    onExport: (String) -> Unit,
) {
    var passphrase by remember { mutableStateOf("") }
    var again by remember { mutableStateOf("") }
    val tooShort = passphrase.isNotEmpty() && passphrase.length < 8
    val mismatch = again.isNotEmpty() && passphrase != again
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Filled.VpnKey, contentDescription = null) },
        title = { Text(tr("crypto.account.exportSecret")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                NoteBox(tr("crypto.account.exportSecretWarning"))
                OutlinedTextField(
                    passphrase,
                    { passphrase = it },
                    label = { Text(tr("crypto.account.exportPassphrase")) },
                    singleLine = true,
                    supportingText = if (tooShort) ({ Text(tr("crypto.account.passphraseTooShort")) }) else null,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Password),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    again,
                    { again = it },
                    label = { Text(tr("settings.backup.passphraseConfirm")) },
                    singleLine = true,
                    isError = mismatch,
                    supportingText = if (mismatch) ({ Text(tr("settings.backup.passphraseMismatch")) }) else null,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Password),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(enabled = passphrase.length >= 8 && passphrase == again, onClick = { onExport(passphrase) }) {
                Text(tr("common.export"))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

/**
 * Settings → Encrypted email: everyone else's keys in one place, as on the
 * desktop. Keys arrive by themselves (Autocrypt, attachments); this is where a
 * keyring is imported, a contact verified, or a key removed. One's own key
 * lives in each account's settings.
 */
@Composable
internal fun EncryptionSettingsPage(modifier: Modifier = Modifier) {
    val state = LocalCryptoState.current ?: return
    val services = LocalPlatformServices.current
    val scope = rememberCoroutineScope()
    var peers by remember { mutableStateOf<List<JsonObject>?>(null) }
    var sources by remember { mutableStateOf<List<JsonObject>?>(null) }
    var defaults by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var filter by remember { mutableStateOf("") }
    var showAll by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    var verify by remember { mutableStateOf<JsonObject?>(null) }
    var unverify by remember { mutableStateOf<JsonObject?>(null) }
    var remove by remember { mutableStateOf<JsonObject?>(null) }
    var importing by remember { mutableStateOf<Pair<String, JsonObject>?>(null) }
    var lookup by remember { mutableStateOf(false) }
    var addSource by remember { mutableStateOf(false) }
    val sourcesLock = remember { Mutex() }

    fun act(block: suspend () -> Unit) {
        error = ""
        scope.launch {
            try {
                block()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.message.orEmpty()
            }
        }
    }

    suspend fun refreshPeers() {
        peers = state.pgp("pgp.peers.list", buildJsonObject {}).objects("peers")
    }

    // The whole list is saved at once, so each edit builds on the one before
    // it (shown at once, sent in order) rather than on a list the previous
    // save has yet to answer with.
    fun saveSources(next: List<JsonObject>) {
        sources = next
        act {
            sourcesLock.withLock {
                try {
                    val saved =
                        state
                            .pgp("pgp.lookup.setSources", buildJsonObject { put("sources", JsonArray(next)) })
                            .objects("sources")
                    if (sources == next) sources = saved
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    if (sources == next) {
                        sources = runCatching { state.pgp("pgp.lookup.sources", buildJsonObject {}).objects("sources") }.getOrDefault(next)
                    }
                    throw e
                }
            }
        }
    }

    LaunchedEffect(Unit) {
        act { refreshPeers() }
        act {
            val result = state.pgp("pgp.lookup.sources", buildJsonObject {})
            defaults = result.objects("defaults")
            sources = result.objects("sources")
        }
    }

    val all = peers.orEmpty()
    val query = filter.trim().lowercase()
    val shown =
        if (query.isEmpty()) {
            all
        } else {
            all.filter { peer ->
                peer.text("addr").contains(query) || peer.peerUserIds().any { it.lowercase().contains(query) }
            }
        }
    // A long keyring would push import, lookup and the key sources off the
    // screen, so it starts collapsed; a filter searches all of it.
    val collapsible = query.isEmpty() && all.size > COLLAPSED_KEYS
    val listed = if (collapsible && !showAll) shown.take(COLLAPSED_KEYS) else shown
    val listState = rememberLazyListState()
    SettingsList(modifier.appScrollbar(listState), state = listState) {
        plain {
            Text(
                tr("crypto.peers.intro"),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp),
            )
        }
        section(tr("crypto.peers.listTitle"))
        // Stays while a query is typed, or removing a key could hide the only
        // way to clear it.
        if (all.size > COLLAPSED_KEYS || filter.isNotEmpty()) {
            item { SettingsTextRow(value = filter, label = tr("crypto.peers.filter"), onValueChange = { filter = it }) }
        }
        if (peers != null && shown.isEmpty()) {
            item {
                Box(Modifier.padding(vertical = 8.dp)) {
                    SettingsEmptyLabel(tr(if (all.isEmpty()) "crypto.peers.empty" else "crypto.peers.noMatch"))
                }
            }
        }
        items(listed, key = { "peer:${it.text("addr")}/${it.peerFingerprint()}" }) { peer ->
            PeerRow(
                peer = peer,
                onVerify = { verify = peer },
                onUnverify = { unverify = peer },
                onRemove = { remove = peer },
            )
        }
        if (collapsible) {
            item {
                SettingsRow(
                    icon = if (showAll) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                    title = if (showAll) tr("common.showLess") else tr("crypto.peers.showAll", mapOf("count" to all.size)),
                    onClick = { showAll = !showAll },
                )
            }
        }
        item {
            SettingsRow(
                icon = Icons.Filled.UploadFile,
                title = tr("crypto.peers.import"),
                onClick = {
                    services.pickFile(listOf("*/*")) { picked ->
                        if (picked != null) {
                            act {
                                val data = Base64.encode(picked.bytes)
                                importing = data to state.pgp("pgp.peers.importPreview", buildJsonObject { put("data_b64", data) })
                            }
                        }
                    }
                },
            )
        }
        item {
            SettingsRow(
                icon = Icons.Filled.Search,
                title = tr("crypto.lookup.title"),
                onClick = { lookup = true },
            )
        }
        if (error.isNotBlank()) {
            item { WarningRow(error) }
        }

        val lookupSources = sources
        if (lookupSources != null) {
            section(tr("crypto.sources.title"))
            itemsIndexed(lookupSources) { index, source ->
                val isDefault = defaults.any { it.text("kind") == source.text("kind") && it.text("url") == source.text("url") }
                SourceRow(
                    source = source,
                    isDefault = isDefault,
                    onToggle = {
                        saveSources(
                            lookupSources.mapIndexed { i, other ->
                                if (i == index) JsonObject(other + ("enabled" to JsonPrimitive(!other.flag("enabled")))) else other
                            },
                        )
                    },
                    onRemove = { saveSources(lookupSources.filterIndexed { i, _ -> i != index }) },
                )
            }
            item {
                SettingsRow(
                    icon = Icons.Filled.Add,
                    title = tr("crypto.sources.add"),
                    onClick = { addSource = true },
                )
            }
            item {
                SettingsRow(
                    icon = Icons.Filled.RestartAlt,
                    title = tr("crypto.sources.reset"),
                    onClick = { saveSources(defaults) },
                )
            }
        }
    }

    if (lookup) {
        KeyLookupDialog(state, "", onDismiss = { lookup = false }, onAdded = {
            lookup = false
            act { refreshPeers() }
        })
    }
    if (addSource) {
        AddKeyserverDialog(
            onDismiss = { addSource = false },
            onAdd = { url ->
                addSource = false
                saveSources(
                    sources.orEmpty() +
                        buildJsonObject {
                            put("kind", "hkp")
                            put("url", url)
                            put("enabled", true)
                        },
                )
            },
        )
    }
    verify?.let { peer ->
        VerifyKeyDialog(
            peer = peer,
            onDismiss = { verify = null },
            onConfirm = { confirm ->
                verify = null
                act {
                    state.pgp(
                        "pgp.peers.setVerified",
                        buildJsonObject {
                            put("addr", peer.text("addr"))
                            put("fingerprint", peer.peerFingerprint())
                            put("confirm", confirm)
                            put("verified", true)
                        },
                    )
                    refreshPeers()
                }
            },
        )
    }
    unverify?.let { peer ->
        AlertDialog(
            onDismissRequest = { unverify = null },
            title = { Text(tr("crypto.peers.unverifyTitle")) },
            text = { Text(tr("crypto.peers.unverifyMessage", mapOf("sender" to peer.peerName()))) },
            confirmButton = {
                TextButton(onClick = {
                    unverify = null
                    act {
                        state.pgp(
                            "pgp.peers.setVerified",
                            buildJsonObject {
                                put("addr", peer.text("addr"))
                                put("fingerprint", peer.peerFingerprint())
                                put("confirm", "")
                                put("verified", false)
                            },
                        )
                        refreshPeers()
                    }
                }) { Text(tr("crypto.peers.unverify")) }
            },
            dismissButton = { TextButton(onClick = { unverify = null }) { Text(tr("buttons.cancel")) } },
        )
    }
    remove?.let { peer ->
        AlertDialog(
            onDismissRequest = { remove = null },
            title = { Text(tr("crypto.peers.removeTitle")) },
            text = { Text(tr("crypto.peers.removeMessage", mapOf("sender" to peer.peerName()))) },
            confirmButton = {
                TextButton(onClick = {
                    remove = null
                    act {
                        state.pgp(
                            "pgp.peers.remove",
                            buildJsonObject {
                                put("addr", peer.text("addr"))
                                put("fingerprint", peer.peerFingerprint())
                            },
                        )
                        refreshPeers()
                    }
                }) { Text(tr("crypto.peers.remove"), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { remove = null }) { Text(tr("buttons.cancel")) } },
        )
    }
    importing?.let { (data, preview) ->
        ImportKeysDialog(
            preview = preview,
            onDismiss = { importing = null },
            onImport = {
                importing = null
                act {
                    state.pgp(
                        "pgp.peers.import",
                        buildJsonObject {
                            put("data_b64", data)
                            putJsonArray("fingerprints") { preview.objects("ready").forEach { add(it.text("fingerprint")) } }
                        },
                    )
                    refreshPeers()
                }
            },
        )
    }
}

/** Keys listed before "Show all": enough to recognise the list, few enough
 *  that the rows under it stay on the first screen. */
private const val COLLAPSED_KEYS = 4

private fun JsonObject.peerUserIds() = (get("info") as? JsonObject)?.strings("user_ids").orEmpty()

/** The key's name without its `<address>`, or the address when it has none. */
private fun JsonObject.peerName() =
    peerUserIds()
        .firstOrNull()
        ?.replace(Regex("\\s*<[^>]*>\\s*$"), "")
        ?.takeIf { it.isNotBlank() } ?: text("addr")

@Composable
private fun PeerRow(
    peer: JsonObject,
    onVerify: () -> Unit,
    onUnverify: () -> Unit,
    onRemove: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    val name = peer.peerName()
    val addr = peer.text("addr")
    val verified = peer.flag("verified")
    val conflict = peer.flag("conflicts_with_verified")
    val unusable = (peer["info"] as? JsonObject)?.unusable().orEmpty()
    // Not a ListItem: with three supporting lines it pins its content to the
    // top and leaves the slack underneath.
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clickable { menu = true }.padding(start = 16.dp, end = 4.dp, top = 12.dp, bottom = 12.dp),
    ) {
        when {
            conflict -> Icon(Icons.Filled.Warning, contentDescription = null, tint = MaterialTheme.colorScheme.error)
            verified -> Icon(Icons.Filled.VerifiedUser, contentDescription = null, tint = VerifiedColor)
            else -> Icon(Icons.Filled.Key, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Column(Modifier.weight(1f).padding(start = 16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (name != addr) {
                Text(
                    addr,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(
                shortFingerprint(peer.peerFingerprint()),
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                listOfNotNull(
                    tr(if (verified) "crypto.peers.verified" else "crypto.peers.known"),
                    tr("crypto.peers.source.${peer.text("source")}").takeIf { peer.text("source").isNotBlank() },
                ).joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = if (verified) VerifiedColor else MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (conflict) {
                Text(tr("crypto.peers.conflict"), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
            }
            if (unusable.isNotBlank()) {
                Text(
                    tr("crypto.account.unusable.$unusable"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
        Box {
            IconButton(onClick = { menu = true }) {
                Icon(Icons.Filled.MoreVert, contentDescription = tr("common.more"), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                if (verified) {
                    DropdownMenuItem(
                        text = { Text(tr("crypto.peers.unverify")) },
                        onClick = {
                            menu = false
                            onUnverify()
                        },
                    )
                } else {
                    DropdownMenuItem(
                        text = { Text(tr("crypto.verifyKey")) },
                        onClick = {
                            menu = false
                            onVerify()
                        },
                    )
                }
                DropdownMenuItem(
                    text = { Text(tr("crypto.peers.remove"), color = MaterialTheme.colorScheme.error) },
                    onClick = {
                        menu = false
                        onRemove()
                    },
                )
            }
        }
    }
}

@Composable
private fun SourceRow(
    source: JsonObject,
    isDefault: Boolean,
    onToggle: () -> Unit,
    onRemove: () -> Unit,
) {
    val kind = source.text("kind")
    val label = if (kind == "wkd") "WKD" else source.text("url").removePrefix("https://").trimEnd('/')
    ListItem(
        headlineContent = {
            SettingsHeadline(
                title = label,
                hint = if (isDefault && (kind == "wkd" || kind == "vks")) tr("crypto.sources.why.$kind") else null,
            )
        },
        supportingContent = {
            Text(
                tr("crypto.sources.kind.$kind"),
                color = if (kind == "hkp") MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onSurfaceVariant,
            )
        },
        leadingContent = {
            Icon(
                if (kind == "wkd") Icons.Filled.Domain else Icons.Filled.Dns,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (!isDefault) {
                    IconButton(onClick = onRemove) {
                        Icon(Icons.Filled.Delete, contentDescription = tr("crypto.sources.remove"))
                    }
                }
                Switch(checked = source.flag("enabled"), onCheckedChange = { onToggle() }, colors = settingsSwitchColors())
            }
        },
        modifier = Modifier.heightIn(min = 56.dp).clickable(onClick = onToggle),
    )
}

@Composable
private fun AddKeyserverDialog(
    onDismiss: () -> Unit,
    onAdd: (String) -> Unit,
) {
    var value by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("crypto.sources.add")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value,
                    { value = it },
                    placeholder = { Text("keyserver.ubuntu.com") },
                    singleLine = true,
                    supportingText = { Text(tr("crypto.sources.kind.hkp")) },
                    keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Uri),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(enabled = value.isNotBlank(), onClick = {
                val raw = value.trim()
                onAdd(if (raw.startsWith("https://")) raw else "https://" + raw.replace(Regex("^hkps?://"), ""))
            }) { Text(tr("crypto.sources.add")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

/**
 * Verifying a contact's key: compare fingerprints over another channel, then
 * either paste the one they sent (compared here, so nothing is misread) or
 * tick "they match". Verification is local and per key.
 */
@Composable
private fun VerifyKeyDialog(
    peer: JsonObject,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    val name = peer.peerName()
    val fingerprint = peer.peerFingerprint()
    var pasted by remember { mutableStateOf("") }
    var compared by remember { mutableStateOf(false) }
    val matches = pasted.isNotBlank() && fingerprintsMatch(pasted, fingerprint)
    val wrong = pasted.trim().length >= 8 && !matches
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Filled.VerifiedUser, contentDescription = null) },
        title = { Text(tr("crypto.verify.title", mapOf("sender" to name))) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FingerprintBlock(fingerprint, label = tr("crypto.verify.theirs", mapOf("sender" to name)))
                Text(tr("crypto.verify.howTo", mapOf("sender" to name)), style = MaterialTheme.typography.bodySmall)
                OutlinedTextField(
                    pasted,
                    { pasted = it },
                    label = { Text(tr("crypto.fingerprint")) },
                    isError = wrong,
                    supportingText = {
                        when {
                            matches -> Text(tr("crypto.verify.matches"), color = VerifiedColor)
                            wrong -> Text(tr("crypto.verify.mismatch"))
                            else -> Text(tr("crypto.verify.paste", mapOf("sender" to name)))
                        }
                    },
                    textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                    keyboardOptions = nativeTextKeyboardOptions,
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().clickable(enabled = !wrong) { compared = !compared },
                ) {
                    Checkbox(checked = compared, enabled = !wrong, onCheckedChange = { compared = it })
                    Text(tr("crypto.verify.compared"), color = MaterialTheme.colorScheme.onSurface)
                }
            }
        },
        confirmButton = {
            // The core checks the confirmed fingerprint equals the stored key's.
            TextButton(enabled = !wrong && (matches || compared), onClick = { onConfirm(if (matches) pasted else fingerprint) }) {
                Text(tr("crypto.verify.confirm"))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

/** What a picked keyring holds, before anything is imported from it. */
@Composable
private fun ImportKeysDialog(
    preview: JsonObject,
    onDismiss: () -> Unit,
    onImport: () -> Unit,
) {
    fun count(key: String) = (preview[key] as? JsonArray)?.size ?: 0
    val ready = count("ready")
    val updates = count("updates")
    val reasons = preview.objects("skipped").map { it.text("reason") }
    val expired = reasons.count { it == "expired" }
    val revoked = reasons.count { it == "revoked" }
    val unreadable = (preview["unreadable"] as? JsonPrimitive)?.intOrNull ?: 0
    val lines =
        listOfNotNull(
            tr("crypto.peers.alreadyPresent", mapOf("count" to count("already_present"))).takeIf { count("already_present") > 0 },
            tr("crypto.peers.skippedExpired", mapOf("count" to expired)).takeIf { expired > 0 },
            tr("crypto.peers.skippedRevoked", mapOf("count" to revoked)).takeIf { revoked > 0 },
            tr("crypto.peers.skippedOther", mapOf("count" to reasons.size - expired - revoked)).takeIf { reasons.size - expired - revoked > 0 },
            tr("crypto.peers.updates", mapOf("count" to updates)).takeIf { updates > 0 },
            tr("crypto.peers.unreadable", mapOf("count" to unreadable)).takeIf { unreadable > 0 },
            tr("crypto.peers.replaces", mapOf("addresses" to preview.strings("replaces_unverified").joinToString())).takeIf {
                count("replaces_unverified") > 0
            },
        )
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Filled.UploadFile, contentDescription = null) },
        title = { Text(tr("crypto.peers.importTitle")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    tr("crypto.peers.ready", mapOf("count" to ready)),
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                preview.objects("ready").take(MAX_PREVIEW_KEYS).forEach { key ->
                    Text(
                        (key.strings("user_ids") + key.strings("emails")).firstOrNull() ?: shortFingerprint(key.text("fingerprint")),
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (ready > MAX_PREVIEW_KEYS) Text("…", style = MaterialTheme.typography.bodySmall)
                lines.forEach { Text(it, style = MaterialTheme.typography.bodySmall) }
                if (count("conflicts_with_verified") > 0) {
                    Text(
                        tr("crypto.peers.conflictsVerified", mapOf("addresses" to preview.strings("conflicts_with_verified").joinToString())),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                NoteBox(tr("crypto.peers.notVerifiedNote"))
            }
        },
        confirmButton = {
            TextButton(enabled = ready > 0 || updates > 0, onClick = onImport) {
                Text(if (ready == 0) tr("crypto.peers.applyUpdates") else tr("crypto.peers.importAction", mapOf("count" to ready)))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

private const val MAX_PREVIEW_KEYS = 8
