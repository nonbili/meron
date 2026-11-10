package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.MessageBody
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.add
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlin.io.encoding.Base64

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
private fun KeyLookupDialog(
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
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Text(tr("crypto.lookup.note"))
                OutlinedTextField(address, {
                    address = it
                    result = null
                }, label = { Text(tr("crypto.lookup.address")) })
                TextButton(enabled = !busy && address.isNotBlank(), onClick = {
                    busy = true
                    scope.launch {
                        runCatching { state.pgp("pgp.lookup", buildJsonObject { put("addr", address.trim()) }) }
                            .onSuccess { result = it }
                            .onFailure { error = it.message.orEmpty() }
                        busy = false
                    }
                }) { Text(tr("crypto.lookup.action")) }
                (result?.get("found") as? JsonArray)?.forEach { element ->
                    val key = element.jsonObject
                    val source = key["source"]!!.jsonObject
                    Text(key["info"]!!.jsonObject.text("fingerprint"))
                    Text(
                        tr(
                            when {
                                source.text("kind") == "wkd" -> "crypto.lookup.fromWkd"
                                key.flag("verified") -> "crypto.lookup.fromVerified"
                                else -> "crypto.lookup.fromUnverified"
                            },
                            mapOf("server" to source.text("url")),
                        ),
                    )
                    TextButton(enabled = !busy, onClick = {
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
                    }) { Text(tr("crypto.lookup.use")) }
                }
                result?.let { found ->
                    val searched = (found["searched"] as? JsonArray).orEmpty().joinToString { it.jsonPrimitive.content }
                    if ((found["found"] as? JsonArray).orEmpty().isEmpty()) Text(tr("crypto.lookup.nothing", mapOf("sources" to searched)))
                    val failed = (found["failed"] as? JsonArray).orEmpty().joinToString { it.jsonObject.text("source") }
                    if (failed.isNotBlank()) Text(tr("crypto.lookup.failed", mapOf("sources" to failed)), color = MaterialTheme.colorScheme.error)
                }
                if (busy) LinearProgressIndicator()
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error)
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.close")) } },
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

private fun JsonObject.text(key: String) = (get(key) as? JsonPrimitive)?.contentOrNull.orEmpty()

private fun JsonObject.flag(key: String) = (get(key) as? JsonPrimitive)?.booleanOrNull == true

private fun JsonObject.peerFingerprint() = (get("info") as? JsonObject)?.text("fingerprint").orEmpty()

@Composable
internal fun AccountEncryptionSettings(account: String) {
    val state = LocalCryptoState.current ?: return
    val services = LocalPlatformServices.current
    val scope = rememberCoroutineScope()
    var info by remember(account) { mutableStateOf<JsonObject?>(null) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    var file by remember { mutableStateOf<ByteArray?>(null) }
    var passphrase by remember { mutableStateOf("") }
    var choices by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var fingerprint by remember { mutableStateOf("") }
    var exportSecret by remember { mutableStateOf(false) }
    var confirmCreate by remember { mutableStateOf(false) }
    var confirmDisable by remember { mutableStateOf(false) }
    var deleteKey by remember { mutableStateOf<String?>(null) }
    val accountParams = buildJsonObject { put("account", account) }

    suspend fun refresh() {
        info = state.pgp("pgp.account.state", accountParams)
    }

    fun run(action: suspend () -> Unit) {
        if (busy) return
        busy = true
        error = ""
        scope.launch {
            try {
                action()
                refresh()
            } catch (e: Exception) {
                error = e.message.orEmpty()
            } finally {
                busy = false
            }
        }
    }

    fun importKey() =
        run {
            val data = file ?: return@run
            val result =
                state
                    .pgp(
                        "pgp.account.importKey",
                        buildJsonObject {
                            put("account", account)
                            put("data_b64", Base64.encode(data))
                            put("passphrase", passphrase)
                            if (fingerprint.isNotBlank()) put("fingerprint", fingerprint)
                        },
                    )["result"]!!
                    .jsonObject
            when (result.text("status")) {
                "imported" -> {
                    file = null
                    passphrase = ""
                    choices = emptyList()
                    fingerprint = ""
                    if (!result.flag("address_matches")) error = state.trs("crypto.enable.addressMismatch")
                }

                "choose" -> {
                    choices = result["keys"]!!.jsonArray.map { it.jsonObject }
                }

                "needs_passphrase" -> {
                    error = state.trs("crypto.enable.passphraseOnce")
                }

                "on_card" -> {
                    error = state.trs("crypto.enable.onCard")
                }

                else -> {
                    error = state.trs("crypto.enable.noSecretKey")
                }
            }
        }
    LaunchedEffect(account) { run { refresh() } }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(tr("crypto.account.title"), style = MaterialTheme.typography.titleMedium)
        Text(tr("crypto.enable.otherDevices"), style = MaterialTheme.typography.bodySmall)
        info?.let { current ->
            val key = current["key"] as? JsonObject
            if (key != null) {
                Text(key.text("fingerprint"), style = MaterialTheme.typography.bodySmall)
                if (key["unusable"] is JsonPrimitive && key["unusable"] != JsonNull) Text(tr("crypto.toggle.ownKeyUnusable"), color = MaterialTheme.colorScheme.error)
                TextButton(enabled = !busy, onClick = {
                    if (current.flag("enabled")) {
                        confirmDisable = true
                    } else {
                        run { state.pgp("pgp.account.enable", accountParams) }
                    }
                }) { Text(tr(if (current.flag("enabled")) "crypto.account.disable" else "crypto.enable.title")) }
                Row {
                    TextButton(enabled = !busy, onClick = {
                        run {
                            val armored = state.pgp("pgp.account.exportPublic", accountParams).text("armored")
                            services.saveFile(armored.encodeToByteArray(), "public-key.asc", "application/pgp-keys")
                        }
                    }) { Text(tr("crypto.account.exportPublic")) }
                    TextButton(enabled = !busy, onClick = {
                        passphrase = ""
                        exportSecret = true
                    }) {
                        Text(tr("crypto.account.exportSecret"))
                    }
                }
                TextButton(enabled = !busy, onClick = { deleteKey = key.text("fingerprint") }) { Text(tr("crypto.account.delete")) }
            }
            (current["previous"] as? JsonArray).orEmpty().forEach { previous ->
                Text(previous.jsonObject.text("fingerprint"), style = MaterialTheme.typography.bodySmall)
                TextButton(enabled = !busy, onClick = { deleteKey = previous.jsonObject.text("fingerprint") }) { Text(tr("crypto.account.delete")) }
            }
            TextButton(enabled = !busy, onClick = { confirmCreate = true }) { Text(tr("crypto.enable.create")) }
            TextButton(enabled = !busy, onClick = {
                services.pickFile(listOf("*/*")) { picked ->
                    if (picked != null) {
                        file = picked.bytes
                        passphrase = ""
                        fingerprint = ""
                        choices = emptyList()
                        importKey()
                    }
                }
            }) { Text(tr("crypto.enable.useExisting")) }
        }
        if (file != null) {
            choices.forEach { key ->
                TextButton(onClick = {
                    fingerprint = key.text("fingerprint")
                    importKey()
                }, enabled = !busy) {
                    Text(key.text("fingerprint"))
                }
            }
            OutlinedTextField(
                passphrase,
                { passphrase = it },
                label = { Text(tr("crypto.enable.passphrase")) },
                visualTransformation = PasswordVisualTransformation(),
            )
            TextButton(onClick = { importKey() }, enabled = !busy) { Text(tr("crypto.enable.unlock")) }
            TextButton(onClick = {
                file = null
                passphrase = ""
                choices = emptyList()
            }) { Text(tr("buttons.cancel")) }
        }
        if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error)
        PeerEncryptionSettings(state)
    }
    deleteKey?.let { fingerprintToDelete ->
        AlertDialog(
            onDismissRequest = { deleteKey = null },
            title = { Text(tr("crypto.account.deleteTitle")) },
            text = { Text(tr("crypto.account.deleteMessage")) },
            confirmButton = {
                TextButton(onClick = {
                    deleteKey = null
                    run {
                        state.pgp(
                            "pgp.account.deleteKey",
                            buildJsonObject {
                                put("account", account)
                                put("fingerprint", fingerprintToDelete)
                            },
                        )
                    }
                }) { Text(tr("crypto.account.delete")) }
            },
            dismissButton = { TextButton(onClick = { deleteKey = null }) { Text(tr("buttons.cancel")) } },
        )
    }
    if (confirmCreate || confirmDisable) {
        AlertDialog(
            onDismissRequest = {
                confirmCreate = false
                confirmDisable = false
            },
            title = { Text(tr(if (confirmDisable) "crypto.account.disableTitle" else "crypto.enable.title")) },
            text = { Text(tr(if (confirmDisable) "crypto.account.disableMessage" else "crypto.account.replaceHint")) },
            confirmButton = {
                TextButton(onClick = {
                    val method = if (confirmDisable) "pgp.account.disable" else "pgp.account.create"
                    confirmCreate = false
                    confirmDisable = false
                    run { state.pgp(method, accountParams) }
                }) { Text(tr(if (confirmDisable) "crypto.account.disable" else "crypto.enable.create")) }
            },
            dismissButton = {
                TextButton(onClick = {
                    confirmCreate = false
                    confirmDisable = false
                }) { Text(tr("buttons.cancel")) }
            },
        )
    }
    if (exportSecret) {
        AlertDialog(
            onDismissRequest = {
                exportSecret = false
                passphrase = ""
            },
            title = { Text(tr("crypto.account.exportSecret")) },
            text = {
                Column {
                    Text(tr("crypto.account.exportSecretWarning"))
                    OutlinedTextField(
                        passphrase,
                        { passphrase = it },
                        label = { Text(tr("crypto.account.exportPassphrase")) },
                        visualTransformation = PasswordVisualTransformation(),
                    )
                }
            },
            confirmButton = {
                TextButton(enabled = passphrase.length >= 8 && !busy, onClick = {
                    val password = passphrase
                    passphrase = ""
                    exportSecret = false
                    run {
                        val armored =
                            state
                                .pgp(
                                    "pgp.account.exportSecret",
                                    buildJsonObject {
                                        put("account", account)
                                        put("passphrase", password)
                                    },
                                ).text("armored")
                        services.saveFile(armored.encodeToByteArray(), "secret-key.asc", "application/pgp-keys")
                    }
                }) { Text(tr("crypto.account.exportSecret")) }
            },
            dismissButton = {
                TextButton(onClick = {
                    exportSecret = false
                    passphrase = ""
                }) { Text(tr("buttons.cancel")) }
            },
        )
    }
}

@Composable
private fun PeerEncryptionSettings(state: MeronMobileState) {
    val scope = rememberCoroutineScope()
    val services = LocalPlatformServices.current
    var open by remember { mutableStateOf(false) }
    var peers by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var error by remember { mutableStateOf("") }
    var selected by remember { mutableStateOf<JsonObject?>(null) }
    var confirm by remember { mutableStateOf("") }
    var remove by remember { mutableStateOf<JsonObject?>(null) }
    var unverify by remember { mutableStateOf<JsonObject?>(null) }
    var importData by remember { mutableStateOf<String?>(null) }
    var importPreview by remember { mutableStateOf<JsonObject?>(null) }

    suspend fun refresh() {
        peers = state.pgp("pgp.peers.list", buildJsonObject {})["peers"]!!.jsonArray.map { it.jsonObject }
    }
    TextButton(onClick = {
        open = true
        scope.launch { runCatching { refresh() }.onFailure { error = it.message.orEmpty() } }
    }) {
        Text(tr("crypto.peers.title"))
    }
    if (open) {
        AlertDialog(
            onDismissRequest = { open = false },
            title = { Text(tr("crypto.peers.title")) },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    TextButton(onClick = {
                        services.pickFile(listOf("*/*")) { picked ->
                            if (picked != null) {
                                scope.launch {
                                    runCatching {
                                        val data = Base64.encode(picked.bytes)
                                        importPreview = state.pgp("pgp.peers.importPreview", buildJsonObject { put("data_b64", data) })
                                        importData = data
                                    }.onFailure { error = it.message.orEmpty() }
                                }
                            }
                        }
                    }) { Text(tr("crypto.peers.import")) }
                    if (peers.isEmpty()) Text(tr("crypto.peers.empty"))
                    peers.forEach { peer ->
                        Text(peer.text("addr"))
                        Text(peer.peerFingerprint(), style = MaterialTheme.typography.bodySmall)
                        if (peer.flag("verified")) Text(tr("crypto.trust.verified", mapOf("sender" to peer.text("addr"))))
                        if (peer.flag("verified")) TextButton(onClick = { unverify = peer }) { Text(tr("crypto.peers.unverify")) }
                        TextButton(onClick = {
                            selected = peer
                            confirm = ""
                        }) { Text(tr("crypto.verify.title", mapOf("sender" to peer.text("addr")))) }
                        TextButton(onClick = { remove = peer }) { Text(tr("crypto.peers.remove")) }
                    }
                    if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error)
                }
            },
            confirmButton = { TextButton(onClick = { open = false }) { Text(tr("buttons.close")) } },
        )
    }
    unverify?.let { peer ->
        AlertDialog(
            onDismissRequest = { unverify = null },
            title = { Text(tr("crypto.peers.unverifyTitle")) },
            text = { Text(tr("crypto.peers.unverifyMessage", mapOf("sender" to peer.text("addr")))) },
            confirmButton = {
                TextButton(onClick = {
                    scope.launch {
                        runCatching {
                            state.pgp(
                                "pgp.peers.setVerified",
                                buildJsonObject {
                                    put("addr", peer.text("addr"))
                                    put("fingerprint", peer.peerFingerprint())
                                    put("verified", false)
                                },
                            )
                            unverify = null
                            refresh()
                        }.onFailure {
                            error = it.message.orEmpty()
                            unverify = null
                        }
                    }
                }) { Text(tr("crypto.peers.unverify")) }
            },
            dismissButton = { TextButton(onClick = { unverify = null }) { Text(tr("buttons.cancel")) } },
        )
    }
    importPreview?.let { preview ->
        AlertDialog(
            onDismissRequest = {
                importPreview = null
                importData = null
            },
            title = { Text(tr("crypto.peers.importTitle")) },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    (preview["ready"] as? JsonArray).orEmpty().forEach { key ->
                        Text(key.jsonObject.text("fingerprint"))
                        Text((key.jsonObject["emails"] as? JsonArray).orEmpty().joinToString { it.jsonPrimitive.content })
                    }
                    (preview["conflicts_with_verified"] as? JsonArray).orEmpty().forEach { addr ->
                        Text(tr("crypto.trust.verifiedConflict", mapOf("sender" to addr.jsonPrimitive.content)), color = MaterialTheme.colorScheme.error)
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    scope.launch {
                        runCatching {
                            state.pgp(
                                "pgp.peers.import",
                                buildJsonObject {
                                    put("data_b64", importData.orEmpty())
                                    putJsonArray("fingerprints") {
                                        (preview["ready"] as? JsonArray).orEmpty().forEach { add(it.jsonObject.text("fingerprint")) }
                                    }
                                },
                            )
                            importPreview = null
                            importData = null
                            refresh()
                        }.onFailure {
                            error = it.message.orEmpty()
                            importPreview = null
                            importData = null
                        }
                    }
                }) { Text(tr("crypto.peers.import")) }
            },
            dismissButton = {
                TextButton(onClick = {
                    importPreview = null
                    importData = null
                }) { Text(tr("buttons.cancel")) }
            },
        )
    }
    remove?.let { peer ->
        AlertDialog(
            onDismissRequest = { remove = null },
            title = { Text(tr("crypto.peers.removeTitle")) },
            text = { Text(tr("crypto.peers.removeMessage", mapOf("sender" to peer.text("addr")))) },
            confirmButton = {
                TextButton(onClick = {
                    scope.launch {
                        runCatching {
                            state.pgp(
                                "pgp.peers.remove",
                                buildJsonObject {
                                    put("addr", peer.text("addr"))
                                    put("fingerprint", peer.peerFingerprint())
                                },
                            )
                            remove = null
                            refresh()
                        }.onFailure {
                            error = it.message.orEmpty()
                            remove = null
                        }
                    }
                }) { Text(tr("crypto.peers.remove")) }
            },
            dismissButton = { TextButton(onClick = { remove = null }) { Text(tr("buttons.cancel")) } },
        )
    }
    selected?.let { peer ->
        AlertDialog(
            onDismissRequest = { selected = null },
            title = { Text(tr("crypto.verify.title", mapOf("sender" to peer.text("addr")))) },
            text = {
                Column {
                    Text(peer.text("addr"))
                    Text(peer.peerFingerprint())
                    Text(tr("crypto.verify.howTo", mapOf("sender" to peer.text("addr"))))
                    OutlinedTextField(confirm, { confirm = it }, label = { Text(tr("crypto.account.copyFingerprint")) })
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    scope.launch {
                        runCatching {
                            state.pgp(
                                "pgp.peers.setVerified",
                                buildJsonObject {
                                    put("addr", peer.text("addr"))
                                    put("fingerprint", peer.peerFingerprint())
                                    put("confirm", confirm)
                                },
                            )
                            selected = null
                            refresh()
                        }.onFailure {
                            error = it.message.orEmpty()
                            selected = null
                        }
                    }
                }) { Text(tr("crypto.verify.confirm")) }
            },
            dismissButton = { TextButton(onClick = { selected = null }) { Text(tr("buttons.cancel")) } },
        )
    }
}

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
