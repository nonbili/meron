package jp.nonbili.meron.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTag
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileCommand
import jp.nonbili.meron.shared.NewerSchema
import jp.nonbili.meron.shared.parseNewerSchemaResponse
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private sealed interface StoreGate {
    data object Checking : StoreGate

    data object Open : StoreGate

    data class Newer(
        val schema: NewerSchema,
    ) : StoreGate

    data object BackedUp : StoreGate

    data object LeftAsItWas : StoreGate
}

/**
 * Shows [content] once the store may be opened. A store a newer Meron last used
 * waits for the user's choice first — back it up and open it, open it anyway, or
 * quit — before anything in [content] can touch it. The check reads only the
 * store's version, so an ordinary launch passes straight through.
 *
 * [onQuit] closes the app; null where an app may not close itself (iOS), and
 * then quitting leaves a screen saying the store was left as it was.
 */
@Composable
fun NewerStoreGate(
    core: MeronCore,
    coreLoaded: Boolean,
    prefs: AppPreferences,
    locale: LocaleController,
    onQuit: (() -> Unit)?,
    content: @Composable () -> Unit,
) {
    var gate by remember(core) { mutableStateOf<StoreGate>(if (coreLoaded) StoreGate.Checking else StoreGate.Open) }
    LaunchedEffect(core) {
        if (gate != StoreGate.Checking) return@LaunchedEffect
        val newer =
            runCatching { withContext(ioDispatcher) { core.invoke(MobileCommand.StoreCheckNewer) } }
                .getOrNull()
                ?.let(::parseNewerSchemaResponse)
        gate = newer?.let { StoreGate.Newer(it) } ?: StoreGate.Open
    }
    when (val current = gate) {
        StoreGate.Open -> {
            content()
        }

        else -> {
            GateSurface(prefs, locale) {
                when (current) {
                    is StoreGate.Newer -> {
                        NewerStoreDialog(
                            schema = current.schema,
                            core = core,
                            onOpened = { backedUp, stillNewer ->
                                gate =
                                    when {
                                        stillNewer != null -> StoreGate.Newer(stillNewer)
                                        backedUp -> StoreGate.BackedUp
                                        else -> StoreGate.Open
                                    }
                            },
                            onQuit = { onQuit?.invoke() ?: run { gate = StoreGate.LeftAsItWas } },
                        )
                    }

                    StoreGate.BackedUp -> {
                        AlertDialog(
                            onDismissRequest = { gate = StoreGate.Open },
                            title = { Text(tr("store.newer.backedUpHere")) },
                            confirmButton = {
                                TextButton(onClick = { gate = StoreGate.Open }) { Text(tr("buttons.done")) }
                            },
                        )
                    }

                    StoreGate.LeftAsItWas -> {
                        Text(
                            tr("store.newer.leftAsItWas"),
                            style = MaterialTheme.typography.bodyLarge,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.padding(32.dp),
                        )
                    }

                    // Checking: the store's version is being read, a moment.
                    else -> {}
                }
            }
        }
    }
}

/** The app's theme and language, read from the stored preferences. */
@Composable
private fun GateSurface(
    prefs: AppPreferences,
    locale: LocaleController,
    content: @Composable () -> Unit,
) {
    val systemDark = isSystemInDarkTheme()
    val theme = remember(prefs, systemDark) { loadThemeChoice(prefs, loadCustomThemes(prefs)).resolve(systemDark) }
    val language =
        remember(prefs, locale) {
            resolveAppLanguageTag(locale.systemLanguageTag(), loadAppLanguageTag(prefs))
                .ifBlank { resolveDeviceLanguageTag(locale.deviceLanguageTag()) }
        }
    CompositionLocalProvider(LocalAppLocale provides language) {
        MeronTheme(appearanceMode = theme) {
            Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                Box(contentAlignment = Alignment.Center) { content() }
            }
        }
    }
}

@Composable
private fun NewerStoreDialog(
    schema: NewerSchema,
    core: MeronCore,
    onOpened: (backedUp: Boolean, stillNewer: NewerSchema?) -> Unit,
    onQuit: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf<Boolean?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    val open: (Boolean) -> Unit = { backup ->
        busy = backup
        error = null
        scope.launch {
            runCatching {
                withContext(ioDispatcher) {
                    val opened = core.invoke(MobileCommand.StoreOpenNewer, """{"backup":$backup,"found":${schema.found}}""")
                    // No copy when another window chose first and the store
                    // may be in use: say nothing was saved.
                    val backedUp = backup && !opened.replace(" ", "").contains("\"backups\":[]")
                    // A schema newer than the one shown isn't covered by the
                    // choice: the store is still refused, and asks again.
                    val newer = runCatching { core.invoke(MobileCommand.StoreCheckNewer) }.getOrNull()?.let(::parseNewerSchemaResponse)
                    backedUp to newer
                }
            }.onSuccess { (backedUp, newer) -> onOpened(backedUp, newer) }
                .onFailure { error = it.message ?: it.toString() }
            busy = null
        }
    }
    AlertDialog(
        // Nothing loads until the user chooses, so it can't be dismissed.
        onDismissRequest = {},
        modifier = Modifier.semantics { testTag = "newer-store-dialog" },
        title = { Text(tr("store.newer.title")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(tr("store.newer.message", mapOf("found" to schema.found, "supported" to schema.supported)))
                error?.let { message ->
                    Text(
                        tr("store.newer.failed", mapOf("error" to message)),
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        },
        confirmButton = {
            Column(horizontalAlignment = Alignment.End) {
                TextButton(onClick = { open(true) }, enabled = busy == null) {
                    if (busy == true) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                    Text(tr("store.newer.backupAndOpen"))
                }
                TextButton(onClick = { open(false) }, enabled = busy == null) {
                    if (busy == false) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                    Text(tr("store.newer.openAnyway"))
                }
                TextButton(onClick = onQuit, enabled = busy == null) {
                    Text(tr("store.newer.quit"))
                }
            }
        },
    )
}
