package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.LockOpen
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.MessageBody
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.add
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray

@Composable
internal fun ComposeEncryptionControl() {
    val state = LocalCryptoState.current ?: return
    val account = state.selectedComposeIdentity()?.accountId ?: state.defaultSendAccountId()
    var enabled by remember { mutableStateOf(false) }
    var missing by remember { mutableStateOf<List<String>>(emptyList()) }
    var error by remember { mutableStateOf("") }
    var offConfirm by remember { mutableStateOf(false) }
    var lookup by remember { mutableStateOf(false) }
    LaunchedEffect(account, state.to, state.cc, state.bcc, state.composeSessionGeneration) {
        try {
            val recipients =
                listOf(state.to, state.cc, state.bcc).flatMap { value ->
                    val (completed, active) = parseRecipients(value)
                    (completed + active).filter { it.isNotBlank() }.map { it.substringAfter('<').substringBefore('>').trim() }
                }
            val result =
                state.pgp(
                    "pgp.keys.forRecipients",
                    buildJsonObject {
                        put("account", account)
                        putJsonArray("addrs") { recipients.forEach { add(it) } }
                    },
                )
            enabled = result.flag("enabled") && !result.flag("own_key_unusable")
            missing =
                (result["missing"] as? JsonArray).orEmpty().map { element ->
                    (element as? JsonObject)?.text("addr") ?: element.jsonPrimitive.content
                }
            if (state.composeEncrypt == null && enabled && recipients.isNotEmpty() && missing.isEmpty()) {
                state.composeEncrypt = true
                state.autoSaveComposeDraft()
            }
            error = if (result.flag("own_key_unusable")) state.trs("crypto.toggle.ownKeyUnusable") else ""
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            error = e.message.orEmpty()
        }
    }
    if (enabled || state.composeEncrypt == true) {
        IconButton(onClick = {
            when {
                state.composeEncrypt == true && state.composeCryptoLocked -> {
                    error = state.trs("crypto.toggle.decryptedLocked")
                }

                state.composeEncrypt == true -> {
                    offConfirm = true
                }

                missing.isNotEmpty() -> {
                    lookup = true
                }

                else -> {
                    state.composeEncrypt = true
                    state.autoSaveComposeDraft()
                }
            }
        }) {
            Icon(
                if (state.composeEncrypt == true) Icons.Default.Lock else Icons.Default.LockOpen,
                tr(if (state.composeEncrypt == true) "crypto.toggle.on" else "crypto.toggle.off"),
                tint = if (missing.isNotEmpty() || !enabled) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
            )
        }
    }
    if (error.isNotBlank()) {
        AlertDialog(
            onDismissRequest = { error = "" },
            text = { Text(error) },
            confirmButton = { TextButton(onClick = { error = "" }) { Text(tr("buttons.close")) } },
        )
    }
    if (offConfirm) {
        AlertDialog(
            onDismissRequest = { offConfirm = false },
            title = { Text(tr("crypto.toggle.offTitle")) },
            text = { Text(tr("crypto.toggle.offDraftMessage")) },
            confirmButton = {
                TextButton(onClick = {
                    state.composeEncrypt = false
                    state.composeFoldedQuote = null
                    offConfirm = false
                    state.autoSaveComposeDraft()
                }) { Text(tr("crypto.toggle.offConfirm")) }
            },
            dismissButton = { TextButton(onClick = { offConfirm = false }) { Text(tr("buttons.cancel")) } },
        )
    }
    if (lookup) {
        KeyLookupDialog(state, missing.firstOrNull().orEmpty(), onDismiss = { lookup = false }, onAdded = {
            lookup = false
            state.composeEncrypt = true
            state.autoSaveComposeDraft()
        })
    }
}

@Composable
internal fun KeyLookupDialog(
    state: MeronMobileState,
    initialAddress: String,
    onDismiss: () -> Unit,
    onAdded: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var address by remember { mutableStateOf(initialAddress) }
    var result by remember { mutableStateOf<JsonObject?>(null) }
    var error by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("crypto.lookup.title")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(tr("crypto.lookup.note"), style = MaterialTheme.typography.bodySmall)
                OutlinedTextField(
                    address,
                    {
                        address = it
                        result = null
                    },
                    label = { Text(tr("crypto.lookup.address")) },
                    singleLine = true,
                    keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Email),
                    modifier = Modifier.fillMaxWidth(),
                )
                (result?.get("found") as? JsonArray)?.forEach { element ->
                    val key = element.jsonObject
                    val source = key["source"]!!.jsonObject
                    Surface(
                        color = MaterialTheme.colorScheme.surface,
                        shape = RoundedCornerShape(12.dp),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Column(Modifier.padding(start = 12.dp, end = 12.dp, top = 12.dp, bottom = 4.dp)) {
                            Text(
                                formatFingerprint(key["info"]!!.jsonObject.text("fingerprint")),
                                style = MaterialTheme.typography.bodySmall,
                                fontFamily = FontFamily.Monospace,
                            )
                            Text(
                                tr(
                                    when {
                                        source.text("kind") == "wkd" -> "crypto.lookup.fromWkd"
                                        key.flag("verified") -> "crypto.lookup.fromVerified"
                                        else -> "crypto.lookup.fromUnverified"
                                    },
                                    mapOf("server" to source.text("url")),
                                ),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(top = 4.dp),
                            )
                            TextButton(
                                enabled = !busy,
                                modifier = Modifier.align(Alignment.End),
                                onClick = {
                                    busy = true
                                    scope.launch {
                                        runCatching {
                                            state.pgp(
                                                "pgp.lookup.add",
                                                buildJsonObject {
                                                    put("addr", address.trim())
                                                    put("data_b64", key.text("data_b64"))
                                                    put("source", source.text("kind"))
                                                },
                                            )
                                        }.onSuccess { onAdded() }.onFailure { error = it.message.orEmpty() }
                                        busy = false
                                    }
                                },
                            ) { Text(tr("crypto.lookup.use")) }
                        }
                    }
                }
                result?.let { found ->
                    val searched = (found["searched"] as? JsonArray).orEmpty().joinToString { it.jsonPrimitive.content }
                    if ((found["found"] as? JsonArray).orEmpty().isEmpty()) {
                        Text(tr("crypto.lookup.nothing", mapOf("sources" to searched)), style = MaterialTheme.typography.bodyMedium)
                    }
                    val failed = (found["failed"] as? JsonArray).orEmpty().joinToString { it.jsonObject.text("source") }
                    if (failed.isNotBlank()) {
                        Text(
                            tr("crypto.lookup.failed", mapOf("sources" to failed)),
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
                if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
        },
        confirmButton = {
            TextButton(enabled = !busy && address.isNotBlank(), onClick = {
                busy = true
                error = ""
                scope.launch {
                    runCatching { state.pgp("pgp.lookup", buildJsonObject { put("addr", address.trim()) }) }
                        .onSuccess { result = it }
                        .onFailure { error = it.message.orEmpty() }
                    busy = false
                }
            }) { Text(tr("crypto.lookup.action")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.close")) } },
    )
}

internal val LocalCryptoState = staticCompositionLocalOf<MeronMobileState?> { null }

internal suspend fun MeronMobileState.repliesToEncryptedMail(
    account: String,
    inReplyTo: String,
): Boolean =
    inReplyTo.isNotBlank() &&
        pgp(
            "pgp.reply.encrypted",
            buildJsonObject {
                put("account", account)
                put("in_reply_to", inReplyTo)
            },
        ).flag("encrypted")

internal fun MeronMobileState.warnUndelivered(response: String) {
    val envelope = Json.parseToJsonElement(response).jsonObject
    val result = (envelope["result"] as? JsonObject) ?: envelope
    val addresses =
        (result["undelivered"] as? JsonArray)
            .orEmpty()
            .mapNotNull { (it as? JsonPrimitive)?.contentOrNull }
    if (addresses.isNotEmpty()) {
        errorBanner = trs("compose.toast.notDeliveredTo", mapOf("addresses" to addresses.joinToString()))
        status = errorBanner.orEmpty()
    }
}

internal suspend fun MeronMobileState.confirmChangedRecipientKeys(
    account: String,
    to: String,
    cc: String,
    bcc: String,
): Boolean {
    val result =
        pgp(
            "pgp.keys.forRecipients",
            buildJsonObject {
                put("account", account)
                putJsonArray("addrs") {
                    listOf(to, cc, bcc).forEach { field ->
                        val (completed, active) = parseRecipients(field)
                        (completed + active).filter { it.isNotBlank() }.forEach { add(it.substringAfter('<').substringBefore('>').trim()) }
                    }
                }
            },
        )
    val changed = (result["changed"] as? JsonArray).orEmpty()
    if (changed.isEmpty()) return true
    val answer = kotlinx.coroutines.CompletableDeferred<Boolean>()
    cryptoKeyChangeQuestion = changed.joinToString { it.jsonObject.text("addr") } to answer
    return try {
        answer.await()
    } finally {
        if (cryptoKeyChangeQuestion?.second == answer) cryptoKeyChangeQuestion = null
    }
}

@Composable
internal fun CryptoKeyChangeQuestion(state: MeronMobileState) {
    state.cryptoKeyChangeQuestion?.let { (addresses, answer) ->
        AlertDialog(
            onDismissRequest = { answer.complete(false) },
            title = { Text(tr("crypto.keyChange.confirmTitle")) },
            text = { Text(tr("crypto.keyChange.confirmMessage", mapOf("addresses" to addresses))) },
            confirmButton = { TextButton(onClick = { answer.complete(true) }) { Text(tr("crypto.keyChange.sendAnyway")) } },
            dismissButton = { TextButton(onClick = { answer.complete(false) }) { Text(tr("buttons.cancel")) } },
        )
    }
}

internal suspend fun MeronMobileState.pgp(
    method: String,
    params: JsonObject,
): JsonObject =
    withContext(ioDispatcher) {
        val response =
            Json
                .parseToJsonElement(
                    jp.nonbili.meron.shared
                        .requireCoreOk(core.invoke(method, params.toString())),
                ).jsonObject
        (response["result"] as? JsonObject) ?: response
    }

internal fun JsonObject.text(key: String) = (get(key) as? JsonPrimitive)?.contentOrNull.orEmpty()

internal fun JsonObject.flag(key: String) = (get(key) as? JsonPrimitive)?.booleanOrNull == true

internal fun JsonObject.peerFingerprint() = (get("info") as? JsonObject)?.text("fingerprint").orEmpty()

@Composable
internal fun MessageCryptoStatus(message: MessageBody) {
    val crypto = message.crypto ?: return
    if (!crypto.encrypted && !crypto.signed) return
    val danger = crypto.problem.isNotBlank() || crypto.verifiedConflict
    val color =
        when {
            danger -> {
                MaterialTheme.colorScheme.error
            }

            crypto.trust == "verified" -> {
                androidx.compose.ui.graphics
                    .Color(0xFF278049)
            }

            crypto.trust == "known" -> {
                MaterialTheme.colorScheme.onSurfaceVariant
            }

            else -> {
                MaterialTheme.colorScheme.tertiary
            }
        }
    Column(Modifier.padding(horizontal = 8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Icon(if (crypto.encrypted) Icons.Default.Lock else Icons.Default.LockOpen, null, tint = color, modifier = Modifier.size(16.dp))
            Text(tr(if (crypto.encrypted) "crypto.account.title" else "crypto.badge.signed"), color = color, style = MaterialTheme.typography.labelMedium)
        }
        if (crypto.verifiedConflict) {
            Text(tr("crypto.trust.verifiedConflict", mapOf("sender" to message.from)), color = color)
        } else if (crypto.problem.isNotBlank()) {
            Text(tr("crypto.problem.${crypto.problem}", mapOf("sender" to message.from)), color = color)
        } else {
            Text(
                tr(
                    when {
                        crypto.verifiedConflict -> "crypto.trust.verifiedConflict"
                        crypto.partial -> "crypto.trust.partial"
                        crypto.weakHash -> "crypto.trust.weakHash"
                        !crypto.signed -> "crypto.trust.unsigned"
                        crypto.trust == "verified" -> "crypto.trust.verified"
                        crypto.trust == "known" -> "crypto.trust.known"
                        else -> "crypto.trust.unknownKey"
                    },
                    mapOf("sender" to message.from),
                ),
                color = color,
                style = MaterialTheme.typography.bodySmall,
            )
        }
        if (crypto.keyChanged) Text(tr("crypto.toggle.keyChanged", mapOf("addresses" to message.from)), color = color)
    }
}
